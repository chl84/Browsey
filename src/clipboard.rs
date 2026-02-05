use crate::{
    fs_utils::sanitize_path_follow,
    tasks::CancelState,
    undo::{move_with_fallback, run_actions, temp_backup_path, Action, Direction, UndoState},
};
mod clipboard_size;
use clipboard_size::estimate_total_size;
use once_cell::sync::Lazy;
use serde::Serialize;
#[cfg(not(target_os = "windows"))]
use std::io::BufRead;
#[cfg(not(target_os = "windows"))]
use std::process::Command;
use std::{
    fs,
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::{atomic::AtomicBool, Mutex},
};
use tauri::Emitter;

#[derive(Clone, Copy)]
enum ClipboardMode {
    Copy,
    Cut,
}

#[derive(Clone)]
struct ClipboardState {
    entries: Vec<PathBuf>,
    mode: ClipboardMode,
}

#[derive(Clone, Copy)]
enum ConflictPolicy {
    Rename,
    Overwrite,
}

#[derive(Serialize, Clone, Copy)]
struct CopyProgressPayload {
    bytes: u64,
    total: u64,
    finished: bool,
}

#[derive(serde::Serialize)]
pub struct ConflictInfo {
    pub src: String,
    pub target: String,
    pub exists: bool,
    pub is_dir: bool,
}

static CLIPBOARD: Lazy<Mutex<Option<ClipboardState>>> = Lazy::new(|| Mutex::new(None));

fn ensure_not_child(src: &Path, dest: &Path) -> Result<(), String> {
    if dest.starts_with(src) {
        return Err("Cannot paste a directory into itself".into());
    }
    Ok(())
}

fn copy_dir(
    src: &Path,
    dest: &Path,
    app: Option<&tauri::AppHandle>,
    progress_event: Option<&str>,
    cancel: Option<&AtomicBool>,
) -> Result<(), String> {
    fs::create_dir_all(dest).map_err(|e| format!("Failed to create dir {:?}: {e}", dest))?;
    for entry in fs::read_dir(src).map_err(|e| format!("Failed to read dir {:?}: {e}", src))? {
        let entry = entry.map_err(|e| format!("Failed to read dir entry: {e}"))?;
        let path = entry.path();
        let meta =
            fs::symlink_metadata(&path).map_err(|e| format!("Failed to read metadata: {e}"))?;
        if cancel
            .map(|c| c.load(std::sync::atomic::Ordering::Relaxed))
            .unwrap_or(false)
        {
            return Err("Copy cancelled".into());
        }
        if meta.file_type().is_symlink() {
            return Err("Refusing to copy symlinks".into());
        }
        let target = dest.join(entry.file_name());
        if meta.is_dir() {
            ensure_not_child(&path, &target)?;
            copy_dir(&path, &target, app, progress_event, cancel)?;
        } else {
            copy_file_best_effort(&path, &target, app, progress_event, cancel, None)?;
        }
    }
    Ok(())
}

fn backup_existing_target(target: &Path, actions: &mut Vec<Action>) -> Result<(), String> {
    let backup = temp_backup_path(target);
    let parent = backup
        .parent()
        .ok_or_else(|| "Invalid backup path".to_string())?;
    fs::create_dir_all(parent)
        .map_err(|e| format!("Failed to create backup parent {}: {e}", parent.display()))?;
    move_with_fallback(target, &backup)?;
    actions.push(Action::Delete {
        path: target.to_path_buf(),
        backup,
    });
    Ok(())
}

