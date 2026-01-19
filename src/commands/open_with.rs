use crate::{db, fs_utils::sanitize_path_follow};
use serde::{Deserialize, Serialize};
use std::path::Path;
#[cfg(target_os = "linux")]
use std::path::PathBuf;
#[cfg(target_os = "windows")]
use winreg::{enums::*, RegKey};
use std::process::{Command, Stdio};
use std::thread;
use tracing::{info, warn};

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct OpenWithApp {
    pub id: String,
    pub name: String,
    pub comment: Option<String>,
    pub exec: String,
    pub icon: Option<String>,
    pub matches: bool,
    pub terminal: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenWithChoice {
    pub app_id: Option<String>,
    pub custom_command: Option<String>,
    pub custom_args: Option<String>,
}

#[tauri::command]
pub fn list_open_with_apps(path: String) -> Result<Vec<OpenWithApp>, String> {
    let target = sanitize_path_follow(&path, false)?;
    #[cfg(target_os = "linux")]
    {
        return Ok(list_linux_apps(&target));
    }
    #[cfg(target_os = "windows")]
    {
        return Ok(list_windows_apps(&target));
    }
    #[cfg(not(any(target_os = "linux", target_os = "windows")))]
    {
        let _ = target;
        Ok(Vec::new())
    }
}

#[tauri::command]
pub fn open_with(path: String, choice: OpenWithChoice) -> Result<(), String> {
    let target = sanitize_path_follow(&path, false)?;
    let OpenWithChoice {
        app_id,
        custom_command,
        custom_args,
    } = choice;

    let conn = db::open()?;
    if let Err(e) = db::touch_recent(&conn, &target.to_string_lossy()) {
        warn!("Failed to record recent for {:?}: {}", target, e);
    }

    let custom_cmd = custom_command
        .as_ref()
        .map(|c| c.trim())
        .filter(|c| !c.is_empty())
        .map(|c| c.to_string());

    if matches!(app_id.as_deref(), Some("__default__"))
        || (app_id.is_none() && custom_cmd.is_none())
    {
        return crate::commands::fs::open_entry(target.to_string_lossy().to_string());
    }

    if let Some(cmd) = custom_cmd {
        let args = custom_args.unwrap_or_default();
        info!("Opening {:?} with custom command {:?}", target, cmd);
        return launch_custom(&target, &cmd, &args);
    }

    #[cfg(target_os = "linux")]
    {
        if let Some(app_id) = app_id {
            info!("Opening {:?} with desktop entry {}", target, app_id);
            return launch_desktop_entry(&target, &app_id);
        }
    }
    #[cfg(target_os = "windows")]
    {
        if let Some(app_id) = app_id {
            return launch_windows_handler(&target, &app_id);
        }
    }

    Err("No application selected".into())
}

fn launch_custom(target: &Path, command: &str, args: &str) -> Result<(), String> {
    let mut parts =
        shell_words::split(command).map_err(|e| format!("Failed to parse command: {e}"))?;
    if parts.is_empty() {
        return Err("Command cannot be empty".into());
    }
    let program = parts.remove(0);
    let mut parsed_args = parts;
    if !args.trim().is_empty() {
        let extra = shell_words::split(args)
            .map_err(|e| format!("Failed to parse additional arguments: {e}"))?;
        parsed_args.extend(extra);
    }
    parsed_args.push(target.to_string_lossy().to_string());
    let mut cmd = Command::new(&program);
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .args(parsed_args);
    spawn_detached(cmd).map_err(|e| format!("Failed to launch {program}: {e}"))
}

#[cfg(target_os = "linux")]
fn list_linux_apps(target: &Path) -> Vec<OpenWithApp> {
    use std::collections::HashSet;

    let target_mime = mime_for_path(target);
    let is_dir = target.is_dir();
    let mut matches_list = Vec::new();
    let mut fallback = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();
    for dir in linux_application_dirs() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
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
            let Some(desktop) = parse_desktop_entry(&path) else {
                continue;
            };
            let matches = matches_mime(&desktop.mime_types, &target_mime, is_dir);
            let id = path.to_string_lossy().to_string();
            if !seen.insert(id.clone()) {
                continue;
            }
            let app = OpenWithApp {
                id,
                name: desktop.name,
                comment: desktop.comment,
                exec: desktop.exec,
                icon: desktop.icon,
                matches,
                terminal: desktop.terminal,
            };
            if matches {
                matches_list.push(app);
            } else {
                fallback.push(app);
            }
        }
    }
    matches_list.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    fallback.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    matches_list.extend(fallback);
    matches_list
}

