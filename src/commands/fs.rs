//! File-system oriented Tauri commands: listings, mounts, trash, opening, renaming, deleting, and watch wiring.

use super::tasks::CancelState;
use crate::{
    db,
    entry::{build_entry, CachedMeta, FsEntry},
    fs_utils::{
        check_no_symlink_components, debug_log, sanitize_path_follow, sanitize_path_nofollow,
    },
    runtime_lifecycle,
    sorting::{sort_entries, SortSpec},
    undo::{
        assert_path_snapshot, copy_entry as undo_copy_entry, delete_entry_path as undo_delete_path,
        move_with_fallback, rename_entry_nofollow_io, run_actions, snapshot_existing_path,
        temp_backup_path, Action, Direction, PathSnapshot, UndoState,
    },
};
#[cfg(target_os = "windows")]
#[path = "fs_windows.rs"]
pub mod fs_windows;
#[cfg(not(target_os = "windows"))]
pub mod gvfs;
pub use crate::commands::listing::DirListing;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::ffi::OsString;
#[cfg(target_os = "windows")]
use std::os::windows::prelude::*;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use std::{
    fs, io,
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
};
#[cfg(debug_assertions)]
use tracing::info;
use tracing::{error, warn};
use trash::{
    delete as trash_delete,
    os_limited::{list as trash_list, metadata as trash_metadata, purge_all, restore_all},
    TrashItem, TrashItemSize,
};

pub fn expand_path(raw: Option<String>) -> Result<PathBuf, String> {
    if let Some(p) = raw {
        if p == "~" {
            dirs_next::home_dir().ok_or_else(|| "Home directory not found".to_string())
        } else if let Some(stripped) = p.strip_prefix("~/") {
            let home =
                dirs_next::home_dir().ok_or_else(|| "Home directory not found".to_string())?;
            Ok(home.join(stripped))
        } else {
            Ok(PathBuf::from(p))
        }
    } else if let Some(home) = dirs_next::home_dir() {
        Ok(home)
    } else {
        std::env::current_dir().map_err(|e| format!("Failed to read working directory: {e}"))
    }
}

#[derive(Serialize, Clone)]
pub struct MountInfo {
    pub label: String,
    pub path: String,
    pub fs: String,
    pub removable: bool,
}

#[cfg(not(target_os = "windows"))]
const OPEN_TIMEOUT_GVFS: Duration = Duration::from_secs(8);

#[cfg(not(target_os = "windows"))]
fn is_gvfs_path(path: &Path) -> bool {
    let s = path.to_string_lossy();
    s.contains("/gvfs/") || s.contains("\\gvfs\\")
}

pub(crate) fn entry_from_cached(path: &Path, cached: &CachedMeta, starred: bool) -> FsEntry {
    let kind = if cached.is_link {
        "link"
    } else if cached.is_dir {
        "dir"
    } else {
        "file"
    }
    .to_string();
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string());
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_string());

    FsEntry {
        name,
        path: path.to_string_lossy().into_owned(),
        kind,
        ext,
        size: cached.size,
        items: None,
        modified: cached.modified.clone(),
        original_path: None,
        trash_id: None,
        icon_id: cached.icon_id,
        starred,
        hidden: cached.hidden,
        network: cached.network,
        read_only: cached.read_only,
        read_denied: cached.read_denied,
    }
}

#[cfg(not(target_os = "windows"))]
fn restorable_file_in_trash_from_info_file(info_file: &Path) -> PathBuf {
    let trash_folder = info_file.parent().and_then(|p| p.parent());
    let name_in_trash = info_file.file_stem();
    match (trash_folder, name_in_trash) {
        (Some(folder), Some(name)) => folder.join("files").join(name),
        _ => PathBuf::from(info_file),
    }
}

fn trash_item_path(item: &TrashItem) -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        PathBuf::from(&item.id)
    }
    #[cfg(not(target_os = "windows"))]
    {
        restorable_file_in_trash_from_info_file(Path::new(&item.id))
    }
}

