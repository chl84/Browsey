#[cfg(all(unix, target_os = "linux"))]
use std::{
    ffi::CString,
    os::{
        fd::{AsRawFd, FromRawFd, OwnedFd},
        unix::ffi::OsStrExt,
    },
    path::Component,
};
use std::{
    fs::{self, File},
    io::{self, BufReader, Read, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
        Arc,
    },
    time::{SystemTime, UNIX_EPOCH},
};

use serde::Serialize;

use crate::{
    fs_utils::{debug_log, unique_path},
    runtime_lifecycle,
};

pub(super) const CHUNK: usize = 4 * 1024 * 1024;
pub(super) const EXTRACT_TOTAL_BYTES_CAP: u64 = 100_000_000_000; // 100 GB
pub(super) const EXTRACT_TOTAL_ENTRIES_CAP: u64 = 2_000_000; // 2 million entries

#[derive(Clone)]
pub(super) struct ExtractBudget {
    max_total_bytes: u64,
    max_total_entries: u64,
    written_total: Arc<AtomicU64>,
    entries_total: Arc<AtomicU64>,
}

impl ExtractBudget {
    pub(super) fn new(max_total_bytes: u64, max_total_entries: u64) -> Self {
        Self {
            max_total_bytes,
            max_total_entries,
            written_total: Arc::new(AtomicU64::new(0)),
            entries_total: Arc::new(AtomicU64::new(0)),
        }
    }

    pub(super) fn max_total_bytes(&self) -> u64 {
        self.max_total_bytes
    }

    pub(super) fn reserve_bytes(&self, delta: u64) -> io::Result<()> {
        loop {
            let current = self.written_total.load(Ordering::Relaxed);
            let projected = current.saturating_add(delta);
            if projected > self.max_total_bytes {
                return Err(io::Error::other(format!(
                    "Extraction exceeds size cap ({} bytes > {} bytes)",
                    projected, self.max_total_bytes
                )));
            }
            if self
                .written_total
                .compare_exchange(current, projected, Ordering::Relaxed, Ordering::Relaxed)
                .is_ok()
            {
                return Ok(());
            }
        }
    }

    pub(super) fn reserve_entry(&self, delta: u64) -> io::Result<()> {
        loop {
            let current = self.entries_total.load(Ordering::Relaxed);
            let projected = current.saturating_add(delta);
            if projected > self.max_total_entries {
                return Err(io::Error::other(format!(
                    "Extraction exceeds entry cap ({} entries > {} entries)",
                    projected, self.max_total_entries
                )));
            }
            if self
                .entries_total
                .compare_exchange(current, projected, Ordering::Relaxed, Ordering::Relaxed)
                .is_ok()
            {
                return Ok(());
            }
        }
    }
}

#[derive(Default, Clone)]
pub(super) struct SkipStats {
    pub(super) symlinks: Arc<AtomicUsize>,
    pub(super) unsupported: Arc<AtomicUsize>,
}

impl SkipStats {
    pub(super) fn skip_symlink(&self, path: &str) {
        self.symlinks.fetch_add(1, Ordering::Relaxed);
        debug_log(&format!("Skipping symlink entry while extracting: {path}"));
    }

    pub(super) fn skip_unsupported(&self, path: &str, reason: &str) {
        self.unsupported.fetch_add(1, Ordering::Relaxed);
        debug_log(&format!("Skipping unsupported entry {path}: {reason}"));
    }
}

pub(super) struct CreatedPaths {
    pub(super) files: Vec<PathBuf>,
    pub(super) dirs: Vec<PathBuf>,
    active: bool,
}

impl Default for CreatedPaths {
    fn default() -> Self {
        Self {
            files: Vec::new(),
            dirs: Vec::new(),
            active: true,
        }
    }
}

impl CreatedPaths {
    pub(super) fn record_file(&mut self, path: PathBuf) {
        self.files.push(path);
    }

    pub(super) fn record_dir(&mut self, path: PathBuf) {
        self.dirs.push(path);
    }

    pub(super) fn disarm(&mut self) {
        self.active = false;
    }
}

impl Drop for CreatedPaths {
    fn drop(&mut self) {
        if !self.active {
            return;
        }
        // Remove files first, then dirs in reverse to clean up partially extracted content.
        for file in self.files.iter().rev() {
            let _ = fs::remove_file(file);
        }
        for dir in self.dirs.iter().rev() {
            let _ = fs::remove_dir_all(dir);
        }
    }
}

