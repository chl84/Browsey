use crate::errors::api_error::ApiResult;
use crate::{db, fs_utils::sanitize_path_follow};
use error::{map_api_result, OpenWithError, OpenWithErrorCode, OpenWithResult};
use serde::{Deserialize, Serialize};
use std::process::Command;
use std::thread;
#[cfg(debug_assertions)]
use tracing::info;
use tracing::warn;

mod error;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "windows")]
mod windows;

fn map_db_open_error(error: crate::db::DbError) -> OpenWithError {
    OpenWithError::new(OpenWithErrorCode::DatabaseOpenFailed, error.to_string())
}

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
pub fn list_open_with_apps(path: String) -> ApiResult<Vec<OpenWithApp>> {
    map_api_result(list_open_with_apps_impl(path))
}

fn list_open_with_apps_impl(path: String) -> OpenWithResult<Vec<OpenWithApp>> {
    let target =
        sanitize_path_follow(&path, false).map_err(OpenWithError::from_external_message)?;
    #[cfg(target_os = "linux")]
    {
        Ok(linux::list_linux_apps(&target))
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
pub fn open_with(path: String, choice: OpenWithChoice) -> ApiResult<()> {
    map_api_result(open_with_impl(path, choice))
}

fn open_with_impl(path: String, choice: OpenWithChoice) -> OpenWithResult<()> {
    let target =
        sanitize_path_follow(&path, false).map_err(OpenWithError::from_external_message)?;
    let OpenWithChoice { app_id } = choice;

    let conn = db::open().map_err(map_db_open_error)?;
    if let Err(e) = db::touch_recent(&conn, &target.to_string_lossy()) {
        warn!("Failed to record recent for {:?}: {}", target, e);
    }

    if matches!(app_id.as_deref(), Some("__default__")) || app_id.is_none() {
        return crate::commands::fs::open_entry(target.to_string_lossy().to_string())
            .map_err(|error| OpenWithError::from_external_message(error.message));
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

    Err(OpenWithError::invalid_input("No application selected"))
}

fn spawn_detached(mut cmd: Command) -> OpenWithResult<()> {
    match cmd.spawn() {
        Ok(mut child) => {
            thread::spawn(move || {
                let _ = child.wait();
            });
            Ok(())
        }
        Err(e) => Err(OpenWithError::new(
            OpenWithErrorCode::LaunchFailed,
            format!("Failed to launch process: {e}"),
        )),
    }
}
