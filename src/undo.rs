use std::collections::{hash_map::DefaultHasher, VecDeque};
#[cfg(unix)]
use std::ffi::{CStr, CString};
use std::fs;
use std::io::{self, ErrorKind};
#[cfg(all(unix, target_os = "linux"))]
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;
#[cfg(unix)]
use std::os::unix::fs::MetadataExt;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::{hash::Hash, hash::Hasher};
#[cfg(target_os = "windows")]
use std::{os::windows::ffi::OsStrExt, ptr};
use tracing::{debug, warn};

use crate::fs_utils::check_no_symlink_components;

#[cfg(target_os = "windows")]
use windows_sys::Win32::Foundation::{
    LocalFree, ERROR_ALREADY_EXISTS, ERROR_FILE_EXISTS, ERROR_SUCCESS,
};
#[cfg(target_os = "windows")]
use windows_sys::Win32::Security::Authorization::{
    GetNamedSecurityInfoW, SetNamedSecurityInfoW, SE_FILE_OBJECT,
};
#[cfg(target_os = "windows")]
use windows_sys::Win32::Security::{
    GetSecurityDescriptorDacl, ACL, DACL_SECURITY_INFORMATION, PSECURITY_DESCRIPTOR,
};
#[cfg(target_os = "windows")]
use windows_sys::Win32::Storage::FileSystem::{
    GetFileAttributesW, MoveFileExW, SetFileAttributesW, FILE_ATTRIBUTE_HIDDEN,
    MOVEFILE_WRITE_THROUGH,
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
    /// Represents a newly created path. Undo (Backward) moves the path to a
    /// backup location (effectively deleting it while retaining data); redo
    /// (Forward) moves it back.
    Create {
        path: PathBuf,
        backup: PathBuf,
    },
    Delete {
        path: PathBuf,
        backup: PathBuf,
    },
    #[cfg(target_os = "windows")]
    SetHidden {
        path: PathBuf,
        hidden: bool,
    },
    CreateFolder {
        path: PathBuf,
    },
    Batch(Vec<Action>),
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

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct OwnershipSnapshot {
    #[cfg(unix)]
    pub uid: u32,
    #[cfg(unix)]
    pub gid: u32,
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
    pub fn clone_inner(&self) -> Arc<Mutex<UndoManager>> {
        self.inner.clone()
    }
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

    if let Err(e) = validate_undo_dir(&base) {
        warn!("Skip cleanup; unsafe undo dir {:?}: {}", base, e);
        return;
    }

    if base.exists() {
        match fs::read_dir(&base) {
            Ok(entries) => {
                for entry in entries.flatten() {
                    let path = entry.path();
                    let res = if path.is_dir() {
                        fs::remove_dir_all(&path)
                    } else {
                        fs::remove_file(&path)
                    };
                    if let Err(err) = res {
                        warn!("Failed to remove {:?}: {}", path, err);
                    }
                }
                debug!("Cleaned contents of backup directory {:?}", base);
            }
            Err(e) => warn!("Failed to read backup directory {:?}: {}", base, e),
        }
    }

    if let Err(e) = fs::create_dir_all(&base) {
        warn!("Failed to ensure backup directory {:?}: {}", base, e);
    }
}

pub(crate) fn run_actions(actions: &mut [Action], direction: Direction) -> Result<(), String> {
    execute_batch(actions, direction)
}

#[cfg(all(unix, target_os = "linux"))]
fn absolute_path(path: &Path) -> Result<PathBuf, std::io::Error> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Ok(std::env::current_dir()?.join(path))
    }
}

#[cfg(all(unix, target_os = "linux"))]
fn parent_fd_and_name(path: &Path) -> Result<(OwnedFd, CString), std::io::Error> {
    let abs = absolute_path(path)?;
    let parent = abs.parent().ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "Path is missing parent")
    })?;
    let name = abs.file_name().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Path is missing file name",
        )
    })?;
    let c_name = CString::new(name.as_bytes()).map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Path contains invalid NUL byte",
        )
    })?;
    let parent_fd = open_nofollow_path_fd(parent)
        .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?;
    Ok((parent_fd, c_name))
}

#[cfg(all(unix, target_os = "linux"))]
fn fstatat_nofollow(parent_fd: libc::c_int, name: &CString) -> Result<libc::stat, std::io::Error> {
    let mut stat = std::mem::MaybeUninit::<libc::stat>::uninit();
    let rc = unsafe {
        libc::fstatat(
            parent_fd,
            name.as_ptr(),
            stat.as_mut_ptr(),
            libc::AT_SYMLINK_NOFOLLOW,
        )
    };
    if rc != 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(unsafe { stat.assume_init() })
    }
}

#[cfg(all(unix, target_os = "linux"))]
fn rename_noreplace_unsupported_error(code: Option<i32>) -> std::io::Error {
    let detail = code
        .map(|raw| format!(" (os error {raw})"))
        .unwrap_or_default();
    std::io::Error::new(
        ErrorKind::Unsupported,
        format!("RENAME_NOREPLACE is unavailable{detail}"),
    )
}