#[derive(Serialize, Clone, Copy)]
struct ExtractProgressPayload {
    bytes: u64,
    total: u64,
    finished: bool,
}

#[derive(Clone)]
pub(super) struct ProgressEmitter {
    app: tauri::AppHandle,
    event: String,
    total: u64,
    done: Arc<AtomicU64>,
    last_emit: Arc<AtomicU64>,
    last_emit_time_ms: Arc<AtomicU64>,
}

impl ProgressEmitter {
    pub(super) fn new(app: tauri::AppHandle, event: String, total: u64) -> Self {
        Self {
            app,
            event,
            total,
            done: Arc::new(AtomicU64::new(0)),
            last_emit: Arc::new(AtomicU64::new(0)),
            last_emit_time_ms: Arc::new(AtomicU64::new(0)),
        }
    }

    pub(super) fn add(&self, delta: u64) {
        let done = self
            .done
            .fetch_add(delta, Ordering::Relaxed)
            .saturating_add(delta);
        let last = self.last_emit.load(Ordering::Relaxed);
        let now_ms = current_millis();
        let last_time = self.last_emit_time_ms.load(Ordering::Relaxed);
        if done != last && now_ms.saturating_sub(last_time) >= 1000 {
            if self
                .last_emit
                .compare_exchange(last, done, Ordering::Relaxed, Ordering::Relaxed)
                .is_ok()
            {
                let _ = self.last_emit_time_ms.compare_exchange(
                    last_time,
                    now_ms,
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                );
                let _ = runtime_lifecycle::emit_if_running(
                    &self.app,
                    &self.event,
                    ExtractProgressPayload {
                        bytes: done,
                        total: self.total,
                        finished: false,
                    },
                );
            }
        }
    }

    pub(super) fn finish(&self) {
        let done = self.done.load(Ordering::Relaxed);
        self.last_emit.store(done, Ordering::Relaxed);
        self.last_emit_time_ms
            .store(current_millis(), Ordering::Relaxed);
        let _ = runtime_lifecycle::emit_if_running(
            &self.app,
            &self.event,
            ExtractProgressPayload {
                bytes: done,
                total: self.total,
                finished: true,
            },
        );
    }
}

pub(super) fn is_cancelled(cancel: Option<&AtomicBool>) -> bool {
    cancel.map(|c| c.load(Ordering::Relaxed)).unwrap_or(false)
}

pub(super) fn check_cancel(cancel: Option<&AtomicBool>) -> io::Result<()> {
    if is_cancelled(cancel) {
        Err(io::Error::new(io::ErrorKind::Interrupted, "cancelled"))
    } else {
        Ok(())
    }
}

pub(super) fn map_copy_err(context: &str, err: io::Error) -> String {
    if err.kind() == io::ErrorKind::Interrupted {
        "Extraction cancelled".into()
    } else {
        format!("{context}: {err}")
    }
}

pub(super) fn map_io(action: &'static str) -> impl FnOnce(io::Error) -> String {
    move |e| format!("Failed to {action}: {e}")
}

pub(super) fn clean_relative_path(path: &Path) -> Result<PathBuf, String> {
    let mut cleaned = PathBuf::new();
    for comp in path.components() {
        match comp {
            std::path::Component::Normal(p) => cleaned.push(p),
            std::path::Component::CurDir => {}
            _ => return Err("Refusing path with traversal or absolute components".into()),
        }
    }
    Ok(cleaned)
}

pub(super) fn first_component(path: &Path) -> Option<PathBuf> {
    path.components().find_map(|c| match c {
        std::path::Component::Normal(p) => Some(PathBuf::from(p)),
        _ => None,
    })
}

#[cfg(all(unix, target_os = "linux"))]
fn absolute_path(path: &Path) -> io::Result<PathBuf> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Ok(std::env::current_dir()?.join(path))
    }
}

#[cfg(all(unix, target_os = "linux"))]
fn cstring_from_os_component(component: &std::ffi::OsStr) -> io::Result<CString> {
    CString::new(component.as_bytes()).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "Path component contains an unexpected NUL byte",
        )
    })
}

