#[cfg(not(all(unix, target_os = "linux")))]
use std::fs;
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
    fs::File,
    io,
    path::{Path, PathBuf},
};

use crate::fs_utils::unique_path;

use crate::commands::decompress::error::{DecompressError, DecompressResult};

pub(crate) fn clean_relative_path(path: &Path) -> DecompressResult<PathBuf> {
    let mut cleaned = PathBuf::new();
    for comp in path.components() {
        match comp {
            std::path::Component::Normal(p) => cleaned.push(p),
            std::path::Component::CurDir => {}
            _ => {
                return Err(DecompressError::from_external_message(
                    "Refusing path with traversal or absolute components",
                ))
            }
        }
    }
    Ok(cleaned)
}

pub(crate) fn first_component(path: &Path) -> Option<PathBuf> {
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

pub(crate) fn ensure_dir_nofollow(path: &Path) -> DecompressResult<Vec<PathBuf>> {
    #[cfg(all(unix, target_os = "linux"))]
    {
        let abs = absolute_path(path)
            .map_err(|e| format!("Failed to resolve directory {}: {e}", path.display()))?;
        if !abs.is_absolute() {
            return Err(format!("Directory path must be absolute: {}", abs.display()).into());
        }

        let root = CString::new("/").map_err(|_| "Invalid root path")?;
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
            )
            .into());
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
                    )
                    .into());
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
                            )
                            .into());
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
                            )
                            .into());
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
                        )
                        .into());
                    }
                    current = unsafe { OwnedFd::from_raw_fd(next_fd) };
                }
                Component::Prefix(_) => {
                    return Err(format!("Unsupported path prefix: {}", abs.display()).into());
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

pub(crate) fn path_exists_nofollow(path: &Path) -> DecompressResult<bool> {
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
                )
                .into())
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
                Err(format!("Failed to stat path {}: {err}", abs.display()).into())
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

pub(crate) fn create_unique_dir_nofollow(parent: &Path, base: &str) -> DecompressResult<PathBuf> {
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
            )
            .into());
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

pub(crate) fn open_unique_file(dest_path: &Path) -> DecompressResult<(File, PathBuf)> {
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
                return Err(format!("Failed to create file {}: {e}", candidate.display()).into())
            }
        }
    }
}

pub(crate) fn strip_known_suffixes(name: &str) -> String {
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