#[cfg(all(unix, target_os = "linux"))]
fn rename_nofollow_io(src: &Path, dst: &Path) -> Result<(), std::io::Error> {
    if absolute_path(src)? == absolute_path(dst)? {
        return Ok(());
    }

    let (src_parent_fd, src_name) = parent_fd_and_name(src)?;
    let (dst_parent_fd, dst_name) = parent_fd_and_name(dst)?;

    let src_stat = fstatat_nofollow(src_parent_fd.as_raw_fd(), &src_name)?;
    if (src_stat.st_mode & libc::S_IFMT) == libc::S_IFLNK {
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "Symlinks are not allowed",
        ));
    }

    let rc = unsafe {
        libc::syscall(
            libc::SYS_renameat2 as libc::c_long,
            src_parent_fd.as_raw_fd(),
            src_name.as_ptr(),
            dst_parent_fd.as_raw_fd(),
            dst_name.as_ptr(),
            libc::RENAME_NOREPLACE as libc::c_uint,
        )
    } as libc::c_int;
    if rc == 0 {
        Ok(())
    } else {
        let err = std::io::Error::last_os_error();
        match err.raw_os_error() {
            Some(code)
                if code == libc::ENOSYS
                    || code == libc::EINVAL
                    || code == libc::ENOTSUP
                    || code == libc::EOPNOTSUPP =>
            {
                Err(rename_noreplace_unsupported_error(Some(code)))
            }
            _ => Err(err),
        }
    }
}

#[cfg(target_os = "windows")]
fn rename_nofollow_io(src: &Path, dst: &Path) -> Result<(), std::io::Error> {
    if src == dst {
        return Ok(());
    }

    // Validate source without following symlinks/reparse points via metadata.
    let src_meta = fs::symlink_metadata(src)?;
    if src_meta.file_type().is_symlink() {
        return Err(std::io::Error::new(
            ErrorKind::Other,
            "Symlinks are not allowed",
        ));
    }

    let src_wide: Vec<u16> = src
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let dst_wide: Vec<u16> = dst
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    let ok = unsafe { MoveFileExW(src_wide.as_ptr(), dst_wide.as_ptr(), MOVEFILE_WRITE_THROUGH) };
    if ok != 0 {
        return Ok(());
    }

    let err = std::io::Error::last_os_error();
    match err.raw_os_error() {
        Some(code) if code == ERROR_ALREADY_EXISTS as i32 || code == ERROR_FILE_EXISTS as i32 => {
            Err(std::io::Error::new(
                ErrorKind::AlreadyExists,
                "Destination already exists",
            ))
        }
        _ => Err(err),
    }
}

#[cfg(all(unix, not(target_os = "linux")))]
fn rename_nofollow_io(src: &Path, dst: &Path) -> Result<(), std::io::Error> {
    let _ = (src, dst);
    Err(std::io::Error::new(
        ErrorKind::Unsupported,
        "RENAME_NOREPLACE is unavailable on this platform",
    ))
}

#[cfg(not(any(unix, target_os = "windows")))]
fn rename_nofollow_io(src: &Path, dst: &Path) -> Result<(), std::io::Error> {
    let _ = (src, dst);
    Err(std::io::Error::new(
        ErrorKind::Unsupported,
        "RENAME_NOREPLACE is unavailable on this platform",
    ))
}

#[cfg(all(unix, target_os = "linux"))]
fn read_dir_entry_names(dir_fd: &OwnedFd) -> Result<Vec<CString>, std::io::Error> {
    let dup_fd = unsafe { libc::fcntl(dir_fd.as_raw_fd(), libc::F_DUPFD_CLOEXEC, 0) };
    if dup_fd < 0 {
        return Err(std::io::Error::last_os_error());
    }
    let dir = unsafe { libc::fdopendir(dup_fd) };
    if dir.is_null() {
        let err = std::io::Error::last_os_error();
        let _ = unsafe { libc::close(dup_fd) };
        return Err(err);
    }

    let mut names = Vec::new();
    let mut read_err: Option<std::io::Error> = None;
    loop {
        unsafe {
            *libc::__errno_location() = 0;
        }
        let entry = unsafe { libc::readdir(dir) };
        if entry.is_null() {
            let errno = unsafe { *libc::__errno_location() };
            if errno != 0 {
                read_err = Some(std::io::Error::from_raw_os_error(errno));
            }
            break;
        }
        let raw = unsafe { CStr::from_ptr((*entry).d_name.as_ptr()) };
        let bytes = raw.to_bytes();
        if bytes == b"." || bytes == b".." {
            continue;
        }
        let name = CString::new(bytes).map_err(|_| {
            std::io::Error::new(
                ErrorKind::InvalidData,
                "Directory entry contains an unexpected NUL byte",
            )
        })?;
        names.push(name);
    }

    if unsafe { libc::closedir(dir) } != 0 {
        return Err(std::io::Error::last_os_error());
    }
    if let Some(err) = read_err {
        return Err(err);
    }
    Ok(names)
}

