#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod db;
mod entry;
mod icons;
mod context_menu;
mod search;
mod sorting;
mod statusbar;
mod watcher;
mod fs_utils;
mod clipboard;

use db::{
    delete_bookmark, list_bookmarks, load_column_widths, save_column_widths, upsert_bookmark,
};
use entry::{build_entry, entry_times, EntryTimes, FsEntry};
use context_menu::context_menu_actions;
use fs_utils::{sanitize_path_follow, sanitize_path_nofollow, unique_path};
use clipboard::{set_clipboard_cmd, paste_clipboard_cmd};
use once_cell::sync::OnceCell;
use search::search_recursive;
use serde::Serialize;
use sorting::{sort_entries, SortSpec};
use statusbar::dir_sizes;
use std::collections::HashSet;
use std::{fs, path::PathBuf};
use sysinfo::Disks;
use tracing::{error, info, warn};
use watcher::WatchState;

fn expand_path(raw: Option<String>) -> Result<PathBuf, String> {
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
struct DirListing {
    current: String,
    entries: Vec<FsEntry>,
}

#[derive(Serialize, Clone)]
struct MountInfo {
    label: String,
    path: String,
    fs: String,
    removable: bool,
}

#[derive(Serialize, Clone)]
struct Bookmark {
    label: String,
    path: String,
}

#[tauri::command]
fn list_dir(path: Option<String>, sort: Option<SortSpec>) -> Result<DirListing, String> {
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

#[tauri::command]
fn list_mounts() -> Vec<MountInfo> {
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
fn get_bookmarks() -> Result<Vec<Bookmark>, String> {
    let conn = db::open()?;
    let rows = list_bookmarks(&conn)?;
    Ok(rows
        .into_iter()
        .map(|(label, path)| Bookmark { label, path })
        .collect())
}

#[tauri::command]
fn add_bookmark(label: String, path: String) -> Result<(), String> {
    let conn = db::open()?;
    upsert_bookmark(&conn, &label, &path)
}

#[tauri::command]
fn remove_bookmark(path: String) -> Result<(), String> {
    let conn = db::open()?;
    delete_bookmark(&conn, &path)
}

#[cfg(target_os = "windows")]
fn list_windows_trash(
    star_set: &HashSet<String>,
    sort: Option<SortSpec>,
) -> Result<DirListing, String> {
    // Aggregate entries from user SID folders under $Recycle.Bin and map $I/$R pairs to original names.
    let system_drive = std::env::var("SystemDrive").unwrap_or_else(|_| "C:".to_string());
    let recycle_root = PathBuf::from(format!("{}\\$Recycle.Bin", system_drive));
    if !recycle_root.exists() {
        return Ok(DirListing {
            current: "Trash (unavailable)".into(),
            entries: Vec::new(),
        });
    }

    use std::collections::HashMap;
    let mut entries = Vec::new();
    let roots = match fs::read_dir(&recycle_root) {
        Ok(r) => r,
        Err(_) => {
            return Ok(DirListing {
                current: "Trash (unavailable)".into(),
                entries: Vec::new(),
            });
        }
    };

    #[derive(Clone)]
    struct TrashItem {
        original: Option<String>,
        r_path: Option<PathBuf>,
    }

    for sid_dir in roots.flatten() {
        let sid_path = sid_dir.path();
        if !sid_path.is_dir() {
            continue;
        }
        let mut map: HashMap<String, TrashItem> = HashMap::new();

        if let Ok(rd) = fs::read_dir(&sid_path) {
            for entry in rd.flatten() {
                let path = entry.path();
                if let Some(fname) = path.file_name().and_then(|s| s.to_str()) {
                    if fname.starts_with("$I") {
                        // metadata file; parse for original path
                        if let Ok(bytes) = std::fs::read(&path) {
                            if bytes.len() > 24 {
                                let utf16: Vec<u16> = bytes[24..]
                                    .chunks(2)
                                    .take_while(|chunk| chunk.len() == 2)
                                    .map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]]))
                                    .take_while(|&c| c != 0)
                                    .collect();
                                if let Ok(orig) = String::from_utf16(&utf16) {
                                    let key = fname.trim_start_matches("$I").to_string();
                                    let entry = map.entry(key).or_insert(TrashItem {
                                        original: None,
                                        r_path: None,
                                    });
                                    entry.original = Some(orig);
                                }
                            }
                        }
                    } else if fname.starts_with("$R") {
                        let key = fname.trim_start_matches("$R").to_string();
                        let entry = map.entry(key).or_insert(TrashItem {
                            original: None,
                            r_path: None,
                        });
                        entry.r_path = Some(path.clone());
                    }
                }
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

    sort_entries(&mut entries, sort);
    Ok(DirListing {
        current: recycle_root.to_string_lossy().into_owned(),
        entries,
    })
}

#[tauri::command]
fn search(
    path: Option<String>,
    query: String,
    sort: Option<SortSpec>,
) -> Result<Vec<FsEntry>, String> {
    let base_path = expand_path(path)?;
    let target = if base_path.exists() {
        base_path
    } else if let Some(home) = dirs_next::home_dir() {
        home
    } else {
        return Err("Fant ikke startkatalog".to_string());
    };
    let star_conn = db::open()?;
    let star_set: HashSet<String> = db::starred_set(&star_conn)?;
    let mut res = search_recursive(target, query)?;
    for item in &mut res {
        if star_set.contains(&item.path) {
            item.starred = true;
        }
    }
    sort_entries(&mut res, sort);
    Ok(res)
}

#[tauri::command]
fn store_column_widths(widths: Vec<f64>) -> Result<(), String> {
    let conn = db::open()?;
    save_column_widths(&conn, &widths)
}

#[tauri::command]
fn load_saved_column_widths() -> Result<Option<Vec<f64>>, String> {
    let conn = db::open()?;
    load_column_widths(&conn)
}

#[tauri::command]
fn watch_dir(
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
fn open_entry(path: String) -> Result<(), String> {
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
fn toggle_star(path: String) -> Result<bool, String> {
    let conn = db::open()?;
    let res = db::toggle_star(&conn, &path);
    match &res {
        Ok(state) => info!("Toggled star for {} -> {}", path, state),
        Err(e) => error!("Failed to toggle star for {}: {}", path, e),
    }
    res
}

#[tauri::command]
fn list_starred(sort: Option<SortSpec>) -> Result<Vec<FsEntry>, String> {
    let conn = db::open()?;
    let entries = db::starred_entries(&conn)?;
    let mut out = Vec::new();
    for (p, _) in &entries {
        let pb = PathBuf::from(p);
        if let Ok(meta) = fs::symlink_metadata(&pb) {
            let is_link = meta.file_type().is_symlink();
            out.push(build_entry(&pb, &meta, is_link, true));
        }
    }
    sort_entries(&mut out, sort);
    Ok(out)
}

#[tauri::command]
fn list_recent(sort: Option<SortSpec>) -> Result<Vec<FsEntry>, String> {
    let conn = db::open()?;
    let star_set: HashSet<String> = db::starred_set(&conn)?;
    let mut out = Vec::new();
    for p in db::recent_paths(&conn)? {
        let pb = PathBuf::from(&p);
        if let Ok(meta) = fs::symlink_metadata(&pb) {
            let is_link = meta.file_type().is_symlink();
            let starred = star_set.contains(&p);
            out.push(build_entry(&pb, &meta, is_link, starred));
        }
    }
    sort_entries(&mut out, sort);
    Ok(out)
}

#[tauri::command]
fn list_trash(sort: Option<SortSpec>) -> Result<DirListing, String> {
    let conn = db::open()?;
    let star_set: HashSet<String> = db::starred_set(&conn)?;
    match resolve_trash_dir()? {
        Some(target) => list_dir_with_star(target, &star_set, sort),
        None => {
            #[cfg(target_os = "windows")]
            {
                return list_windows_trash(&star_set, sort);
            }
            Ok(DirListing {
                current: "Trash (unavailable)".to_string(),
                entries: Vec::new(),
            })
        }
    }
}

#[tauri::command]
fn rename_entry(path: String, new_name: String) -> Result<String, String> {
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
fn move_to_trash(path: String) -> Result<(), String> {
    let src = sanitize_path_nofollow(&path, true)?;
    let trash_dir = resolve_trash_dir()?.ok_or_else(|| "Trash not available on this platform".to_string())?;
    std::fs::create_dir_all(&trash_dir).map_err(|e| format!("Failed to create trash dir: {e}"))?;
    let file_name = src
        .file_name()
        .ok_or_else(|| "Invalid path".to_string())?;
    let dest = unique_path(&trash_dir.join(file_name));
    fs::rename(&src, &dest).map_err(|e| format!("Failed to move to trash: {e}"))
}

#[tauri::command]
fn delete_entry(path: String) -> Result<(), String> {
    let pb = sanitize_path_nofollow(&path, true)?;
    let meta = fs::symlink_metadata(&pb).map_err(|e| format!("Failed to read metadata: {e}"))?;
    if meta.is_dir() {
        fs::remove_dir_all(&pb).map_err(|e| format!("Failed to delete directory: {e}"))
    } else {
        fs::remove_file(&pb).map_err(|e| format!("Failed to delete file: {e}"))
    }
}

fn init_logging() {
    static GUARD: OnceCell<tracing_appender::non_blocking::WorkerGuard> = OnceCell::new();
    let log_dir = dirs_next::data_dir()
        .unwrap_or_else(|| std::env::temp_dir())
        .join("filey")
        .join("logs");
    if let Err(e) = std::fs::create_dir_all(&log_dir) {
        eprintln!("Failed to create log dir {:?}: {}", log_dir, e);
        return;
    }
    let file_appender = tracing_appender::rolling::never(&log_dir, "filey.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
    let _ = GUARD.set(guard);
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("info".parse().unwrap()),
        )
        .with_ansi(false)
        .with_writer(non_blocking)
        .try_init();
}

fn main() {
    init_logging();
    tauri::Builder::default()
        .manage(WatchState::default())
        .invoke_handler(tauri::generate_handler![
            list_dir,
            search,
            list_mounts,
            get_bookmarks,
            add_bookmark,
            remove_bookmark,
            watch_dir,
            open_entry,
            toggle_star,
            list_starred,
            list_recent,
            list_trash,
            store_column_widths,
            load_saved_column_widths,
            dir_sizes,
            context_menu_actions,
            rename_entry,
            move_to_trash,
            delete_entry,
            entry_times_cmd,
            set_clipboard_cmd,
            paste_clipboard_cmd
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[tauri::command]
fn entry_times_cmd(path: String) -> Result<EntryTimes, String> {
    let pb = PathBuf::from(path);
    entry_times(&pb)
}
