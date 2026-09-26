use super::{error::OpenWithError, spawn_detached, OpenWithApp, OpenWithResult};
use gio::prelude::*;
use std::collections::HashSet;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};

pub(super) fn list_linux_apps(target: &Path) -> Vec<OpenWithApp> {
    let mut matches_list = Vec::new();
    let mut fallback = Vec::new();
    let content_type = default_content_type(target).ok();
    for app in linux_app_candidates(target) {
        let default_content_type = content_type
            .as_ref()
            .filter(|_| registered_app(&app).is_some())
            .cloned();
        let open_app = OpenWithApp {
            id: app.id,
            name: app.desktop.name,
            comment: app.desktop.comment,
            exec: app.desktop.exec,
            icon: app.desktop.icon,
            matches: app.matches,
            terminal: app.desktop.terminal,
            default_content_type,
        };
        if app.matches {
            matches_list.push(open_app);
        } else {
            fallback.push(open_app);
        }
    }
    matches_list.sort_by_key(|app| app.name.to_lowercase());
    fallback.sort_by_key(|app| app.name.to_lowercase());
    matches_list.extend(fallback);
    matches_list
}

// Resolve a real desktop ID through GIO, not the opaque ID sent by the UI.
// Comparing canonical filenames prevents a shadowed system entry from silently
// selecting a different user entry with the same desktop ID (also handles Flatpak exports).
fn registered_app(app: &LinuxAppCandidate) -> Option<gio::DesktopAppInfo> {
    let info = gio::DesktopAppInfo::new(&app.desktop_id)?;
    let resolved = fs::canonicalize(info.filename()?).ok()?;
    (resolved == app.desktop.path).then_some(info)
}

fn default_content_type(target: &Path) -> OpenWithResult<String> {
    let info = gio::File::for_path(target)
        .query_info(
            "standard::type,standard::content-type",
            gio::FileQueryInfoFlags::NONE,
            gio::Cancellable::NONE,
        )
        .map_err(|error| {
            OpenWithError::invalid_input(format!("Could not determine file type: {error}"))
        })?;
    if info.file_type() != gio::FileType::Regular {
        return Err(OpenWithError::invalid_input(
            "A default application can only be set for regular files",
        ));
    }
    info.content_type()
        .filter(|value| !gio::content_type_is_unknown(value))
        .map(|value| value.to_string())
        .ok_or_else(|| {
            OpenWithError::invalid_input("The file type is unknown; no default was changed")
        })
}

pub(super) fn set_default_app(
    target: &Path,
    app_id: &str,
    expected_type: &str,
) -> OpenWithResult<()> {
    let content_type = default_content_type(target)?;
    if content_type != expected_type {
        return Err(OpenWithError::invalid_input(
            "The file type changed. Reopen Open with and try again; no default was changed",
        ));
    }
    let app = resolve_linux_app_for_target(target, app_id)
        .and_then(|app| registered_app(&app))
        .ok_or_else(|| OpenWithError::app_not_found("Selected application is unavailable"))?;
    app.set_as_default_for_type(&content_type).map_err(|error| {
        OpenWithError::new(
            super::OpenWithErrorCode::DefaultAppFailed,
            format!("Could not save the default application: {error}"),
        )
    })
}

pub(super) fn launch_desktop_entry_by_id(target: &Path, app_id: &str) -> OpenWithResult<()> {
    let app = resolve_linux_app_for_target(target, app_id)
        .ok_or_else(|| OpenWithError::app_not_found("Selected application is unavailable"))?;
    launch_desktop_entry(target, &app.desktop)
}