#[tauri::command]
pub fn open_entry(path: String) -> Result<(), String> {
    let pb = sanitize_path_follow(&path, false)?;
    let conn = db::open()?;
    if let Err(e) = db::touch_recent(&conn, &pb.to_string_lossy()) {
        warn!("Failed to record recent for {:?}: {}", pb, e);
    }
    #[cfg(debug_assertions)]
    info!("Opening path {:?}", pb);
    #[cfg(not(target_os = "windows"))]
    {
        if is_gvfs_path(&pb) {
            let (tx, rx) = mpsc::channel();
            let path_for_open = pb.clone();
            std::thread::spawn(move || {
                let res =
                    open::that_detached(&path_for_open).map_err(|e| format!("Failed to open: {e}"));
                let _ = tx.send(res);
            });
            let res = match rx.recv_timeout(OPEN_TIMEOUT_GVFS) {
                Ok(res) => res.map_err(|e| {
                    error!("Failed to open {:?}: {}", pb, e);
                    e
                }),
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    error!("Open timed out for {:?}", pb);
                    Err("Open timed out on remote device".into())
                }
                Err(_) => {
                    error!("Open channel closed for {:?}", pb);
                    Err("Failed to open".into())
                }
            };
            return res;
        }
    }
    open::that_detached(&pb).map_err(|e| {
        error!("Failed to open {:?}: {}", pb, e);
        format!("Failed to open: {e}")
    })
}

#[cfg(target_os = "windows")]
fn set_hidden_attr(path: &Path, hidden: bool) -> Result<PathBuf, String> {
    use windows_sys::Win32::Storage::FileSystem::{
        GetFileAttributesW, SetFileAttributesW, FILE_ATTRIBUTE_HIDDEN,
    };
    let wide: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let attrs = unsafe { GetFileAttributesW(wide.as_ptr()) };
    if attrs == u32::MAX {
        return Err("GetFileAttributes failed".into());
    }
    let mut new_attrs = attrs;
    if hidden {
        new_attrs |= FILE_ATTRIBUTE_HIDDEN;
    } else {
        new_attrs &= !FILE_ATTRIBUTE_HIDDEN;
    }
    let ok = unsafe { SetFileAttributesW(wide.as_ptr(), new_attrs) };
    if ok == 0 {
        return Err("SetFileAttributes failed".into());
    }
    Ok(path.to_path_buf())
}

#[cfg(not(target_os = "windows"))]
fn set_hidden_attr(path: &Path, hidden: bool) -> Result<PathBuf, String> {
    // On Unix, hidden = leading dot. Rename within same directory.
    let file_name = path
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| "Invalid file name".to_string())?;
    let is_dot = file_name.starts_with('.');
    if hidden == is_dot {
        return Ok(path.to_path_buf());
    }
    let target_name = if hidden {
        format!(".{file_name}")
    } else {
        file_name.trim_start_matches('.').to_string()
    };
    if target_name.is_empty() {
        return Err("Cannot derive visible name".into());
    }
    let parent = path.parent().ok_or_else(|| "Missing parent".to_string())?;
    let target = parent.join(&target_name);
    if target == path {
        return Ok(path.to_path_buf());
    }
    if target.exists() {
        return Err(format!("Target already exists: {}", target.display()));
    }
    fs::rename(path, &target).map_err(|e| format!("Failed to rename: {e}"))?;
    Ok(target)
}

#[tauri::command]
pub fn set_hidden(
    path: Option<String>,
    paths: Option<Vec<String>>,
    hidden: bool,
) -> Result<Vec<String>, String> {
    let targets: Vec<String> = match (paths, path) {
        (Some(list), _) if !list.is_empty() => list,
        (_, Some(single)) => vec![single],
        _ => return Err("No paths provided".into()),
    };
    if targets.is_empty() {
        return Err("No paths provided".into());
    }
    let mut results = Vec::with_capacity(targets.len());
    for raw in targets {
        let pb = sanitize_path_nofollow(&raw, true)?;
        check_no_symlink_components(&pb)?;
        let new_path = set_hidden_attr(&pb, hidden)?;
        results.push(new_path.to_string_lossy().into_owned());
    }
    Ok(results)
}