#[cfg(target_os = "linux")]
fn launch_desktop_entry(target: &Path, app_id: &str) -> Result<(), String> {
    let path = PathBuf::from(app_id);
    let entry =
        parse_desktop_entry(&path).ok_or_else(|| "Selected application is unavailable".to_string())?;
    let (program, args) = command_from_exec(&entry, target)?;
    let mut cmd = Command::new(&program);
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .args(&args);
    spawn_detached(cmd).map_err(|e| format!("Failed to launch {}: {e}", entry.name))
}

fn spawn_detached(mut cmd: Command) -> Result<(), String> {
    match cmd.spawn() {
        Ok(mut child) => {
            thread::spawn(move || {
                let _ = child.wait();
            });
            Ok(())
        }
        Err(e) => Err(e.to_string()),
    }
}

#[cfg(target_os = "linux")]
struct DesktopEntry {
    name: String,
    comment: Option<String>,
    exec: String,
    mime_types: Vec<String>,
    icon: Option<String>,
    terminal: bool,
    path: PathBuf,
}

#[cfg(target_os = "linux")]
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

#[cfg(target_os = "linux")]
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

#[cfg(target_os = "linux")]
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

#[cfg(target_os = "linux")]
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

#[cfg(target_os = "linux")]
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

#[cfg(target_os = "linux")]
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
        if token.contains("%f") || token.contains("%F") || token.contains("%u") || token.contains("%U")
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


#[cfg(target_os = "windows")]
fn list_windows_apps(target: &Path) -> Vec<OpenWithApp> {
    use std::collections::HashSet;

    let hkcr = RegKey::predef(HKEY_CLASSES_ROOT);
    let is_dir = target.is_dir();
    let ext = target
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase());

    let mut apps = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    if is_dir {
        collect_progids(&hkcr, "Directory", true, &mut apps, &mut seen);
    }

    if let Some(ext) = &ext {
        collect_progids(&hkcr, &format!(".{ext}"), false, &mut apps, &mut seen);
        collect_applications(&hkcr, &format!(".{ext}"), &mut apps, &mut seen);
    }

    apps.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    apps
}

#[cfg(target_os = "windows")]
fn collect_progids(
    hkcr: &RegKey,
    key_name: &str,
    is_dir: bool,
    out: &mut Vec<OpenWithApp>,
    seen: &mut std::collections::HashSet<String>,
) {
    if let Ok(key) = hkcr.open_subkey_with_flags(key_name, KEY_READ) {
        if let Ok(prog) = key.get_value::<String, _>("") {
            push_prog_id(hkcr, &prog, out, seen);
        }
        if let Ok(owp) = key.open_subkey_with_flags("OpenWithProgids", KEY_READ) {
            for value in owp.enum_values().flatten() {
                let progid = value.0;
                push_prog_id(hkcr, &progid, out, seen);
            }
        }
        if is_dir {
            // Directory may map to Folder
            push_prog_id(hkcr, "Folder", out, seen);
        }
    }
}

