mod execute;
mod logging;
mod preview;
mod route;

use crate::errors::api_error::ApiResult;
use crate::tasks::CancelState;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MixedTransferConflictInfo {
    pub src: String,
    pub target: String,
    pub exists: bool,
    pub is_dir: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "camelCase")]
pub struct MixedTransferWriteOptions {
    pub overwrite: bool,
    pub prechecked: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum MixedTransferOp {
    Copy,
    Move,
}

#[tauri::command]
pub async fn preview_mixed_transfer_conflicts(
    sources: Vec<String>,
    dest_dir: String,
    app: tauri::AppHandle,
) -> ApiResult<Vec<MixedTransferConflictInfo>> {
    preview::preview_mixed_transfer_conflicts(sources, dest_dir, app).await
}

#[tauri::command]
pub async fn copy_mixed_entries(
    sources: Vec<String>,
    dest_dir: String,
    overwrite: Option<bool>,
    prechecked: Option<bool>,
    cancel: tauri::State<'_, CancelState>,
    progress_event: Option<String>,
) -> ApiResult<Vec<String>> {
    execute::execute_mixed_entries(
        MixedTransferOp::Copy,
        sources,
        dest_dir,
        MixedTransferWriteOptions {
            overwrite: overwrite.unwrap_or(false),
            prechecked: prechecked.unwrap_or(false),
        },
        cancel.inner().clone(),
        progress_event,
    )
    .await
}

#[tauri::command]
pub async fn move_mixed_entries(
    sources: Vec<String>,
    dest_dir: String,
    overwrite: Option<bool>,
    prechecked: Option<bool>,
    cancel: tauri::State<'_, CancelState>,
    progress_event: Option<String>,
) -> ApiResult<Vec<String>> {
    execute::execute_mixed_entries(
        MixedTransferOp::Move,
        sources,
        dest_dir,
        MixedTransferWriteOptions {
            overwrite: overwrite.unwrap_or(false),
            prechecked: prechecked.unwrap_or(false),
        },
        cancel.inner().clone(),
        progress_event,
    )
    .await
}

#[tauri::command]
pub async fn copy_mixed_entry_to(
    src: String,
    dst: String,
    overwrite: Option<bool>,
    prechecked: Option<bool>,
    cancel: tauri::State<'_, CancelState>,
    progress_event: Option<String>,
) -> ApiResult<String> {
    execute::execute_mixed_entry_to(
        MixedTransferOp::Copy,
        src,
        dst,
        MixedTransferWriteOptions {
            overwrite: overwrite.unwrap_or(false),
            prechecked: prechecked.unwrap_or(false),
        },
        cancel.inner().clone(),
        progress_event,
    )
    .await
}

#[tauri::command]
pub async fn move_mixed_entry_to(
    src: String,
    dst: String,
    overwrite: Option<bool>,
    prechecked: Option<bool>,
    cancel: tauri::State<'_, CancelState>,
    progress_event: Option<String>,
) -> ApiResult<String> {
    execute::execute_mixed_entry_to(
        MixedTransferOp::Move,
        src,
        dst,
        MixedTransferWriteOptions {
            overwrite: overwrite.unwrap_or(false),
            prechecked: prechecked.unwrap_or(false),
        },
        cancel.inner().clone(),
        progress_event,
    )
    .await
}
