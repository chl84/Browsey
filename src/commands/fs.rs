//! File-system oriented Tauri commands: listings, mounts, trash, opening, renaming, deleting, and watch wiring.

use crate::{
    db,
    entry::{build_entry, FsEntry},
    fs_utils::{sanitize_path_follow, sanitize_path_nofollow},
    sorting::{sort_entries, SortSpec},
    watcher::{self, WatchState},
};
#[cfg(not(target_os = "windows"))]
use crate::fs_utils::unique_path;
use serde::Serialize;
#[cfg(target_os = "windows")]
use std::collections::HashMap;
use std::collections::HashSet;
use std::{fs, path::PathBuf};
use sysinfo::Disks;
use tracing::{error, info, warn};

pub fn expand_path(raw: Option<String>) -> Result<PathBuf, String> {
    if let Some(p) = raw {
        if p == "~" {
            dirs_next::home_dir().ok_or_else(|| "Fant ikke hjemmekatalog".to_string())
        } else if let Some(stripped) = p.strip_prefix("~/") {
            let home =
                dirs_next::home_dir().ok_or_else(|| "Fant ikke hjemmekatalog".to_string())?;
            Ok(home.join(stripped))
        } else {
            Ok(PathBuf::from(p))
        }
    } else if let Some(home) = dirs_next::home_dir() {
        Ok(home)
    } else {
        std::env::current_dir().map_err(|e| format!("Kunne ikke lese arbeidskatalog: {e}"))
    }
}

#[derive(Serialize)]
pub struct DirListing {
    pub current: String,
    pub entries: Vec<FsEntry>,
}

#[derive(Serialize, Clone)]
pub struct MountInfo {
    pub label: String,
    pub path: String,
    pub fs: String,
    pub removable: bool,
}