#[tauri::command]
pub fn list_trash(sort: Option<SortSpec>) -> Result<DirListing, String> {
    let items = trash_list().map_err(|e| format!("Failed to list trash: {e}"))?;
    let mut entries = Vec::new();
    for item in items {
        let path = trash_item_path(&item);
        let meta = match fs::symlink_metadata(&path) {
            Ok(m) => m,
            Err(e) => {
                debug_log(&format!(
                    "trash list: missing item path={}, skipping: {e:?}",
                    path.display()
                ));
                continue;
            }
        };
        let is_link = meta.file_type().is_symlink();
        let mut entry = build_entry(&path, &meta, is_link, false);
        let original_path = item.original_path();
        entry.name = original_path
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| original_path.to_string_lossy().into_owned());
        entry.ext = original_path
            .extension()
            .and_then(|e| e.to_str())
            .map(|s| s.to_string());
        entry.original_path = Some(original_path.to_string_lossy().into_owned());
        entry.trash_id = Some(item.id.to_string_lossy().into_owned());
        if let Ok(info) = trash_metadata(&item) {
            match info.size {
                TrashItemSize::Bytes(b) => entry.size = Some(b),
                TrashItemSize::Entries(n) => entry.items = Some(n as u64),
            }
        }
        entries.push(entry);
    }
    sort_entries(&mut entries, sort);
    Ok(DirListing {
        current: "Trash".to_string(),
        entries,
    })
}

#[tauri::command]
pub fn rename_entry(
    path: String,
    new_name: String,
    state: tauri::State<UndoState>,
) -> Result<String, String> {
    let (from, to) = prepare_rename_pair(path.as_str(), new_name.as_str())?;
    apply_rename(&from, &to)?;
    let _ = state.record_applied(Action::Rename {
        from: from.clone(),
        to: to.clone(),
    });
    Ok(to.to_string_lossy().to_string())
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RenameEntryRequest {
    pub path: String,
    pub new_name: String,
}

fn build_rename_target(from: &Path, new_name: &str) -> Result<PathBuf, String> {
    if new_name.trim().is_empty() {
        return Err("New name cannot be empty".into());
    }
    let parent = from
        .parent()
        .ok_or_else(|| "Cannot rename root".to_string())?;
    Ok(parent.join(new_name.trim()))
}

fn prepare_rename_pair(path: &str, new_name: &str) -> Result<(PathBuf, PathBuf), String> {
    let from = sanitize_path_nofollow(path, true)?;
    let to = build_rename_target(&from, new_name)?;
    Ok((from, to))
}

fn apply_rename(from: &Path, to: &Path) -> Result<(), String> {
    ensure_existing_path_nonsymlink(from)?;
    let from_snapshot = snapshot_existing_path(from)?;
    if let Some(parent) = to.parent() {
        ensure_existing_dir_nonsymlink(parent)?;
        let parent_snapshot = snapshot_existing_path(parent)?;
        assert_path_snapshot(parent, &parent_snapshot)?;
    } else {
        return Err("Invalid destination path".into());
    }
    assert_path_snapshot(from, &from_snapshot)?;
    match rename_entry_nofollow_io(from, to) {
        Ok(_) => Ok(()),
        Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
            Err("A file or directory with that name already exists".into())
        }
        Err(e) => Err(format!("Failed to rename: {e}")),
    }
}

#[tauri::command]
pub fn rename_entries(
    entries: Vec<RenameEntryRequest>,
    undo: tauri::State<UndoState>,
) -> Result<Vec<String>, String> {
    if entries.is_empty() {
        return Ok(Vec::new());
    }

    let mut pairs: Vec<(PathBuf, PathBuf)> = Vec::with_capacity(entries.len());
    let mut seen_sources: HashSet<PathBuf> = HashSet::with_capacity(entries.len());
    let mut seen_targets: HashSet<PathBuf> = HashSet::with_capacity(entries.len());

    for (idx, entry) in entries.into_iter().enumerate() {
        let (from, to) = prepare_rename_pair(entry.path.as_str(), entry.new_name.as_str())?;

        if !seen_sources.insert(from.clone()) {
            return Err(format!(
                "Duplicate source path in request (item {})",
                idx + 1
            ));
        }
        if !seen_targets.insert(to.clone()) {
            return Err(format!(
                "Duplicate target name in request (item {})",
                idx + 1
            ));
        }

        pairs.push((from, to));
    }

    let mut performed: Vec<Action> = Vec::new();
    let mut renamed_paths: Vec<String> = Vec::with_capacity(pairs.len());

    for (from, to) in pairs {
        if from == to {
            continue;
        }
        if let Err(err) = apply_rename(&from, &to) {
            if !performed.is_empty() {
                let mut rollback = performed.clone();
                if let Err(rb_err) = run_actions(&mut rollback, Direction::Backward) {
                    return Err(format!("{}; rollback also failed: {}", err, rb_err));
                }
            }
            return Err(err);
        }

        renamed_paths.push(to.to_string_lossy().to_string());
        performed.push(Action::Rename {
            from: from.clone(),
            to: to.clone(),
        });
    }

    if !performed.is_empty() {
        let recorded = if performed.len() == 1 {
            performed.pop().unwrap()
        } else {
            Action::Batch(performed)
        };
        let _ = undo.record_applied(recorded);
    }

    Ok(renamed_paths)
}