fn merge_dir(
    src: &Path,
    dest: &Path,
    mode: ClipboardMode,
    actions: &mut Vec<Action>,
    app: Option<&tauri::AppHandle>,
    progress_event: Option<&str>,
    cancel: Option<&AtomicBool>,
) -> Result<(), String> {
    // Ensure both exist and are directories.
    let src_meta =
        fs::symlink_metadata(src).map_err(|e| format!("Failed to read source metadata: {e}"))?;
    let dest_meta =
        fs::symlink_metadata(dest).map_err(|e| format!("Failed to read target metadata: {e}"))?;
    if !src_meta.is_dir() || !dest_meta.is_dir() {
        return Err("Merge requires both source and target to be directories".into());
    }

    for entry in fs::read_dir(src).map_err(|e| format!("Failed to read dir {:?}: {e}", src))? {
        let entry = entry.map_err(|e| format!("Failed to read dir entry: {e}"))?;
        let path = entry.path();
        let meta =
            fs::symlink_metadata(&path).map_err(|e| format!("Failed to read metadata: {e}"))?;
        if meta.file_type().is_symlink() {
            return Err("Refusing to copy symlinks".into());
        }
        if cancel
            .map(|c| c.load(std::sync::atomic::Ordering::Relaxed))
            .unwrap_or(false)
        {
            return Err("Copy cancelled".into());
        }
        let target = dest.join(entry.file_name());
        if meta.is_dir() {
            if target.exists() && target.is_dir() {
                merge_dir(&path, &target, mode, actions, app, progress_event, cancel)?;
            } else {
                if target.exists() {
                    backup_existing_target(&target, actions)?;
                }
                match mode {
                    ClipboardMode::Copy => {
                        copy_dir(&path, &target, app, progress_event, cancel)?;
                        actions.push(Action::Copy {
                            from: path.clone(),
                            to: target.clone(),
                        });
                    }
                    ClipboardMode::Cut => {
                        move_entry(&path, &target, app, progress_event, cancel)?;
                        actions.push(Action::Move {
                            from: path.clone(),
                            to: target.clone(),
                        });
                    }
                }
            }
        } else {
            if target.exists() {
                backup_existing_target(&target, actions)?;
            }
            match mode {
                ClipboardMode::Copy => {
                    fs::copy(&path, &target)
                        .map_err(|e| format!("Failed to copy file {:?}: {e}", path))?;
                    actions.push(Action::Copy {
                        from: path.clone(),
                        to: target.clone(),
                    });
                }
                ClipboardMode::Cut => {
                    move_entry(&path, &target, app, progress_event, cancel)?;
                    actions.push(Action::Move {
                        from: path.clone(),
                        to: target.clone(),
                    });
                }
            }
        }
    }

    if let ClipboardMode::Cut = mode {
        // Remove source directory but keep an empty backup so undo can recreate it
        // before moving items back.
        let backup = temp_backup_path(src);
        if let Some(parent) = backup.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create backup parent {}: {e}", parent.display()))?;
        }
        fs::create_dir_all(&backup)
            .map_err(|e| format!("Failed to create backup dir {}: {e}", backup.display()))?;
        fs::remove_dir_all(src).map_err(|e| format!("Failed to remove source dir: {e}"))?;
        actions.push(Action::Delete {
            path: src.to_path_buf(),
            backup,
        });
    }
    Ok(())
}

fn copy_entry(
    src: &Path,
    dest: &Path,
    app: Option<&tauri::AppHandle>,
    progress_event: Option<&str>,
    cancel: Option<&AtomicBool>,
) -> Result<(), String> {
    let meta = fs::symlink_metadata(src).map_err(|e| format!("Failed to read metadata: {e}"))?;
    if meta.file_type().is_symlink() {
        return Err("Refusing to copy symlinks".into());
    }
    if meta.is_dir() {
        ensure_not_child(src, dest)?;
        copy_dir(src, dest, app, progress_event, cancel)
    } else {
        if cancel
            .map(|c| c.load(std::sync::atomic::Ordering::Relaxed))
            .unwrap_or(false)
        {
            return Err("Copy cancelled".into());
        }
        let size_hint = Some(meta.len());
        copy_file_best_effort(src, dest, app, progress_event, cancel, size_hint)?;
        Ok(())
    }
}

