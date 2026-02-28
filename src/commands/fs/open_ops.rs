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
use tracing::{error, warn};

#[cfg(not(target_os = "windows"))]
const OPEN_TIMEOUT_GVFS: Duration = Duration::from_secs(8);

#[cfg(not(target_os = "windows"))]
fn is_gvfs_path(path: &std::path::Path) -> bool {
    let s = path.to_string_lossy();
    s.contains("/gvfs/") || s.contains("\\gvfs\\")
}

#[tauri::command]
pub fn open_entry(path: String) -> ApiResult<()> {
    map_api_result(open_entry_impl(path))
}

fn open_entry_impl(path: String) -> FsResult<()> {
    let pb = sanitize_path_follow(&path, false).map_err(FsError::from_external_message)?;
    let conn = db::open().map_err(|error| {
        FsError::new(
            FsErrorCode::OpenFailed,
            format!("Failed to open database for recent tracking: {error}"),
        )
    })?;
    if let Err(e) = db::touch_recent(&conn, &pb.to_string_lossy()) {
        warn!("Failed to record recent for {:?}: {}", pb, e);
    }
    open_path_impl(&pb)
}

pub(crate) fn open_path_without_recent(path: &Path) -> Result<(), String> {
    open_path_impl(path).map_err(|error| error.to_string())
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
                let res =
                    open::that_detached(&path_for_open).map_err(|e| format!("Failed to open: {e}"));
                let _ = tx.send(res);
            });
            let res = match rx.recv_timeout(OPEN_TIMEOUT_GVFS) {
                Ok(res) => res.map_err(|error_message| {
                    error!("Failed to open {:?}: {}", path, error_message);
                    FsError::new(FsErrorCode::OpenFailed, error_message)
                }),
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    error!("Open timed out for {:?}", path);
                    Err(FsError::new(
                        FsErrorCode::OpenFailed,
                        "Open timed out on remote device",
                    ))
                }
                Err(_) => {
                    error!("Open channel closed for {:?}", path);
                    Err(FsError::new(FsErrorCode::OpenFailed, "Failed to open"))
                }
            };
            return res;
        }
    }
    open::that_detached(path).map_err(|error| {
        error!("Failed to open {:?}: {}", path, error);
        FsError::new(FsErrorCode::OpenFailed, format!("Failed to open: {error}"))
    })
}
