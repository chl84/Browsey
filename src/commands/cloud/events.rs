use super::path::CloudPath;
use crate::runtime_lifecycle;
use serde::Serialize;

const CLOUD_DIR_REFRESHED_EVENT: &str = "cloud-dir-refreshed";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct CloudDirRefreshedEvent {
    path: String,
    entry_count: usize,
}

pub(crate) fn emit_cloud_dir_refreshed(
    app: &tauri::AppHandle,
    path: &CloudPath,
    entry_count: usize,
) {
    let _ = runtime_lifecycle::emit_if_running(
        app,
        CLOUD_DIR_REFRESHED_EVENT,
        CloudDirRefreshedEvent {
            path: path.to_string(),
            entry_count,
        },
    );
}
