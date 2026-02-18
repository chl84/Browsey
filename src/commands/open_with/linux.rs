use super::{spawn_detached, OpenWithApp};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub(super) fn list_linux_apps(target: &Path) -> Vec<OpenWithApp> {
    let mut matches_list = Vec::new();
    let mut fallback = Vec::new();
    for app in linux_app_candidates(target) {
        let open_app = OpenWithApp {
            id: app.id,
            name: app.desktop.name,
            comment: app.desktop.comment,
            exec: app.desktop.exec,
            icon: app.desktop.icon,
            matches: app.matches,
            terminal: app.desktop.terminal,
        };
        if app.matches {
            matches_list.push(open_app);
        } else {
            fallback.push(open_app);
        }
    }
    matches_list.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    fallback.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    matches_list.extend(fallback);
    matches_list
}

pub(super) fn launch_desktop_entry_by_id(target: &Path, app_id: &str) -> Result<(), String> {
    let app = resolve_linux_app_for_target(target, app_id)
        .ok_or_else(|| "Selected application is unavailable".to_string())?;
    launch_desktop_entry(target, &app.desktop)
}

fn launch_desktop_entry(target: &Path, entry: &DesktopEntry) -> Result<(), String> {
    let (program, args) = command_from_exec(entry, target)?;
    if !command_exists(&program) {
        return Err(format!(
            "Selected application is unavailable: {}",
            entry.name
        ));
    }
    let mut cmd = Command::new(&program);
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .args(&args);
    spawn_detached(cmd).map_err(|e| format!("Failed to launch {}: {e}", entry.name))
}

#[derive(Clone)]
struct DesktopEntry {
    name: String,
    comment: Option<String>,
    exec: String,
    mime_types: Vec<String>,
    icon: Option<String>,
    terminal: bool,
    path: PathBuf,
}

#[derive(Clone)]
struct LinuxAppCandidate {
    id: String,
    desktop: DesktopEntry,
    matches: bool,
}

fn linux_desktop_entry_id(path: &Path) -> String {
    let hash = blake3::hash(path.to_string_lossy().as_bytes());
    format!("desktop:{}", hash.to_hex())
}

fn canonical_application_dirs(app_dirs: &[PathBuf]) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut seen: HashSet<PathBuf> = HashSet::new();
    for dir in app_dirs {
        let Ok(canon) = fs::canonicalize(dir) else {
            continue;
        };
        let Ok(meta) = fs::symlink_metadata(&canon) else {
            continue;
        };
        if !meta.is_dir() {
            continue;
        }
        if seen.insert(canon.clone()) {
            out.push(canon);
        }
    }
    out
}

fn linux_app_candidates(target: &Path) -> Vec<LinuxAppCandidate> {
    let dirs = linux_application_dirs();
    linux_app_candidates_in_dirs(target, &dirs)
}

fn linux_app_candidates_in_dirs(target: &Path, app_dirs: &[PathBuf]) -> Vec<LinuxAppCandidate> {
    let target_mime = mime_for_path(target);
    let is_dir = target.is_dir();
    let mut out = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    for dir in canonical_application_dirs(app_dirs) {
        let Ok(entries) = fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path
                .extension()
                .and_then(|e| e.to_str())
                .map(|ext| ext.eq_ignore_ascii_case("desktop"))
                != Some(true)
            {
                continue;
            }
            let Ok(meta) = fs::symlink_metadata(&path) else {
                continue;
            };
            if meta.file_type().is_symlink() || !meta.is_file() {
                continue;
            }
            let Ok(canon) = fs::canonicalize(&path) else {
                continue;
            };
            if !canon.starts_with(&dir) {
                continue;
            }
            let Ok(canon_meta) = fs::symlink_metadata(&canon) else {
                continue;
            };
            if canon_meta.file_type().is_symlink() || !canon_meta.is_file() {
                continue;
            }
            let Some(desktop) = parse_desktop_entry(&canon) else {
                continue;
            };
            let id = linux_desktop_entry_id(&canon);
            if !seen.insert(id.clone()) {
                continue;
            }
            let matches = matches_mime(&desktop.mime_types, &target_mime, is_dir);
            out.push(LinuxAppCandidate {
                id,
                desktop,
                matches,
            });
        }
    }

    out
}

fn resolve_linux_app_for_target(target: &Path, app_id: &str) -> Option<LinuxAppCandidate> {
    let dirs = linux_application_dirs();
    resolve_linux_app_for_target_in_dirs(target, app_id, &dirs)
}

fn resolve_linux_app_for_target_in_dirs(
    target: &Path,
    app_id: &str,
    app_dirs: &[PathBuf],
) -> Option<LinuxAppCandidate> {
    linux_app_candidates_in_dirs(target, app_dirs)
        .into_iter()
        .find(|app| app.id == app_id)
}

