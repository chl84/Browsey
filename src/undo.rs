use std::collections::VecDeque;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

const MAX_HISTORY: usize = 50;

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum Action {
    Rename { from: PathBuf, to: PathBuf },
    Move { from: PathBuf, to: PathBuf },
    Delete { path: PathBuf, backup: PathBuf },
    CreateFolder { path: PathBuf },
    SetPermissions {
        path: PathBuf,
        before: PermissionsSnapshot,
        after: PermissionsSnapshot,
    },
}

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub enum Direction {
    Forward,
    Backward,
}

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub struct PermissionsSnapshot {
    pub readonly: bool,
    #[cfg(unix)]
    pub mode: u32,
}

#[derive(Default)]
#[allow(dead_code)]
pub struct UndoManager {
    undo_stack: VecDeque<Action>,
    redo_stack: VecDeque<Action>,
}

impl UndoManager {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self {
            undo_stack: VecDeque::new(),
            redo_stack: VecDeque::new(),
        }
    }

    #[allow(dead_code)]
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    #[allow(dead_code)]
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// Apply a new action and push it onto the undo stack. Clears redo history.
    #[allow(dead_code)]
    pub fn apply(&mut self, mut action: Action) -> Result<(), String> {
        execute_action(&mut action, Direction::Forward)?;
        self.undo_stack.push_back(action);
        self.redo_stack.clear();
        self.trim();
        Ok(())
    }

    pub fn undo(&mut self) -> Result<(), String> {
        let mut action = self.undo_stack.pop_back().ok_or_else(|| "Nothing to undo".to_string())?;
        execute_action(&mut action, Direction::Backward)?;
        self.redo_stack.push_back(action);
        Ok(())
    }

    pub fn redo(&mut self) -> Result<(), String> {
        let mut action = self.redo_stack.pop_back().ok_or_else(|| "Nothing to redo".to_string())?;
        execute_action(&mut action, Direction::Forward)?;
        self.undo_stack.push_back(action);
        self.trim();
        Ok(())
    }

    #[allow(dead_code)]
    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
    }

    pub fn record_applied(&mut self, action: Action) {
        self.undo_stack.push_back(action);
        self.redo_stack.clear();
        self.trim();
    }

    fn trim(&mut self) {
        while self.undo_stack.len() > MAX_HISTORY {
            let _ = self.undo_stack.pop_front();
        }
    }
}

#[derive(Clone, Default)]
pub struct UndoState {
    inner: Arc<Mutex<UndoManager>>,
}

impl UndoState {
    #[allow(dead_code)]
    pub fn record(&self, action: Action) -> Result<(), String> {
        let mut mgr = self.inner.lock().map_err(|_| "Undo manager poisoned")?;
        mgr.apply(action)?;
        Ok(())
    }

    pub fn record_applied(&self, action: Action) -> Result<(), String> {
        let mut mgr = self.inner.lock().map_err(|_| "Undo manager poisoned")?;
        mgr.record_applied(action);
        Ok(())
    }

    pub fn undo(&self) -> Result<(), String> {
        let mut mgr = self.inner.lock().map_err(|_| "Undo manager poisoned")?;
        mgr.undo()
    }

    pub fn redo(&self) -> Result<(), String> {
        let mut mgr = self.inner.lock().map_err(|_| "Undo manager poisoned")?;
        mgr.redo()
    }
}

#[tauri::command]
pub fn undo_action(state: tauri::State<'_, UndoState>) -> Result<(), String> {
    state.undo()
}

#[tauri::command]
pub fn redo_action(state: tauri::State<'_, UndoState>) -> Result<(), String> {
    state.redo()
}

fn execute_action(action: &mut Action, direction: Direction) -> Result<(), String> {
    match action {
        Action::Rename { from, to } | Action::Move { from, to } => {
            let (src, dst) = match direction {
                Direction::Forward => (from, to),
                Direction::Backward => (to, from),
            };
            ensure_absent(dst)?;
            fs::rename(&*src, &*dst).map_err(|e| {
                format!(
                    "Failed to rename {} -> {}: {e}",
                    src.display(),
                    dst.display()
                )
            })
        }
        Action::Delete { path, backup } => match direction {
            Direction::Forward => {
                if !path.exists() {
                    return Err(format!("Path does not exist: {}", path.display()));
                }
                let parent = backup
                    .parent()
                    .ok_or_else(|| "Invalid backup path".to_string())?;
                fs::create_dir_all(parent)
                    .map_err(|e| format!("Failed to create backup dir {}: {e}", parent.display()))?;
                ensure_absent(backup)?;
                fs::rename(&*path, &*backup).map_err(|e| {
                    format!(
                        "Failed to move {} to backup {}: {e}",
                        path.display(),
                        backup.display()
                    )
                })
            }
            Direction::Backward => {
                if !backup.exists() {
                    return Err(format!("Backup missing: {}", backup.display()));
                }
                ensure_absent(path)?;
                fs::rename(&*backup, &*path).map_err(|e| {
                    format!(
                        "Failed to restore {} from backup {}: {e}",
                        path.display(),
                        backup.display()
                    )
                })
            }
        },
        Action::CreateFolder { path } => match direction {
            Direction::Forward => {
                ensure_absent(path)?;
                fs::create_dir_all(&*path)
                    .map_err(|e| format!("Failed to create directory {}: {e}", path.display()))
            }
            Direction::Backward => {
                if path.exists() {
                    fs::remove_dir_all(&*path).map_err(|e| {
                        format!("Failed to remove directory {}: {e}", path.display())
                    })
                } else {
                    Ok(())
                }
            }
        },
        Action::SetPermissions {
            path,
            before,
            after,
        } => {
            let snap = match direction {
                Direction::Forward => after,
                Direction::Backward => before,
            };
            apply_permissions(path, snap)
        }
    }
}

