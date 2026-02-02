use crate::{
    fs_utils::sanitize_path_follow,
    undo::{move_with_fallback, run_actions, temp_backup_path, Action, Direction, UndoState},
};
use once_cell::sync::Lazy;
use std::{
    fs,
    path::{Path, PathBuf},
    sync::Mutex,
};

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

fn copy_dir(src: &Path, dest: &Path) -> Result<(), String> {
    fs::create_dir_all(dest).map_err(|e| format!("Failed to create dir {:?}: {e}", dest))?;
    for entry in fs::read_dir(src).map_err(|e| format!("Failed to read dir {:?}: {e}", src))? {
        let entry = entry.map_err(|e| format!("Failed to read dir entry: {e}"))?;
        let path = entry.path();
        let meta =
            fs::symlink_metadata(&path).map_err(|e| format!("Failed to read metadata: {e}"))?;
        if meta.file_type().is_symlink() {
            return Err("Refusing to copy symlinks".into());
        }
        let target = dest.join(entry.file_name());
        if meta.is_dir() {
            ensure_not_child(&path, &target)?;
            copy_dir(&path, &target)?;
        } else {
            fs::copy(&path, &target).map_err(|e| format!("Failed to copy file {:?}: {e}", path))?;
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
        let target = dest.join(entry.file_name());
        if meta.is_dir() {
            if target.exists() && target.is_dir() {
                merge_dir(&path, &target, mode, actions)?;
            } else {
                if target.exists() {
                    backup_existing_target(&target, actions)?;
                }
                match mode {
                    ClipboardMode::Copy => {
                        copy_dir(&path, &target)?;
                        actions.push(Action::Copy {
                            from: path.clone(),
                            to: target.clone(),
                        });
                    }
                    ClipboardMode::Cut => {
                        move_entry(&path, &target)?;
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
                    move_entry(&path, &target)?;
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

fn copy_entry(src: &Path, dest: &Path) -> Result<(), String> {
    let meta = fs::symlink_metadata(src).map_err(|e| format!("Failed to read metadata: {e}"))?;
    if meta.file_type().is_symlink() {
        return Err("Refusing to copy symlinks".into());
    }
    if meta.is_dir() {
        ensure_not_child(src, dest)?;
        copy_dir(src, dest)
    } else {
        fs::copy(src, dest).map_err(|e| format!("Failed to copy file: {e}"))?;
        Ok(())
    }
}

fn delete_entry_path(path: &Path) -> Result<(), String> {
    let meta = fs::symlink_metadata(path).map_err(|e| format!("Failed to read metadata: {e}"))?;
    if meta.is_dir() {
        fs::remove_dir_all(path).map_err(|e| format!("Failed to delete directory: {e}"))
    } else {
        fs::remove_file(path).map_err(|e| format!("Failed to delete file: {e}"))
    }
}

fn move_entry(src: &Path, dest: &Path) -> Result<(), String> {
    ensure_not_child(src, dest)?;
    match fs::rename(src, dest) {
        Ok(_) => Ok(()),
        Err(_) => {
            copy_entry(src, dest)?;
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
pub fn paste_clipboard_cmd(
    dest: String,
    policy: Option<String>,
    undo: tauri::State<UndoState>,
) -> Result<Vec<String>, String> {
    let dest = sanitize_path_follow(&dest, false)?;
    let state = current_clipboard().ok_or_else(|| "Clipboard is empty".to_string())?;
    let policy = policy
        .map(|p| policy_from_str(&p))
        .transpose()?
        .unwrap_or(ConflictPolicy::Rename);

    let mut created = Vec::new();
    let mut performed: Vec<Action> = Vec::with_capacity(state.entries.len() * 4);
    for src in state.entries.iter() {
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
                merge_dir(src, &target, state.mode, &mut performed)?;
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
                ClipboardMode::Copy => copy_entry(src, &target),
                ClipboardMode::Cut => move_entry(src, &target),
            };

            match result {
                Ok(_) => break,
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

    if !performed.is_empty() {
        let recorded = if performed.len() == 1 {
            performed.pop().unwrap()
        } else {
            Action::Batch(performed)
        };
        let _ = undo.record_applied(recorded);
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
        merge_dir(&src, &dest, ClipboardMode::Copy, &mut actions).unwrap();

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
        merge_dir(&src, &dest, ClipboardMode::Cut, &mut actions).unwrap();

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