fn copy_file_best_effort(
    src: &Path,
    dest: &Path,
    app: Option<&tauri::AppHandle>,
    progress_event: Option<&str>,
    cancel: Option<&AtomicBool>,
    total_hint: Option<u64>,
) -> Result<u64, String> {
    #[cfg(not(target_os = "windows"))]
    {
        if is_gvfs_path(src) || is_gvfs_path(dest) {
            if let Some(app) = app {
                if let Some(bytes) =
                    try_gio_copy_progress(src, dest, app, progress_event, cancel, total_hint)?
                {
                    return Ok(bytes);
                }
            }
        }
    }

    // Fallback: manual chunked copy with progress
    let mut reader =
        fs::File::open(src).map_err(|e| format!("Failed to open source for copy: {e}"))?;
    let mut writer = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(true)
        .open(dest)
        .map_err(|e| format!("Failed to open target for copy: {e}"))?;

    let mut buf = vec![0u8; 512 * 1024];
    let mut done: u64 = 0;
    let total =
        total_hint.or_else(|| progress_event.and_then(|_| fs::metadata(src).ok().map(|m| m.len())));
    let mut last_emit = 0u64;
    let mut last_time = std::time::Instant::now();
    loop {
        if cancel
            .map(|c| c.load(std::sync::atomic::Ordering::Relaxed))
            .unwrap_or(false)
        {
            let _ = fs::remove_file(dest);
            if let (Some(app), Some(evt)) = (app, progress_event) {
                let _ = app.emit(
                    evt,
                    CopyProgressPayload {
                        bytes: done,
                        total: total.unwrap_or(done),
                        finished: true,
                    },
                );
            }
            return Err("Copy cancelled".into());
        }
        let n = reader
            .read(&mut buf)
            .map_err(|e| format!("Read failed: {e}"))?;
        if n == 0 {
            break;
        }
        writer
            .write_all(&buf[..n])
            .map_err(|e| format!("Write failed: {e}"))?;
        done = done.saturating_add(n as u64);
        if let (Some(app), Some(evt)) = (app, progress_event) {
            let elapsed = last_time.elapsed();
            if done.saturating_sub(last_emit) >= 64 * 1024
                || elapsed >= std::time::Duration::from_millis(200)
            {
                let _ = app.emit(
                    evt,
                    CopyProgressPayload {
                        bytes: done,
                        total: total.unwrap_or(0),
                        finished: false,
                    },
                );
                last_emit = done;
                last_time = std::time::Instant::now();
            }
        }
    }
    if let (Some(app), Some(evt)) = (app, progress_event) {
        let _ = app.emit(
            evt,
            CopyProgressPayload {
                bytes: done,
                total: total.unwrap_or(done),
                finished: true,
            },
        );
    }
    Ok(done)
}

#[cfg(not(target_os = "windows"))]
fn try_gio_copy_progress(
    src: &Path,
    dest: &Path,
    app: &tauri::AppHandle,
    progress_event: Option<&str>,
    cancel: Option<&AtomicBool>,
    total_hint: Option<u64>,
) -> Result<Option<u64>, String> {
    let mut cmd = Command::new("gio");
    cmd.arg("copy").arg("--progress").arg(src).arg(dest);
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    let mut child = match cmd.spawn() {
        Ok(c) => c,
        Err(_) => return Ok(None),
    };

    let stdout = child.stdout.take();
    let mut total_seen: Option<u64> = total_hint;
    let mut last_bytes: u64 = 0;

    if let Some(out) = stdout {
        let reader = std::io::BufReader::new(out);
        for line in reader.lines().flatten() {
            if cancel
                .map(|c| c.load(std::sync::atomic::Ordering::Relaxed))
                .unwrap_or(false)
            {
                let _ = child.kill();
                let _ = child.wait();
                return Err("Copy cancelled".into());
            }

            // Parse integers in the line; expect two numbers = transferred, total.
            let nums: Vec<u64> = line
                .split(|c: char| !c.is_ascii_digit())
                .filter(|s| !s.is_empty())
                .filter_map(|s| s.parse::<u64>().ok())
                .collect();
            if nums.len() >= 2 {
                last_bytes = nums[0];
                total_seen = Some(nums[1]);
                if let (Some(evt), Some(total)) = (progress_event, total_seen) {
                    let _ = app.emit(
                        evt,
                        CopyProgressPayload {
                            bytes: last_bytes,
                            total,
                            finished: false,
                        },
                    );
                }
            }
        }
    }

    let status = child
        .wait()
        .map_err(|e| format!("gio copy wait failed: {e}"))?;
    if status.success() {
        if let Some(evt) = progress_event {
            let _ = app.emit(
                evt,
                CopyProgressPayload {
                    bytes: last_bytes,
                    total: total_seen.unwrap_or(last_bytes),
                    finished: true,
                },
            );
        }
        return Ok(Some(last_bytes));
    }

    Ok(None)
}

#[cfg(not(target_os = "windows"))]
fn is_gvfs_path(path: &Path) -> bool {
    path.to_string_lossy().to_lowercase().contains("/gvfs/")
}

fn delete_entry_path(path: &Path) -> Result<(), String> {
    let meta = fs::symlink_metadata(path).map_err(|e| format!("Failed to read metadata: {e}"))?;
    if meta.is_dir() {
        fs::remove_dir_all(path).map_err(|e| format!("Failed to delete directory: {e}"))
    } else {
        fs::remove_file(path).map_err(|e| format!("Failed to delete file: {e}"))
    }
}