#[tauri::command]
pub fn create_folder(
    path: String,
    name: String,
    state: tauri::State<UndoState>,
) -> Result<String, String> {
    let base = sanitize_path_follow(&path, true)?;
    ensure_existing_dir_nonsymlink(&base)?;
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("Folder name cannot be empty".into());
    }
    if trimmed.contains(['/', '\\']) {
        return Err("Folder name cannot contain path separators".into());
    }
    let target = base.join(trimmed);
    if target.exists() {
        return Err("A file or directory with that name already exists".into());
    }
    fs::create_dir(&target).map_err(|e| format!("Failed to create folder: {e}"))?;
    let _ = state.record_applied(Action::CreateFolder {
        path: target.clone(),
    });
    Ok(target.to_string_lossy().into_owned())
}

#[tauri::command]
pub fn create_file(
    path: String,
    name: String,
    state: tauri::State<UndoState>,
) -> Result<String, String> {
    let base = sanitize_path_follow(&path, true)?;
    ensure_existing_dir_nonsymlink(&base)?;

    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err("File name cannot be empty".into());
    }
    if trimmed.contains(['/', '\\']) {
        return Err("File name cannot contain path separators".into());
    }

    let target = base.join(trimmed);
    if target.exists() {
        return Err("A file or directory with that name already exists".into());
    }

    fs::File::options()
        .write(true)
        .create_new(true)
        .open(&target)
        .map_err(|e| format!("Failed to create file: {e}"))?;

    let backup = temp_backup_path(&target);
    let _ = state.record_applied(Action::Create {
        path: target.clone(),
        backup,
    });

    Ok(target.to_string_lossy().into_owned())
}

#[tauri::command]
pub async fn move_to_trash(
    path: String,
    app: tauri::AppHandle,
    undo: tauri::State<'_, UndoState>,
) -> Result<(), String> {
    let app_handle = app.clone();
    let undo_state = undo.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let action = move_single_to_trash(&path, &app_handle, true)?;
        let _ = undo_state.record_applied(action);
        Ok(())
    })
    .await
    .map_err(|e| format!("Move to trash task failed: {e}"))?
}

fn emit_trash_progress(
    app: &tauri::AppHandle,
    event: Option<&String>,
    done: u64,
    total: u64,
    finished: bool,
    last_emit: &mut Instant,
) {
    if let Some(evt) = event {
        let now = Instant::now();
        if finished || now.duration_since(*last_emit) >= Duration::from_millis(100) {
            let payload = DeleteProgressPayload {
                bytes: done,
                total,
                finished,
            };
            let _ = runtime_lifecycle::emit_if_running(app, evt, payload);
            *last_emit = now;
        }
    }
}

struct PreparedTrashMove {
    src: std::path::PathBuf,
    backup: std::path::PathBuf,
    src_snapshot: PathSnapshot,
    staged_src: Option<std::path::PathBuf>,
}

fn stage_for_trash(src: &Path) -> Result<PathBuf, String> {
    let parent = src
        .parent()
        .ok_or_else(|| format!("Invalid source path: {}", src.display()))?;
    let seed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let pid = std::process::id();
    for attempt in 0..64u32 {
        let staged = parent.join(format!(".browsey-trash-stage-{pid}-{seed}-{attempt}"));
        match rename_entry_nofollow_io(src, &staged) {
            Ok(_) => return Ok(staged),
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(err) => {
                return Err(format!(
                    "Failed to stage {} for trash: {err}",
                    src.display()
                ));
            }
        }
    }
    Err(format!(
        "Failed to allocate a temporary staged path for {}",
        src.display()
    ))
}