#[tauri::command]
pub fn list_dir(path: Option<String>, sort: Option<SortSpec>) -> Result<DirListing, String> {
    let base_path = expand_path(path)?;
    let fallback_home = dirs_next::home_dir();

    let target = if base_path.exists() {
        base_path.clone()
    } else if let Some(home) = &fallback_home {
        home.clone()
    } else {
        return Err(format!("{}: finnes ikke", base_path.display()));
    };

    let target = target.canonicalize().unwrap_or_else(|_| target.clone());

    let star_conn = db::open()?;
    let star_set: HashSet<String> = db::starred_set(&star_conn)?;

    let mut entries = Vec::new();
    let read_dir = fs::read_dir(&target).map_err(|e| format!("{}: {e}", target.display()))?;

    for entry in read_dir {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        let meta = fs::symlink_metadata(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let is_link = meta.file_type().is_symlink();
        let starred = star_set.contains(&path.to_string_lossy().to_string());
        entries.push(build_entry(&path, &meta, is_link, starred));
    }

    sort_entries(&mut entries, sort);

    Ok(DirListing {
        current: target.to_string_lossy().into_owned(),
        entries,
    })
}

fn list_dir_with_star(
    target: PathBuf,
    star_set: &HashSet<String>,
    sort: Option<SortSpec>,
) -> Result<DirListing, String> {
    let mut entries = Vec::new();
    let read_dir = fs::read_dir(&target).map_err(|e| format!("{}: {e}", target.display()))?;

    for entry in read_dir {
        let entry = entry.map_err(|e| e.to_string())?;
        let path = entry.path();
        let meta = fs::symlink_metadata(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let is_link = meta.file_type().is_symlink();
        let starred = star_set.contains(&path.to_string_lossy().to_string());
        entries.push(build_entry(&path, &meta, is_link, starred));
    }

    sort_entries(&mut entries, sort);

    Ok(DirListing {
        current: target.to_string_lossy().into_owned(),
        entries,
    })
}

#[cfg(target_os = "windows")]
fn decode_recycle_original_path(bytes: &[u8]) -> Option<String> {
    // Windows $I format (NTFS): version + size + time, then an optional path length, then UTF-16LE path.
    // Real-world files show the UTF-16 path starting at 0x1C (28) for version 2 records,
    // but older records start at 0x10 (16). Try the likely offsets and validate.
    const OFFSETS: [usize; 3] = [28, 24, 16];
    for offset in OFFSETS {
        if bytes.len() <= offset {
            continue;
        }
        let utf16: Vec<u16> = bytes[offset..]
            .chunks(2)
            .take_while(|chunk| chunk.len() == 2)
            .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
            .take_while(|&c| c != 0)
            .collect();
        if let Ok(mut s) = String::from_utf16(&utf16) {
            if s.is_empty() {
                continue;
            }
            // Basic validation: path-like, no control characters.
            let looks_like_path = s.contains(':') || s.starts_with("\\\\");
            if !looks_like_path {
                continue;
            }
            s.retain(|c| !c.is_control());
            if !s.is_empty() {
                return Some(s);
            }
        }
    }
    None
}

#[cfg(target_os = "windows")]
fn list_windows_trash(
    star_set: &HashSet<String>,
    sort: Option<SortSpec>,
) -> Result<DirListing, String> {
    // Aggregate entries from user SID folders under $Recycle.Bin (one per drive) and map $I/$R pairs to original names.
    let mut roots = Vec::new();
    let disks = Disks::new_with_refreshed_list();
    for disk in disks.iter() {
        let mut root = disk.mount_point().to_path_buf();
        root.push("$Recycle.Bin");
        roots.push(root);
    }
    if roots.is_empty() {
        let system_drive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".to_string());
        roots.push(PathBuf::from(format!(r"{}\\$Recycle.Bin", system_drive)));
    }

    let mut entries = Vec::new();
    let mut any_access = false;

    #[derive(Clone)]
    struct TrashItem {
        original: Option<String>,
        r_path: Option<PathBuf>,
    }

        for recycle_root in roots {
            if !recycle_root.exists() {
                continue;
            }
        let roots_iter = match fs::read_dir(&recycle_root) {
            Ok(r) => r,
            Err(_) => continue,
        };

        for sid_dir in roots_iter.flatten() {
            let sid_path = sid_dir.path();
            if !sid_path.is_dir() {
                continue;
            }
            any_access = true;
            let mut map: HashMap<String, TrashItem> = HashMap::new();

            let rd = match fs::read_dir(&sid_path) {
                Ok(rd) => rd,
                Err(_) => continue,
            };

            for entry in rd.flatten() {
                let path = entry.path();
                let Some(fname) = path.file_name().and_then(|s| s.to_str()) else {
                    continue;
                };
                // Expect file names to start with $I (info) or $R (resource).
                if !(fname.starts_with("$I") || fname.starts_with("$R")) {
                    continue;
                }
                let key = fname.trim_start_matches(['$','I','R']).to_string();
                let entry = map.entry(key).or_insert(TrashItem {
                    original: None,
                    r_path: None,
                });
                if fname.starts_with("$I") {
                    if let Ok(bytes) = std::fs::read(&path) {
                        if let Some(orig) = decode_recycle_original_path(&bytes) {
                            entry.original = Some(orig);
                        }
                    }
                } else if fname.starts_with("$R") {
                    entry.r_path = Some(path.clone());
                }
            }

            for (_k, item) in map {
                if let Some(rp) = item.r_path {
                    if let Ok(meta) = fs::symlink_metadata(&rp) {
                        let is_link = meta.file_type().is_symlink();
                        let name = item
                            .original
                            .as_ref()
                            .and_then(|p| {
                                PathBuf::from(p)
                                    .file_name()
                                    .map(|s| s.to_string_lossy().into_owned())
                            })
                            .filter(|s| !s.is_empty())
                            .unwrap_or_else(|| {
                                rp.file_name()
                                    .map(|s| s.to_string_lossy().into_owned())
                                    .unwrap_or_default()
                            });
                        let mut entry = build_entry(
                            &rp,
                            &meta,
                            is_link,
                            star_set.contains(&rp.to_string_lossy().to_string()),
                        );
                        entry.name = name;
                        entries.push(entry);
                    }
                }
            }
        }
    }

    sort_entries(&mut entries, sort);
    Ok(DirListing {
        current: if any_access {
            "Recycle Bin".to_string()
        } else {
            "Trash (unavailable)".to_string()
        },
        entries,
    })
}

fn resolve_trash_dir() -> Result<Option<PathBuf>, String> {
    #[cfg(target_os = "linux")]
    {
        let base = std::env::var("XDG_DATA_HOME")
            .map(PathBuf::from)
            .ok()
            .or_else(|| dirs_next::data_dir())
            .unwrap_or_else(|| PathBuf::from("~/.local/share"));
        let path = base.join("Trash").join("files");
        if path.exists() {
            return Ok(Some(path));
        }
        // Fallback to ~/.local/share/Trash/files
        if let Some(home) = dirs_next::home_dir() {
            let fallback = home.join(".local/share/Trash/files");
            if fallback.exists() {
                return Ok(Some(fallback));
            }
        }
        Ok(None)
    }
    #[cfg(target_os = "windows")]
    {
        Ok(None)
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        Ok(None)
    }
}

#[tauri::command]
pub fn list_mounts() -> Vec<MountInfo> {
    let disks = Disks::new_with_refreshed_list();
    disks
        .iter()
        .filter_map(|d| {
            let mount_point = d.mount_point().to_string_lossy().to_string();
            if mount_point.is_empty() {
                return None;
            }
            let fs = d.file_system().to_string_lossy().to_string();
            let fs_lc = fs.to_lowercase();
            if matches!(
                fs_lc.as_str(),
                "tmpfs"
                    | "devtmpfs"
                    | "proc"
                    | "sysfs"
                    | "cgroup"
                    | "cgroup2"
                    | "overlay"
                    | "squashfs"
            ) {
                return None;
            }

            let label = mount_point.clone();

            Some(MountInfo {
                label,
                path: mount_point,
                fs,
                removable: d.is_removable(),
            })
        })
        .collect()
}

#[tauri::command]
pub fn watch_dir(
    path: Option<String>,
    state: tauri::State<WatchState>,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let base_path = expand_path(path)?;
    let target = if base_path.exists() {
        base_path
    } else if let Some(home) = dirs_next::home_dir() {
        home
    } else {
        return Err("Fant ikke startkatalog".to_string());
    };
    watcher::start_watch(app, target, &state)
}

#[tauri::command]
pub fn open_entry(path: String) -> Result<(), String> {
    let pb = sanitize_path_follow(&path, false)?;
    let conn = db::open()?;
    if let Err(e) = db::touch_recent(&conn, &pb.to_string_lossy()) {
        warn!("Failed to record recent for {:?}: {}", pb, e);
    }
    info!("Opening path {:?}", pb);
    open::that_detached(&pb).map_err(|e| {
        error!("Failed to open {:?}: {}", pb, e);
        format!("Failed to open: {e}")
    })
}

#[tauri::command]
pub fn list_trash(sort: Option<SortSpec>) -> Result<DirListing, String> {
    let conn = db::open()?;
    let star_set: HashSet<String> = db::starred_set(&conn)?;
    match resolve_trash_dir()? {
        Some(target) => list_dir_with_star(target, &star_set, sort),
        None => {
            #[cfg(target_os = "windows")]
            {
                return list_windows_trash(&star_set, sort);
            }
            #[cfg(not(target_os = "windows"))]
            Ok(DirListing {
                current: "Trash (unavailable)".to_string(),
                entries: Vec::new(),
            })
        }
    }
}

#[tauri::command]
pub fn rename_entry(path: String, new_name: String) -> Result<String, String> {
    let from = sanitize_path_nofollow(&path, true)?;
    if new_name.trim().is_empty() {
        return Err("New name cannot be empty".into());
    }
    let parent = from.parent().ok_or_else(|| "Cannot rename root".to_string())?;
    let to = parent.join(new_name.trim());
    fs::rename(&from, &to)
        .map_err(|e| format!("Failed to rename: {e}"))?;
    Ok(to.to_string_lossy().to_string())
}

#[tauri::command]
pub fn move_to_trash(path: String) -> Result<(), String> {
    let src = sanitize_path_nofollow(&path, true)?;

    #[cfg(target_os = "windows")]
    {
        return trash::delete(&src).map_err(|e| format!("Failed to move to trash: {e}"));
    }

    #[cfg(not(target_os = "windows"))]
    {
        let trash_dir = resolve_trash_dir()?
            .ok_or_else(|| "Trash not available on this platform".to_string())?;
        std::fs::create_dir_all(&trash_dir)
            .map_err(|e| format!("Failed to create trash dir: {e}"))?;
        let file_name = src
            .file_name()
            .ok_or_else(|| "Invalid path".to_string())?;
        let dest = unique_path(&trash_dir.join(file_name));
        fs::rename(&src, &dest).map_err(|e| format!("Failed to move to trash: {e}"))
    }
}

#[tauri::command]
pub fn delete_entry(path: String) -> Result<(), String> {
    let pb = sanitize_path_nofollow(&path, true)?;
    let meta = fs::symlink_metadata(&pb).map_err(|e| format!("Failed to read metadata: {e}"))?;
    if meta.is_dir() {
        fs::remove_dir_all(&pb).map_err(|e| format!("Failed to delete directory: {e}"))
    } else {
        fs::remove_file(&pb).map_err(|e| format!("Failed to delete file: {e}"))
    }
}
