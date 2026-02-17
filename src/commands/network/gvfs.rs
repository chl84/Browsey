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
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::{Duration, Instant},
};

#[cfg(not(target_os = "windows"))]
fn instant_ago(d: Duration) -> Instant {
    Instant::now().checked_sub(d).unwrap_or_else(Instant::now)
}

#[cfg(not(target_os = "windows"))]
fn gvfs_root() -> Option<PathBuf> {
    dirs_next::runtime_dir().map(|p| p.join("gvfs"))
}

#[cfg(not(target_os = "windows"))]
pub fn ensure_gvfsd_fuse_running() {
    #[derive(Clone, Copy)]
    struct FuseState {
        last_check: Instant,
        ok: bool,
    }
    static STATE: OnceCell<Mutex<FuseState>> = OnceCell::new();
    static LOG_STATE: OnceCell<Mutex<Instant>> = OnceCell::new();
    let guard = STATE.get_or_init(|| {
        Mutex::new(FuseState {
            last_check: instant_ago(Duration::from_secs(60)),
            ok: false,
        })
    });

    let mut lock = guard.lock().unwrap_or_else(|e| e.into_inner());

    // Throttle to once every 30s if last attempt was OK; retry sooner if last was failure.
    let retry_after = if lock.ok {
        Duration::from_secs(30)
    } else {
        Duration::from_secs(10)
    };

    if lock.last_check.elapsed() < retry_after {
        return;
    }

    // If we recently confirmed it's running, skip further checks
    if lock.ok {
        lock.last_check = Instant::now();
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
        lock.last_check = Instant::now();
        lock.ok = true;
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

    lock.last_check = Instant::now();
    lock.ok = ok;

    if !ok {
        let log_guard = LOG_STATE
            .get_or_init(|| Mutex::new(instant_ago(Duration::from_secs(600))))
            .lock()
            .map_err(|e| e.into_inner())
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
    let needle = format!("{}:", prefix.to_ascii_lowercase());
    if let Some(root) = gvfs_root() {
        if let Ok(rd) = fs::read_dir(root) {
            return rd.flatten().any(|e| {
                e.file_name()
                    .to_string_lossy()
                    .to_ascii_lowercase()
                    .starts_with(&needle)
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
    let conf = dirs_next::config_dir()?
        .join("goa-1.0")
        .join("accounts.conf");
    let contents = fs::read_to_string(conf).ok()?;
    parse_onedrive_uri_goa(&contents)
}

#[cfg(not(target_os = "windows"))]
fn list_onedrive_mountables() -> Option<Vec<(String, String)>> {
    const CACHE_TTL: Duration = Duration::from_secs(30);
    static CACHE: OnceCell<Mutex<(Instant, Vec<(String, String)>)>> = OnceCell::new();
    let cache = CACHE.get_or_init(|| Mutex::new((instant_ago(CACHE_TTL), Vec::new())));

    // Serve cached data if it is still fresh (30s) to avoid running `gio mount -li` too often.
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
    let mut entries = parse_onedrive_mountables(&stdout);

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

    static LOG_STATE: OnceCell<Mutex<Instant>> = OnceCell::new();

    let mut cmd = Command::new("gio");
    cmd.arg("mount")
        .arg(uri)
        .stdout(Stdio::null())
        .stderr(Stdio::null());

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
            retry
                .arg("mount")
                .arg(uri)
                .stdout(Stdio::null())
                .stderr(Stdio::null());
            if retry.status().map(|s| s.success()).unwrap_or(false) {
                let retry_deadline = Instant::now() + Duration::from_secs(2);
                while Instant::now() < retry_deadline {
                    if has_mount_prefix(&prefix) {
                        return true;
                    }
                    std::thread::sleep(Duration::from_millis(120));
                }
            }
            if let Some(mut ts) = LOG_STATE
                .get_or_init(|| Mutex::new(instant_ago(Duration::from_secs(600))))
                .lock()
                .map_err(|e| e.into_inner())
                .ok()
            {
                let now = Instant::now();
                if now.duration_since(*ts) >= Duration::from_secs(300) {
                    debug_log(&format!(
                        "mount_uri: gio mount {uri} reported success but mount path not visible"
                    ));
                    *ts = now;
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
        // Handle localized output by looking for the attribute key token before ':'
        if let Some((key, rest)) = trimmed.split_once(':') {
            if key.to_ascii_lowercase().contains("display-name") {
                return Some(rest.trim().to_string());
            }
        }
        // Fallback: search anywhere in the line for the key and take the part after the last ':'
        if trimmed.to_ascii_lowercase().contains("display-name") {
            if let Some(pos) = trimmed.rfind(':') {
                return Some(trimmed[(pos + 1)..].trim().to_string());
            }
        }
    }
    None
}

#[cfg(not(target_os = "windows"))]
pub fn list_gvfs_mounts() -> Vec<MountInfo> {
    let mut mounts = Vec::new();
    // Track mounted paths separately; only used to avoid adding duplicate entries when already mounted.
    let mut mounted_paths: std::collections::HashSet<String> = std::collections::HashSet::new();
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
            let name = entry.file_name().to_string_lossy().into_owned();

            let (fs, removable) = match name.split_once(':').map(|(p, _)| p) {
                Some("mtp") => ("mtp", true),
                Some("onedrive") => ("onedrive", true),
                _ => continue,
            };

            let label = display_name(&path).unwrap_or_else(|| name.clone());
            let path_str = path.to_string_lossy().into_owned();
            mounted_paths.insert(path_str.clone());

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
                if mounted_paths.contains(&uri) {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_onedrive_mountables_basic() {
        let sample = r#"
Volume(Personal OneDrive) -> one drive
    uuid=abc
    activation_root=onedrive://abc-123/
    can_mount=1
Volume(Work OneDrive)
    activation_root=onedrive://work-999/
    can_mount=1
        "#;
        let parsed = parse_onedrive_mountables(sample);
        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].1, "onedrive://abc-123/");
        assert!(parsed[0].0.starts_with("OneDrive (Personal"));
        assert_eq!(parsed[1].1, "onedrive://work-999/");
    }

    #[test]
    fn short_label_user_and_truncate() {
        // user= path
        let lbl = short_label(
            "onedrive",
            "This display name is deliberately very very long to force user parse",
            "onedrive,user=alice",
        );
        assert_eq!(lbl, "OneDrive (alice)");

        // truncate long display without user
        let long = "This is a very very long display name that exceeds thirty chars";
        let lbl2 = short_label("onedrive", long, "onedrive://abc/");
        assert!(lbl2.starts_with("OneDrive ("));
        assert!(lbl2.contains("..."));
        assert!(lbl2.len() <= "OneDrive ()".len() + 30);

        // short display preserved
        let lbl3 = short_label("onedrive", "Alice", "onedrive://abc/");
        assert_eq!(lbl3, "OneDrive (Alice)");
    }

    #[test]
    fn goa_picks_first_valid_section() {
        let contents = r#"
[account_1]
Provider=msgraph
Id=firstid
Identity=first@example.com

[account_2]
Provider=msgraph
# missing identity
        "#;
        let uri = parse_onedrive_uri_goa(contents).expect("should parse first");
        assert_eq!(uri, "onedrive://first@example.com/");
    }

    #[test]
    fn goa_prefers_presentation_identity() {
        let contents = r#"
[account_1]
Provider=msgraph
Id=firstid
PresentationIdentity=pretty name
Identity=first@example.com
        "#;
        let uri = parse_onedrive_uri_goa(contents).expect("should parse");
        assert_eq!(uri, "onedrive://pretty name/");
    }
}
#[cfg(not(target_os = "windows"))]
fn short_label(fs: &str, display: &str, raw_name: &str) -> String {
    if fs == "onedrive" {
        // Prefer display name if already short; otherwise derive from raw mount name.
        let trimmed = display.trim();
        if !trimmed.is_empty() && trimmed.len() <= 32 {
            return format!("OneDrive ({})", trimmed);
        }

        if let Some(user) = raw_name
            .split(',')
            .find_map(|part| part.strip_prefix("user="))
        {
            return format!("OneDrive ({})", user);
        }

        // Fallback: truncate display if it's too long and we don't have a user= hint.
        let truncated = truncate_label(trimmed, 30);
        return format!("OneDrive ({})", truncated);
    }
    display.to_string()
}

#[cfg(not(target_os = "windows"))]
fn truncate_label(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    if max <= 3 {
        return "...".to_string();
    }
    let mut out = String::with_capacity(max);
    out.push_str(&s[..max - 3]);
    out.push_str("...");
    out
}

#[cfg(not(target_os = "windows"))]
fn parse_onedrive_mountables(stdout: &str) -> Vec<(String, String)> {
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
                entries.push((short_label("onedrive", &label, &uri), uri));
            }
        }
    }
    entries
}

#[cfg(not(target_os = "windows"))]
fn parse_onedrive_uri_goa(contents: &str) -> Option<String> {
    let mut id: Option<String> = None;
    let mut identity: Option<String> = None;
    let mut presentation: Option<String> = None;
    let mut provider = false;
    for line in contents.lines() {
        let line = line.trim();
        if line.starts_with('[') && line.ends_with(']') {
            if provider {
                if let Some(chosen) = presentation.clone().or(identity.clone()).or(id.clone()) {
                    return Some(format!("onedrive://{chosen}/"));
                }
            }
            provider = false;
            id = None;
            identity = None;
            presentation = None;
            continue;
        }
        if line.eq_ignore_ascii_case("Provider=msgraph")
            || line.eq_ignore_ascii_case("Provider=ms_graph")
        {
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
    if provider {
        if let Some(chosen) = presentation.or(identity).or(id) {
            return Some(format!("onedrive://{chosen}/"));
        }
    }
    None
}