fn trash_delete_via_staged_rename(
    src: &Path,
    src_snapshot: &PathSnapshot,
) -> Result<PathBuf, String> {
    assert_path_snapshot(src, src_snapshot)?;
    let staged = stage_for_trash(src)?;
    match trash_delete(&staged) {
        Ok(_) => Ok(staged),
        Err(err) => {
            let rollback = rename_entry_nofollow_io(&staged, src)
                .err()
                .map(|e| format!("; rollback failed: {e}"))
                .unwrap_or_default();
            Err(format!("Failed to move to trash: {err}{rollback}"))
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn rewrite_trash_info_original_path(item: &TrashItem, original_path: &Path) -> Result<(), String> {
    let info_path = PathBuf::from(&item.id);
    let contents = fs::read_to_string(&info_path).map_err(|e| {
        format!(
            "Failed to read trash info file {}: {e}",
            info_path.display()
        )
    })?;

    let replacement = format!("Path={}", original_path.to_string_lossy());
    let mut output = String::with_capacity(contents.len() + replacement.len() + 8);
    let mut replaced = false;
    for line in contents.lines() {
        if line.starts_with("Path=") {
            output.push_str(&replacement);
            output.push('\n');
            replaced = true;
        } else {
            output.push_str(line);
            output.push('\n');
        }
    }
    if !replaced {
        if !output.ends_with('\n') {
            output.push('\n');
        }
        output.push_str(&replacement);
        output.push('\n');
    }

    fs::write(&info_path, output).map_err(|e| {
        format!(
            "Failed to update trash info file {}: {e}",
            info_path.display()
        )
    })
}

#[cfg(target_os = "windows")]
fn rewrite_trash_info_original_path(
    _item: &TrashItem,
    _original_path: &Path,
) -> Result<(), String> {
    Ok(())
}

fn should_abort_fs_op(app: &tauri::AppHandle, cancel: Option<&AtomicBool>) -> bool {
    runtime_lifecycle::is_shutting_down(app)
        || cancel
            .map(|flag| flag.load(Ordering::Relaxed))
            .unwrap_or(false)
}

fn rollback_prepared_trash(prepared: &[PreparedTrashMove]) {
    let mut rollback: Vec<Action> = prepared
        .iter()
        .map(|p| Action::Delete {
            path: p.src.clone(),
            backup: p.backup.clone(),
        })
        .collect();
    let _ = run_actions(&mut rollback, Direction::Backward);
}

fn prepare_trash_move(raw: &str) -> Result<PreparedTrashMove, String> {
    let src = sanitize_path_nofollow(raw, true)?;
    check_no_symlink_components(&src)?;
    let src_snapshot = snapshot_existing_path(&src)?;

    // Backup into the central undo directory in case we cannot locate the trash item path later.
    let backup = temp_backup_path(&src);
    if let Some(parent) = backup.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create backup dir {}: {e}", parent.display()))?;
    }
    undo_copy_entry(&src, &backup)?;
    if let Err(err) = assert_path_snapshot(&src, &src_snapshot) {
        let _ = undo_delete_path(&backup);
        return Err(err);
    }

    Ok(PreparedTrashMove {
        src,
        backup,
        src_snapshot,
        staged_src: None,
    })
}

fn move_to_trash_many_blocking(
    paths: Vec<String>,
    app: tauri::AppHandle,
    undo: UndoState,
    progress_event: Option<String>,
    cancel: Option<&AtomicBool>,
) -> Result<(), String> {
    let total = paths.len() as u64;
    if total == 0 {
        emit_trash_progress(
            &app,
            progress_event.as_ref(),
            0,
            0,
            true,
            &mut Instant::now(),
        );
        return Ok(());
    }
    // Capture current trash contents once to avoid O(n^2) directory scans.
    let before_ids: HashSet<OsString> = trash_list()
        .map_err(|e| format!("Failed to list trash: {e}"))?
        .into_iter()
        .map(|item| item.id)
        .collect();

    let mut prepared: Vec<PreparedTrashMove> = Vec::with_capacity(paths.len());
    let mut done = 0u64;
    let mut last_emit = Instant::now();
    for path in paths {
        if should_abort_fs_op(&app, cancel) {
            rollback_prepared_trash(&prepared);
            emit_trash_progress(
                &app,
                progress_event.as_ref(),
                done,
                total,
                true,
                &mut last_emit,
            );
            return Err("Move to trash cancelled".into());
        }
        match prepare_trash_move(&path) {
            Ok(mut prep) => match trash_delete_via_staged_rename(&prep.src, &prep.src_snapshot) {
                Ok(staged_src) => {
                    prep.staged_src = Some(staged_src);
                    done = done.saturating_add(1);
                    emit_trash_progress(
                        &app,
                        progress_event.as_ref(),
                        done,
                        total,
                        done == total,
                        &mut last_emit,
                    );
                    prepared.push(prep);
                }
                Err(err) => {
                    rollback_prepared_trash(&prepared);
                    let _ = undo_delete_path(&prep.backup);
                    emit_trash_progress(
                        &app,
                        progress_event.as_ref(),
                        done,
                        total,
                        true,
                        &mut last_emit,
                    );
                    return Err(err);
                }
            },
            Err(err) => {
                // Nothing was moved for this entry; roll back previous ones.
                rollback_prepared_trash(&prepared);
                emit_trash_progress(
                    &app,
                    progress_event.as_ref(),
                    done,
                    total,
                    true,
                    &mut last_emit,
                );
                return Err(err);
            }
        }
    }

    // Identify new trash items with a single post-scan.
    let mut new_items: HashMap<std::path::PathBuf, TrashItem> = HashMap::new();
    if let Ok(after) = trash_list() {
        for item in after.into_iter().filter(|i| !before_ids.contains(&i.id)) {
            new_items.insert(item.original_path(), item);
        }
    }

    let mut actions = Vec::with_capacity(prepared.len());
    for prep in prepared {
        let lookup = prep.staged_src.as_ref().unwrap_or(&prep.src);
        if let Some(item) = new_items.remove(lookup) {
            if let Err(err) = rewrite_trash_info_original_path(&item, &prep.src) {
                warn!(
                    "Failed to rewrite trash info for {}: {}",
                    prep.src.display(),
                    err
                );
            }
            let _ = undo_delete_path(&prep.backup);
            actions.push(Action::Move {
                from: prep.src,
                to: trash_item_path(&item),
            });
        } else {
            actions.push(Action::Delete {
                path: prep.src,
                backup: prep.backup,
            });
        }
    }

    let recorded = if actions.len() == 1 {
        actions.pop().unwrap()
    } else {
        Action::Batch(actions)
    };
    let _ = undo.record_applied(recorded);
    let _ = runtime_lifecycle::emit_if_running(&app, "trash-changed", ());
    Ok(())
}

#[tauri::command]
pub async fn move_to_trash_many(
    paths: Vec<String>,
    app: tauri::AppHandle,
    undo: tauri::State<'_, UndoState>,
    cancel: tauri::State<'_, CancelState>,
    progress_event: Option<String>,
) -> Result<(), String> {
    let app_handle = app.clone();
    let undo_state = undo.inner().clone();
    let cancel_state = cancel.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        let cancel_guard = progress_event
            .as_ref()
            .map(|id| cancel_state.register(id.clone()))
            .transpose()?;
        let cancel_token = cancel_guard.as_ref().map(|g| g.token());
        move_to_trash_many_blocking(
            paths,
            app_handle,
            undo_state,
            progress_event,
            cancel_token.as_deref(),
        )
    })
    .await
    .map_err(|e| format!("Move to trash task failed: {e}"))?
}