#[cfg(all(unix, target_os = "linux"))]
fn open_nofollow_dir_fd(path: &Path) -> io::Result<OwnedFd> {
    let abs = absolute_path(path)?;
    if !abs.is_absolute() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("Path must be absolute: {}", abs.display()),
        ));
    }

    let root = CString::new("/")
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "Invalid root path"))?;
    let root_fd = unsafe {
        libc::open(
            root.as_ptr(),
            libc::O_PATH | libc::O_DIRECTORY | libc::O_CLOEXEC,
        )
    };
    if root_fd < 0 {
        return Err(io::Error::last_os_error());
    }
    let mut current = unsafe { OwnedFd::from_raw_fd(root_fd) };

    for component in abs.components() {
        match component {
            Component::RootDir | Component::CurDir => continue,
            Component::ParentDir => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!(
                        "Parent directory components are not allowed: {}",
                        abs.display()
                    ),
                ));
            }
            Component::Normal(seg) => {
                let c_seg = cstring_from_os_component(seg)?;
                let fd = unsafe {
                    libc::openat(
                        current.as_raw_fd(),
                        c_seg.as_ptr(),
                        libc::O_PATH | libc::O_NOFOLLOW | libc::O_DIRECTORY | libc::O_CLOEXEC,
                    )
                };
                if fd < 0 {
                    return Err(io::Error::last_os_error());
                }
                current = unsafe { OwnedFd::from_raw_fd(fd) };
            }
            Component::Prefix(_) => {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("Unsupported path prefix: {}", abs.display()),
                ));
            }
        }
    }

    Ok(current)
}

#[cfg(all(unix, target_os = "linux"))]
fn open_parent_dir_fd_and_name(path: &Path) -> io::Result<(OwnedFd, CString)> {
    let abs = absolute_path(path)?;
    let parent = abs.parent().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("Path is missing parent: {}", abs.display()),
        )
    })?;
    let name = abs.file_name().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("Path is missing file name: {}", abs.display()),
        )
    })?;
    let parent_fd = open_nofollow_dir_fd(parent)?;
    let c_name = cstring_from_os_component(name)?;
    Ok((parent_fd, c_name))
}

#[cfg(all(unix, target_os = "linux"))]
fn open_unique_file_nofollow(path: &Path) -> io::Result<File> {
    let (parent_fd, name) = open_parent_dir_fd_and_name(path)?;
    let fd = unsafe {
        libc::openat(
            parent_fd.as_raw_fd(),
            name.as_ptr(),
            libc::O_WRONLY | libc::O_CREAT | libc::O_EXCL | libc::O_NOFOLLOW | libc::O_CLOEXEC,
            0o644,
        )
    };
    if fd < 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(unsafe { File::from_raw_fd(fd) })
    }
}