fn parse_desktop_entry(path: &Path) -> Option<DesktopEntry> {
    let content = std::fs::read_to_string(path).ok()?;
    let mut in_entry = false;
    let mut name: Option<String> = None;
    let mut exec: Option<String> = None;
    let mut comment: Option<String> = None;
    let mut mime_types: Vec<String> = Vec::new();
    let mut icon: Option<String> = None;
    let mut terminal = false;
    let mut hidden = false;
    let mut no_display = false;
    let mut try_exec: Option<String> = None;

    for line in content.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') {
            in_entry = line.eq_ignore_ascii_case("[desktop entry]");
            continue;
        }
        if !in_entry {
            continue;
        }
        if let Some((key, value)) = line.split_once('=') {
            let val = value.trim();
            match key.trim() {
                "Name" => {
                    if !val.is_empty() {
                        name = Some(val.to_string());
                    }
                }
                "Comment" => {
                    if !val.is_empty() {
                        comment = Some(val.to_string());
                    }
                }
                "Exec" => {
                    if !val.is_empty() {
                        exec = Some(val.to_string());
                    }
                }
                "MimeType" => {
                    mime_types = val
                        .split(';')
                        .filter(|s| !s.trim().is_empty())
                        .map(|s| s.trim().to_string())
                        .collect();
                }
                "Icon" => {
                    if !val.is_empty() {
                        icon = Some(val.to_string());
                    }
                }
                "Terminal" => terminal = val.eq_ignore_ascii_case("true"),
                "Hidden" => hidden = val.eq_ignore_ascii_case("true"),
                "NoDisplay" => no_display = val.eq_ignore_ascii_case("true"),
                "TryExec" => {
                    if !val.is_empty() {
                        try_exec = Some(val.to_string());
                    }
                }
                _ => {}
            }
        }
    }

    if hidden || no_display {
        return None;
    }
    let exec = exec?;
    if let Some(cmd) = &try_exec {
        if !command_exists(cmd) {
            return None;
        }
    }
    let name = name.unwrap_or_else(|| {
        path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or_default()
            .to_string()
    });

    Some(DesktopEntry {
        name,
        comment,
        exec,
        mime_types,
        icon,
        terminal,
        path: path.to_path_buf(),
    })
}

fn command_exists(cmd: &str) -> bool {
    use std::os::unix::fs::PermissionsExt;

    let candidate = PathBuf::from(cmd);
    if candidate.is_absolute() {
        return candidate
            .metadata()
            .map(|m| m.permissions().mode() & 0o111 != 0)
            .unwrap_or(false);
    }
    let Some(paths) = std::env::var_os("PATH") else {
        return false;
    };
    for dir in std::env::split_paths(&paths) {
        let full = dir.join(cmd);
        if full
            .metadata()
            .map(|m| m.permissions().mode() & 0o111 != 0)
            .unwrap_or(false)
        {
            return true;
        }
    }
    false
}

fn matches_mime(list: &[String], target: &str, is_dir: bool) -> bool {
    if list.is_empty() {
        return false;
    }
    let target_lower = target.to_ascii_lowercase();
    for mime in list {
        let m = mime.to_ascii_lowercase();
        if m == target_lower {
            return true;
        }
        if m == "application/octet-stream" {
            return true;
        }
        if is_dir && m == "inode/directory" {
            return true;
        }
        if let Some((ty, _)) = target_lower.split_once('/') {
            if m == format!("{ty}/*") {
                return true;
            }
        }
    }
    false
}

fn mime_for_path(path: &Path) -> String {
    if path.is_dir() {
        "inode/directory".to_string()
    } else {
        mime_guess::from_path(path)
            .first_or_octet_stream()
            .essence_str()
            .to_string()
    }
}

fn linux_application_dirs() -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    if let Some(home) = dirs_next::home_dir() {
        dirs.push(home.join(".local/share/applications"));
    }
    if let Ok(raw) = std::env::var("XDG_DATA_DIRS") {
        for dir in raw.split(':') {
            if dir.is_empty() {
                continue;
            }
            dirs.push(PathBuf::from(dir).join("applications"));
        }
    } else {
        dirs.push(PathBuf::from("/usr/local/share/applications"));
        dirs.push(PathBuf::from("/usr/share/applications"));
    }
    dirs
}