fn move_single_to_trash(
    path: &str,
    app: &tauri::AppHandle,
    emit_event: bool,
) -> Result<Action, String> {
    let src = sanitize_path_nofollow(&path, true)?;
    check_no_symlink_components(&src)?;
    let src_snapshot = snapshot_existing_path(&src)?;

    // Backup into the central undo directory in case the OS trash item can't be found.
    let backup = temp_backup_path(&src);
    if let Some(parent) = backup.parent() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create backup dir {}: {e}", parent.display()))?;
    }
    undo_copy_entry(&src, &backup)?;

    let before: HashSet<OsString> = trash_list()
        .map_err(|e| format!("Failed to list trash: {e}"))?
        .into_iter()
        .map(|item| item.id)
        .collect();

    let staged_src = match trash_delete_via_staged_rename(&src, &src_snapshot) {
        Ok(staged) => staged,
        Err(err) => {
            let _ = undo_delete_path(&backup);
            return Err(err);
        }
    };
    if emit_event {
        let _ = runtime_lifecycle::emit_if_running(app, "trash-changed", ());
    }

    let trashed_item = trash_list().ok().and_then(|after| {
        after
            .into_iter()
            .find(|item| !before.contains(&item.id) && item.original_path() == staged_src)
    });

    match trashed_item {
        Some(item) => {
            if let Err(err) = rewrite_trash_info_original_path(&item, &src) {
                warn!(
                    "Failed to rewrite trash info for {}: {}",
                    src.display(),
                    err
                );
            }
            // Remove the backup once we know the trash location.
            let _ = undo_delete_path(&backup);
            Ok(Action::Move {
                from: src,
                to: trash_item_path(&item),
            })
        }
        None => Ok(Action::Delete { path: src, backup }),
    }
}

