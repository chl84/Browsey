use crate::{
    runtime_lifecycle,
    undo::{move_with_fallback, temp_backup_path, Action},
};
#[cfg(not(target_os = "windows"))]
use std::io::BufRead;
#[cfg(not(target_os = "windows"))]
use std::process::Command;
use std::{
    fs,
    io::{ErrorKind, Read, Write},
    path::Path,
    sync::atomic::AtomicBool,
};

use super::{
    error::{ClipboardError, ClipboardErrorCode, ClipboardResult},
    ClipboardMode, CopyProgressPayload,
};

fn ensure_not_child(src: &Path, dest: &Path) -> ClipboardResult<()> {
    if dest.starts_with(src) {
        return Err(ClipboardError::invalid_input(
            "Cannot paste a directory into itself",
        ));
    }
    Ok(())
}

pub(super) fn transfer_cancelled(
    cancel: Option<&AtomicBool>,
    app: Option<&tauri::AppHandle>,
) -> bool {
    cancel
        .map(|c| c.load(std::sync::atomic::Ordering::Relaxed))
        .unwrap_or(false)
        || app
            .map(runtime_lifecycle::is_shutting_down)
            .unwrap_or(false)
}

fn emit_copy_progress(
    app: Option<&tauri::AppHandle>,
    event: Option<&str>,
    payload: CopyProgressPayload,
) {
    if let (Some(app), Some(evt)) = (app, event) {
        let _ = runtime_lifecycle::emit_if_running(app, evt, payload);
    }
}

