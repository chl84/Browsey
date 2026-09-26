use super::error::{map_api_result, FsError, FsErrorCode, FsResult};
use crate::errors::api_error::ApiResult;
use crate::{db, fs_utils::sanitize_path_follow};
use std::path::Path;
#[cfg(not(target_os = "windows"))]
use std::sync::mpsc;
#[cfg(not(target_os = "windows"))]
use std::time::Duration;
#[cfg(debug_assertions)]
use tracing::info;
use tracing::{debug, warn};

#[cfg(not(target_os = "windows"))]
const OPEN_TIMEOUT_GVFS: Duration = Duration::from_secs(8);

#[cfg(not(target_os = "windows"))]
fn is_gvfs_path(path: &std::path::Path) -> bool {
    let s = path.to_string_lossy();
    s.contains("/gvfs/") || s.contains("\\gvfs\\")
}

fn map_db_open_error(error: crate::db::DbError) -> FsError {
    let code = match error.code() {
        crate::db::DbErrorCode::NotFound => FsErrorCode::NotFound,
        crate::db::DbErrorCode::PermissionDenied => FsErrorCode::PermissionDenied,
        crate::db::DbErrorCode::ReadOnlyFilesystem => FsErrorCode::ReadOnlyFilesystem,
        _ => FsErrorCode::OpenFailed,
    };
    FsError::new(code, error.to_string())
}

#[tauri::command]
pub async fn open_entry(path: String) -> ApiResult<()> {
    let result = tauri::async_runtime::spawn_blocking(move || open_entry_impl(path))
        .await
        .map_err(|error| {
            crate::errors::api_error::ApiError::new("task_failed", error.to_string())
        })?;
    map_api_result(result)
}

fn open_entry_impl(path: String) -> FsResult<()> {
    let pb = sanitize_path_follow(&path, false).map_err(FsError::from)?;
    let conn = db::open().map_err(map_db_open_error)?;
    if let Err(e) = db::touch_recent(&conn, &pb.to_string_lossy()) {
        debug!(path = %pb.display(), error = %e, "failed to record recent entry");
    }
    open_path_impl(&pb)
}

pub(crate) fn open_path_without_recent(path: &Path) -> FsResult<()> {
    open_path_impl(path)
}

fn open_path_impl(path: &Path) -> FsResult<()> {
    #[cfg(debug_assertions)]
    info!("Opening path {:?}", path);
    #[cfg(not(target_os = "windows"))]
    {
        if is_gvfs_path(path) {
            let (tx, rx) = mpsc::channel();
            let path_for_open = path.to_path_buf();
            std::thread::spawn(move || {
                let res = launch_default(&path_for_open);
                let _ = tx.send(res);
            });
            let res = match rx.recv_timeout(OPEN_TIMEOUT_GVFS) {
                Ok(res) => res,
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    warn!(
                        path = %path.display(),
                        timeout_secs = OPEN_TIMEOUT_GVFS.as_secs(),
                        "open timed out for gvfs path"
                    );
                    Err(FsError::new(
                        FsErrorCode::OpenFailed,
                        "Opening the file on the remote device is taking too long. It may still open; check before retrying",
                    ))
                }
                Err(_) => {
                    warn!(path = %path.display(), "gvfs open channel closed unexpectedly");
                    Err(FsError::new(FsErrorCode::OpenFailed, "Failed to open"))
                }
            };
            return res;
        }
    }
    launch_default(path)
}

fn launch_default(path: &Path) -> FsResult<()> {
    #[cfg(target_os = "linux")]
    let result = {
        use gio::prelude::*;
        // Match the GIO MIME lookup used by Set as default. xdg-open's generic
        // backend can identify the same Python file as a different MIME type.
        // GIO also returns handler lookup/spawn errors, unlike a detached xdg-open.
        let file = gio::File::for_path(path);
        file.query_default_handler(gio::Cancellable::NONE)
            .and_then(|app| app.launch(&[file], gio::AppLaunchContext::NONE))
    };
    #[cfg(not(target_os = "linux"))]
    let result = open::that_detached(path);
    result.map_err(|error| {
        warn!(path = %path.display(), error = %error, "failed to open path");
        FsError::new(FsErrorCode::OpenFailed, format!("Failed to open: {error}"))
    })
}