#[tauri::command]
pub fn restore_trash_items(ids: Vec<String>, app: tauri::AppHandle) -> Result<(), String> {
    let wanted: HashSet<OsString> = ids.into_iter().map(OsString::from).collect();
    if wanted.is_empty() {
        return Ok(());
    }
    let items = trash_list().map_err(|e| format!("Failed to list trash: {e}"))?;
    let selected: Vec<_> = items
        .into_iter()
        .filter(|item| wanted.contains(&item.id))
        .collect();
    if selected.is_empty() {
        return Err("Nothing to restore".into());
    }
    restore_all(selected)
        .map_err(|e| format!("Failed to restore: {e}"))
        .map(|_| {
            let _ = runtime_lifecycle::emit_if_running(&app, "trash-changed", ());
        })
}

#[tauri::command]
pub fn purge_trash_items(ids: Vec<String>, app: tauri::AppHandle) -> Result<(), String> {
    let wanted: HashSet<OsString> = ids.into_iter().map(OsString::from).collect();
    if wanted.is_empty() {
        return Ok(());
    }
    let items = trash_list().map_err(|e| format!("Failed to list trash: {e}"))?;
    let selected: Vec<_> = items
        .into_iter()
        .filter(|item| wanted.contains(&item.id))
        .collect();
    if selected.is_empty() {
        return Err("Nothing to delete".into());
    }
    purge_all(selected)
        .map_err(|e| format!("Failed to delete permanently: {e}"))
        .map(|_| {
            let _ = runtime_lifecycle::emit_if_running(&app, "trash-changed", ());
        })
}

#[tauri::command]
pub fn delete_entry(path: String, state: tauri::State<UndoState>) -> Result<(), String> {
    let pb = sanitize_path_nofollow(&path, true)?;
    let action = delete_with_backup(&pb)?;
    let _ = state.record_applied(action);
    Ok(())
}

fn delete_with_backup(path: &Path) -> Result<Action, String> {
    ensure_existing_path_nonsymlink(path)?;
    let src_snapshot = snapshot_existing_path(path)?;
    let backup = temp_backup_path(path);
    if let Some(parent) = path.parent() {
        ensure_existing_dir_nonsymlink(parent)?;
    }
    if let Some(parent) = backup.parent() {
        ensure_no_symlink_components_existing_prefix(parent)?;
        fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create backup dir {}: {e}", parent.display()))?;
        ensure_existing_dir_nonsymlink(parent)?;
    }
    // Bruk samme robuste flytt som undo-systemet (copy+delete fallback)
    assert_path_snapshot(path, &src_snapshot)?;
    move_with_fallback(path, &backup)?;
    Ok(Action::Delete {
        path: path.to_path_buf(),
        backup,
    })
}

#[derive(Serialize, Clone)]
pub struct DeleteProgressPayload {
    pub bytes: u64,
    pub total: u64,
    pub finished: bool,
}

fn emit_delete_progress(
    app: &tauri::AppHandle,
    event: Option<&String>,
    done: u64,
    total: u64,
    finished: bool,
    last_emit: &mut Instant,
) {
    if let Some(evt) = event {
        let now = Instant::now();
        if finished || now.duration_since(*last_emit) >= Duration::from_millis(100) {
            let payload = DeleteProgressPayload {
                bytes: done,
                total,
                finished,
            };
            let _ = runtime_lifecycle::emit_if_running(app, evt, payload);
            *last_emit = now;
        }
    }
}

