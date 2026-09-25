use crate::errors::api_error::ApiResult;
use crate::{db, fs_utils::sanitize_path_follow};
use error::{map_api_result, OpenWithError, OpenWithErrorCode, OpenWithResult};
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::process::Command;
use std::thread;
use tracing::debug;
#[cfg(debug_assertions)]
use tracing::info;

mod error;
#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "windows")]
mod windows;

fn map_db_open_error(error: crate::db::DbError) -> OpenWithError {
    OpenWithError::new(OpenWithErrorCode::DatabaseOpenFailed, error.to_string())
}

fn map_open_entry_api_error(error: crate::errors::api_error::ApiError) -> OpenWithError {
    let code = match error.code.as_str() {
        "path_not_absolute" => OpenWithErrorCode::PathNotAbsolute,
        "invalid_path" | "root_forbidden" | "symlink_unsupported" => OpenWithErrorCode::InvalidPath,
        "not_found" => OpenWithErrorCode::NotFound,
        "permission_denied" | "read_only_filesystem" => OpenWithErrorCode::PermissionDenied,
        "open_failed" => OpenWithErrorCode::LaunchFailed,
        _ => OpenWithErrorCode::UnknownError,
    };
    OpenWithError::new(code, error.message)
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
    #[cfg(target_os = "linux")]
    pub default_content_type: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenWithChoice {
    pub app_id: Option<String>,
}

#[tauri::command]
pub async fn list_open_with_apps(path: String) -> ApiResult<Vec<OpenWithApp>> {
    let result = tauri::async_runtime::spawn_blocking(move || list_open_with_apps_impl(path))
        .await
        .map_err(|error| {
            crate::errors::api_error::ApiError::new("unknown_error", error.to_string())
        })?;
    map_api_result(result)
}

#[tauri::command]
pub async fn set_default_app(path: String, app_id: String, content_type: String) -> ApiResult<()> {
    let result = tauri::async_runtime::spawn_blocking(move || {
        set_default_app_impl(&path, &app_id, &content_type)
    })
    .await
    .map_err(|error| crate::errors::api_error::ApiError::new("unknown_error", error.to_string()))?;
    map_api_result(result)
}

fn set_default_app_impl(path: &str, app_id: &str, content_type: &str) -> OpenWithResult<()> {
    if !Path::new(path).is_absolute() {
        return Err(OpenWithError::new(
            OpenWithErrorCode::PathNotAbsolute,
            format!("Path must be absolute: {path}"),
        ));
    }
    let target = sanitize_path_follow(path, false).map_err(OpenWithError::from)?;
    #[cfg(target_os = "linux")]
    {
        linux::set_default_app(&target, app_id, content_type)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (target, app_id, content_type);
        Err(OpenWithError::invalid_input(
            "Setting a default application is currently supported only on Linux",
        ))
    }
}

fn list_open_with_apps_impl(path: String) -> OpenWithResult<Vec<OpenWithApp>> {
    if !Path::new(&path).is_absolute() {
        return Err(OpenWithError::new(
            OpenWithErrorCode::PathNotAbsolute,
            format!("Path must be absolute: {path}"),
        ));
    }
    let target = sanitize_path_follow(&path, false).map_err(OpenWithError::from)?;
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
    if !Path::new(&path).is_absolute() {
        return Err(OpenWithError::new(
            OpenWithErrorCode::PathNotAbsolute,
            format!("Path must be absolute: {path}"),
        ));
    }
    let target = sanitize_path_follow(&path, false).map_err(OpenWithError::from)?;
    let OpenWithChoice { app_id } = choice;

    let conn = db::open().map_err(map_db_open_error)?;
    if let Err(e) = db::touch_recent(&conn, &target.to_string_lossy()) {
        debug!(path = %target.display(), error = %e, "failed to record recent entry");
    }

    if matches!(app_id.as_deref(), Some("__default__")) || app_id.is_none() {
        return crate::commands::fs::open_entry(target.to_string_lossy().to_string())
            .map_err(map_open_entry_api_error);
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::errors::domain::DomainError;

    #[test]
    fn list_open_with_apps_rejects_relative_paths_before_fs_sanitization() {
        let error = list_open_with_apps_impl("relative/path.txt".to_string())
            .expect_err("relative path should fail");
        assert_eq!(error.code_str(), "path_not_absolute");
        assert_eq!(error.message(), "Path must be absolute: relative/path.txt");
    }

    #[test]
    fn open_with_rejects_relative_paths_before_fs_sanitization() {
        let error = open_with_impl(
            "relative/path.txt".to_string(),
            OpenWithChoice { app_id: None },
        )
        .expect_err("relative path should fail");
        assert_eq!(error.code_str(), "path_not_absolute");
        assert_eq!(error.message(), "Path must be absolute: relative/path.txt");
    }

    #[test]
    fn maps_open_entry_api_error_by_typed_code() {
        let error = map_open_entry_api_error(crate::errors::api_error::ApiError::new(
            "open_failed",
            "Failed to open: launcher missing",
        ));
        assert_eq!(error.code_str(), "launch_failed");
        assert_eq!(error.message(), "Failed to open: launcher missing");
    }

    #[test]
    fn set_default_rejects_relative_paths() {
        let error = set_default_app_impl("file.txt", "desktop:fake", "text/plain").unwrap_err();
        assert_eq!(error.code_str(), "path_not_absolute");
    }
}