#[cfg(all(unix, target_os = "linux"))]
fn unlinkat_nofollow(
    parent_fd: libc::c_int,
    name: &CString,
    flags: libc::c_int,
) -> Result<(), std::io::Error> {
    let rc = unsafe { libc::unlinkat(parent_fd, name.as_ptr(), flags) };
    if rc == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

#[cfg(all(unix, target_os = "linux"))]
fn delete_dir_recursive_at(parent_fd: libc::c_int, name: &CString) -> Result<(), std::io::Error> {
    let child_raw = unsafe {
        libc::openat(
            parent_fd,
            name.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    if child_raw < 0 {
        return Err(std::io::Error::last_os_error());
    }
    let child_fd = unsafe { OwnedFd::from_raw_fd(child_raw) };

    for child_name in read_dir_entry_names(&child_fd)? {
        let child_stat = match fstatat_nofollow(child_fd.as_raw_fd(), &child_name) {
            Ok(stat) => stat,
            Err(err) if err.kind() == ErrorKind::NotFound => continue,
            Err(err) => return Err(err),
        };
        let mode = child_stat.st_mode & libc::S_IFMT;
        if mode == libc::S_IFLNK {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Other,
                "Symlinks are not allowed",
            ));
        }
        if mode == libc::S_IFDIR {
            delete_dir_recursive_at(child_fd.as_raw_fd(), &child_name)?;
        } else {
            match unlinkat_nofollow(child_fd.as_raw_fd(), &child_name, 0) {
                Ok(()) => {}
                Err(err) if err.kind() == ErrorKind::NotFound => {}
                Err(err) => return Err(err),
            }
        }
    }

    unlinkat_nofollow(parent_fd, name, libc::AT_REMOVEDIR)
}

#[cfg(all(unix, target_os = "linux"))]
fn delete_nofollow_io(path: &Path) -> Result<(), std::io::Error> {
    let (parent_fd, name) = parent_fd_and_name(path)?;
    let stat = fstatat_nofollow(parent_fd.as_raw_fd(), &name)?;
    let mode = stat.st_mode & libc::S_IFMT;
    if mode == libc::S_IFLNK {
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "Symlinks are not allowed",
        ));
    }
    if mode == libc::S_IFDIR {
        delete_dir_recursive_at(parent_fd.as_raw_fd(), &name)
    } else {
        unlinkat_nofollow(parent_fd.as_raw_fd(), &name, 0)
    }
}

#[cfg(not(all(unix, target_os = "linux")))]
fn metadata_nofollow_path(path: &Path) -> Result<fs::Metadata, std::io::Error> {
    check_no_symlink_components(path).map_err(|e| std::io::Error::new(ErrorKind::Other, e))?;
    let meta = fs::symlink_metadata(path)?;
    if meta.file_type().is_symlink() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::Other,
            "Symlinks are not allowed",
        ));
    }
    Ok(meta)
}

#[cfg(not(all(unix, target_os = "linux")))]
fn delete_dir_recursive_nofollow_path(path: &Path) -> Result<(), std::io::Error> {
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        let child = entry.path();
        let child_meta = metadata_nofollow_path(&child)?;
        if child_meta.is_dir() {
            delete_dir_recursive_nofollow_path(&child)?;
        } else {
            fs::remove_file(&child)?;
        }
    }
    fs::remove_dir(path)
}

#[cfg(not(all(unix, target_os = "linux")))]
fn delete_nofollow_io(path: &Path) -> Result<(), std::io::Error> {
    let meta = metadata_nofollow_path(path)?;
    if meta.is_dir() {
        delete_dir_recursive_nofollow_path(path)
    } else {
        fs::remove_file(path)
    }
}

