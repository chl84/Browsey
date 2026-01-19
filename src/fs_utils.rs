use std::path::{Component, Path, PathBuf};
use std::{fs::OpenOptions, io};
use tracing::debug;

#[cfg(target_os = "windows")]
use std::path::Prefix;

#[cfg(target_os = "windows")]
fn normalize_drive_root(raw: &str) -> String {
    let mut chars = raw.chars();
    match (chars.next(), chars.next(), chars.next()) {
        // "J" -> "J:\"
        (Some(c1), None, _) if c1.is_ascii_alphabetic() => format!("{}:\\", c1),
        // "J:" -> "J:\"
        (Some(c1), Some(':'), None) if c1.is_ascii_alphabetic() => format!("{}:\\", c1),
        _ => raw.to_string(),
    }
}

#[cfg(not(target_os = "windows"))]
fn normalize_drive_root(raw: &str) -> String {
    raw.to_string()
}

pub fn sanitize_path_follow(raw: &str, forbid_root: bool) -> Result<PathBuf, String> {
    let raw = normalize_drive_root(raw);
    let pb = PathBuf::from(&raw);
    let drive = drive_letter(&pb);
    let is_net = is_network_path(&pb) && !is_special_drive(drive);
    #[cfg(target_os = "windows")]
    let is_remote_drive = drive.is_some() && is_remote_drive(&pb);
    #[cfg(not(target_os = "windows"))]
    let is_remote_drive = false;
    debug!(
        raw = %raw,
        resolved = %pb.display(),
        network = is_net || is_remote_drive,
        "sanitize_follow start"
    );
    let canon = if is_net || is_remote_drive {
        // Skip canonicalize on UNC or remote drives to avoid DFS/SMB resolution; use as-given.
        pb.clone()
    } else {
        match pb.canonicalize() {
            Ok(c) => c,
            Err(e) => {
                debug!(path = %pb.display(), error = ?e, "canonicalize failed");
                return Err(format!("Failed to canonicalize path: {e}"));
            }
        }
    };
    debug!(
        raw = %raw,
        canon = %canon.display(),
        "sanitize_follow result"
    );
    if forbid_root && canon.is_absolute() && canon.parent().is_none() {
        return Err("Refusing to operate on filesystem root".into());
    }
    Ok(normalize_verbatim(&canon))
}

pub fn sanitize_path_nofollow(raw: &str, forbid_root: bool) -> Result<PathBuf, String> {
    let raw = normalize_drive_root(raw);
    let pb = PathBuf::from(&raw);
    let drive = drive_letter(&pb);
    let is_net = is_network_path(&pb) && !is_special_drive(drive);
    debug!(
        raw = %raw,
        resolved = %pb.display(),
        network = is_net,
        "sanitize_nofollow start"
    );
    if !is_net {
        let _meta = std::fs::symlink_metadata(&pb).map_err(|e| {
            debug!(path = %pb.display(), error = ?e, "symlink_metadata failed");
            format!("Path does not exist or unreadable: {e}")
        })?;
    }
    if forbid_root && pb.is_absolute() && pb.parent().is_none() {
        return Err("Refusing to operate on filesystem root".into());
    }
    Ok(normalize_verbatim(&pb))
}

#[allow(dead_code)]
pub fn unique_path(dest: &Path) -> PathBuf {
    if !dest.exists() {
        return dest.to_path_buf();
    }
    let mut idx = 1;
    let stem = dest
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "item".to_string());
    let ext = dest.extension().map(|e| e.to_string_lossy().to_string());
    let parent = dest.parent().unwrap_or_else(|| Path::new("."));
    loop {
        let mut candidate = parent.join(format!("{}-{}", stem, idx));
        if let Some(ext) = &ext {
            candidate.set_extension(ext);
        }
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(file) => {
                drop(file);
                let _ = std::fs::remove_file(&candidate);
                return candidate;
            }
            Err(e)
                if matches!(
                    e.kind(),
                    io::ErrorKind::AlreadyExists | io::ErrorKind::PermissionDenied
                ) =>
            {
                idx += 1;
                continue;
            }
            Err(_) if candidate.exists() => {
                idx += 1;
                continue;
            }
            Err(_) => return candidate,
        }
    }
}

#[cfg(target_os = "windows")]
fn normalize_verbatim(path: &Path) -> PathBuf {
    let s = path.to_string_lossy();
    if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
        return PathBuf::from(format!(r"\\{rest}"));
    }
    if let Some(rest) = s.strip_prefix(r"\\?\") {
        return PathBuf::from(rest);
    }
    path.to_path_buf()
}

#[cfg(not(target_os = "windows"))]
fn normalize_verbatim(path: &Path) -> PathBuf {
    path.to_path_buf()
}

#[cfg(target_os = "windows")]
fn is_network_path(path: &Path) -> bool {
    match path.components().next() {
        Some(Component::Prefix(prefix)) => {
            matches!(prefix.kind(), Prefix::VerbatimUNC(..) | Prefix::UNC(..))
        }
        _ => false,
    }
}

#[cfg(not(target_os = "windows"))]
fn is_network_path(_path: &Path) -> bool {
    false
}

#[cfg(target_os = "windows")]
fn drive_letter(path: &Path) -> Option<u8> {
    match path.components().next() {
        Some(Component::Prefix(p)) => match p.kind() {
            Prefix::Disk(c) | Prefix::VerbatimDisk(c) => Some(c),
            _ => None,
        },
        _ => None,
    }
}

#[cfg(not(target_os = "windows"))]
fn drive_letter(_path: &Path) -> Option<u8> {
    None
}

#[cfg(target_os = "windows")]
fn is_remote_drive(path: &Path) -> bool {
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::GetDriveTypeW;

    if let Some(letter) = drive_letter(path) {
        let root = format!("{}:\\", letter as char);
        let wide: Vec<u16> = std::ffi::OsStr::new(&root).encode_wide().chain(std::iter::once(0)).collect();
        unsafe { GetDriveTypeW(wide.as_ptr()) == 4 }
    } else {
        false
    }
}

fn is_special_drive(_letter: Option<u8>) -> bool {
    // Treat all drive letters uniformly; avoid hard-coded exceptions.
    false
}

pub fn check_no_symlink_components(path: &Path) -> Result<(), String> {
    if is_network_path(path) {
        return Ok(());
    }
    let mut acc = PathBuf::new();
    for comp in path.components() {
        match comp {
            Component::Prefix(p) => {
                acc.push(p.as_os_str());
                continue;
            }
            Component::RootDir => {
                acc.push(Component::RootDir.as_os_str());
                continue;
            }
            Component::CurDir => continue,
            Component::ParentDir => {
                acc.pop();
                continue;
            }
            Component::Normal(seg) => acc.push(seg),
        }
        // Skip empty or just root
        if acc.as_os_str().is_empty() {
            continue;
        }
        let meta = std::fs::symlink_metadata(&acc)
            .map_err(|e| format!("Failed to read metadata for {:?}: {e}", acc))?;
        if meta.file_type().is_symlink() {
            return Err(format!(
                "Symlinks are not allowed in path: {}",
                acc.display()
            ));
        }
    }
    Ok(())
}

pub fn debug_log(line: &str) {
    // Route debug traces through the standard tracing backend (configured in init_logging).
    debug!("{line}");
}
