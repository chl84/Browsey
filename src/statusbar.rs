use serde::Serialize;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use tauri::Emitter;

#[cfg(unix)]
use std::os::unix::fs::MetadataExt;

/// Height hint for the status bar in the UI.
#[allow(dead_code)]
pub const HEIGHT: f32 = 48.0;

#[derive(Serialize, Clone)]
pub struct DirSizeEntry {
    pub path: String,
    pub bytes: u64,
}

#[derive(Serialize, Clone)]
pub struct DirSizeResult {
    pub total: u64,
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

fn dir_size_recursive<F>(
    root: &Path,
    #[cfg_attr(not(unix), allow(unused_variables))] root_dev: Option<u64>,
    pseudo_roots: &HashSet<&str>,
    mut on_progress: F,
) -> u64
where
    F: FnMut(u64),
{
    let mut total: u64 = 0;
    let mut stack = vec![root.to_path_buf()];
    let mut pending: u64 = 0;
    const PROGRESS_BYTES: u64 = 500 * 1024 * 1024; // emit roughly every 500MB

    while let Some(path) = stack.pop() {
        if should_skip(&path, pseudo_roots) {
            continue;
        }

        let meta = match fs::symlink_metadata(&path) {
            Ok(m) => m,
            Err(_) => continue,
        };

        if meta.file_type().is_symlink() {
            continue;
        }

        #[cfg(unix)]
        if let Some(dev) = root_dev {
            if meta.dev() != dev {
                continue;
            }
        }

        if meta.is_file() {
            total = total.saturating_add(meta.len());
            pending = pending.saturating_add(meta.len());
            if pending >= PROGRESS_BYTES {
                on_progress(pending);
                pending = 0;
            }
            continue;
        }

        if meta.is_dir() {
            let iter = match fs::read_dir(&path) {
                Ok(i) => i,
                Err(_) => continue,
            };
            for entry in iter.flatten() {
                stack.push(entry.path());
            }
        }
    }

    if pending > 0 {
        on_progress(pending);
    }

    total
}

#[tauri::command]
pub async fn dir_sizes(
    app: tauri::AppHandle,
    paths: Vec<String>,
    progress_event: Option<String>,
) -> Result<DirSizeResult, String> {
    let task = tauri::async_runtime::spawn_blocking(move || {
        let mut entries = Vec::new();
        let mut total: u64 = 0;
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
        let emitter = progress_event.map(|evt| (app, evt));

        for raw in paths {
            let path = PathBuf::from(&raw);
            if !path.exists() {
                continue;
            }

            #[cfg(unix)]
            let root_dev = fs::symlink_metadata(&path).ok().map(|m| m.dev());
            #[cfg(not(unix))]
            let root_dev: Option<u64> = None;

            let mut partial: u64 = 0;
            let emit_progress = |delta: u64, raw: &str, emitter: &Option<(tauri::AppHandle, String)>| {
                if let Some((app, evt)) = emitter {
                    let _ = app.emit(
                        evt,
                        DirSizeEntry {
                            path: raw.to_string(),
                            bytes: delta,
                        },
                    );
                }
            };

            let size = dir_size_recursive(&path, root_dev, &pseudo_roots, |delta| {
                partial = partial.saturating_add(delta);
                emit_progress(partial, &raw, &emitter);
            });
            total = total.saturating_add(size);
            entries.push(DirSizeEntry {
                path: raw,
                bytes: size,
            });
        }

        DirSizeResult { total, entries }
    });

    task.await
        .map_err(|e| format!("Failed to compute directory sizes: {e}"))
}