fn launch_desktop_entry(target: &Path, entry: &DesktopEntry) -> OpenWithResult<()> {
    let (program, args) = command_from_exec(entry, target)?;
    if !command_exists(&program) {
        return Err(OpenWithError::app_not_found(format!(
            "Selected application is unavailable: {}",
            entry.name
        )));
    }
    let mut cmd = Command::new(&program);
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .args(&args);
    spawn_detached(cmd).map_err(|error| {
        OpenWithError::launch_failed(format!("Failed to launch {}: {error}", entry.name))
    })
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
    desktop_id: String,
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
        let flatpak_app_root = flatpak_app_root_for_export_dir(&dir);
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
            let is_symlink = meta.file_type().is_symlink();
            if !is_symlink && !meta.is_file() {
                continue;
            }
            let Ok(canon) = fs::canonicalize(&path) else {
                continue;
            };
            let Ok(canon_meta) = fs::symlink_metadata(&canon) else {
                continue;
            };
            if canon_meta.file_type().is_symlink() || !canon_meta.is_file() {
                continue;
            }
            if is_symlink {
                let Some(app_root) = flatpak_app_root.as_ref() else {
                    continue;
                };
                if !is_allowed_flatpak_desktop_target(&canon, app_root) {
                    continue;
                }
            } else if !canon.starts_with(&dir) {
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
                desktop_id: entry.file_name().to_string_lossy().into_owned(),
                desktop,
                matches,
            });
        }
    }

    out
}

fn flatpak_app_root_for_export_dir(dir: &Path) -> Option<PathBuf> {
    if !dir.ends_with(Path::new("flatpak/exports/share/applications")) {
        return None;
    }
    let flatpak_root = dir.parent()?.parent()?.parent()?;
    Some(flatpak_root.join("app"))
}

fn path_contains_component_sequence(path: &Path, sequence: &[&str]) -> bool {
    if sequence.is_empty() {
        return true;
    }
    let components: Vec<&str> = path
        .components()
        .filter_map(|component| match component {
            Component::Normal(segment) => segment.to_str(),
            _ => None,
        })
        .collect();
    components
        .windows(sequence.len())
        .any(|window| window == sequence)
}