pub(crate) fn delete_entry_nofollow_io(path: &Path) -> Result<(), std::io::Error> {
    delete_nofollow_io(path)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PathKind {
    File,
    Dir,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PathSnapshot {
    kind: PathKind,
    #[cfg(unix)]
    dev: u64,
    #[cfg(unix)]
    ino: u64,
    #[cfg(not(unix))]
    len: u64,
    #[cfg(not(unix))]
    modified_nanos: Option<u128>,
}

fn path_kind_from_meta(meta: &fs::Metadata) -> PathKind {
    if meta.is_file() {
        PathKind::File
    } else if meta.is_dir() {
        PathKind::Dir
    } else {
        PathKind::Other
    }
}

fn path_snapshot_from_meta(meta: &fs::Metadata) -> PathSnapshot {
    PathSnapshot {
        kind: path_kind_from_meta(meta),
        #[cfg(unix)]
        dev: meta.dev(),
        #[cfg(unix)]
        ino: meta.ino(),
        #[cfg(not(unix))]
        len: meta.len(),
        #[cfg(not(unix))]
        modified_nanos: meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_nanos()),
    }
}

pub(crate) fn snapshot_existing_path(path: &Path) -> Result<PathSnapshot, String> {
    let meta = ensure_existing_path_nonsymlink(path)?;
    Ok(path_snapshot_from_meta(&meta))
}

fn snapshots_match(expected: &PathSnapshot, current: &PathSnapshot) -> bool {
    if expected.kind != current.kind {
        return false;
    }
    #[cfg(unix)]
    {
        expected.dev == current.dev && expected.ino == current.ino
    }
    #[cfg(not(unix))]
    {
        expected.len == current.len && expected.modified_nanos == current.modified_nanos
    }
}

pub(crate) fn assert_path_snapshot(path: &Path, expected: &PathSnapshot) -> Result<(), String> {
    let meta = ensure_existing_path_nonsymlink(path)?;
    let current = path_snapshot_from_meta(&meta);
    if snapshots_match(expected, &current) {
        Ok(())
    } else {
        Err(format!("Path changed during operation: {}", path.display()))
    }
}

fn execute_action(action: &mut Action, direction: Direction) -> Result<(), String> {
    match action {
        Action::Batch(actions) => execute_batch(actions, direction),
        Action::Rename { from, to } | Action::Move { from, to } => {
            let (src, dst) = match direction {
                Direction::Forward => (from, to),
                Direction::Backward => (to, from),
            };
            move_with_fallback(src, dst)
        }
        Action::Copy { from, to } => match direction {
            Direction::Forward => copy_entry(from, to),
            Direction::Backward => delete_entry_path(to),
        },
        Action::Create { path, backup } => match direction {
            Direction::Forward => move_with_fallback(backup, path),
            Direction::Backward => {
                let parent = backup
                    .parent()
                    .ok_or_else(|| "Invalid backup path".to_string())?;
                fs::create_dir_all(parent).map_err(|e| {
                    format!("Failed to create backup dir {}: {e}", parent.display())
                })?;
                move_with_fallback(path, backup)
            }
        },
        Action::Delete { path, backup } => match direction {
            Direction::Forward => {
                let parent = backup
                    .parent()
                    .ok_or_else(|| "Invalid backup path".to_string())?;
                fs::create_dir_all(parent).map_err(|e| {
                    format!("Failed to create backup dir {}: {e}", parent.display())
                })?;
                move_with_fallback(path, backup)
            }
            Direction::Backward => move_with_fallback(backup, path),
        },
        #[cfg(target_os = "windows")]
        Action::SetHidden { path, hidden } => {
            let next = match direction {
                Direction::Forward => *hidden,
                Direction::Backward => !*hidden,
            };
            set_windows_hidden_attr(path, next)
        }
        Action::CreateFolder { path } => match direction {
            Direction::Forward => fs::create_dir(&*path)
                .map_err(|e| format!("Failed to create directory {}: {e}", path.display())),
            Direction::Backward => match delete_entry_nofollow_io(path) {
                Ok(()) => Ok(()),
                Err(err) if err.kind() == ErrorKind::NotFound => Ok(()),
                Err(err) => Err(format!(
                    "Failed to remove directory {}: {err}",
                    path.display()
                )),
            },
        },
    }
}

#[cfg(target_os = "windows")]
fn set_windows_hidden_attr(path: &Path, hidden: bool) -> Result<(), String> {
    check_no_symlink_components(path)?;
    let no_follow = fs::symlink_metadata(path)
        .map_err(|e| format!("Failed to read metadata for {}: {e}", path.display()))?;
    if no_follow.file_type().is_symlink() {
        return Err(format!("Symlinks are not allowed: {}", path.display()));
    }

    let wide: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();
    let attrs = unsafe { GetFileAttributesW(wide.as_ptr()) };
    if attrs == u32::MAX {
        return Err(format!("GetFileAttributes failed for {}", path.display()));
    }

    let is_hidden = attrs & FILE_ATTRIBUTE_HIDDEN != 0;
    if is_hidden == hidden {
        return Ok(());
    }

    let mut next = attrs;
    if hidden {
        next |= FILE_ATTRIBUTE_HIDDEN;
    } else {
        next &= !FILE_ATTRIBUTE_HIDDEN;
    }
    let ok = unsafe { SetFileAttributesW(wide.as_ptr(), next) };
    if ok == 0 {
        return Err(format!("SetFileAttributes failed for {}", path.display()));
    }
    Ok(())
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

pub(crate) fn apply_permissions(path: &Path, snap: &PermissionsSnapshot) -> Result<(), String> {
    check_no_symlink_components(path)?;
    let meta_no_follow = fs::symlink_metadata(path)
        .map_err(|e| format!("Failed to read metadata for {}: {e}", path.display()))?;
    if meta_no_follow.file_type().is_symlink() {
        return Err(format!("Symlinks are not allowed: {}", path.display()));
    }
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

#[allow(dead_code)]
pub fn ownership_snapshot(path: &Path) -> Result<OwnershipSnapshot, String> {
    #[cfg(unix)]
    {
        check_no_symlink_components(path)?;
        let meta = fs::symlink_metadata(path)
            .map_err(|e| format!("Failed to read metadata for {}: {e}", path.display()))?;
        if meta.file_type().is_symlink() {
            return Err(format!("Symlinks are not allowed: {}", path.display()));
        }
        return Ok(OwnershipSnapshot {
            uid: meta.uid(),
            gid: meta.gid(),
        });
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        Err("Ownership changes are not supported on this platform".into())
    }
}

#[cfg(all(unix, target_os = "linux"))]
fn open_nofollow_path_fd(path: &Path) -> Result<OwnedFd, String> {
    use std::io;

    if !path.is_absolute() {
        return Err(format!("Path must be absolute: {}", path.display()));
    }

    let root = CString::new("/").map_err(|_| "Failed to build root path".to_string())?;
    let root_fd = unsafe {
        libc::open(
            root.as_ptr(),
            libc::O_PATH | libc::O_DIRECTORY | libc::O_CLOEXEC,
        )
    };
    if root_fd < 0 {
        return Err(format!(
            "Failed to open root while resolving {}: {}",
            path.display(),
            io::Error::last_os_error()
        ));
    }

    let mut current = unsafe { OwnedFd::from_raw_fd(root_fd) };
    let mut components = path.components().peekable();
    while let Some(component) = components.next() {
        match component {
            Component::RootDir | Component::CurDir => continue,
            Component::ParentDir => {
                return Err(format!(
                    "Parent directory components are not allowed: {}",
                    path.display()
                ));
            }
            Component::Normal(seg) => {
                let seg_name = seg.to_string_lossy().into_owned();
                let c_seg = CString::new(seg.as_bytes()).map_err(|_| {
                    format!(
                        "Invalid path component (NUL byte) while resolving {}",
                        path.display()
                    )
                })?;
                let is_last = components.peek().is_none();
                let mut flags = libc::O_PATH | libc::O_NOFOLLOW | libc::O_CLOEXEC;
                if !is_last {
                    flags |= libc::O_DIRECTORY;
                }
                let fd = unsafe { libc::openat(current.as_raw_fd(), c_seg.as_ptr(), flags) };
                if fd < 0 {
                    return Err(format!(
                        "Failed to open path component '{}' for {}: {}",
                        seg_name,
                        path.display(),
                        io::Error::last_os_error()
                    ));
                }
                current = unsafe { OwnedFd::from_raw_fd(fd) };
            }
            Component::Prefix(_) => {
                return Err(format!("Unsupported path prefix: {}", path.display()));
            }
        }
    }

    Ok(current)
}

pub(crate) fn set_ownership_nofollow(
    path: &Path,
    uid: Option<u32>,
    gid: Option<u32>,
) -> Result<(), String> {
    #[cfg(all(unix, target_os = "linux"))]
    {
        use std::io;

        if uid.is_none() && gid.is_none() {
            return Ok(());
        }
        let fd = open_nofollow_path_fd(path)?;
        let uid_arg = uid.map(|v| v as libc::uid_t).unwrap_or(!0 as libc::uid_t);
        let gid_arg = gid.map(|v| v as libc::gid_t).unwrap_or(!0 as libc::gid_t);
        let empty: [libc::c_char; 1] = [0];
        let rc = unsafe {
            libc::fchownat(
                fd.as_raw_fd(),
                empty.as_ptr(),
                uid_arg,
                gid_arg,
                libc::AT_EMPTY_PATH,
            )
        };
        if rc == 0 {
            return Ok(());
        }
        let err = io::Error::last_os_error();
        let suffix = match err.raw_os_error() {
            Some(code) if code == libc::EPERM || code == libc::EACCES => {
                " (requires elevated privileges: root or CAP_CHOWN)"
            }
            _ => "",
        };
        return Err(format!(
            "Failed to change owner/group for {}: {}{}",
            path.display(),
            err,
            suffix
        ));
    }
    #[cfg(all(unix, not(target_os = "linux")))]
    {
        use std::io;

        if uid.is_none() && gid.is_none() {
            return Ok(());
        }
        check_no_symlink_components(path)?;
        let bytes = path.as_os_str().as_bytes();
        let c_path = CString::new(bytes)
            .map_err(|_| format!("Path contains NUL byte: {}", path.display()))?;
        let uid_arg = uid.map(|v| v as libc::uid_t).unwrap_or(!0 as libc::uid_t);
        let gid_arg = gid.map(|v| v as libc::gid_t).unwrap_or(!0 as libc::gid_t);
        let rc = unsafe {
            libc::fchownat(
                libc::AT_FDCWD,
                c_path.as_ptr(),
                uid_arg,
                gid_arg,
                libc::AT_SYMLINK_NOFOLLOW,
            )
        };
        if rc == 0 {
            return Ok(());
        }
        let err = io::Error::last_os_error();
        let suffix = match err.raw_os_error() {
            Some(code) if code == libc::EPERM || code == libc::EACCES => {
                " (requires elevated privileges: root or CAP_CHOWN)"
            }
            _ => "",
        };
        return Err(format!(
            "Failed to change owner/group for {}: {}{}",
            path.display(),
            err,
            suffix
        ));
    }
    #[cfg(not(unix))]
    {
        let _ = (path, uid, gid);
        Err("Ownership changes are not supported on this platform".into())
    }
}

#[cfg(all(unix, target_os = "linux"))]
pub(crate) fn set_unix_mode_nofollow(path: &Path, mode: u32) -> Result<(), String> {
    use std::io;

    let fd = open_nofollow_path_fd(path)?;
    let empty: [libc::c_char; 1] = [0];
    let rc = unsafe {
        libc::fchmodat(
            fd.as_raw_fd(),
            empty.as_ptr(),
            mode as libc::mode_t,
            libc::AT_EMPTY_PATH,
        )
    };
    if rc == 0 {
        return Ok(());
    }
    let err = io::Error::last_os_error();
    Err(format!(
        "Failed to update permissions for {}: {}",
        path.display(),
        err
    ))
}

pub(crate) fn apply_ownership(path: &Path, snap: &OwnershipSnapshot) -> Result<(), String> {
    #[cfg(unix)]
    {
        check_no_symlink_components(path)?;
        let meta = fs::symlink_metadata(path)
            .map_err(|e| format!("Failed to read metadata for {}: {e}", path.display()))?;
        if meta.file_type().is_symlink() {
            return Err(format!("Symlinks are not allowed: {}", path.display()));
        }
        let current_uid = meta.uid();
        let current_gid = meta.gid();
        let uid = if current_uid != snap.uid {
            Some(snap.uid)
        } else {
            None
        };
        let gid = if current_gid != snap.gid {
            Some(snap.gid)
        } else {
            None
        };
        return set_ownership_nofollow(path, uid, gid);
    }
    #[cfg(not(unix))]
    {
        let _ = (path, snap);
        Err("Ownership changes are not supported on this platform".into())
    }
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
    let meta = ensure_existing_path_nonsymlink(src)?;
    let src_snapshot = path_snapshot_from_meta(&meta);
    if let Some(parent) = dest.parent() {
        ensure_existing_dir_nonsymlink(parent)?;
    }
    if meta.is_dir() {
        assert_path_snapshot(src, &src_snapshot)?;
        copy_dir(src, dest)
    } else {
        if let Some(parent) = dest.parent() {
            ensure_existing_dir_nonsymlink(parent)?;
        }
        assert_path_snapshot(src, &src_snapshot)?;
        copy_file_noreplace(src, dest)
    }
}

fn copy_file_noreplace(src: &Path, dest: &Path) -> Result<(), String> {
    let mut src_file =
        fs::File::open(src).map_err(|e| format!("Failed to open source file {:?}: {e}", src))?;
    let mut dst_file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(dest)
        .map_err(|e| {
            if e.kind() == ErrorKind::AlreadyExists {
                format!("Destination already exists: {}", dest.display())
            } else {
                format!("Failed to create destination file {:?}: {e}", dest)
            }
        })?;
    io::copy(&mut src_file, &mut dst_file)
        .map_err(|e| format!("Failed to copy file {:?} -> {:?}: {e}", src, dest))?;
    let perms = src_file
        .metadata()
        .map_err(|e| format!("Failed to read source permissions {:?}: {e}", src))?
        .permissions();
    fs::set_permissions(dest, perms)
        .map_err(|e| format!("Failed to set permissions on {:?}: {e}", dest))
}

fn copy_dir(src: &Path, dest: &Path) -> Result<(), String> {
    let src_snapshot = snapshot_existing_path(src)?;
    if let Some(parent) = dest.parent() {
        ensure_existing_dir_nonsymlink(parent)?;
    }
    assert_path_snapshot(src, &src_snapshot)?;
    fs::create_dir(dest).map_err(|e| {
        if e.kind() == ErrorKind::AlreadyExists {
            format!("Destination already exists: {}", dest.display())
        } else {
            format!("Failed to create dir {:?}: {e}", dest)
        }
    })?;
    for entry in fs::read_dir(src).map_err(|e| format!("Failed to read dir {:?}: {e}", src))? {
        let entry = entry.map_err(|e| format!("Failed to read dir entry: {e}"))?;
        let path = entry.path();
        let meta = ensure_existing_path_nonsymlink(&path)?;
        let child_snapshot = path_snapshot_from_meta(&meta);
        let target = dest.join(entry.file_name());
        if meta.is_dir() {
            assert_path_snapshot(&path, &child_snapshot)?;
            copy_dir(&path, &target)?;
        } else {
            assert_path_snapshot(&path, &child_snapshot)?;
            copy_file_noreplace(&path, &target)?;
        }
    }
    Ok(())
}

pub(crate) fn delete_entry_path(path: &Path) -> Result<(), String> {
    let snapshot = snapshot_existing_path(path)?;
    assert_path_snapshot(path, &snapshot)?;
    delete_entry_nofollow_io(path).map_err(|e| format!("Failed to delete {}: {e}", path.display()))
}

pub fn move_with_fallback(src: &Path, dst: &Path) -> Result<(), String> {
    let src_meta = ensure_existing_path_nonsymlink(src)?;
    let src_snapshot = path_snapshot_from_meta(&src_meta);
    if let Some(parent) = dst.parent() {
        ensure_existing_dir_nonsymlink(parent)?;
        let parent_snapshot = snapshot_existing_path(parent)?;
        assert_path_snapshot(parent, &parent_snapshot)?;
    } else {
        return Err("Invalid destination path".into());
    }
    assert_path_snapshot(src, &src_snapshot)?;
    match rename_nofollow_io(src, dst) {
        Ok(_) => Ok(()),
        Err(rename_err) => {
            if !is_cross_device(&rename_err) && !is_noreplace_unsupported(&rename_err) {
                return Err(format!(
                    "Failed to rename {} -> {}: {rename_err}",
                    src.display(),
                    dst.display()
                ));
            }
            move_by_copy_delete_noreplace(src, dst, &src_snapshot)
        }
    }
}

fn move_by_copy_delete_noreplace(
    src: &Path,
    dst: &Path,
    src_snapshot: &PathSnapshot,
) -> Result<(), String> {
    // Controlled fallback when atomic no-replace rename is unavailable
    // (or across filesystems): copy + delete without destination overwrite.
    copy_entry(src, dst).and_then(|_| {
        assert_path_snapshot(src, src_snapshot)?;
        delete_entry_path(src).map_err(|del_err| {
            // Best effort: clean up destination if delete failed to avoid duplicates.
            let _ = delete_entry_path(dst);
            format!(
                "Copied {} -> {} after fallback move, but failed to delete source: {del_err}",
                src.display(),
                dst.display()
            )
        })
    })
}

fn is_cross_device(err: &std::io::Error) -> bool {
    matches!(err.raw_os_error(), Some(17) | Some(18))
}

fn is_noreplace_unsupported(err: &std::io::Error) -> bool {
    err.kind() == ErrorKind::Unsupported
}

pub(crate) fn is_destination_exists_error(err: &str) -> bool {
    let lower = err.to_lowercase();
    lower.contains("destination already exists")
        || lower.contains("already exists")
        || lower.contains("file exists")
        || lower.contains("os error 17")
        || lower.contains("os error 183")
}

fn ensure_existing_path_nonsymlink(path: &Path) -> Result<fs::Metadata, String> {
    check_no_symlink_components(path)?;
    let meta = fs::symlink_metadata(path)
        .map_err(|e| format!("Failed to read metadata for {}: {e}", path.display()))?;
    if meta.file_type().is_symlink() {
        return Err(format!(
            "Refusing path with symlink target: {}",
            path.display()
        ));
    }
    Ok(meta)
}

fn ensure_existing_dir_nonsymlink(path: &Path) -> Result<(), String> {
    let meta = ensure_existing_path_nonsymlink(path)?;
    if !meta.is_dir() {
        return Err(format!("Expected directory path: {}", path.display()));
    }
    Ok(())
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
    use std::sync::OnceLock;
    use std::time::{Duration, SystemTime};

    fn uniq_path(label: &str) -> PathBuf {
        let ts = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or(Duration::from_secs(0))
            .as_nanos();
        std::env::temp_dir().join(format!("browsey-undo-test-{label}-{ts}"))
    }

    fn test_undo_dir() -> PathBuf {
        static DIR: OnceLock<PathBuf> = OnceLock::new();
        DIR.get_or_init(|| {
            let dir = uniq_path("undo-base");
            let _ = fs::remove_dir_all(&dir);
            std::env::set_var("BROWSEY_UNDO_DIR", &dir);
            dir
        })
        .clone()
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
        let _ = test_undo_dir();
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
    fn create_file_action_undo_redo() {
        let path = uniq_path("create-file").join("file.txt");
        write_file(&path, b"hello");
        assert!(path.exists());

        let backup = temp_backup_path(&path);
        let mut mgr = UndoManager::new();
        mgr.record_applied(Action::Create {
            path: path.clone(),
            backup: backup.clone(),
        });

        mgr.undo().unwrap();
        assert!(!path.exists());
        assert!(backup.exists());

        mgr.redo().unwrap();
        assert!(path.exists());
        assert!(!backup.exists());

        let _ = fs::remove_dir_all(path.parent().unwrap_or_else(|| Path::new(".")));
    }

    #[test]
    fn create_dir_action_undo_redo() {
        let dir = uniq_path("create-dir");
        fs::create_dir_all(&dir).unwrap();
        let backup = temp_backup_path(&dir);

        let mut mgr = UndoManager::new();
        mgr.record_applied(Action::Create {
            path: dir.clone(),
            backup: backup.clone(),
        });

        mgr.undo().unwrap();
        assert!(!dir.exists());
        assert!(backup.exists());

        mgr.redo().unwrap();
        assert!(dir.exists());
        assert!(!backup.exists());

        let _ = fs::remove_dir_all(&dir);
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
    fn move_with_fallback_refuses_existing_destination() {
        let dir = uniq_path("move-no-overwrite");
        let _ = fs::create_dir_all(&dir);
        let source = dir.join("source.txt");
        let dest = dir.join("dest.txt");
        write_file(&source, b"source-data");
        write_file(&dest, b"dest-data");

        let err = move_with_fallback(&source, &dest).expect_err("existing destination should fail");
        assert!(
            err.contains("File exists") || err.contains("already exists") || err.contains("rename"),
            "unexpected error: {err}"
        );
        assert!(source.exists(), "source should remain when move fails");
        assert_eq!(fs::read(&dest).unwrap_or_default(), b"dest-data");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn copy_delete_fallback_moves_file_without_overwrite() {
        let dir = uniq_path("move-copy-delete");
        let _ = fs::create_dir_all(&dir);
        let source = dir.join("source.txt");
        let dest = dir.join("dest.txt");
        write_file(&source, b"source-data");
        let src_snapshot = snapshot_existing_path(&source).expect("snapshot");

        move_by_copy_delete_noreplace(&source, &dest, &src_snapshot).expect("fallback move");
        assert!(
            !source.exists(),
            "source should be deleted after fallback move"
        );
        assert_eq!(fs::read(&dest).unwrap_or_default(), b"source-data");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn copy_delete_fallback_refuses_existing_destination() {
        let dir = uniq_path("move-copy-delete-exists");
        let _ = fs::create_dir_all(&dir);
        let source = dir.join("source.txt");
        let dest = dir.join("dest.txt");
        write_file(&source, b"source-data");
        write_file(&dest, b"dest-data");
        let src_snapshot = snapshot_existing_path(&source).expect("snapshot");

        let err = move_by_copy_delete_noreplace(&source, &dest, &src_snapshot)
            .expect_err("fallback move should fail when destination exists");
        assert!(is_destination_exists_error(&err), "unexpected error: {err}");
        assert!(
            source.exists(),
            "source should remain when destination exists"
        );
        assert_eq!(fs::read(&dest).unwrap_or_default(), b"dest-data");

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn delete_entry_path_removes_non_empty_directory() {
        let dir = uniq_path("delete-dir-recursive");
        let nested = dir.join("nested");
        let deep_file = nested.join("child.txt");
        let _ = fs::create_dir_all(&nested);
        write_file(&deep_file, b"child");

        delete_entry_path(&dir).expect("recursive delete should succeed");
        assert!(!dir.exists(), "directory should be removed recursively");
    }

    #[test]
    fn undo_failure_restores_stack() {
        let _ = test_undo_dir();
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
        assert!(
            err.contains("Backup")
                || err.contains("rename")
                || err.contains("metadata")
                || err.contains("does not exist")
        );
        assert!(mgr.can_undo());
        assert!(!mgr.can_redo());

        let _ = fs::remove_dir_all(&dir);
        let _ = fs::remove_dir_all(backup.parent().unwrap_or_else(|| Path::new(".")));
    }

    #[test]
    fn cleanup_prunes_stale_backup_dirs() {
        let base = test_undo_dir();
        let target = base.join("dummy");
        fs::create_dir_all(&target).unwrap();

        cleanup_stale_backups(Some(Duration::from_secs(0)));

        assert!(
            !target.exists(),
            "backup base contents should be removed during cleanup"
        );
    }

    #[test]
    fn path_snapshot_accepts_unchanged_path() {
        let dir = uniq_path("snapshot-unchanged");
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("file.txt");
        write_file(&path, b"one");

        let snapshot = snapshot_existing_path(&path).expect("snapshot should succeed");
        assert!(
            assert_path_snapshot(&path, &snapshot).is_ok(),
            "unchanged path should pass snapshot check"
        );

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn path_snapshot_detects_replaced_path() {
        let dir = uniq_path("snapshot-replaced");
        let _ = fs::create_dir_all(&dir);
        let path = dir.join("file.txt");
        write_file(&path, b"first");

        let snapshot = snapshot_existing_path(&path).expect("snapshot should succeed");
        let _ = fs::remove_file(&path);
        write_file(&path, b"second");

        let err = assert_path_snapshot(&path, &snapshot).expect_err("snapshot mismatch expected");
        assert!(err.contains("Path changed during operation"));

        let _ = fs::remove_dir_all(&dir);
    }
}

fn base_undo_dir() -> PathBuf {
    if let Ok(custom) = std::env::var("BROWSEY_UNDO_DIR") {
        return PathBuf::from(custom);
    }
    default_undo_dir()
}

fn default_undo_dir() -> PathBuf {
    dirs_next::data_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("browsey")
        .join("undo")
}

fn validate_undo_dir(path: &Path) -> Result<(), String> {
    if cfg!(test) {
        return Ok(());
    }
    if !path.is_absolute() {
        return Err("Undo directory must be an absolute path".into());
    }
    if path.parent().is_none() {
        return Err("Undo directory cannot be the filesystem root".into());
    }
    let default_parent = default_undo_dir()
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("/"));
    if !path.starts_with(&default_parent) {
        return Err(format!(
            "Undo directory must reside under {}",
            default_parent.display()
        ));
    }
    Ok(())
}