fn copy_dir(
    src: &Path,
    dest: &Path,
    app: Option<&tauri::AppHandle>,
    progress_event: Option<&str>,
    cancel: Option<&AtomicBool>,
) -> ClipboardResult<()> {
    fs::create_dir(dest).map_err(|e| {
        if e.kind() == ErrorKind::AlreadyExists {
            ClipboardError::new(
                ClipboardErrorCode::DestinationExists,
                format!("Destination already exists: {}", dest.display()),
            )
        } else {
            ClipboardError::new(
                ClipboardErrorCode::IoError,
                format!("Failed to create dir {:?}: {e}", dest),
            )
        }
    })?;
    for entry in fs::read_dir(src).map_err(|e| {
        ClipboardError::new(
            ClipboardErrorCode::IoError,
            format!("Failed to read dir {:?}: {e}", src),
        )
    })? {
        let entry = entry.map_err(|e| {
            ClipboardError::new(
                ClipboardErrorCode::IoError,
                format!("Failed to read dir entry: {e}"),
            )
        })?;
        let path = entry.path();
        let meta = fs::symlink_metadata(&path).map_err(|e| {
            ClipboardError::new(
                ClipboardErrorCode::IoError,
                format!("Failed to read metadata: {e}"),
            )
        })?;
        if transfer_cancelled(cancel, app) {
            return Err(ClipboardError::cancelled());
        }
        if meta.file_type().is_symlink() {
            return Err(ClipboardError::new(
                ClipboardErrorCode::SymlinkUnsupported,
                "Refusing to copy symlinks",
            ));
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

pub(super) fn backup_existing_target(
    target: &Path,
    actions: &mut Vec<Action>,
) -> ClipboardResult<()> {
    let backup = temp_backup_path(target);
    let parent = backup
        .parent()
        .ok_or_else(|| ClipboardError::invalid_input("Invalid backup path"))?;
    fs::create_dir_all(parent).map_err(|e| {
        ClipboardError::new(
            ClipboardErrorCode::IoError,
            format!("Failed to create backup parent {}: {e}", parent.display()),
        )
    })?;
    move_with_fallback(target, &backup).map_err(ClipboardError::from_external_message)?;
    actions.push(Action::Delete {
        path: target.to_path_buf(),
        backup,
    });
    Ok(())
}

pub(super) fn merge_dir(
    src: &Path,
    dest: &Path,
    mode: ClipboardMode,
    actions: &mut Vec<Action>,
    app: Option<&tauri::AppHandle>,
    progress_event: Option<&str>,
    cancel: Option<&AtomicBool>,
) -> ClipboardResult<()> {
    // Ensure both exist and are directories.
    let src_meta = fs::symlink_metadata(src).map_err(|e| {
        ClipboardError::new(
            ClipboardErrorCode::IoError,
            format!("Failed to read source metadata: {e}"),
        )
    })?;
    let dest_meta = fs::symlink_metadata(dest).map_err(|e| {
        ClipboardError::new(
            ClipboardErrorCode::IoError,
            format!("Failed to read target metadata: {e}"),
        )
    })?;
    if !src_meta.is_dir() || !dest_meta.is_dir() {
        return Err(ClipboardError::new(
            ClipboardErrorCode::InvalidInput,
            "Merge requires both source and target to be directories",
        ));
    }

    for entry in fs::read_dir(src).map_err(|e| {
        ClipboardError::new(
            ClipboardErrorCode::IoError,
            format!("Failed to read dir {:?}: {e}", src),
        )
    })? {
        let entry = entry.map_err(|e| {
            ClipboardError::new(
                ClipboardErrorCode::IoError,
                format!("Failed to read dir entry: {e}"),
            )
        })?;
        let path = entry.path();
        let meta = fs::symlink_metadata(&path).map_err(|e| {
            ClipboardError::new(
                ClipboardErrorCode::IoError,
                format!("Failed to read metadata: {e}"),
            )
        })?;
        if meta.file_type().is_symlink() {
            return Err(ClipboardError::new(
                ClipboardErrorCode::SymlinkUnsupported,
                "Refusing to copy symlinks",
            ));
        }
        if transfer_cancelled(cancel, app) {
            return Err(ClipboardError::cancelled());
        }
        let target = dest.join(entry.file_name());
        let target_meta = metadata_if_exists_nofollow(&target)?;
        if meta.is_dir() {
            if matches!(target_meta, Some(ref m) if m.file_type().is_symlink()) {
                return Err(ClipboardError::new(
                    ClipboardErrorCode::SymlinkUnsupported,
                    "Refusing to overwrite symlinks",
                ));
            }
            if matches!(target_meta, Some(ref m) if m.is_dir()) {
                merge_dir(&path, &target, mode, actions, app, progress_event, cancel)?;
            } else {
                if target_meta.is_some() {
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
            if matches!(target_meta, Some(ref m) if m.file_type().is_symlink()) {
                return Err(ClipboardError::new(
                    ClipboardErrorCode::SymlinkUnsupported,
                    "Refusing to overwrite symlinks",
                ));
            }
            if target_meta.is_some() {
                backup_existing_target(&target, actions)?;
            }
            match mode {
                ClipboardMode::Copy => {
                    let hint = Some(meta.len());
                    copy_file_best_effort(&path, &target, app, progress_event, cancel, hint)?;
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
            fs::create_dir_all(parent).map_err(|e| {
                ClipboardError::new(
                    ClipboardErrorCode::IoError,
                    format!("Failed to create backup parent {}: {e}", parent.display()),
                )
            })?;
        }
        fs::create_dir_all(&backup).map_err(|e| {
            ClipboardError::new(
                ClipboardErrorCode::IoError,
                format!("Failed to create backup dir {}: {e}", backup.display()),
            )
        })?;
        fs::remove_dir_all(src).map_err(|e| {
            ClipboardError::new(
                ClipboardErrorCode::IoError,
                format!("Failed to remove source dir: {e}"),
            )
        })?;
        actions.push(Action::Delete {
            path: src.to_path_buf(),
            backup,
        });
    }
    Ok(())
}

pub(super) fn copy_entry(
    src: &Path,
    dest: &Path,
    app: Option<&tauri::AppHandle>,
    progress_event: Option<&str>,
    cancel: Option<&AtomicBool>,
) -> ClipboardResult<()> {
    let meta = fs::symlink_metadata(src).map_err(|e| {
        ClipboardError::new(
            ClipboardErrorCode::IoError,
            format!("Failed to read metadata: {e}"),
        )
    })?;
    if meta.file_type().is_symlink() {
        return Err(ClipboardError::new(
            ClipboardErrorCode::SymlinkUnsupported,
            "Refusing to copy symlinks",
        ));
    }
    if meta.is_dir() {
        ensure_not_child(src, dest)?;
        copy_dir(src, dest, app, progress_event, cancel)
    } else {
        if transfer_cancelled(cancel, app) {
            return Err(ClipboardError::cancelled());
        }
        let size_hint = Some(meta.len());
        copy_file_best_effort(src, dest, app, progress_event, cancel, size_hint)?;
        Ok(())
    }
}

pub(super) fn copy_file_best_effort(
    src: &Path,
    dest: &Path,
    app: Option<&tauri::AppHandle>,
    progress_event: Option<&str>,
    cancel: Option<&AtomicBool>,
    total_hint: Option<u64>,
) -> ClipboardResult<u64> {
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
    let mut reader = fs::File::open(src).map_err(|e| {
        ClipboardError::new(
            ClipboardErrorCode::IoError,
            format!("Failed to open source for copy: {e}"),
        )
    })?;
    let mut writer = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(dest)
        .map_err(|e| {
            if e.kind() == ErrorKind::AlreadyExists {
                ClipboardError::new(
                    ClipboardErrorCode::DestinationExists,
                    format!("Destination already exists: {}", dest.display()),
                )
            } else {
                ClipboardError::new(
                    ClipboardErrorCode::IoError,
                    format!("Failed to open target for copy: {e}"),
                )
            }
        })?;

    let mut buf = vec![0u8; 512 * 1024];
    let mut done: u64 = 0;
    let total =
        total_hint.or_else(|| progress_event.and_then(|_| fs::metadata(src).ok().map(|m| m.len())));
    let mut last_emit = 0u64;
    let mut last_time = std::time::Instant::now();
    loop {
        if transfer_cancelled(cancel, app) {
            let _ = fs::remove_file(dest);
            emit_copy_progress(
                app,
                progress_event,
                CopyProgressPayload {
                    bytes: done,
                    total: total.unwrap_or(done),
                    finished: true,
                },
            );
            return Err(ClipboardError::cancelled());
        }
        let n = reader.read(&mut buf).map_err(|e| {
            ClipboardError::new(ClipboardErrorCode::IoError, format!("Read failed: {e}"))
        })?;
        if n == 0 {
            break;
        }
        writer.write_all(&buf[..n]).map_err(|e| {
            ClipboardError::new(ClipboardErrorCode::IoError, format!("Write failed: {e}"))
        })?;
        done = done.saturating_add(n as u64);
        if progress_event.is_some() {
            let elapsed = last_time.elapsed();
            if done.saturating_sub(last_emit) >= 64 * 1024
                || elapsed >= std::time::Duration::from_millis(200)
            {
                emit_copy_progress(
                    app,
                    progress_event,
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
    emit_copy_progress(
        app,
        progress_event,
        CopyProgressPayload {
            bytes: done,
            total: total.unwrap_or(done),
            finished: true,
        },
    );
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
) -> ClipboardResult<Option<u64>> {
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
        for line in reader.lines().map_while(Result::ok) {
            if transfer_cancelled(cancel, Some(app)) {
                let _ = child.kill();
                let _ = child.wait();
                return Err(ClipboardError::cancelled());
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
                    let _ = runtime_lifecycle::emit_if_running(
                        app,
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

    let status = child.wait().map_err(|e| {
        ClipboardError::new(
            ClipboardErrorCode::IoError,
            format!("gio copy wait failed: {e}"),
        )
    })?;
    if status.success() {
        if let Some(evt) = progress_event {
            let _ = runtime_lifecycle::emit_if_running(
                app,
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

fn delete_entry_path(path: &Path) -> ClipboardResult<()> {
    let meta = fs::symlink_metadata(path).map_err(|e| {
        ClipboardError::new(
            ClipboardErrorCode::IoError,
            format!("Failed to read metadata: {e}"),
        )
    })?;
    if meta.is_dir() {
        fs::remove_dir_all(path).map_err(|e| {
            ClipboardError::new(
                ClipboardErrorCode::IoError,
                format!("Failed to delete directory: {e}"),
            )
        })
    } else {
        fs::remove_file(path).map_err(|e| {
            ClipboardError::new(
                ClipboardErrorCode::IoError,
                format!("Failed to delete file: {e}"),
            )
        })
    }
}

pub(super) fn move_entry(
    src: &Path,
    dest: &Path,
    app: Option<&tauri::AppHandle>,
    progress_event: Option<&str>,
    cancel: Option<&AtomicBool>,
) -> ClipboardResult<()> {
    ensure_not_child(src, dest)?;
    match fs::rename(src, dest) {
        Ok(_) => Ok(()),
        Err(_) => {
            copy_entry(src, dest, app, progress_event, cancel)?;
            delete_entry_path(src)
        }
    }
}

pub(super) fn metadata_if_exists_nofollow(path: &Path) -> ClipboardResult<Option<fs::Metadata>> {
    match fs::symlink_metadata(path) {
        Ok(meta) => Ok(Some(meta)),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(None),
        Err(err) => Err(ClipboardError::new(
            ClipboardErrorCode::IoError,
            format!("Failed to read metadata for {}: {err}", path.display()),
        )),
    }
}

pub(super) fn is_destination_exists_error(err: &ClipboardError) -> bool {
    if err.code() == ClipboardErrorCode::DestinationExists {
        return true;
    }
    let lower = err.to_string().to_lowercase();
    lower.contains("already exists")
        || lower.contains("file exists")
        || lower.contains("destination exists")
        || lower.contains("os error 17")
        || lower.contains("os error 183")
}