fn command_from_exec(entry: &DesktopEntry, target: &Path) -> Result<(String, Vec<String>), String> {
    let mut tokens =
        shell_words::split(&entry.exec).map_err(|e| format!("Failed to parse Exec: {e}"))?;
    if tokens.is_empty() {
        return Err("Exec is empty".into());
    }
    let target_str = target.to_string_lossy().to_string();
    let desktop_str = entry.path.to_string_lossy().to_string();
    let parent = target
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    let file_name = target
        .file_name()
        .and_then(|n| n.to_str())
        .map(|s| s.to_string())
        .unwrap_or_default();

    let mut used_placeholder = false;
    for token in &mut tokens {
        if token.contains("%f")
            || token.contains("%F")
            || token.contains("%u")
            || token.contains("%U")
        {
            *token = token
                .replace("%f", &target_str)
                .replace("%F", &target_str)
                .replace("%u", &target_str)
                .replace("%U", &target_str);
            used_placeholder = true;
        }
        if token.contains("%d") || token.contains("%D") {
            *token = token.replace("%d", &parent).replace("%D", &parent);
            used_placeholder = true;
        }
        if token.contains("%n") || token.contains("%N") {
            *token = token.replace("%n", &file_name).replace("%N", &file_name);
            used_placeholder = true;
        }
        if token.contains("%k") {
            *token = token.replace("%k", &desktop_str);
        }
        if token.contains("%c") {
            *token = token.replace("%c", &entry.name);
        }
        if token.contains("%i") {
            *token = token.replace("%i", "");
        }
        if token.contains("%m") {
            *token = token.replace("%m", "");
        }
        if token.contains("%%") {
            *token = token.replace("%%", "%");
        }
    }

    let mut args: Vec<String> = tokens.into_iter().filter(|s| !s.is_empty()).collect();
    if args.is_empty() {
        return Err("Exec is empty".into());
    }
    let program = args.remove(0);
    if !used_placeholder {
        args.push(target_str);
    }
    Ok((program, args))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;
    use std::time::{Duration, SystemTime};

    fn uniq_dir(label: &str) -> PathBuf {
        let ts = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap_or(Duration::from_secs(0))
            .as_nanos();
        std::env::temp_dir().join(format!("browsey-open-with-{label}-{ts}"))
    }

    fn write_desktop(path: &Path, name: &str) {
        let contents = format!(
            "[Desktop Entry]\nType=Application\nName={name}\nExec=/bin/echo %f\nMimeType=text/plain;\n"
        );
        fs::write(path, contents).expect("failed to write desktop file");
    }

    #[test]
    fn linux_open_with_resolves_only_listed_ids() {
        let root = uniq_dir("resolve");
        let allowed_dir = root.join("allowed");
        let outsider_dir = root.join("outsider");
        fs::create_dir_all(&allowed_dir).expect("failed to create allowed dir");
        fs::create_dir_all(&outsider_dir).expect("failed to create outsider dir");

        let app_path = allowed_dir.join("viewer.desktop");
        write_desktop(&app_path, "Viewer");
        let outsider_path = outsider_dir.join("evil.desktop");
        write_desktop(&outsider_path, "Evil");

        let target = root.join("sample.txt");
        fs::write(&target, b"data").expect("failed to write target");

        let dirs = vec![allowed_dir.clone()];
        let listed = linux_app_candidates_in_dirs(&target, &dirs);
        assert_eq!(listed.len(), 1);
        let listed_id = listed[0].id.clone();
        assert!(
            listed_id.starts_with("desktop:"),
            "expected hashed desktop id"
        );

        let resolved = resolve_linux_app_for_target_in_dirs(&target, &listed_id, &dirs);
        assert!(resolved.is_some(), "listed app id should resolve");

        let outsider_id = linux_desktop_entry_id(
            &outsider_path
                .canonicalize()
                .expect("failed to canonicalize outsider desktop"),
        );
        let outsider_resolved = resolve_linux_app_for_target_in_dirs(&target, &outsider_id, &dirs);
        assert!(
            outsider_resolved.is_none(),
            "outsider app id must not resolve"
        );

        let raw_path_id = outsider_path.to_string_lossy().to_string();
        let raw_path_resolved = resolve_linux_app_for_target_in_dirs(&target, &raw_path_id, &dirs);
        assert!(
            raw_path_resolved.is_none(),
            "raw path app id must not resolve"
        );

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn linux_open_with_skips_symlink_desktop_files() {
        let root = uniq_dir("symlink");
        let allowed_dir = root.join("allowed");
        let outside_dir = root.join("outside");
        fs::create_dir_all(&allowed_dir).expect("failed to create allowed dir");
        fs::create_dir_all(&outside_dir).expect("failed to create outside dir");

        let real_path = allowed_dir.join("real.desktop");
        write_desktop(&real_path, "Real");

        let linked_source = outside_dir.join("linked.desktop");
        write_desktop(&linked_source, "Linked");
        let linked_path = allowed_dir.join("linked-symlink.desktop");
        symlink(&linked_source, &linked_path).expect("failed to create desktop symlink");

        let target = root.join("sample.txt");
        fs::write(&target, b"data").expect("failed to write target");

        let dirs = vec![allowed_dir];
        let listed = linux_app_candidates_in_dirs(&target, &dirs);
        assert_eq!(listed.len(), 1, "symlink desktop entries should be ignored");
        assert_eq!(listed[0].desktop.name, "Real");

        let _ = fs::remove_dir_all(&root);
    }
}