fn ensure_absent(path: &Path) -> Result<(), String> {
    if path.exists() {
        return Err(format!("Destination already exists: {}", path.display()));
    }
    Ok(())
}

fn apply_permissions(path: &Path, snap: &PermissionsSnapshot) -> Result<(), String> {
    let meta = fs::metadata(path)
        .map_err(|e| format!("Failed to read metadata for {}: {e}", path.display()))?;
    let mut perms = meta.permissions();
    perms.set_readonly(snap.readonly);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        perms.set_mode(snap.mode);
    }
    fs::set_permissions(path, perms)
        .map_err(|e| format!("Failed to update permissions for {}: {e}", path.display()))
}

#[allow(dead_code)]
pub fn permissions_snapshot(path: &Path) -> Result<PermissionsSnapshot, String> {
    let meta = fs::metadata(path)
        .map_err(|e| format!("Failed to read metadata for {}: {e}", path.display()))?;
    let readonly = meta.permissions().readonly();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = meta.permissions().mode();
        Ok(PermissionsSnapshot { readonly, mode })
    }
    #[cfg(not(unix))]
    {
        Ok(PermissionsSnapshot { readonly })
    }
}

#[allow(dead_code)]
pub fn temp_backup_path(original: &Path) -> PathBuf {
    let name = original
        .file_name()
        .map(|n| n.to_string_lossy())
        .unwrap_or_else(|| "item".into());
    let base = original
        .parent()
        .map(|p| p.join(".browsey-undo"))
        .unwrap_or_else(|| std::env::temp_dir().join("browsey-undo"));
    let mut candidate = base.join(name.as_ref());
    let mut idx = 1u32;
    while candidate.exists() {
        let with_idx = format!("{}-{}", name, idx);
        candidate = base.join(with_idx);
        idx += 1;
    }
    candidate
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs::{self, OpenOptions};
    use std::io::Write;
    use std::time::{Duration, SystemTime};

    fn uniq_path(label: &str) -> PathBuf {
        let ts = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or(Duration::from_secs(0))
            .as_nanos();
        std::env::temp_dir().join(format!("browsey-undo-test-{label}-{ts}"))
    }

    fn write_file(path: &Path, content: &[u8]) {
        if let Some(parent) = path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(path)
            .unwrap();
        file.write_all(content).unwrap();
    }

    #[test]
    fn rename_and_undo_redo() {
        let dir = uniq_path("rename");
        let _ = fs::create_dir_all(&dir);
        let from = dir.join("a.txt");
        let to = dir.join("b.txt");
        write_file(&from, b"hello");

        let mut mgr = UndoManager::new();
        mgr.apply(Action::Rename {
            from: from.clone(),
            to: to.clone(),
        })
        .unwrap();
        assert!(!from.exists());
        assert!(to.exists());

        mgr.undo().unwrap();
        assert!(from.exists());
        assert!(!to.exists());

        mgr.redo().unwrap();
        assert!(!from.exists());
        assert!(to.exists());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn delete_and_restore() {
        let dir = uniq_path("delete");
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("file.txt");
        write_file(&path, b"bye");
        let backup = temp_backup_path(&path);

        let mut mgr = UndoManager::new();
        mgr.apply(Action::Delete {
            path: path.clone(),
            backup: backup.clone(),
        })
        .unwrap();
        assert!(!path.exists());
        assert!(backup.exists());

        mgr.undo().unwrap();
        assert!(path.exists());
        assert!(!backup.exists());

        let _ = fs::remove_dir_all(&dir);
        let _ = fs::remove_dir_all(backup.parent().unwrap_or_else(|| Path::new(".")));
    }

    #[test]
    fn create_folder_and_undo() {
        let path = uniq_path("mkdir");
        let mut mgr = UndoManager::new();
        mgr.apply(Action::CreateFolder { path: path.clone() })
            .unwrap();
        assert!(path.is_dir());

        mgr.undo().unwrap();
        assert!(!path.exists());

        let _ = fs::remove_dir_all(&path);
    }

    #[cfg(unix)]
    #[test]
    fn permissions_roundtrip() {
        use std::os::unix::fs::PermissionsExt;

        let dir = uniq_path("perm");
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("file.txt");
        write_file(&path, b"perm");

        let before = permissions_snapshot(&path).unwrap();
        let mut after = before;
        // Flip owner exec bit for the test.
        after.mode ^= 0o100;

        let mut mgr = UndoManager::new();
        mgr.apply(Action::SetPermissions {
            path: path.clone(),
            before,
            after,
        })
        .unwrap();
        let mode = fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o100, after.mode & 0o100);

        mgr.undo().unwrap();
        let mode = fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o100, before.mode & 0o100);

        let _ = fs::remove_dir_all(&dir);
    }
}
