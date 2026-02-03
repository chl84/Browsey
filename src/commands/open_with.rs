use crate::{db, fs_utils::sanitize_path_follow};
use serde::{Deserialize, Serialize};
#[cfg(target_os = "windows")]
use std::ffi::c_void;
use std::path::Path;
#[cfg(target_os = "linux")]
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread;
use tracing::{info, warn};
#[cfg(target_os = "windows")]
use windows::{
    core::{HSTRING, PWSTR},
    Win32::{
        Foundation::{RPC_E_CHANGED_MODE, S_FALSE, S_OK},
        System::Com::{
            CoInitializeEx, CoTaskMemFree, CoUninitialize, IDataObject, COINIT_APARTMENTTHREADED,
        },
        UI::Shell::{
            BHID_DataObject, IAssocHandler, IShellItem, IShellItemArray, SHAssocEnumHandlers,
            SHCreateItemFromParsingName, SHCreateShellItemArrayFromShellItem, ASSOC_FILTER,
        },
    },
};
#[cfg(target_os = "windows")]
use winreg::{enums::*, RegKey};

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
    let OpenWithChoice { app_id } = choice;

    let conn = db::open()?;
    if let Err(e) = db::touch_recent(&conn, &target.to_string_lossy()) {
        warn!("Failed to record recent for {:?}: {}", target, e);
    }

    if matches!(app_id.as_deref(), Some("__default__")) || app_id.is_none() {
        return crate::commands::fs::open_entry(target.to_string_lossy().to_string());
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
    let entry = parse_desktop_entry(&path)
        .ok_or_else(|| "Selected application is unavailable".to_string())?;
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

#[cfg(target_os = "windows")]
fn list_windows_apps(target: &Path) -> Vec<OpenWithApp> {
    use std::collections::HashSet;

    let mut apps = Vec::new();
    let mut seen: HashSet<String> = HashSet::new();

    if let Ok(_com_guard) = ComGuard::new() {
        let query = windows_query_string(target);
        if let Err(e) = unsafe { collect_assoc_handlers(&query, &mut apps, &mut seen) } {
            warn!("Failed to enumerate shell handlers: {e}");
        }
    } else {
        warn!("Failed to initialize COM for shell handler enumeration");
    }

    let mut registry_apps = list_windows_apps_registry(target);
    for app in registry_apps.drain(..) {
        if seen.insert(app.id.clone()) {
            apps.push(app);
        }
    }

    apps.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    apps
}

#[cfg(target_os = "windows")]
fn launch_windows_handler(target: &Path, app_id: &str) -> Result<(), String> {
    if app_id.starts_with("progid:") || app_id.starts_with("app:") {
        return launch_registry_handler(target, app_id);
    }

    let _com_guard = ComGuard::new().map_err(|e| format!("Failed to initialize COM: {e}"))?;
    let query = windows_query_string(target);
    let handler = unsafe {
        find_assoc_handler(&query, app_id)
            .map_err(|e| format!("Failed to enumerate handlers: {e}"))?
    }
    .ok_or_else(|| "Selected application is unavailable".to_string())?;

    let data_object = create_data_object_for_path(target)?;

    unsafe {
        if let Ok(invoker) = handler.CreateInvoker(&data_object) {
            let _ = invoker.SupportsSelection();
            invoker
                .Invoke()
                .map_err(|e| format!("Failed to invoke handler: {e}"))
        } else {
            handler
                .Invoke(&data_object)
                .map_err(|e| format!("Failed to invoke handler: {e}"))
        }
    }
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

#[cfg(target_os = "windows")]
fn launch_registry_handler(target: &Path, app_id: &str) -> Result<(), String> {
    let apps = list_windows_apps_registry(target);
    let app = apps
        .into_iter()
        .find(|a| a.id == app_id)
        .ok_or_else(|| "Selected application is unavailable".to_string())?;
    launch_windows_command(&app.exec, target)
}

#[cfg(target_os = "windows")]
fn list_windows_apps_registry(target: &Path) -> Vec<OpenWithApp> {
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
                if let Ok(cmd_key) =
                    app_key.open_subkey_with_flags(r"shell\\open\\command", KEY_READ)
                {
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
unsafe fn collect_assoc_handlers(
    query: &HSTRING,
    out: &mut Vec<OpenWithApp>,
    seen: &mut std::collections::HashSet<String>,
) -> Result<(), windows::core::Error> {
    let enum_handlers = SHAssocEnumHandlers(query, ASSOC_FILTER::default())?;
    let mut buffer: [Option<IAssocHandler>; 8] = Default::default();
    loop {
        let mut fetched: u32 = 0;
        match enum_handlers.Next(&mut buffer, Some(&mut fetched)) {
            Ok(()) => {
                if fetched == 0 {
                    break;
                }
                for handler in buffer.iter_mut().take(fetched as usize) {
                    let Some(handler) = handler.take() else {
                        continue;
                    };
                    if let Some(app) = handler_to_app(handler) {
                        if seen.insert(app.id.clone()) {
                            out.push(app);
                        }
                    }
                }
            }
            Err(e) if e.code() == S_FALSE => break,
            Err(e) => return Err(e),
        }
    }
    Ok(())
}

#[cfg(target_os = "windows")]
unsafe fn handler_to_app(handler: IAssocHandler) -> Option<OpenWithApp> {
    let id = handler
        .GetName()
        .ok()
        .and_then(|ptr| pwstr_to_string(ptr))?;
    let name = handler
        .GetUIName()
        .ok()
        .and_then(|ptr| pwstr_to_string(ptr))
        .filter(|ui| !ui.is_empty())
        .unwrap_or_else(|| id.clone());
    let mut icon_ptr: PWSTR = PWSTR::null();
    let mut icon_index: i32 = 0;
    let icon = handler
        .GetIconLocation(&mut icon_ptr, &mut icon_index)
        .ok()
        .and_then(|_| pwstr_to_string(icon_ptr));
    let matches = handler.IsRecommended() == S_OK;
    let comment = if name != id { Some(id.clone()) } else { None };
    let exec = comment.clone().unwrap_or_else(|| id.clone());
    Some(OpenWithApp {
        id,
        name,
        comment,
        exec,
        icon,
        matches,
        terminal: false,
    })
}

#[cfg(target_os = "windows")]
unsafe fn find_assoc_handler(
    query: &HSTRING,
    app_id: &str,
) -> Result<Option<IAssocHandler>, windows::core::Error> {
    let enum_handlers = SHAssocEnumHandlers(query, ASSOC_FILTER::default())?;
    let mut buffer: [Option<IAssocHandler>; 8] = Default::default();
    let target_id = app_id.to_ascii_lowercase();
    loop {
        let mut fetched: u32 = 0;
        match enum_handlers.Next(&mut buffer, Some(&mut fetched)) {
            Ok(()) => {
                if fetched == 0 {
                    break;
                }
                for handler in buffer.iter_mut().take(fetched as usize) {
                    let Some(handler) = handler.take() else {
                        continue;
                    };
                    if let Some(id) = handler.GetName().ok().and_then(|ptr| pwstr_to_string(ptr)) {
                        if id.to_ascii_lowercase() == target_id {
                            return Ok(Some(handler));
                        }
                    }
                }
            }
            Err(e) if e.code() == S_FALSE => break,
            Err(e) => return Err(e),
        }
    }
    Ok(None)
}

#[cfg(target_os = "windows")]
fn create_data_object_for_path(target: &Path) -> Result<IDataObject, String> {
    let path = target.to_string_lossy().into_owned();
    unsafe {
        let item: IShellItem = SHCreateItemFromParsingName(&HSTRING::from(path.as_str()), None)
            .map_err(|e| {
                format!(
                    "Failed to create shell item for {}: {e}",
                    target.to_string_lossy()
                )
            })?;
        let array: IShellItemArray = SHCreateShellItemArrayFromShellItem(&item)
            .map_err(|e| format!("Failed to create shell item array: {e}"))?;
        array
            .BindToHandler::<_, IDataObject>(None, &BHID_DataObject)
            .map_err(|e| format!("Failed to create data object: {e}"))
    }
}

#[cfg(target_os = "windows")]
fn windows_query_string(target: &Path) -> HSTRING {
    if target.is_dir() {
        HSTRING::from(target.to_string_lossy().into_owned())
    } else if let Some(ext) = target.extension().and_then(|e| e.to_str()) {
        HSTRING::from(format!(".{}", ext.to_ascii_lowercase()))
    } else {
        HSTRING::from(target.to_string_lossy().into_owned())
    }
}

#[cfg(target_os = "windows")]
unsafe fn pwstr_to_string(pwstr: PWSTR) -> Option<String> {
    if pwstr.0.is_null() {
        return None;
    }
    let mut len = 0usize;
    while *pwstr.0.add(len) != 0 {
        len += 1;
    }
    let slice = std::slice::from_raw_parts(pwstr.0, len);
    let value = String::from_utf16_lossy(slice);
    CoTaskMemFree(Some(pwstr.0 as *mut c_void));
    Some(value)
}

#[cfg(target_os = "windows")]
struct ComGuard {
    should_uninit: bool,
}

#[cfg(target_os = "windows")]
impl ComGuard {
    fn new() -> Result<Self, windows::core::Error> {
        unsafe {
            let hr = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
            if hr == S_OK {
                Ok(Self {
                    should_uninit: true,
                })
            } else if hr == RPC_E_CHANGED_MODE {
                Ok(Self {
                    should_uninit: false,
                })
            } else if hr.is_ok() {
                Ok(Self {
                    should_uninit: true,
                })
            } else {
                Err(hr.into())
            }
        }
    }
}

#[cfg(target_os = "windows")]
impl Drop for ComGuard {
    fn drop(&mut self) {
        if self.should_uninit {
            unsafe {
                CoUninitialize();
            }
        }
    }
}
