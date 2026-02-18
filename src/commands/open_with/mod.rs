use crate::{db, fs_utils::sanitize_path_follow};
use serde::{Deserialize, Serialize};
use std::process::Command;
use std::thread;
#[cfg(debug_assertions)]
use tracing::info;
use tracing::warn;

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "windows")]
mod windows;

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
        return Ok(linux::list_linux_apps(&target));
    }
    #[cfg(target_os = "windows")]
    {
        return Ok(windows::list_windows_apps(&target));
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
            #[cfg(debug_assertions)]
            info!("Opening {:?} with desktop entry {}", target, app_id);
            return linux::launch_desktop_entry_by_id(&target, &app_id);
        }
    }
    #[cfg(target_os = "windows")]
    {
        if let Some(app_id) = app_id {
            return windows::launch_windows_handler(&target, &app_id);
        }
    }

    Err("No application selected".into())
}

pub(super) fn spawn_detached(mut cmd: Command) -> Result<(), String> {
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