fn is_allowed_flatpak_desktop_target(path: &Path, app_root: &Path) -> bool {
    path.starts_with(app_root)
        && path
            .extension()
            .and_then(|e| e.to_str())
            .map(|ext| ext.eq_ignore_ascii_case("desktop"))
            == Some(true)
        && path_contains_component_sequence(path, &["export", "share", "applications"])
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
                "TryExec" if !val.is_empty() => try_exec = Some(val.to_string()),
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
    if let Some(data_home) = dirs_next::data_dir() {
        dirs.push(data_home.join("applications"));
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

fn command_from_exec(entry: &DesktopEntry, target: &Path) -> OpenWithResult<(String, Vec<String>)> {
    let mut tokens = shell_words::split(&entry.exec)
        .map_err(|e| OpenWithError::invalid_input(format!("Failed to parse Exec: {e}")))?;
    if tokens.is_empty() {
        return Err(OpenWithError::invalid_input("Exec is empty"));
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
        return Err(OpenWithError::invalid_input("Exec is empty"));
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
    use crate::errors::domain::DomainError;
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

    #[test]
    fn linux_open_with_allows_flatpak_exported_symlink_desktop_files() {
        let root = uniq_dir("flatpak-symlink");
        let export_dir = root.join("flatpak/exports/share/applications");
        let app_dir = root
            .join("flatpak/app/com.example.App/x86_64/stable/testhash/export/share/applications");
        fs::create_dir_all(&export_dir).expect("failed to create export dir");
        fs::create_dir_all(&app_dir).expect("failed to create app dir");

        let real_path = app_dir.join("com.example.App.desktop");
        write_desktop(&real_path, "Flatpak App");
        let linked_path = export_dir.join("com.example.App.desktop");
        symlink(&real_path, &linked_path).expect("failed to create desktop symlink");

        let target = root.join("sample.txt");
        fs::write(&target, b"data").expect("failed to write target");

        let dirs = vec![export_dir];
        let listed = linux_app_candidates_in_dirs(&target, &dirs);
        assert_eq!(listed.len(), 1, "flatpak-export symlink should be allowed");
        assert_eq!(listed[0].desktop.name, "Flatpak App");

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn linux_open_with_reports_missing_app_with_typed_code() {
        let root = uniq_dir("missing-app");
        fs::create_dir_all(&root).expect("failed to create root");
        let target = root.join("sample.txt");
        fs::write(&target, b"data").expect("failed to write target");

        let error = launch_desktop_entry_by_id(&target, "desktop:missing")
            .expect_err("missing desktop id should fail");
        assert_eq!(error.code_str(), "app_not_found");
        assert_eq!(error.message(), "Selected application is unavailable");

        let _ = fs::remove_dir_all(&root);
    }

    #[test]
    fn command_from_exec_reports_empty_exec_as_invalid_input() {
        let target = PathBuf::from("/tmp/example.txt");
        let entry = DesktopEntry {
            name: "Viewer".to_string(),
            comment: None,
            exec: "   ".to_string(),
            mime_types: vec!["text/plain".to_string()],
            icon: None,
            terminal: false,
            path: PathBuf::from("/tmp/viewer.desktop"),
        };

        let error = command_from_exec(&entry, &target).expect_err("empty exec should fail");
        assert_eq!(error.code_str(), "invalid_input");
        assert_eq!(error.message(), "Exec is empty");
    }
    // GIO caches XDG paths globally. Exercise real persistence in a fresh child,
    // never mutate the test runner's environment or the user's mimeapps.list.
    #[test]
    fn default_app_persists_in_isolated_xdg_config() {
        use std::os::unix::fs::PermissionsExt;
        let root = uniq_dir("default-app");
        fs::create_dir(&root).unwrap();
        let bin_dir = root.join("bin");
        fs::create_dir(&bin_dir).unwrap();
        let xdg_open = bin_dir.join("xdg-open");
        fs::write(&xdg_open, "#!/bin/sh\nexit 1\n").unwrap();
        fs::set_permissions(&xdg_open, fs::Permissions::from_mode(0o700)).unwrap();
        for blocked in [false, true] {
            let config = root.join(if blocked { "blocked-config" } else { "config" });
            if blocked {
                fs::write(&config, b"not a directory").unwrap();
            } else {
                fs::create_dir(&config).unwrap();
            }
            let output = Command::new(std::env::current_exe().unwrap())
                .args([
                    "--exact",
                    "commands::open_with::linux::tests::default_app_isolated_child",
                    "--nocapture",
                ])
                .env("BROWSEY_DEFAULT_APP_TEST_ROOT", &root)
                .env(
                    "BROWSEY_DEFAULT_APP_TEST_BLOCKED",
                    if blocked { "1" } else { "0" },
                )
                .env("XDG_CONFIG_HOME", &config)
                .env("XDG_CONFIG_DIRS", root.join("config-dirs"))
                .env("XDG_DATA_HOME", root.join("data"))
                .env(
                    "XDG_DATA_DIRS",
                    std::env::join_paths([
                        root.join("flatpak/exports/share"),
                        PathBuf::from("/usr/local/share"),
                        PathBuf::from("/usr/share"),
                    ])
                    .unwrap(),
                )
                .env("XDG_CACHE_HOME", root.join("cache"))
                .env(
                    "PATH",
                    std::env::join_paths([
                        bin_dir.clone(),
                        PathBuf::from("/usr/bin"),
                        PathBuf::from("/bin"),
                    ])
                    .unwrap(),
                )
                .env("XDG_CURRENT_DESKTOP", "BrowseyTest")
                .env("GTK_USE_PORTAL", "0")
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{}\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed"));
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn default_app_isolated_child() {
        let Some(root) = std::env::var_os("BROWSEY_DEFAULT_APP_TEST_ROOT") else {
            return;
        };
        let root = PathBuf::from(root);
        let app_dir = root.join("data/applications");
        fs::create_dir_all(&app_dir).unwrap();
        let desktop = app_dir.join("browsey-default-test.desktop");
        // /bin/true is never launched: setting a default is not opening a file.
        fs::write(&desktop, "[Desktop Entry]\nType=Application\nName=Browsey default test\nExec=/bin/true %f\nMimeType=text/plain;application/pdf;\n").unwrap();
        let export_dir = root.join("flatpak/exports/share/applications");
        let flatpak_dir =
            root.join("flatpak/app/test/x86_64/stable/hash/export/share/applications");
        fs::create_dir_all(&export_dir).unwrap();
        fs::create_dir_all(&flatpak_dir).unwrap();
        let flatpak_file = flatpak_dir.join("browsey-flatpak-test.desktop");
        write_desktop(&flatpak_file, "Browsey Flatpak test");
        let export = export_dir.join("browsey-flatpak-test.desktop");
        if !export.exists() {
            symlink(&flatpak_file, &export).unwrap();
        }
        let target = root.join("notes.txt");
        fs::write(&target, "ordinary text\n").unwrap();
        let apps = list_linux_apps(&target);
        let app = apps
            .iter()
            .find(|app| app.name == "Browsey default test")
            .unwrap();
        assert_eq!(app.default_content_type.as_deref(), Some("text/plain"));
        assert_eq!(
            apps.iter()
                .find(|app| app.name == "Browsey Flatpak test")
                .unwrap()
                .default_content_type
                .as_deref(),
            Some("text/plain")
        );
        let shadow_dir = root.join("shadowed");
        fs::create_dir_all(&shadow_dir).unwrap();
        write_desktop(
            &shadow_dir.join("browsey-default-test.desktop"),
            "Shadowed app",
        );
        let shadowed = linux_app_candidates_in_dirs(&target, &[shadow_dir]);
        assert!(
            registered_app(&shadowed[0]).is_none(),
            "must not silently save a different app sharing the same desktop ID"
        );
        for invalid_id in ["__default__", "desktop:missing", desktop.to_str().unwrap()] {
            assert_eq!(
                set_default_app(&target, invalid_id, "text/plain")
                    .unwrap_err()
                    .code_str(),
                "app_not_found"
            );
        }
        assert_eq!(
            set_default_app(&target, &app.id, "application/pdf")
                .unwrap_err()
                .code_str(),
            "invalid_input"
        );
        assert_eq!(
            set_default_app(&root, &app.id, "inode/directory")
                .unwrap_err()
                .code_str(),
            "invalid_input"
        );
        let unknown = root.join("unknown.browsey-unknown-extension");
        fs::write(&unknown, [0u8, 255, 0, 255]).unwrap();
        assert!(default_content_type(&unknown).is_err());

        if std::env::var("BROWSEY_DEFAULT_APP_TEST_BLOCKED").as_deref() == Ok("1") {
            assert_eq!(
                set_default_app(&target, &app.id, "text/plain")
                    .unwrap_err()
                    .code_str(),
                "default_app_failed"
            );
            return;
        }
        let config = PathBuf::from(std::env::var_os("XDG_CONFIG_HOME").unwrap());
        let mimeapps = config.join("mimeapps.list");
        assert!(
            !mimeapps.exists(),
            "invalid requests must not write defaults"
        );
        // A separate existing association must survive our update.
        fs::write(
            &mimeapps,
            "[Default Applications]\napplication/pdf=browsey-default-test.desktop;\n",
        )
        .unwrap();
        super::super::set_default_app_impl(target.to_str().unwrap(), &app.id, "text/plain")
            .unwrap();
        let saved = fs::read_to_string(&mimeapps).unwrap();
        assert!(saved.contains("text/plain=browsey-default-test.desktop"));
        assert!(saved.contains("application/pdf=browsey-default-test.desktop"));
        // A fresh GIO consumer must see the persisted desktop ID, not our hashed UI ID.
        let query = Command::new("gio")
            .args(["mime", "text/plain"])
            .env("LC_ALL", "C")
            .output()
            .unwrap();
        assert!(query.status.success());
        assert!(
            String::from_utf8_lossy(&query.stdout)
                .lines()
                .next()
                .is_some_and(|line| line.ends_with(": browsey-default-test.desktop")),
            "{}",
            String::from_utf8_lossy(&query.stdout)
        );
        let extensionless = root.join("extensionless");
        fs::write(&extensionless, "plain text without an extension\n").unwrap();
        assert_eq!(default_content_type(&extensionless).unwrap(), "text/plain");
        assert!(list_linux_apps(&root)
            .iter()
            .all(|app| app.default_content_type.is_none()));
        exercise_default_launch(&root, &app_dir);
    }

    fn exercise_default_launch(root: &Path, app_dir: &Path) {
        use std::os::unix::fs::PermissionsExt;
        use std::time::{Duration, Instant};

        let recorder = root.join("record-open");
        let record = root.join("opened-path");
        fs::write(
            &recorder,
            "#!/bin/sh\nprintf '%s' \"$1\" > \"$BROWSEY_DEFAULT_APP_TEST_ROOT/opened-path\"\n",
        )
        .unwrap();
        fs::set_permissions(&recorder, fs::Permissions::from_mode(0o700)).unwrap();
        let desktop = app_dir.join("browsey-open-test.desktop");
        fs::write(&desktop, format!("[Desktop Entry]\nType=Application\nName=Browsey open test\nExec={} %F\nMimeType=text/x-python;\n", recorder.display())).unwrap();
        let target = root.join("æ python #100% file.py");
        fs::write(&target, "#!/usr/bin/env python\nraise RuntimeError('This file must be opened, never executed')\n").unwrap();
        fs::set_permissions(&target, fs::Permissions::from_mode(0o700)).unwrap();
        assert_eq!(default_content_type(&target).unwrap(), "text/x-python");
        let app = list_linux_apps(&target)
            .into_iter()
            .find(|app| app.name == "Browsey open test")
            .unwrap();
        set_default_app(&target, &app.id, "text/x-python").unwrap();

        let assert_opened = || {
            let start = Instant::now();
            loop {
                if fs::read_to_string(&record).ok().as_deref() == target.to_str() {
                    break;
                }
                assert!(
                    start.elapsed() < Duration::from_secs(3),
                    "default handler did not receive the exact file path"
                );
                std::thread::sleep(Duration::from_millis(10));
            }
            fs::remove_file(&record).unwrap();
        };
        // The actual double-click IPC command, including path validation.
        tauri::async_runtime::block_on(crate::commands::fs::open_entry(
            target.to_string_lossy().into_owned(),
        ))
        .unwrap();
        assert_opened();
        // Open normally must use the same default-handler route.
        super::super::open_with_impl(
            target.to_string_lossy().into_owned(),
            super::super::OpenWithChoice {
                app_id: Some("__default__".into()),
            },
        )
        .unwrap();
        assert_opened();
        // Cached cloud files also enter this shared opening helper.
        crate::commands::fs::open_path_without_recent(&target).unwrap();
        assert_opened();

        // A discoverable handler with an invalid working directory: GIO reports
        // this launch error before its launcher helper is started. Applications
        // that start successfully and then exit/crash cannot be monitored here.
        fs::write(app_dir.join("browsey-broken-open-test.desktop"), format!(
            "[Desktop Entry]\nType=Application\nName=Browsey broken launch test\nExec={} %F\nPath={}\nMimeType=text/x-python;\n",
            recorder.display(), root.join("nonexistent-working-directory").display(),
        )).unwrap();
        let broken = list_linux_apps(&target)
            .into_iter()
            .find(|app| app.name == "Browsey broken launch test")
            .unwrap();
        set_default_app(&target, &broken.id, "text/x-python").unwrap();
        let error = tauri::async_runtime::block_on(crate::commands::fs::open_entry(
            target.to_string_lossy().into_owned(),
        ))
        .unwrap_err();
        assert_eq!(error.code, "open_failed");
        assert!(error.message.contains("Failed to open:"));
        assert!(!record.exists());
    }
}