#[cfg(target_os = "windows")]
fn push_prog_id(
    hkcr: &RegKey,
    progid: &str,
    out: &mut Vec<OpenWithApp>,
    seen: &mut std::collections::HashSet<String>,
) {
    if progid.is_empty() || !seen.insert(format!("progid:{progid}")) {
        return;
    }
    let cmd_key = format!("{progid}\\shell\\open\\command");
    if let Ok(key) = hkcr.open_subkey_with_flags(cmd_key, KEY_READ) {
        if let Ok(command) = key.get_value::<String, _>("") {
            let name = hkcr
                .open_subkey_with_flags(progid, KEY_READ)
                .ok()
                .and_then(|k| k.get_value::<String, _>("FriendlyTypeName").ok())
                .unwrap_or_else(|| progid.to_string());
            out.push(OpenWithApp {
                id: format!("progid:{progid}"),
                name,
                comment: None,
                exec: command,
                icon: None,
                matches: true,
                terminal: false,
            });
        }
    }
}

#[cfg(target_os = "windows")]
fn collect_applications(
    hkcr: &RegKey,
    ext_key: &str,
    out: &mut Vec<OpenWithApp>,
    seen: &mut std::collections::HashSet<String>,
) {
    let allowed = open_with_list(hkcr, ext_key);
    if allowed.is_empty() {
        return;
    }
    if let Ok(apps_key) = hkcr.open_subkey_with_flags("Applications", KEY_READ) {
        for app in &allowed {
            if let Ok(app_key) = apps_key.open_subkey_with_flags(app, KEY_READ) {
                if let Ok(cmd_key) = app_key.open_subkey_with_flags(r"shell\\open\\command", KEY_READ) {
                    if let Ok(command) = cmd_key.get_value::<String, _>("") {
                        let id = format!("app:{app}");
                        if !seen.insert(id.clone()) {
                            continue;
                        }
                        let name = app.trim_end_matches(".exe").to_string();
                        let comment = app_key
                            .get_value::<String, _>("FriendlyAppName")
                            .ok()
                            .or_else(|| app_key.get_value::<String, _>("FriendlyName").ok());
                        out.push(OpenWithApp {
                            id,
                            name: comment.clone().unwrap_or_else(|| name.clone()),
                            comment,
                            exec: command,
                            icon: None,
                            matches: true,
                            terminal: false,
                        });
                    }
                }
            }
        }
    }
}

#[cfg(target_os = "windows")]
fn open_with_list(hkcr: &RegKey, ext_key: &str) -> Vec<String> {
    let mut apps = Vec::new();
    let key_name = format!(r"{ext_key}\OpenWithList");
    if let Ok(key) = hkcr.open_subkey_with_flags(key_name, KEY_READ) {
        for value in key.enum_values().flatten() {
            let name = value.0;
            if name.to_ascii_lowercase().ends_with(".exe") {
                apps.push(name);
            }
        }
    }
    apps
}

#[cfg(target_os = "windows")]
fn launch_windows_handler(target: &Path, app_id: &str) -> Result<(), String> {
    let apps = list_windows_apps(target);
    let app = apps
        .into_iter()
        .find(|a| a.id == app_id)
        .ok_or_else(|| "Selected application is unavailable".to_string())?;
    launch_windows_command(&app.exec, target)
}

#[cfg(target_os = "windows")]
fn launch_windows_command(template: &str, target: &Path) -> Result<(), String> {
    let target_str = target.to_string_lossy().to_string();
    let replaced = template
        .replace("%1", &target_str)
        .replace("%l", &target_str)
        .replace("%L", &target_str)
        .replace("%*", &target_str);
    let mut parts = shell_words::split(&replaced)
        .map_err(|e| format!("Failed to parse command template: {e}"))?;
    if parts.is_empty() {
        return Err("Command cannot be empty".into());
    }
    if !replaced.contains(&target_str) {
        parts.push(target_str);
    }
    let program = parts.remove(0);
    let mut cmd = Command::new(&program);
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .args(parts);
    spawn_detached(cmd).map_err(|e| format!("Failed to launch {program}: {e}"))
}