fn delete_entries_blocking(
    app: tauri::AppHandle,
    paths: Vec<String>,
    progress_event: Option<String>,
    undo: UndoState,
    cancel: Option<&AtomicBool>,
) -> Result<(), String> {
    if paths.is_empty() {
        return Ok(());
    }
    let total = paths.len() as u64;
    let mut done = 0u64;
    let mut last_emit = Instant::now();
    let mut performed: Vec<Action> = Vec::with_capacity(paths.len());
    for raw in paths {
        if should_abort_fs_op(&app, cancel) {
            if !performed.is_empty() {
                let mut rollback = performed.clone();
                let _ = run_actions(&mut rollback, Direction::Backward);
            }
            emit_delete_progress(
                &app,
                progress_event.as_ref(),
                done,
                total,
                true,
                &mut last_emit,
            );
            return Err("Delete cancelled".into());
        }
        let path = sanitize_path_nofollow(&raw, true)?;
        match delete_with_backup(&path) {
            Ok(action) => {
                performed.push(action);
                done = done.saturating_add(1);
                emit_delete_progress(
                    &app,
                    progress_event.as_ref(),
                    done,
                    total,
                    false,
                    &mut last_emit,
                );
            }
            Err(err) => {
                if !performed.is_empty() {
                    let mut rollback = performed.clone();
                    if let Err(rb_err) = run_actions(&mut rollback, Direction::Backward) {
                        return Err(format!(
                            "Failed to delete {}: {}; rollback also failed: {}",
                            path.display(),
                            err,
                            rb_err
                        ));
                    }
                }
                return Err(format!("Failed to delete {}: {}", path.display(), err));
            }
        }
    }
    if !performed.is_empty() {
        let recorded = if performed.len() == 1 {
            performed.pop().unwrap()
        } else {
            Action::Batch(performed)
        };
        let _ = undo.record_applied(recorded);
    }
    emit_delete_progress(
        &app,
        progress_event.as_ref(),
        done,
        total,
        true,
        &mut last_emit,
    );
    Ok(())
}

#[tauri::command]
pub async fn delete_entries(
    app: tauri::AppHandle,
    paths: Vec<String>,
    progress_event: Option<String>,
    undo: tauri::State<'_, UndoState>,
    cancel: tauri::State<'_, CancelState>,
) -> Result<(), String> {
    let undo = undo.inner().clone();
    let cancel_state = cancel.inner().clone();
    let task = tauri::async_runtime::spawn_blocking(move || {
        let cancel_guard = progress_event
            .as_ref()
            .map(|id| cancel_state.register(id.clone()))
            .transpose()?;
        let cancel_token = cancel_guard.as_ref().map(|g| g.token());
        delete_entries_blocking(app, paths, progress_event, undo, cancel_token.as_deref())
    });
    task.await.map_err(|e| format!("Delete task failed: {e}"))?
}

fn ensure_existing_path_nonsymlink(path: &Path) -> Result<fs::Metadata, String> {
    check_no_symlink_components(path)?;
    let meta = fs::symlink_metadata(path)
        .map_err(|e| format!("Failed to read metadata for {}: {e}", path.display()))?;
    if meta.file_type().is_symlink() {
        return Err(format!("Symlinks are not allowed: {}", path.display()));
    }
    Ok(meta)
}

fn ensure_existing_dir_nonsymlink(path: &Path) -> Result<(), String> {
    let meta = ensure_existing_path_nonsymlink(path)?;
    if !meta.is_dir() {
        return Err("Destination is not a directory".into());
    }
    Ok(())
}

fn ensure_no_symlink_components_existing_prefix(path: &Path) -> Result<(), String> {
    let mut acc = PathBuf::new();
    for comp in path.components() {
        match comp {
            std::path::Component::Prefix(p) => {
                acc.push(p.as_os_str());
                continue;
            }
            std::path::Component::RootDir => {
                acc.push(std::path::Component::RootDir.as_os_str());
                continue;
            }
            std::path::Component::CurDir => continue,
            std::path::Component::ParentDir => {
                acc.pop();
                continue;
            }
            std::path::Component::Normal(seg) => acc.push(seg),
        }
        if acc.as_os_str().is_empty() {
            continue;
        }
        match fs::symlink_metadata(&acc) {
            Ok(meta) => {
                if meta.file_type().is_symlink() {
                    return Err(format!(
                        "Symlinks are not allowed in path: {}",
                        acc.display()
                    ));
                }
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => break,
            Err(e) => {
                return Err(format!(
                    "Failed to read metadata for {}: {e}",
                    acc.display()
                ));
            }
        }
    }
    Ok(())
}
