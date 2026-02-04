//! Helpers for GVFS-backed mounts (e.g., MTP over GVFS).

#[cfg(not(target_os = "windows"))]
use crate::commands::fs::MountInfo;
#[cfg(not(target_os = "windows"))]
use crate::fs_utils::debug_log;
#[cfg(not(target_os = "windows"))]
use once_cell::sync::OnceCell;
#[cfg(not(target_os = "windows"))]
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::{Duration, Instant},
};

#[cfg(not(target_os = "windows"))]
fn gvfs_root() -> Option<PathBuf> {
    dirs_next::runtime_dir().map(|p| p.join("gvfs"))
}

#[cfg(not(target_os = "windows"))]
pub fn has_mount_prefix(prefix: &str) -> bool {
    if let Some(root) = gvfs_root() {
        if let Ok(rd) = fs::read_dir(root) {
            return rd.flatten().any(|e| {
                e.file_name()
                    .to_string_lossy()
                    .to_lowercase()
                    .starts_with(&format!("{}:", prefix.to_lowercase()))
            });
        }
    }
    false
}

#[cfg(not(target_os = "windows"))]
pub fn ensure_mount(prefix: &str) -> bool {
    static LAST_ATTEMPT: OnceCell<std::sync::Mutex<Instant>> = OnceCell::new();
    let guard = LAST_ATTEMPT.get_or_init(|| std::sync::Mutex::new(Instant::now() - Duration::from_secs(10)));
    if let Ok(last) = guard.lock() {
        if last.elapsed() < Duration::from_secs(5) {
            return false;
        }
    }

    let uri = find_onedrive_uri(prefix).or_else(|| find_onedrive_uri_goa(prefix));
    let Some(uri) = uri else {
        debug_log("ensure_mount: no OneDrive URI found");
        return false;
    };

    if let Ok(mut last) = guard.lock() {
        *last = Instant::now();
    }

    match Command::new("gio").arg("mount").arg(&uri).status() {
        Ok(status) if status.success() => {
            // Wait briefly for the mount to appear
            let deadline = Instant::now() + Duration::from_secs(2);
            while Instant::now() < deadline {
                if has_mount_prefix(prefix) {
                    return true;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
            true
        }
        Ok(status) => {
            debug_log(&format!("ensure_mount: gio mount {uri} failed: {status:?}"));
            false
        }
        Err(e) => {
            debug_log(&format!("ensure_mount: spawn failed for gio mount {uri}: {e}"));
            false
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn find_onedrive_uri(_prefix: &str) -> Option<String> {
    let output = Command::new("gio").arg("mount").arg("-li").output().ok()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    stdout
        .lines()
        .flat_map(|l| l.split_whitespace())
        .find(|p| p.to_ascii_lowercase().starts_with("onedrive://"))
        .map(|s| s.to_string())
}

#[cfg(not(target_os = "windows"))]
fn find_onedrive_uri_goa(prefix: &str) -> Option<String> {
    if prefix.to_ascii_lowercase() != "onedrive" {
        return None;
    }
    let conf = dirs_next::config_dir()?.join("goa-1.0").join("accounts.conf");
    let contents = fs::read_to_string(conf).ok()?;
    let mut id: Option<String> = None;
    let mut identity: Option<String> = None;
    let mut presentation: Option<String> = None;
    let mut provider = false;
    for line in contents.lines() {
        let line = line.trim();
        if line.starts_with('[') && line.ends_with(']') {
            provider = false;
            id = None;
            identity = None;
            presentation = None;
            continue;
        }
        if line.eq_ignore_ascii_case("Provider=msgraph") || line.eq_ignore_ascii_case("Provider=ms_graph") {
            provider = true;
            continue;
        }
        if !provider {
            continue;
        }
        if let Some(rest) = line.strip_prefix("Id=") {
            id = Some(rest.trim().to_string());
        }
        if let Some(rest) = line.strip_prefix("Identity=") {
            identity = Some(rest.trim().to_string());
        }
        if let Some(rest) = line.strip_prefix("PresentationIdentity=") {
            presentation = Some(rest.trim().to_string());
        }
    }
    let chosen = presentation
        .or(identity)
        .or(id);
    chosen.map(|s| format!("onedrive://{s}/"))
}

#[cfg(not(target_os = "windows"))]
pub fn spawn_mount_async(prefix: &str) {
    let pref = prefix.to_string();
    std::thread::spawn(move || {
        let _ = ensure_mount(&pref);
    });
}

#[cfg(target_os = "windows")]
pub fn spawn_mount_async(_prefix: &str) {}

#[cfg(not(target_os = "windows"))]
fn display_name(path: &Path) -> Option<String> {
    // Use `gio info` to fetch the user-facing name. Falls back to directory name on failure.
    let output = Command::new("gio")
        .arg("info")
        .arg("--attributes=standard::display-name")
        .arg(path)
        .output()
        .ok()?;

    if !output.status.success() {
        debug_log(&format!(
            "gio info failed for {}: status {:?}",
            path.display(),
            output.status.code()
        ));
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        let trimmed = line.trim_start();
        // Usually: "display name: Pixel 7" (localized key should still contain the attribute name)
        if let Some(rest) = trimmed.strip_prefix("display name:") {
            return Some(rest.trim().to_string());
        }
        if let Some(rest) = trimmed.strip_prefix("standard::display-name:") {
            return Some(rest.trim().to_string());
        }
    }
    None
}

#[cfg(not(target_os = "windows"))]
pub fn list_gvfs_mounts() -> Vec<MountInfo> {
    let mut mounts = Vec::new();
    let root = match gvfs_root() {
        Some(p) => p,
        None => return mounts,
    };

    if !root.exists() {
        return mounts;
    }

    let entries = match fs::read_dir(&root) {
        Ok(rd) => rd,
        Err(e) => {
            debug_log(&format!("Failed to read gvfs dir {}: {e}", root.display()));
            return mounts;
        }
    };

    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry
            .file_name()
            .to_string_lossy()
            .into_owned();

        let (fs, removable) = match name.split_once(':').map(|(p, _)| p) {
            Some("mtp") => ("mtp", true),
            Some("onedrive") => ("onedrive", true),
            _ => continue,
        };

        let label = display_name(&path).unwrap_or_else(|| name.clone());

        mounts.push(MountInfo {
            label: short_label(fs, &label, &name),
            path: path.to_string_lossy().into_owned(),
            fs: fs.to_string(),
            removable,
        });
    }

    mounts
}

#[cfg(target_os = "windows")]
pub fn list_gvfs_mounts() -> Vec<MountInfo> {
    Vec::new()
}

#[cfg(not(target_os = "windows"))]
fn short_label(fs: &str, display: &str, raw_name: &str) -> String {
    if fs == "onedrive" {
        // Prefer display name if already short; otherwise derive from raw mount name.
        let trimmed = display.trim();
        if !trimmed.is_empty() && trimmed.len() <= 32 {
            return format!("OneDrive ({})", trimmed);
        }
        let user = raw_name
            .split(',')
            .find_map(|part| part.strip_prefix("user="))
            .unwrap_or("OneDrive");
        return format!("OneDrive ({})", user);
    }
    display.to_string()
}