pub(super) fn ensure_dir_nofollow(path: &Path) -> Result<Vec<PathBuf>, String> {
    #[cfg(all(unix, target_os = "linux"))]
    {
        let abs = absolute_path(path)
            .map_err(|e| format!("Failed to resolve directory {}: {e}", path.display()))?;
        if !abs.is_absolute() {
            return Err(format!(
                "Directory path must be absolute: {}",
                abs.display()
            ));
        }

        let root = CString::new("/").map_err(|_| "Invalid root path".to_string())?;
        let root_fd = unsafe {
            libc::open(
                root.as_ptr(),
                libc::O_PATH | libc::O_DIRECTORY | libc::O_CLOEXEC,
            )
        };
        if root_fd < 0 {
            return Err(format!(
                "Failed to open root while creating {}: {}",
                abs.display(),
                io::Error::last_os_error()
            ));
        }

        let mut current = unsafe { OwnedFd::from_raw_fd(root_fd) };
        let mut current_path = PathBuf::from("/");
        let mut created = Vec::new();

        for component in abs.components() {
            match component {
                Component::RootDir | Component::CurDir => continue,
                Component::ParentDir => {
                    return Err(format!(
                        "Parent directory components are not allowed: {}",
                        abs.display()
                    ));
                }
                Component::Normal(seg) => {
                    let c_seg = cstring_from_os_component(seg)
                        .map_err(|e| format!("Invalid path component in {}: {e}", abs.display()))?;
                    let mkdir_rc =
                        unsafe { libc::mkdirat(current.as_raw_fd(), c_seg.as_ptr(), 0o755) };
                    if mkdir_rc != 0 {
                        let err = io::Error::last_os_error();
                        if err.kind() != io::ErrorKind::AlreadyExists {
                            return Err(format!(
                                "Failed to create directory {}: {err}",
                                abs.display()
                            ));
                        }
                    } else {
                        current_path.push(seg);
                        created.push(current_path.clone());
                        let next_fd = unsafe {
                            libc::openat(
                                current.as_raw_fd(),
                                c_seg.as_ptr(),
                                libc::O_PATH
                                    | libc::O_NOFOLLOW
                                    | libc::O_DIRECTORY
                                    | libc::O_CLOEXEC,
                            )
                        };
                        if next_fd < 0 {
                            return Err(format!(
                                "Failed to open directory {}: {}",
                                current_path.display(),
                                io::Error::last_os_error()
                            ));
                        }
                        current = unsafe { OwnedFd::from_raw_fd(next_fd) };
                        continue;
                    }

                    current_path.push(seg);
                    let next_fd = unsafe {
                        libc::openat(
                            current.as_raw_fd(),
                            c_seg.as_ptr(),
                            libc::O_PATH | libc::O_NOFOLLOW | libc::O_DIRECTORY | libc::O_CLOEXEC,
                        )
                    };
                    if next_fd < 0 {
                        return Err(format!(
                            "Failed to open directory {}: {}",
                            current_path.display(),
                            io::Error::last_os_error()
                        ));
                    }
                    current = unsafe { OwnedFd::from_raw_fd(next_fd) };
                }
                Component::Prefix(_) => {
                    return Err(format!("Unsupported path prefix: {}", abs.display()));
                }
            }
        }

        Ok(created)
    }

    #[cfg(not(all(unix, target_os = "linux")))]
    {
        let existed = path.exists();
        fs::create_dir_all(path)
            .map_err(|e| format!("Failed to create directory {}: {e}", path.display()))?;
        if existed {
            Ok(Vec::new())
        } else {
            Ok(vec![path.to_path_buf()])
        }
    }
}

pub(super) fn path_exists_nofollow(path: &Path) -> Result<bool, String> {
    #[cfg(all(unix, target_os = "linux"))]
    {
        let abs = absolute_path(path)
            .map_err(|e| format!("Failed to resolve path {}: {e}", path.display()))?;
        if abs == Path::new("/") {
            return Ok(true);
        }
        let Some(name) = abs.file_name() else {
            return Ok(true);
        };
        let Some(parent) = abs.parent() else {
            return Ok(false);
        };
        let parent_fd = match open_nofollow_dir_fd(parent) {
            Ok(fd) => fd,
            Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(false),
            Err(err) => {
                return Err(format!(
                    "Failed to open parent directory {}: {err}",
                    parent.display()
                ))
            }
        };
        let c_name = cstring_from_os_component(name).map_err(|e| {
            format!(
                "Invalid path component while checking {}: {e}",
                abs.display()
            )
        })?;
        let mut stat = std::mem::MaybeUninit::<libc::stat>::uninit();
        let rc = unsafe {
            libc::fstatat(
                parent_fd.as_raw_fd(),
                c_name.as_ptr(),
                stat.as_mut_ptr(),
                libc::AT_SYMLINK_NOFOLLOW,
            )
        };
        if rc == 0 {
            Ok(true)
        } else {
            let err = io::Error::last_os_error();
            if err.kind() == io::ErrorKind::NotFound {
                Ok(false)
            } else {
                Err(format!("Failed to stat path {}: {err}", abs.display()))
            }
        }
    }

    #[cfg(not(all(unix, target_os = "linux")))]
    {
        match fs::symlink_metadata(path) {
            Ok(_) => Ok(true),
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(err) => Err(format!("Failed to stat path {}: {err}", path.display())),
        }
    }
}