fn move_entry(
    src: &Path,
    dest: &Path,
    app: Option<&tauri::AppHandle>,
    progress_event: Option<&str>,
    cancel: Option<&AtomicBool>,
) -> Result<(), String> {
    ensure_not_child(src, dest)?;
    match fs::rename(src, dest) {
        Ok(_) => Ok(()),
        Err(_) => {
            copy_entry(src, dest, app, progress_event, cancel)?;
            delete_entry_path(src)
        }
    }
}

fn policy_from_str(policy: &str) -> Result<ConflictPolicy, String> {
    match policy.to_lowercase().as_str() {
        "overwrite" => Ok(ConflictPolicy::Overwrite),
        "rename" => Ok(ConflictPolicy::Rename),
        other => Err(format!("Invalid conflict policy: {}", other)),
    }
}

fn next_unique_name(base: &Path) -> PathBuf {
    if !base.exists() {
        return base.to_path_buf();
    }
    let parent = base.parent().unwrap_or_else(|| Path::new("."));
    let original = base
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("item")
        .to_string();
    let (stem, ext) = original
        .rsplit_once('.')
        .map(|(s, e)| (s.to_string(), Some(e.to_string())))
        .unwrap_or_else(|| (original.clone(), None));

    // Start at 1 because base is known to already exist in conflict scenarios.
    let mut idx: usize = 1;
    loop {
        let candidate_name = match &ext {
            Some(ext) => format!("{stem}-{idx}.{ext}"),
            None => format!("{stem}-{idx}"),
        };
        let candidate = parent.join(&candidate_name);
        if !candidate.exists() {
            return candidate;
        }
        idx = idx.saturating_add(1);
    }
}

#[tauri::command]
pub fn set_clipboard_cmd(paths: Vec<String>, mode: String) -> Result<(), String> {
    if paths.is_empty() {
        let mut guard = CLIPBOARD.lock().unwrap();
        *guard = None;
        return Ok(());
    }
    let parsed_mode = match mode.to_lowercase().as_str() {
        "copy" => ClipboardMode::Copy,
        "cut" => ClipboardMode::Cut,
        _ => return Err("Invalid mode".into()),
    };
    let mut entries = Vec::new();
    for p in paths {
        let meta = fs::symlink_metadata(&p).map_err(|e| format!("Path does not exist: {e}"))?;
        if meta.file_type().is_symlink() {
            return Err("Symlinks are not supported in clipboard".into());
        }
        let clean = sanitize_path_follow(&p, true)?;
        entries.push(clean);
    }
    let mut guard = CLIPBOARD.lock().unwrap();
    *guard = Some(ClipboardState {
        entries,
        mode: parsed_mode,
    });
    Ok(())
}

fn current_clipboard() -> Option<ClipboardState> {
    let guard = CLIPBOARD.lock().unwrap();
    guard.clone()
}

#[tauri::command]
pub fn paste_clipboard_preview(dest: String) -> Result<Vec<ConflictInfo>, String> {
    let dest = sanitize_path_follow(&dest, false)?;
    let Some(state) = current_clipboard() else {
        return Err("Clipboard is empty".into());
    };

    let mut conflicts = Vec::new();
    for src in state.entries.iter() {
        if !src.exists() {
            return Err(format!("Source does not exist: {:?}", src));
        }
        let name = src
            .file_name()
            .ok_or_else(|| "Invalid source path".to_string())?;
        let target = dest.join(name);
        let exists = target.exists();
        let is_dir = target.is_dir();
        conflicts.push(ConflictInfo {
            src: src.to_string_lossy().to_string(),
            target: target.to_string_lossy().to_string(),
            exists,
            is_dir,
        });
    }
    Ok(conflicts.into_iter().filter(|c| c.exists).collect())
}

#[tauri::command]
pub async fn paste_clipboard_cmd(
    app: tauri::AppHandle,
    dest: String,
    policy: Option<String>,
    undo: tauri::State<'_, UndoState>,
    cancel: tauri::State<'_, CancelState>,
    progress_event: Option<String>,
) -> Result<Vec<String>, String> {
    let undo_inner = undo.clone_inner();
    let cancel_state = cancel.inner().clone();
    let app_handle = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        paste_clipboard_impl(
            app_handle,
            dest,
            policy,
            undo_inner,
            cancel_state,
            progress_event,
        )
    })
    .await
    .map_err(|e| format!("Paste task failed: {e}"))?
}

