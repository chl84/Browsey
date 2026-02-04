//! Helpers for GVFS-backed mounts (e.g., MTP over GVFS).

#[cfg(not(target_os = "windows"))]
use crate::commands::fs::MountInfo;
#[cfg(not(target_os = "windows"))]
use crate::fs_utils::debug_log;
#[cfg(not(target_os = "windows"))]
use once_cell::sync::OnceCell;
#[cfg(not(target_os = "windows"))]
use std::sync::Mutex;
#[cfg(not(target_os = "windows"))]
use std::{
    borrow::Cow,
    collections::HashMap,
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

#[cfg(not(target_os = "windows"))]
fn gvfs_root() -> Option<PathBuf> {
    dirs_next::runtime_dir().map(|p| p.join("gvfs"))
}

#[cfg(not(target_os = "windows"))]
pub fn ensure_gvfsd_fuse_running() {
    static STATE: OnceCell<Mutex<(Instant, bool)>> = OnceCell::new();
    static LOG_STATE: OnceCell<Mutex<Instant>> = OnceCell::new();
    let guard = STATE.get_or_init(|| Mutex::new((Instant::now() - Duration::from_secs(60), false)));

    let mut lock = match guard.lock() {
        Ok(g) => g,
        Err(_) => return,
    };

    // Throttle to once every 30s
    if lock.0.elapsed() < Duration::from_secs(30) {
        return;
    }

    let Some(root) = gvfs_root() else { return };

    if Command::new("pgrep")
        .arg("gvfsd-fuse")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
    {
        lock.0 = Instant::now();
        lock.1 = true;
        return;
    }

    let _ = fs::create_dir_all(&root);

    let _ = Command::new("gvfsd-fuse")
        .arg(&root)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .stdin(Stdio::null())
        .spawn();

    // Small wait to give gvfsd-fuse time to come up
    std::thread::sleep(Duration::from_millis(150));

    let ok = Command::new("pgrep")
        .arg("gvfsd-fuse")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    lock.0 = Instant::now();
    lock.1 = ok;

    if !ok {
        let log_guard = LOG_STATE
            .get_or_init(|| Mutex::new(Instant::now() - Duration::from_secs(600)))
            .lock()
            .ok();
        if let Some(mut lg) = log_guard {
            if lg.elapsed() >= Duration::from_secs(300) {
                debug_log("gvfsd-fuse did not start successfully");
                *lg = Instant::now();
            }
        }
    }
}

#[cfg(not(target_os = "windows"))]
fn has_mount_prefix(prefix: &str) -> bool {
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
fn find_onedrive_uri_cli(preloaded: Option<&str>) -> Option<String> {
    let text: Cow<'_, str> = if let Some(t) = preloaded {
        Cow::Borrowed(t)
    } else {
        let output = Command::new("gio")
            .arg("mount")
            .arg("-li")
            .stderr(Stdio::null())
            .output()
            .ok()?;
        Cow::Owned(String::from_utf8_lossy(&output.stdout).into_owned())
    };
    text.lines()
        .flat_map(|l| l.split_whitespace())
        .find(|p| p.to_ascii_lowercase().starts_with("onedrive://"))
        .map(|s| s.to_string())
}

#[cfg(not(target_os = "windows"))]
fn find_onedrive_uri_goa() -> Option<String> {
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
fn list_onedrive_mountables() -> Option<Vec<(String, String)>> {
    const CACHE_TTL: Duration = Duration::from_secs(30);
    static CACHE: OnceCell<Mutex<(Instant, Vec<(String, String)>)>> = OnceCell::new();
    let cache = CACHE.get_or_init(|| Mutex::new((Instant::now() - CACHE_TTL, Vec::new())));

    // Serve cached data if it is still fresh (10s) to avoid running `gio mount -li` too often.
    if let Ok(lock) = cache.lock() {
        if lock.0.elapsed() < CACHE_TTL {
            return Some(lock.1.clone());
        }
    }

    let output = Command::new("gio")
        .arg("mount")
        .arg("-li")
        .stderr(Stdio::null()) // avoid gvfsd-fuse noise
        .output()
        .ok()?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let mut entries = Vec::new();
    let mut current_label: Option<String> = None;
    let mut current_uri: Option<String> = None;
    for line in stdout.lines() {
        let l = line.trim();
        if l.starts_with("Volume(") {
            current_label = l
                .trim_start_matches("Volume(")
                .trim_end_matches(')')
                .to_string()
                .split("->")
                .next()
                .map(|s| s.trim().to_string());
            current_uri = None;
        }
        if l.starts_with("uuid=") || l.starts_with("activation_root=") {
            if let Some(rest) = l.split('=').nth(1) {
                let val = rest.trim();
                if val.to_ascii_lowercase().starts_with("onedrive://") {
                    current_uri = Some(val.to_string());
                }
            }
        }
        if l.starts_with("can_mount=") {
            if let Some(uri) = current_uri.take() {
                let label = current_label.clone().unwrap_or_else(|| "OneDrive".into());
                entries.push((short_label("onedrive", &label, &label), uri));
            }
        }
    }
    if entries.is_empty() {
        if let Some(uri) = find_onedrive_uri_cli(Some(&stdout)).or_else(find_onedrive_uri_goa) {
            entries.push((short_label("onedrive", "OneDrive", "onedrive"), uri));
        }
    }

    if let Ok(mut lock) = cache.lock() {
        *lock = (Instant::now(), entries.clone());
    }
    Some(entries)
}

#[cfg(target_os = "windows")]
fn list_onedrive_mountables() -> Option<Vec<(String, String)>> {
    None
}

#[cfg(not(target_os = "windows"))]
pub fn mount_uri(uri: &str) -> bool {
    ensure_gvfsd_fuse_running();
    let prefix = uri
        .split(':')
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();

    static LOG_STATE: OnceCell<Mutex<HashMap<&'static str, Instant>>> = OnceCell::new();

    let mut cmd = Command::new("gio");
    cmd.arg("mount").arg(uri).stdout(Stdio::null()).stderr(Stdio::null());

    match cmd.status() {
        Ok(status) if status.success() => {
            // Wait briefly for the mount to appear under /run/user/.../gvfs
            let deadline = Instant::now() + Duration::from_secs(5);
            while Instant::now() < deadline {
                if has_mount_prefix(&prefix) {
                    return true;
                }
                std::thread::sleep(Duration::from_millis(120));
            }
            // One light retry before giving up
            let mut retry = Command::new("gio");
            retry.arg("mount").arg(uri).stdout(Stdio::null()).stderr(Stdio::null());
            if retry.status().map(|s| s.success()).unwrap_or(false) {
                let retry_deadline = Instant::now() + Duration::from_secs(2);
                while Instant::now() < retry_deadline {
                    if has_mount_prefix(&prefix) {
                        return true;
                    }
                    std::thread::sleep(Duration::from_millis(120));
                }
            }
            if let Some(mut map) = LOG_STATE
                .get_or_init(|| Mutex::new(HashMap::new()))
                .lock()
                .ok()
            {
                let now = Instant::now();
                let entry = map.entry("mount_missing_path").or_insert(now - Duration::from_secs(600));
                if now.duration_since(*entry) >= Duration::from_secs(300) {
                    debug_log(&format!(
                        "mount_uri: gio mount {uri} reported success but mount path not visible"
                    ));
                    *entry = now;
                }
            }
            false
        }
        Ok(status) => {
            debug_log(&format!("mount_uri: gio mount {uri} failed: {status:?}"));
            false
        }
        Err(e) => {
            debug_log(&format!("mount_uri: spawn failed for gio mount {uri}: {e}"));
            false
        }
    }
}

#[cfg(target_os = "windows")]
pub fn mount_uri(_uri: &str) -> bool {
    true
}

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
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut has_onedrive_mount = false;
    ensure_gvfsd_fuse_running();
    let root = match gvfs_root() {
        Some(p) => p,
        None => return mounts,
    };

    if !root.exists() {
        // continue; we might still discover mountable volumes via gio mount -li
    }

    if root.exists() {
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
            let path_str = path.to_string_lossy().into_owned();
            seen.insert(path_str.clone());

            mounts.push(MountInfo {
                label: short_label(fs, &label, &name),
                path: path_str,
                fs: fs.to_string(),
                removable,
            });

            if fs == "onedrive" {
                has_onedrive_mount = true;
            }
        }
    }

    // Add mountable OneDrive volumes even when not mounted
    if !has_onedrive_mount {
        if let Some(extra) = list_onedrive_mountables() {
            for (label, uri) in extra {
                if seen.contains(&uri) {
                    continue;
                }
                mounts.push(MountInfo {
                    label,
                    path: uri.clone(),
                    fs: "onedrive".to_string(),
                    // Unmounted activation roots should not expose "eject" in the UI.
                    removable: false,
                });
            }
        }
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
