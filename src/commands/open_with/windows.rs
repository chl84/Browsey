use super::{error::OpenWithError, spawn_detached, OpenWithApp, OpenWithResult};
use std::ffi::c_void;
use std::path::Path;
use std::process::{Command, Stdio};
use tracing::warn;
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
use winreg::{enums::*, RegKey};

pub(super) fn list_windows_apps(target: &Path) -> Vec<OpenWithApp> {
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

pub(super) fn launch_windows_handler(target: &Path, app_id: &str) -> OpenWithResult<()> {
    if app_id.starts_with("progid:") || app_id.starts_with("app:") {
        return launch_registry_handler(target, app_id);
    }

    let _com_guard = ComGuard::new().map_err(|e| {
        OpenWithError::from_external_message(format!("Failed to initialize COM: {e}"))
    })?;
    let query = windows_query_string(target);
    let handler = unsafe {
        find_assoc_handler(&query, app_id).map_err(|e| {
            OpenWithError::from_external_message(format!("Failed to enumerate handlers: {e}"))
        })?
    }
    .ok_or_else(|| OpenWithError::from_external_message("Selected application is unavailable"))?;

    let data_object = create_data_object_for_path(target)?;

    unsafe {
        if let Ok(invoker) = handler.CreateInvoker(&data_object) {
            let _ = invoker.SupportsSelection();
            invoker.Invoke().map_err(|e| {
                OpenWithError::from_external_message(format!("Failed to invoke handler: {e}"))
            })
        } else {
            handler.Invoke(&data_object).map_err(|e| {
                OpenWithError::from_external_message(format!("Failed to invoke handler: {e}"))
            })
        }
    }
}

fn launch_windows_command(template: &str, target: &Path) -> OpenWithResult<()> {
    let target_str = target.to_string_lossy().to_string();
    let replaced = template
        .replace("%1", &target_str)
        .replace("%l", &target_str)
        .replace("%L", &target_str)
        .replace("%*", &target_str);
    let mut parts = shell_words::split(&replaced).map_err(|e| {
        OpenWithError::from_external_message(format!("Failed to parse command template: {e}"))
    })?;
    if parts.is_empty() {
        return Err(OpenWithError::from_external_message(
            "Command cannot be empty",
        ));
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
    spawn_detached(cmd).map_err(|error| {
        OpenWithError::from_external_message(format!("Failed to launch {program}: {error}"))
    })
}

fn launch_registry_handler(target: &Path, app_id: &str) -> OpenWithResult<()> {
    let apps = list_windows_apps_registry(target);
    let app = apps.into_iter().find(|a| a.id == app_id).ok_or_else(|| {
        OpenWithError::from_external_message("Selected application is unavailable")
    })?;
    launch_windows_command(&app.exec, target)
}

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

fn create_data_object_for_path(target: &Path) -> OpenWithResult<IDataObject> {
    let path = target.to_string_lossy().into_owned();
    unsafe {
        let item: IShellItem = SHCreateItemFromParsingName(&HSTRING::from(path.as_str()), None)
            .map_err(|e| {
                OpenWithError::from_external_message(format!(
                    "Failed to create shell item for {}: {e}",
                    target.to_string_lossy()
                ))
            })?;
        let array: IShellItemArray = SHCreateShellItemArrayFromShellItem(&item).map_err(|e| {
            OpenWithError::from_external_message(format!("Failed to create shell item array: {e}"))
        })?;
        array
            .BindToHandler::<_, IDataObject>(None, &BHID_DataObject)
            .map_err(|e| {
                OpenWithError::from_external_message(format!("Failed to create data object: {e}"))
            })
    }
}

fn windows_query_string(target: &Path) -> HSTRING {
    if target.is_dir() {
        HSTRING::from(target.to_string_lossy().into_owned())
    } else if let Some(ext) = target.extension().and_then(|e| e.to_str()) {
        HSTRING::from(format!(".{}", ext.to_ascii_lowercase()))
    } else {
        HSTRING::from(target.to_string_lossy().into_owned())
    }
}

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

struct ComGuard {
    should_uninit: bool,
}

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

impl Drop for ComGuard {
    fn drop(&mut self) {
        if self.should_uninit {
            unsafe {
                CoUninitialize();
            }
        }
    }
}