fn paste_clipboard_impl(
    app: tauri::AppHandle,
    dest: String,
    policy: Option<String>,
    undo_inner: std::sync::Arc<std::sync::Mutex<crate::undo::UndoManager>>,
    cancel_state: CancelState,
    progress_event: Option<String>,
) -> Result<Vec<String>, String> {
    let dest = sanitize_path_follow(&dest, false)?;
    let state = current_clipboard().ok_or_else(|| "Clipboard is empty".to_string())?;
    let policy = policy
        .map(|p| policy_from_str(&p))
        .transpose()?
        .unwrap_or(ConflictPolicy::Rename);

    let cancel_guard = progress_event
        .as_ref()
        .map(|id| cancel_state.register(id.clone()))
        .transpose()?;
    let cancel_flag = cancel_guard.as_ref().map(|g| g.token());

    let total_items = state.entries.len() as u64;
    let total_bytes = progress_event
        .as_ref()
        .map(|evt| estimate_total_size(&state.entries, evt, &app));
    let mut done_items: u64 = 0;
    if let (Some(evt), Some(total)) = (progress_event.as_ref(), total_bytes) {
        let _ = app.emit(
            evt,
            CopyProgressPayload {
                bytes: done_items,
                total,
                finished: false,
            },
        );
    }

    let mut created = Vec::new();
    let mut performed: Vec<Action> = Vec::with_capacity(state.entries.len() * 4);
    for src in state.entries.iter() {
        if cancel_flag
            .as_ref()
            .map(|c| c.load(std::sync::atomic::Ordering::Relaxed))
            .unwrap_or(false)
        {
            return Err("Copy cancelled".into());
        }
        if !src.exists() {
            return Err(format!("Source does not exist: {:?}", src));
        }
        let src_meta =
            fs::symlink_metadata(src).map_err(|e| format!("Failed to read metadata: {e}"))?;
        if src_meta.file_type().is_symlink() {
            return Err("Symlinks are not supported in clipboard".into());
        }
        let name = src
            .file_name()
            .ok_or_else(|| "Invalid source path".to_string())?;
        let target_base = dest.join(name);
        let mut target = match policy {
            ConflictPolicy::Rename => next_unique_name(&target_base),
            ConflictPolicy::Overwrite => target_base.clone(),
        };

        if matches!(policy, ConflictPolicy::Overwrite) && target.exists() {
            // If both are dirs, merge instead of deleting target (Windows Explorer behavior).
            if src_meta.is_dir() && target.is_dir() {
                merge_dir(
                    src,
                    &target,
                    state.mode,
                    &mut performed,
                    Some(&app),
                    progress_event.as_deref(),
                    cancel_flag.as_deref(),
                )?;
                created.push(target.to_string_lossy().to_string());
                continue;
            }
            // Prevent deleting parent/ancestor of the source.
            if src.starts_with(&target) {
                return Err("Cannot overwrite a parent directory of the source item".into());
            }
            backup_existing_target(&target, &mut performed)?;
        }

        let mut attempts = 0usize;
        loop {
            let result = match state.mode {
                ClipboardMode::Copy => copy_entry(
                    src,
                    &target,
                    Some(&app),
                    progress_event.as_deref(),
                    cancel_flag.as_deref(),
                ),
                ClipboardMode::Cut => move_entry(
                    src,
                    &target,
                    Some(&app),
                    progress_event.as_deref(),
                    cancel_flag.as_deref(),
                ),
            };

            match result {
                Ok(_) => {
                    done_items = done_items.saturating_add(1);
                    if total_bytes.is_none() {
                        if let Some(evt) = progress_event.as_ref() {
                            let _ = app.emit(
                                evt,
                                CopyProgressPayload {
                                    bytes: done_items,
                                    total: total_items,
                                    finished: false,
                                },
                            );
                        }
                    }
                    break;
                }
                Err(err) => {
                    let is_exists = err.contains("exists") || err.contains("AlreadyExists");
                    if matches!(policy, ConflictPolicy::Rename) && is_exists && attempts < 50 {
                        attempts += 1;
                        target = next_unique_name(&target);
                        continue;
                    }
                    if !performed.is_empty() {
                        let mut rollback = performed.clone();
                        if let Err(rb_err) = run_actions(&mut rollback, Direction::Backward) {
                            return Err(format!(
                                "Paste failed for {:?}: {}; rollback also failed: {}",
                                src, err, rb_err
                            ));
                        }
                    }
                    return Err(format!("Paste failed for {:?}: {}", src, err));
                }
            }
        }
        let action = match state.mode {
            ClipboardMode::Copy => Action::Copy {
                from: src.clone(),
                to: target.clone(),
            },
            ClipboardMode::Cut => Action::Move {
                from: src.clone(),
                to: target.clone(),
            },
        };
        performed.push(action);
        created.push(target.to_string_lossy().to_string());
    }

    if let Some(evt) = progress_event.as_ref() {
        let _ = app.emit(
            evt,
            CopyProgressPayload {
                bytes: total_bytes.unwrap_or(done_items),
                total: total_bytes.unwrap_or(total_items),
                finished: true,
            },
        );
    }

    if !performed.is_empty() {
        let recorded = if performed.len() == 1 {
            performed.pop().unwrap()
        } else {
            Action::Batch(performed)
        };
        if let Ok(mut mgr) = undo_inner.lock() {
            mgr.record_applied(recorded);
        }
    }

    if let ClipboardMode::Cut = state.mode {
        let mut guard = CLIPBOARD.lock().unwrap();
        *guard = None;
    }

    Ok(created)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;
    use std::fs;
    use std::io::Write;
    use std::path::Path;
    use std::sync::OnceLock;
    use std::time::{Duration, SystemTime};

    fn uniq_path(label: &str) -> PathBuf {
        let ts = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or(Duration::from_secs(0))
            .as_nanos();
        env::temp_dir().join(format!("browsey-cliptest-{label}-{ts}"))
    }

    fn ensure_undo_dir() -> PathBuf {
        static DIR: OnceLock<PathBuf> = OnceLock::new();
        DIR.get_or_init(|| {
            let dir = uniq_path("undo-base");
            let _ = fs::remove_dir_all(&dir);
            env::set_var("BROWSEY_UNDO_DIR", &dir);
            dir
        })
        .clone()
    }

    fn write_file(path: &Path, content: &[u8]) {
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let mut f = fs::File::create(path).unwrap();
        f.write_all(content).unwrap();
    }

    #[test]
    fn merge_copy_can_undo_without_touching_existing() {
        let _ = ensure_undo_dir();
        let base = uniq_path("merge-copy");
        let dest = base.join("dest");
        fs::create_dir_all(&dest).unwrap();
        write_file(&dest.join("old.txt"), b"old");

        let src = dest.join("child");
        fs::create_dir_all(&src).unwrap();
        write_file(&src.join("a.txt"), b"a");

        let mut actions = Vec::new();
        merge_dir(
            &src,
            &dest,
            ClipboardMode::Copy,
            &mut actions,
            None,
            None,
            None,
        )
        .unwrap();

        assert!(dest.join("old.txt").exists());
        assert!(dest.join("a.txt").exists());
        assert!(src.join("a.txt").exists());

        run_actions(&mut actions, Direction::Backward).unwrap();

        assert!(dest.join("old.txt").exists());
        assert!(!dest.join("a.txt").exists());
        assert!(src.join("a.txt").exists());

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn merge_cut_undo_restores_source_and_target() {
        let _ = ensure_undo_dir();
        let base = uniq_path("merge-cut");
        let dest = base.join("dest");
        fs::create_dir_all(&dest).unwrap();
        write_file(&dest.join("old.txt"), b"old");

        let src = dest.join("child");
        fs::create_dir_all(&src).unwrap();
        write_file(&src.join("a.txt"), b"a");

        let mut actions = Vec::new();
        merge_dir(
            &src,
            &dest,
            ClipboardMode::Cut,
            &mut actions,
            None,
            None,
            None,
        )
        .unwrap();

        assert!(dest.join("old.txt").exists());
        assert!(dest.join("a.txt").exists());
        assert!(!src.exists());

        run_actions(&mut actions, Direction::Backward).unwrap();

        assert!(src.join("a.txt").exists());
        assert!(dest.join("old.txt").exists());
        assert!(!dest.join("a.txt").exists());

        let _ = fs::remove_dir_all(&base);
    }
}
