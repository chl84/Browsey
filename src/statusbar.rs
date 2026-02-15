use serde::Serialize;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;

#[cfg(unix)]
use std::os::unix::fs::MetadataExt;

use crate::{commands::CancelState, runtime_lifecycle};

/// Height hint for the status bar in the UI.
#[allow(dead_code)]
pub const HEIGHT: f32 = 48.0;

#[derive(Serialize, Clone)]
pub struct DirSizeEntry {
    pub path: String,
    pub bytes: u64,
    pub items: u64,
}

#[derive(Serialize, Clone)]
pub struct DirSizeResult {
    pub total: u64,
    pub total_items: u64,
    pub entries: Vec<DirSizeEntry>,
}

fn should_skip(path: &Path, pseudo_roots: &HashSet<&str>) -> bool {
    if let Some(s) = path.to_str() {
        for root in pseudo_roots {
            let prefix = if root.ends_with('/') {
                root.trim_end_matches('/')
            } else {
                *root
            };
            if s == prefix || s.starts_with(&format!("{}/", prefix)) {
                return true;
            }
        }
    }
    false
}

fn dir_size_recursive<F, S>(
    root: &Path,
    #[cfg_attr(not(unix), allow(unused_variables))] root_dev: Option<u64>,
    pseudo_roots: &HashSet<&str>,
    mut on_progress: F,
    should_stop: S,
) -> (u64, u64, bool)
where
    F: FnMut(u64, u64),
    S: Fn() -> bool,
{
    let mut total: u64 = 0;
    let mut items: u64 = 0;
    let mut stack = vec![root.to_path_buf()];
    let mut pending: u64 = 0;
    let mut pending_items: u64 = 0;
    let mut stopped = false;
    const PROGRESS_BYTES: u64 = 500 * 1024 * 1024; // emit roughly every 500MB

    while let Some(path) = stack.pop() {
        if should_stop() {
            stopped = true;
            break;
        }
        if should_skip(&path, pseudo_roots) {
            continue;
        }

        let meta = match fs::symlink_metadata(&path) {
            Ok(m) => m,
            Err(_) => continue,
        };

        items = items.saturating_add(1);
        pending_items = pending_items.saturating_add(1);

        #[cfg(unix)]
        if let Some(dev) = root_dev {
            if meta.dev() != dev {
                continue;
            }
        }

        if meta.file_type().is_symlink() {
            let len = meta.len();
            total = total.saturating_add(len);
            pending = pending.saturating_add(len);
            if pending >= PROGRESS_BYTES {
                on_progress(pending, pending_items);
                pending = 0;
                pending_items = 0;
            }
            continue;
        }

        if meta.is_file() {
            total = total.saturating_add(meta.len());
            pending = pending.saturating_add(meta.len());
            if pending >= PROGRESS_BYTES {
                on_progress(pending, pending_items);
                pending = 0;
                pending_items = 0;
            }
            continue;
        }

        if meta.is_dir() {
            let iter = match fs::read_dir(&path) {
                Ok(i) => i,
                Err(_) => continue,
            };
            for entry in iter.flatten() {
                if should_stop() {
                    stopped = true;
                    break;
                }
                stack.push(entry.path());
            }
            if stopped {
                break;
            }
        }
    }

    if pending > 0 && !stopped {
        on_progress(pending, pending_items);
    }

    (total, items, stopped)
}

#[tauri::command]
pub async fn dir_sizes(
    app: tauri::AppHandle,
    cancel: tauri::State<'_, CancelState>,
    paths: Vec<String>,
    progress_event: Option<String>,
) -> Result<DirSizeResult, String> {
    let cancel_state = cancel.inner().clone();
    let task = tauri::async_runtime::spawn_blocking(move || -> Result<DirSizeResult, String> {
        let cancel_guard = progress_event
            .as_ref()
            .map(|evt| cancel_state.register(evt.clone()))
            .transpose()?;
        let cancel_token = cancel_guard.as_ref().map(|g| g.token());
        let mut entries = Vec::new();
        let mut total: u64 = 0;
        let mut total_items: u64 = 0;
        let pseudo_roots: HashSet<&str> = [
            "/proc",
            "/sys",
            "/dev",
            "/run",
            "/tmp",
            "/var/run",
            "/var/lock",
        ]
        .into_iter()
        .collect();
        let emitter = progress_event;

        for raw in paths {
            if runtime_lifecycle::is_shutting_down(&app)
                || cancel_token
                    .as_ref()
                    .map(|token| token.load(Ordering::Relaxed))
                    .unwrap_or(false)
            {
                break;
            }
            let path = PathBuf::from(&raw);
            if !path.exists() {
                continue;
            }

            #[cfg(unix)]
            let root_dev = fs::symlink_metadata(&path).ok().map(|m| m.dev());
            #[cfg(not(unix))]
            let root_dev: Option<u64> = None;

            let mut partial: u64 = 0;
            let mut partial_items: u64 = 0;
            let emit_progress =
                |delta: u64, items_delta: u64, raw: &str, emitter: &Option<String>| {
                    if let Some(evt) = emitter {
                        let _ = runtime_lifecycle::emit_if_running(
                            &app,
                            evt,
                            DirSizeEntry {
                                path: raw.to_string(),
                                bytes: delta,
                                items: items_delta,
                            },
                        );
                    }
                };

            let (size, items, stopped) = dir_size_recursive(
                &path,
                root_dev,
                &pseudo_roots,
                |delta, items_delta| {
                    partial = partial.saturating_add(delta);
                    partial_items = partial_items.saturating_add(items_delta);
                    emit_progress(partial, partial_items, &raw, &emitter);
                },
                || {
                    runtime_lifecycle::is_shutting_down(&app)
                        || cancel_token
                            .as_ref()
                            .map(|token| token.load(Ordering::Relaxed))
                            .unwrap_or(false)
                },
            );
            total = total.saturating_add(size);
            total_items = total_items.saturating_add(items);
            entries.push(DirSizeEntry {
                path: raw,
                bytes: size,
                items,
            });
            if stopped {
                break;
            }
        }

        Ok(DirSizeResult {
            total,
            total_items,
            entries,
        })
    });

    task.await
        .map_err(|e| format!("Failed to compute directory sizes: {e}"))?
}