pub(super) fn create_unique_dir_nofollow(parent: &Path, base: &str) -> Result<PathBuf, String> {
    #[cfg(all(unix, target_os = "linux"))]
    {
        let abs_parent = absolute_path(parent).map_err(|e| {
            format!(
                "Failed to resolve parent directory {}: {e}",
                parent.display()
            )
        })?;
        let _ = ensure_dir_nofollow(&abs_parent)?;
        let parent_fd = open_nofollow_dir_fd(&abs_parent).map_err(|e| {
            format!(
                "Failed to open parent directory {}: {e}",
                abs_parent.display()
            )
        })?;
        let mut idx = 0usize;
        loop {
            let name = if idx == 0 {
                base.to_string()
            } else {
                format!("{base}-{idx}")
            };
            idx = idx.saturating_add(1);
            let c_name = CString::new(name.as_bytes()).map_err(|_| {
                format!(
                    "Failed to create destination folder with invalid name '{}'",
                    name
                )
            })?;
            let rc = unsafe { libc::mkdirat(parent_fd.as_raw_fd(), c_name.as_ptr(), 0o755) };
            if rc == 0 {
                return Ok(abs_parent.join(name));
            }
            let err = io::Error::last_os_error();
            if err.kind() == io::ErrorKind::AlreadyExists {
                continue;
            }
            return Err(format!(
                "Failed to create destination folder {}: {err}",
                abs_parent.join(name).display()
            ));
        }
    }

    #[cfg(not(all(unix, target_os = "linux")))]
    {
        ensure_dir_nofollow(parent)?;
        let mut candidate = parent.join(base);
        let mut idx = 1usize;
        loop {
            match fs::create_dir(&candidate) {
                Ok(_) => return Ok(candidate),
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                    candidate = parent.join(format!("{base}-{idx}"));
                    idx = idx.saturating_add(1);
                }
                Err(e) => {
                    return Err(format!(
                        "Failed to create destination folder {}: {e}",
                        candidate.display()
                    ))
                }
            }
        }
    }
}

pub(super) fn open_unique_file(dest_path: &Path) -> Result<(File, PathBuf), String> {
    #[cfg(all(unix, target_os = "linux"))]
    let mut candidate = absolute_path(dest_path)
        .map_err(|e| format!("Failed to resolve path {}: {e}", dest_path.display()))?;
    #[cfg(not(all(unix, target_os = "linux")))]
    let mut candidate = dest_path.to_path_buf();
    loop {
        #[cfg(all(unix, target_os = "linux"))]
        let create_result = open_unique_file_nofollow(&candidate);
        #[cfg(not(all(unix, target_os = "linux")))]
        let create_result = File::options()
            .write(true)
            .create_new(true)
            .open(&candidate);

        match create_result {
            Ok(f) => return Ok((f, candidate)),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                candidate = unique_path(&candidate);
                continue;
            }
            Err(e) => {
                return Err(format!(
                    "Failed to create file {}: {e}",
                    candidate.display()
                ))
            }
        }
    }
}

pub(super) fn copy_with_progress<R: Read, W: Write>(
    mut reader: R,
    mut writer: W,
    progress: Option<&ProgressEmitter>,
    cancel: Option<&AtomicBool>,
    budget: &ExtractBudget,
    buf: &mut [u8],
) -> io::Result<u64> {
    let mut written: u64 = 0;
    loop {
        check_cancel(cancel)?;
        let n = reader.read(buf)?;
        if n == 0 {
            break;
        }
        budget.reserve_bytes(n as u64)?;
        writer.write_all(&buf[..n])?;
        written = written.saturating_add(n as u64);
        if let Some(p) = progress {
            p.add(n as u64);
        }
    }
    Ok(written)
}

pub(super) fn open_buffered_file(
    path: &Path,
    action: &'static str,
) -> Result<BufReader<File>, String> {
    let file = File::open(path).map_err(map_io(action))?;
    Ok(BufReader::with_capacity(CHUNK, file))
}

pub(super) fn strip_known_suffixes(name: &str) -> String {
    let lower = name.to_lowercase();
    for suffix in [
        ".tar.gz", ".tgz", ".tar.bz2", ".tbz2", ".tar.xz", ".txz", ".tar.zst", ".tzst", ".tar",
        ".7z", ".rar",
    ] {
        if lower.ends_with(suffix) && name.len() > suffix.len() {
            return name[..name.len() - suffix.len()].to_string();
        }
    }
    if let Some((stem, _ext)) = name.rsplit_once('.') {
        if !stem.is_empty() {
            return stem.to_string();
        }
    }
    name.to_string()
}

pub(super) fn current_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
