use std::path::{Path, PathBuf};
use std::time::SystemTime;
#[cfg(target_os = "windows")]
#[allow(unused_imports)]
use tracing::warn;

#[cfg(target_os = "windows")]
use std::path::{Component, Prefix};

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
    let is_drive = is_drive_path(&pb) && !is_special_drive(drive);
    debug_log(&format!(
        "sanitize_follow start: raw={} resolved={} network={}",
        raw,
        pb.display(),
        is_net
    ));
    let canon = if is_net || is_drive {
        // Skip canonicalize on UNC to avoid DFS/SMB failures; use resolved path as-is.
        pb.clone()
    } else {
        match pb.canonicalize() {
            Ok(c) => c,
            Err(e) => {
                debug_log(&format!(
                    "canonicalize failed: path={} error={:?}",
                    pb.display(),
                    e
                ));
                return Err(format!("Failed to canonicalize path: {e}"));
            }
        }
    };
    debug_log(&format!(
        "sanitize_follow result: raw={} canon={}",
        raw,
        canon.display()
    ));
    if forbid_root && canon.parent().is_none() {
        return Err("Refusing to operate on filesystem root".into());
    }
    Ok(normalize_verbatim(&canon))
}

pub fn sanitize_path_nofollow(raw: &str, forbid_root: bool) -> Result<PathBuf, String> {
    let raw = normalize_drive_root(raw);
    let pb = PathBuf::from(&raw);
    let drive = drive_letter(&pb);
    let is_net = is_network_path(&pb) && !is_special_drive(drive);
    debug_log(&format!(
        "sanitize_nofollow: raw={} resolved={} network={}",
        raw,
        pb.display(),
        is_net
    ));
    if !is_net {
        let _meta = std::fs::symlink_metadata(&pb)
            .map_err(|e| {
                debug_log(&format!(
                    "symlink_metadata failed: path={} error={:?}",
                    pb.display(),
                    e
                ));
                format!("Path does not exist or unreadable: {e}")
            })?;
    }
    if forbid_root && pb.parent().is_none() {
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
        if !candidate.exists() {
            return candidate;
        }
        idx += 1;
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
        Some(Component::Prefix(prefix)) => matches!(
            prefix.kind(),
            Prefix::VerbatimUNC(..) | Prefix::UNC(..)
        ),
        _ => false,
    }
}

#[cfg(not(target_os = "windows"))]
fn is_network_path(_path: &Path) -> bool {
    false
}

#[cfg(target_os = "windows")]
fn is_drive_path(path: &Path) -> bool {
    matches!(
        path.components().next(),
        Some(Component::Prefix(p)) if matches!(p.kind(), Prefix::Disk(_) | Prefix::VerbatimDisk(_))
    )
}

#[cfg(not(target_os = "windows"))]
fn is_drive_path(_path: &Path) -> bool {
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

fn is_special_drive(_letter: Option<u8>) -> bool {
    // Treat all drive letters uniformly; avoid hard-coded exceptions.
    false
}

pub fn debug_log(line: &str) {
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_secs_f64())
        .unwrap_or(0.0);
    let log_file = std::env::temp_dir().join("filey-debug.log");
    let line = format!("{now:.3} {line}\n");
    if let Err(e) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_file)
        .and_then(|mut f| std::io::Write::write_all(&mut f, line.as_bytes()))
    {
        eprintln!("Failed to write debug log {:?}: {}", log_file, e);
    }
}
