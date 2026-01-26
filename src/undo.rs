use std::collections::{hash_map::DefaultHasher, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::{hash::Hash, hash::Hasher};
#[cfg(target_os = "windows")]
use std::{os::windows::ffi::OsStrExt, ptr};
use tracing::{debug, warn};

#[cfg(target_os = "windows")]
use windows_sys::Win32::Foundation::{LocalFree, ERROR_SUCCESS};
#[cfg(target_os = "windows")]
use windows_sys::Win32::Security::Authorization::{
    GetNamedSecurityInfoW, SetNamedSecurityInfoW, SE_FILE_OBJECT,
};
#[cfg(target_os = "windows")]
use windows_sys::Win32::Security::{
    GetSecurityDescriptorDacl, ACL, DACL_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR,
};

const MAX_HISTORY: usize = 50;
// Use a central directory for all undo backups.

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum Action {
    Rename {
        from: PathBuf,
        to: PathBuf,
    },
    Move {
        from: PathBuf,
        to: PathBuf,
    },
    Copy {
        from: PathBuf,
        to: PathBuf,
    },
    Delete {
        path: PathBuf,
        backup: PathBuf,
    },
    CreateFolder {
        path: PathBuf,
    },
    Batch(Vec<Action>),
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

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct PermissionsSnapshot {
    pub readonly: bool,
    #[cfg(unix)]
    pub mode: u32,
    #[cfg(target_os = "windows")]
    pub dacl: Option<Vec<u8>>,
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
        let mut action = self
            .undo_stack
            .pop_back()
            .ok_or_else(|| "Nothing to undo".to_string())?;
        match execute_action(&mut action, Direction::Backward) {
            Ok(_) => {
                self.redo_stack.push_back(action);
                Ok(())
            }
            Err(err) => {
                self.undo_stack.push_back(action);
                Err(err)
            }
        }
    }

    pub fn redo(&mut self) -> Result<(), String> {
        let mut action = self
            .redo_stack
            .pop_back()
            .ok_or_else(|| "Nothing to redo".to_string())?;
        match execute_action(&mut action, Direction::Forward) {
            Ok(_) => {
                self.undo_stack.push_back(action);
                self.trim();
                Ok(())
            }
            Err(err) => {
                self.redo_stack.push_back(action);
                Err(err)
            }
        }
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

/// Best-effort cleanup of stale `.browsey-undo` directories. Runs at startup to
/// avoid leaving orphaned backups after a crash or restart (undo history is
/// in-memory only).
pub fn cleanup_stale_backups(max_age: Option<Duration>) {
    let _ = max_age; // keep the signature; we remove everything regardless.
    let base = base_undo_dir();
    // Remove the entire base; safe because undo history is in-memory only and does not survive restarts.
    if base.exists() {
        if let Err(e) = fs::remove_dir_all(&base) {
            warn!("Failed to clean backup directory {:?}: {}", base, e);
        } else {
            debug!("Cleaned backup directory {:?}", base);
        }
    }
    let _ = fs::create_dir_all(&base);
}

pub(crate) fn run_actions(actions: &mut [Action], direction: Direction) -> Result<(), String> {
    execute_batch(actions, direction)
}

fn execute_action(action: &mut Action, direction: Direction) -> Result<(), String> {
    match action {
        Action::Batch(actions) => execute_batch(actions, direction),
        Action::Rename { from, to } | Action::Move { from, to } => {
            let (src, dst) = match direction {
                Direction::Forward => (from, to),
                Direction::Backward => (to, from),
            };
            if dst.exists() {
                delete_entry_path(dst)?;
            }
            move_with_fallback(src, dst)
        }
        Action::Copy { from, to } => match direction {
            Direction::Forward => {
                ensure_absent(to)?;
                copy_entry(from, to)
            }
            Direction::Backward => delete_entry_path(to),
        },
        Action::Delete { path, backup } => match direction {
            Direction::Forward => {
                if !path.exists() {
                    return Err(format!("Path does not exist: {}", path.display()));
                }
                let parent = backup
                    .parent()
                    .ok_or_else(|| "Invalid backup path".to_string())?;
                fs::create_dir_all(parent).map_err(|e| {
                    format!("Failed to create backup dir {}: {e}", parent.display())
                })?;
                ensure_absent(backup)?;
                move_with_fallback(path, backup)
            }
            Direction::Backward => {
                if !backup.exists() {
                    return Err(format!("Backup missing: {}", backup.display()));
                }
                ensure_absent(path)?;
                move_with_fallback(backup, path)
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
                    fs::remove_dir_all(&*path)
                        .map_err(|e| format!("Failed to remove directory {}: {e}", path.display()))
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

fn execute_batch(actions: &mut [Action], direction: Direction) -> Result<(), String> {
    let order: Vec<usize> = match direction {
        Direction::Forward => (0..actions.len()).collect(),
        Direction::Backward => (0..actions.len()).rev().collect(),
    };

    let mut completed: Vec<usize> = Vec::with_capacity(order.len());
    for idx in order {
        if let Err(err) = execute_action(&mut actions[idx], direction) {
            let rollback_direction = reverse_direction(direction);
            let mut rollback_errors = Vec::new();
            for rollback_idx in completed.into_iter().rev() {
                if let Err(rollback_err) =
                    execute_action(&mut actions[rollback_idx], rollback_direction)
                {
                    rollback_errors.push(format!(
                        "rollback action {} failed: {}",
                        rollback_idx + 1,
                        rollback_err
                    ));
                }
            }
            if rollback_errors.is_empty() {
                return Err(format!("Batch action {} failed: {}", idx + 1, err));
            } else {
                return Err(format!(
                    "Batch action {} failed: {}; additional rollback issues: {}",
                    idx + 1,
                    err,
                    rollback_errors.join("; ")
                ));
            }
        }
        completed.push(idx);
    }
    Ok(())
}

fn reverse_direction(direction: Direction) -> Direction {
    match direction {
        Direction::Forward => Direction::Backward,
        Direction::Backward => Direction::Forward,
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
    #[cfg(target_os = "windows")]
    {
        let mut wide: Vec<u16> = path
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        let dacl_ptr = snap
            .dacl
            .as_ref()
            .map(|v| v.as_ptr() as *mut ACL)
            .unwrap_or(ptr::null_mut());
        let status = unsafe {
            SetNamedSecurityInfoW(
                wide.as_mut_ptr(),
                SE_FILE_OBJECT,
                DACL_SECURITY_INFORMATION,
                ptr::null_mut(),
                ptr::null_mut(),
                dacl_ptr,
                ptr::null_mut(),
            )
        };
        if status != ERROR_SUCCESS {
            return Err(format!(
                "Failed to update permissions for {}: Win32 error {}",
                path.display(),
                status
            ));
        }
    }
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
        return Ok(PermissionsSnapshot { readonly, mode });
    }
    #[cfg(target_os = "windows")]
    {
        let dacl = snapshot_dacl(path)?;
        return Ok(PermissionsSnapshot { readonly, dacl });
    }
    #[cfg(not(any(unix, target_os = "windows")))]
    Ok(PermissionsSnapshot { readonly })
}

#[cfg(target_os = "windows")]
fn snapshot_dacl(path: &Path) -> Result<Option<Vec<u8>>, String> {
    let mut sd: PSECURITY_DESCRIPTOR = ptr::null_mut();
    let mut dacl: *mut ACL = ptr::null_mut();
    let mut wide: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let status = unsafe {
        GetNamedSecurityInfoW(
            wide.as_mut_ptr(),
            SE_FILE_OBJECT,
            DACL_SECURITY_INFORMATION,
            ptr::null_mut(),
            ptr::null_mut(),
            &mut dacl,
            ptr::null_mut(),
            &mut sd,
        )
    };
    if status != ERROR_SUCCESS {
        return Err(format!(
            "GetNamedSecurityInfoW failed for {}: Win32 error {}",
            path.display(),
            status
        ));
    }
    let result = unsafe {
        let mut present = 0i32;
        let mut defaulted = 0i32;
        let mut acl_ptr = dacl;
        let ok = GetSecurityDescriptorDacl(sd, &mut present, &mut acl_ptr, &mut defaulted);
        if ok == 0 {
            Err("GetSecurityDescriptorDacl failed".into())
        } else if present == 0 || acl_ptr.is_null() {
            Ok(None)
        } else {
            let size = (*acl_ptr).AclSize as usize;
            let bytes = std::slice::from_raw_parts(acl_ptr as *const u8, size).to_vec();
            Ok(Some(bytes))
        }
    };
    unsafe {
        LocalFree(sd);
    }
    result
}

pub(crate) fn copy_entry(src: &Path, dest: &Path) -> Result<(), String> {
    let meta = fs::symlink_metadata(src).map_err(|e| format!("Failed to read metadata: {e}"))?;
    if meta.file_type().is_symlink() {
        return Err("Refusing to copy symlinks".into());
    }
    if meta.is_dir() {
        copy_dir(src, dest)
    } else {
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create parent {:?}: {e}", parent))?;
        }
        fs::copy(src, dest)
            .map_err(|e| format!("Failed to copy file: {e}"))
            .map(|_| ())
    }
}

fn copy_dir(src: &Path, dest: &Path) -> Result<(), String> {
    if dest.exists() {
        return Err(format!("Destination already exists: {}", dest.display()));
    }
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
            copy_dir(&path, &target)?;
        } else {
            fs::copy(&path, &target).map_err(|e| format!("Failed to copy file {:?}: {e}", path))?;
        }
    }
    Ok(())
}

pub(crate) fn delete_entry_path(path: &Path) -> Result<(), String> {
    let meta = fs::symlink_metadata(path).map_err(|e| format!("Failed to read metadata: {e}"))?;
    if meta.is_dir() {
        fs::remove_dir_all(path).map_err(|e| format!("Failed to delete directory: {e}"))
    } else {
        fs::remove_file(path).map_err(|e| format!("Failed to delete file: {e}"))
    }
}

pub fn move_with_fallback(src: &Path, dst: &Path) -> Result<(), String> {
    match fs::rename(src, dst) {
        Ok(_) => Ok(()),
        Err(rename_err) => {
            if !is_cross_device(&rename_err) {
                return Err(format!(
                    "Failed to rename {} -> {}: {rename_err}",
                    src.display(),
                    dst.display()
                ));
            }
            // Fallback: copy + delete to tolerate different disks/file systems.
            copy_entry(src, dst).and_then(|_| {
                delete_entry_path(src).map_err(|del_err| {
                    // Best effort: clean up destination if delete failed to avoid duplicates.
                    let _ = delete_entry_path(dst);
                    format!(
                        "Copied {} -> {} after cross-device rename error, but failed to delete source: {del_err}",
                        src.display(),
                        dst.display()
                    )
                })
            })
        }
    }
}

fn is_cross_device(err: &std::io::Error) -> bool {
    matches!(err.raw_os_error(), Some(17) | Some(18))
}

#[allow(dead_code)]
pub fn temp_backup_path(original: &Path) -> PathBuf {
    let base = base_undo_dir();
    let _ = fs::create_dir_all(&base);

    // Use a hash of the full path to group files from the same directory while avoiding long names.
    let mut hasher = DefaultHasher::new();
    original.hash(&mut hasher);
    let bucket = format!("{:016x}", hasher.finish());

    let name = original
        .file_name()
        .map(|n| n.to_string_lossy())
        .unwrap_or_else(|| "item".into());

    let name_str: &str = name.as_ref();
    let mut candidate = base.join(&bucket).join(std::path::Path::new(name_str));
    let mut idx = 1u32;
    while candidate.exists() {
        let with_idx = format!("{}-{}", name, idx);
        candidate = base.join(&bucket).join(std::path::Path::new(&with_idx));
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

    #[test]
    fn batch_apply_and_undo_redo() {
        let dir = uniq_path("batch");
        let _ = fs::create_dir_all(&dir);
        let source = dir.join("a.txt");
        let subdir = dir.join("nested");
        let moved = subdir.join("a.txt");
        let copied = dir.join("b.txt");
        write_file(&source, b"hello");

        let mut mgr = UndoManager::new();
        mgr.apply(Action::Batch(vec![
            Action::CreateFolder {
                path: subdir.clone(),
            },
            Action::Move {
                from: source.clone(),
                to: moved.clone(),
            },
            Action::Copy {
                from: moved.clone(),
                to: copied.clone(),
            },
        ]))
        .unwrap();

        assert!(!source.exists());
        assert!(moved.exists());
        assert!(copied.exists());
        assert!(subdir.exists());

        mgr.undo().unwrap();
        assert!(source.exists());
        assert!(!moved.exists());
        assert!(!copied.exists());
        assert!(!subdir.exists());

        mgr.redo().unwrap();
        assert!(!source.exists());
        assert!(moved.exists());
        assert!(copied.exists());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn batch_rolls_back_on_failure() {
        let dir = uniq_path("batch-fail");
        let _ = fs::create_dir_all(&dir);
        let source = dir.join("source.txt");
        let existing = dir.join("existing.txt");
        let new_dir = dir.join("new-dir");
        write_file(&source, b"hello");
        write_file(&existing, b"keep");

        let mut mgr = UndoManager::new();
        let err = mgr
            .apply(Action::Batch(vec![
                Action::CreateFolder {
                    path: new_dir.clone(),
                },
                Action::Copy {
                    from: source.clone(),
                    to: existing.clone(),
                },
            ]))
            .unwrap_err();
        assert!(err.contains("Batch action 2 failed"));
        assert!(source.exists());
        assert!(existing.exists());
        assert!(!new_dir.exists());
        assert!(!mgr.can_undo());
        assert!(!mgr.can_redo());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn undo_failure_restores_stack() {
        let dir = uniq_path("undo-fail");
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

        let _ = fs::remove_file(&backup);
        let err = mgr.undo().unwrap_err();
        assert!(err.contains("Backup") || err.contains("rename"));
        assert!(mgr.can_undo());
        assert!(!mgr.can_redo());

        let _ = fs::remove_dir_all(&dir);
        let _ = fs::remove_dir_all(backup.parent().unwrap_or_else(|| Path::new(".")));
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
        let mut after = before.clone();
        // Flip owner exec bit for the test.
        after.mode ^= 0o100;
        let after_mode = after.mode;

        let mut mgr = UndoManager::new();
        mgr.apply(Action::SetPermissions {
            path: path.clone(),
            before: before.clone(),
            after,
        })
        .unwrap();
        let mode = fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o100, after_mode & 0o100);

        mgr.undo().unwrap();
        let mode = fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o100, before.mode & 0o100);

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn cleanup_prunes_stale_backup_dirs() {
        let base = base_undo_dir();
        let target = base.join("dummy");
        fs::create_dir_all(&target).unwrap();

        cleanup_stale_backups(Some(Duration::from_secs(0)));

        assert!(
            !target.exists(),
            "backup base contents should be removed during cleanup"
        );
    }
}

fn base_undo_dir() -> PathBuf {
    dirs_next::data_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("browsey")
        .join("undo")
}
