use crate::{db, fs_utils::sanitize_path_follow};
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
pub fn open_entry(path: String) -> Result<(), String> {
    let pb = sanitize_path_follow(&path, false)?;
    let conn = db::open()?;
    if let Err(e) = db::touch_recent(&conn, &pb.to_string_lossy()) {
        warn!("Failed to record recent for {:?}: {}", pb, e);
    }
    #[cfg(debug_assertions)]
    info!("Opening path {:?}", pb);
    #[cfg(not(target_os = "windows"))]
    {
        if is_gvfs_path(&pb) {
            let (tx, rx) = mpsc::channel();
            let path_for_open = pb.clone();
            std::thread::spawn(move || {
                let res =
                    open::that_detached(&path_for_open).map_err(|e| format!("Failed to open: {e}"));
                let _ = tx.send(res);
            });
            let res = match rx.recv_timeout(OPEN_TIMEOUT_GVFS) {
                Ok(res) => res.map_err(|e| {
                    error!("Failed to open {:?}: {}", pb, e);
                    e
                }),
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    error!("Open timed out for {:?}", pb);
                    Err("Open timed out on remote device".into())
                }
                Err(_) => {
                    error!("Open channel closed for {:?}", pb);
                    Err("Failed to open".into())
                }
            };
            return res;
        }
    }
    open::that_detached(&pb).map_err(|e| {
        error!("Failed to open {:?}: {}", pb, e);
        format!("Failed to open: {e}")
    })
}
