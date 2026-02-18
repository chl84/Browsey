use crate::{
    fs_utils::sanitize_path_follow,
    runtime_lifecycle,
    tasks::CancelState,
    undo::{run_actions, Action, Direction, UndoState},
};
mod clipboard_size;
mod drop_mode;
mod ops;
#[cfg(test)]
mod tests;

use clipboard_size::estimate_total_size;
use once_cell::sync::Lazy;
use serde::Serialize;
use std::{
    fs,
    io::ErrorKind,
    path::{Path, PathBuf},
    sync::Mutex,
};

#[cfg(test)]
use drop_mode::resolve_drop_clipboard_mode_impl;
#[cfg(test)]
use ops::copy_file_best_effort;
use ops::{
    backup_existing_target, copy_entry, is_destination_exists_error, merge_dir,
    metadata_if_exists_nofollow, move_entry, transfer_cancelled,
};

pub use drop_mode::resolve_drop_clipboard_mode;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
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
pub(crate) struct CopyProgressPayload {
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

fn policy_from_str(policy: &str) -> Result<ConflictPolicy, String> {
    match policy.to_lowercase().as_str() {
        "overwrite" => Ok(ConflictPolicy::Overwrite),
        "rename" => Ok(ConflictPolicy::Rename),
        other => Err(format!("Invalid conflict policy: {}", other)),
    }
}

fn rename_candidate(base: &Path, idx: usize) -> PathBuf {
    if idx == 0 {
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

    let candidate_name = match &ext {
        Some(ext) => format!("{stem}-{idx}.{ext}"),
        None => format!("{stem}-{idx}"),
    };
    parent.join(candidate_name)
}

fn current_clipboard() -> Option<ClipboardState> {
    let guard = CLIPBOARD.lock().unwrap();
    guard.clone()
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
    if runtime_lifecycle::is_shutting_down(&app) {
        return Err("Copy cancelled".into());
    }
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
        let _ = runtime_lifecycle::emit_if_running(
            &app,
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
        if transfer_cancelled(cancel_flag.as_deref(), Some(&app)) {
            return Err("Copy cancelled".into());
        }
        let src_meta = match fs::symlink_metadata(src) {
            Ok(meta) => meta,
            Err(e) if e.kind() == ErrorKind::NotFound => {
                return Err(format!("Source does not exist: {:?}", src));
            }
            Err(e) => return Err(format!("Failed to read metadata: {e}")),
        };
        if src_meta.file_type().is_symlink() {
            return Err("Symlinks are not supported in clipboard".into());
        }

        let name = src
            .file_name()
            .ok_or_else(|| "Invalid source path".to_string())?;
        let target_base = dest.join(name);
        let mut rename_attempt = 0usize;
        let mut target = match policy {
            ConflictPolicy::Rename => rename_candidate(&target_base, rename_attempt),
            ConflictPolicy::Overwrite => target_base.clone(),
        };

        if matches!(policy, ConflictPolicy::Overwrite) {
            if let Some(target_meta) = metadata_if_exists_nofollow(&target)? {
                if target_meta.file_type().is_symlink() {
                    return Err("Refusing to overwrite symlinks".into());
                }
                // If both are dirs, merge instead of deleting target (Windows Explorer behavior).
                if src_meta.is_dir() && target_meta.is_dir() {
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
        }

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
                            let _ = runtime_lifecycle::emit_if_running(
                                &app,
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
                    if matches!(policy, ConflictPolicy::Rename)
                        && is_destination_exists_error(&err)
                        && rename_attempt < 50
                    {
                        rename_attempt += 1;
                        target = rename_candidate(&target_base, rename_attempt);
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
        let _ = runtime_lifecycle::emit_if_running(
            &app,
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
