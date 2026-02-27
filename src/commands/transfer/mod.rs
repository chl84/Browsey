use crate::commands::cloud;
use crate::commands::cloud::path::CloudPath;
use crate::commands::cloud::rclone_cli::{
    RcloneCli, RcloneCliError, RcloneCommandSpec, RcloneSubcommand,
};
use crate::commands::cloud::types::{CloudEntryKind, CloudProviderKind};
use crate::errors::api_error::{ApiError, ApiResult};
use crate::fs_utils::sanitize_path_follow;
use crate::tasks::{CancelGuard, CancelState};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::ffi::OsString;
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::Instant;
use tracing::{info, warn};

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
enum MixedTransferOp {
    Copy,
    Move,
}

#[tauri::command]
pub async fn preview_mixed_transfer_conflicts(
    sources: Vec<String>,
    dest_dir: String,
) -> ApiResult<Vec<MixedTransferConflictInfo>> {
    let started = Instant::now();
    let source_count = sources.len();
    let route_hint = mixed_route_hint(&sources, &dest_dir);
    if sources.is_empty() {
        let result = Ok(Vec::new());
        log_mixed_preview_result(&result, route_hint, source_count, started);
        return result;
    }

    let dest_is_cloud = is_cloud_path(&dest_dir);
    let source_cloud_count = sources.iter().filter(|p| is_cloud_path(p)).count();
    if source_cloud_count > 0 && source_cloud_count < sources.len() {
        let result = Err(api_err(
            "unsupported",
            "Mixed local/cloud selection is not supported",
        ));
        log_mixed_preview_result(&result, route_hint, source_count, started);
        return result;
    }

    let result = match (source_cloud_count == sources.len(), dest_is_cloud) {
        (false, true) => preview_local_to_cloud_conflicts(sources, dest_dir).await,
        (true, false) => preview_cloud_to_local_conflicts(sources, dest_dir),
        (false, false) => Err(api_err(
            "unsupported",
            "Use local clipboard preview for local-to-local transfers",
        )),
        (true, true) => Err(api_err(
            "unsupported",
            "Use cloud clipboard preview for cloud-to-cloud transfers",
        )),
    };
    log_mixed_preview_result(&result, route_hint, source_count, started);
    result
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
    execute_mixed_entries(
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
    execute_mixed_entries(
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
    execute_mixed_entry_to(
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
    execute_mixed_entry_to(
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

fn is_cloud_path(path: &str) -> bool {
    path.starts_with("rclone://")
}

#[derive(Debug)]
enum MixedTransferRoute {
    LocalToCloud {
        sources: Vec<PathBuf>,
        dest_dir: CloudPath,
    },
    CloudToLocal {
        sources: Vec<CloudPath>,
        dest_dir: PathBuf,
    },
}

#[derive(Debug, Clone, Copy)]
enum MixedRouteHint {
    LocalToCloud,
    CloudToLocal,
    LocalToLocal,
    CloudToCloud,
    MixedSelection,
    Unknown,
}

#[derive(Debug, Clone)]
struct MixedTransferPair {
    src: LocalOrCloudArg,
    dst: LocalOrCloudArg,
    cloud_remote_for_error_mapping: Option<String>,
}

fn api_err(code: &str, message: impl Into<String>) -> ApiError {
    ApiError::new(code, message.into())
}

fn mixed_route_hint(sources: &[String], dest_dir: &str) -> MixedRouteHint {
    if sources.is_empty() {
        return MixedRouteHint::Unknown;
    }
    let dest_is_cloud = is_cloud_path(dest_dir);
    let source_cloud_count = sources.iter().filter(|p| is_cloud_path(p)).count();
    match (source_cloud_count, sources.len(), dest_is_cloud) {
        (0, _, true) => MixedRouteHint::LocalToCloud,
        (n, total, false) if n == total => MixedRouteHint::CloudToLocal,
        (0, _, false) => MixedRouteHint::LocalToLocal,
        (n, total, true) if n == total => MixedRouteHint::CloudToCloud,
        (n, total, _) if n > 0 && n < total => MixedRouteHint::MixedSelection,
        _ => MixedRouteHint::Unknown,
    }
}

fn route_hint_label(hint: MixedRouteHint) -> &'static str {
    match hint {
        MixedRouteHint::LocalToCloud => "local_to_cloud",
        MixedRouteHint::CloudToLocal => "cloud_to_local",
        MixedRouteHint::LocalToLocal => "local_to_local",
        MixedRouteHint::CloudToCloud => "cloud_to_cloud",
        MixedRouteHint::MixedSelection => "mixed_selection",
        MixedRouteHint::Unknown => "unknown",
    }
}

fn log_mixed_preview_result(
    result: &ApiResult<Vec<MixedTransferConflictInfo>>,
    route_hint: MixedRouteHint,
    source_count: usize,
    started: Instant,
) {
    let elapsed_ms = started.elapsed().as_millis() as u64;
    match result {
        Ok(conflicts) => info!(
            op = "mixed_conflict_preview",
            route = route_hint_label(route_hint),
            source_count,
            conflicts = conflicts.len(),
            elapsed_ms,
            "mixed conflict preview completed"
        ),
        Err(err) => warn!(
            op = "mixed_conflict_preview",
            route = route_hint_label(route_hint),
            source_count,
            elapsed_ms,
            error_code = %err.code,
            error_message = %err.message,
            "mixed conflict preview failed"
        ),
    }
}

fn log_mixed_execute_result(
    op: MixedTransferOp,
    result: &ApiResult<Vec<String>>,
    route_hint: MixedRouteHint,
    source_count: usize,
    started: Instant,
) {
    let elapsed_ms = started.elapsed().as_millis() as u64;
    let op_name = match op {
        MixedTransferOp::Copy => "mixed_write_copy",
        MixedTransferOp::Move => "mixed_write_move",
    };
    match result {
        Ok(created) => info!(
            op = op_name,
            route = route_hint_label(route_hint),
            source_count,
            outputs = created.len(),
            elapsed_ms,
            "mixed transfer completed"
        ),
        Err(err) => warn!(
            op = op_name,
            route = route_hint_label(route_hint),
            source_count,
            elapsed_ms,
            error_code = %err.code,
            error_message = %err.message,
            "mixed transfer failed"
        ),
    }
}

fn log_mixed_single_execute_result(
    op: MixedTransferOp,
    result: &ApiResult<String>,
    route_hint: MixedRouteHint,
    started: Instant,
) {
    let elapsed_ms = started.elapsed().as_millis() as u64;
    let op_name = match op {
        MixedTransferOp::Copy => "mixed_write_copy",
        MixedTransferOp::Move => "mixed_write_move",
    };
    match result {
        Ok(_) => info!(
            op = op_name,
            route = route_hint_label(route_hint),
            source_count = 1usize,
            outputs = 1usize,
            elapsed_ms,
            "mixed transfer completed"
        ),
        Err(err) => warn!(
            op = op_name,
            route = route_hint_label(route_hint),
            source_count = 1usize,
            elapsed_ms,
            error_code = %err.code,
            error_message = %err.message,
            "mixed transfer failed"
        ),
    }
}

async fn execute_mixed_entries(
    op: MixedTransferOp,
    sources: Vec<String>,
    dest_dir: String,
    options: MixedTransferWriteOptions,
    cancel_state: CancelState,
    progress_event: Option<String>,
) -> ApiResult<Vec<String>> {
    let started = Instant::now();
    let source_count = sources.len();
    let route_hint = mixed_route_hint(&sources, &dest_dir);
    let route = match validate_mixed_transfer_route(sources, dest_dir).await {
        Ok(route) => route,
        Err(err) => {
            let result = Err(err);
            log_mixed_execute_result(op, &result, route_hint, source_count, started);
            return result;
        }
    };
    let _cancel_guard = register_mixed_cancel(&cancel_state, &progress_event)?;
    let cancel_token = _cancel_guard.as_ref().map(|guard| guard.token());
    let task = tauri::async_runtime::spawn_blocking(move || {
        execute_mixed_entries_blocking(op, route, options, cancel_token)
    });
    let result = match task.await {
        Ok(result) => result,
        Err(error) => Err(api_err(
            "task_failed",
            format!("Mixed transfer task failed: {error}"),
        )),
    };
    log_mixed_execute_result(op, &result, route_hint, source_count, started);
    result
}

async fn execute_mixed_entry_to(
    op: MixedTransferOp,
    src: String,
    dst: String,
    options: MixedTransferWriteOptions,
    cancel_state: CancelState,
    progress_event: Option<String>,
) -> ApiResult<String> {
    let started = Instant::now();
    let route_hint = mixed_route_hint(std::slice::from_ref(&src), &dst);
    let pair = match validate_mixed_transfer_pair(src, dst).await {
        Ok(pair) => pair,
        Err(err) => {
            let result = Err(err);
            log_mixed_single_execute_result(op, &result, route_hint, started);
            return result;
        }
    };
    let _cancel_guard = register_mixed_cancel(&cancel_state, &progress_event)?;
    let cancel_token = _cancel_guard.as_ref().map(|guard| guard.token());
    let task = tauri::async_runtime::spawn_blocking(move || {
        execute_mixed_entry_to_blocking(op, pair, options, cancel_token)
    });
    let result = match task.await {
        Ok(result) => result,
        Err(error) => Err(api_err(
            "task_failed",
            format!("Mixed transfer task failed: {error}"),
        )),
    };
    log_mixed_single_execute_result(op, &result, route_hint, started);
    result
}

async fn validate_mixed_transfer_route(
    sources: Vec<String>,
    dest_dir: String,
) -> ApiResult<MixedTransferRoute> {
    if sources.is_empty() {
        return Err(api_err("invalid_input", "No sources provided"));
    }

    let dest_is_cloud = is_cloud_path(&dest_dir);
    let source_cloud_count = sources.iter().filter(|p| is_cloud_path(p)).count();
    if source_cloud_count > 0 && source_cloud_count < sources.len() {
        return Err(api_err(
            "unsupported",
            "Mixed local/cloud selection is not supported",
        ));
    }

    match (source_cloud_count == sources.len(), dest_is_cloud) {
        (false, true) => validate_local_to_cloud_route(sources, dest_dir).await,
        (true, false) => validate_cloud_to_local_route(sources, dest_dir).await,
        (false, false) => Err(api_err(
            "unsupported",
            "Use local clipboard paste for local-to-local transfers",
        )),
        (true, true) => Err(api_err(
            "unsupported",
            "Use cloud clipboard paste for cloud-to-cloud transfers",
        )),
    }
}

async fn validate_mixed_transfer_pair(src: String, dst: String) -> ApiResult<MixedTransferPair> {
    let src_is_cloud = is_cloud_path(&src);
    let dst_is_cloud = is_cloud_path(&dst);
    match (src_is_cloud, dst_is_cloud) {
        (false, true) => {
            let src_path = sanitize_path_follow(&src, true)
                .map_err(|e| api_err("invalid_path", e.to_string()))?;
            let src_meta = fs::symlink_metadata(&src_path)
                .map_err(|e| api_err("io_error", format!("Failed to read source metadata: {e}")))?;
            if src_meta.file_type().is_symlink() {
                return Err(api_err(
                    "symlink_unsupported",
                    "Symlinks are not supported for mixed local/cloud transfers yet",
                ));
            }

            let dst_path = CloudPath::parse(&dst).map_err(|e| {
                api_err(
                    "invalid_path",
                    format!("Invalid cloud destination path: {e}"),
                )
            })?;
            if dst_path.is_root() {
                return Err(api_err(
                    "invalid_path",
                    "Cloud destination path must include a file or folder name",
                ));
            }
            let Some(parent) = dst_path.parent_dir_path() else {
                return Err(api_err(
                    "invalid_path",
                    "Invalid cloud destination parent path",
                ));
            };
            if !parent.is_root() {
                match cloud::stat_cloud_entry(parent.to_string()).await? {
                    Some(entry) if matches!(entry.kind, CloudEntryKind::Dir) => {}
                    Some(_) => {
                        return Err(api_err(
                            "invalid_path",
                            "Cloud destination parent must be a directory",
                        ))
                    }
                    None => {
                        return Err(api_err(
                            "not_found",
                            "Cloud destination parent was not found",
                        ))
                    }
                }
            }
            Ok(MixedTransferPair {
                src: LocalOrCloudArg::Local(src_path),
                dst: LocalOrCloudArg::Cloud(dst_path.clone()),
                cloud_remote_for_error_mapping: Some(dst_path.remote().to_string()),
            })
        }
        (true, false) => {
            let src_path = CloudPath::parse(&src)
                .map_err(|e| api_err("invalid_path", format!("Invalid cloud source path: {e}")))?;
            match cloud::stat_cloud_entry(src_path.to_string()).await? {
                Some(entry) if matches!(entry.kind, CloudEntryKind::File | CloudEntryKind::Dir) => {
                }
                Some(_) => return Err(api_err("unsupported", "Unsupported cloud entry type")),
                None => return Err(api_err("not_found", "Cloud source was not found")),
            }

            let dst_path = sanitize_local_target_path_allow_missing(&dst)?;
            let parent = dst_path.parent().ok_or_else(|| {
                api_err(
                    "invalid_path",
                    "Local destination path must include a parent directory",
                )
            })?;
            let parent_meta = fs::symlink_metadata(parent).map_err(|e| {
                api_err(
                    "io_error",
                    format!("Failed to read destination parent metadata: {e}"),
                )
            })?;
            if !parent_meta.is_dir() {
                return Err(api_err(
                    "invalid_path",
                    "Local destination parent must be a directory",
                ));
            }

            Ok(MixedTransferPair {
                src: LocalOrCloudArg::Cloud(src_path.clone()),
                dst: LocalOrCloudArg::Local(dst_path),
                cloud_remote_for_error_mapping: Some(src_path.remote().to_string()),
            })
        }
        (false, false) => Err(api_err(
            "unsupported",
            "Use local clipboard paste for local-to-local transfers",
        )),
        (true, true) => Err(api_err(
            "unsupported",
            "Use cloud clipboard paste for cloud-to-cloud transfers",
        )),
    }
}

fn sanitize_local_target_path_allow_missing(raw: &str) -> ApiResult<PathBuf> {
    let pb = PathBuf::from(raw);
    let file_name = pb
        .file_name()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty() && *s != "." && *s != "..")
        .ok_or_else(|| {
            api_err(
                "invalid_path",
                "Local destination path must include a file or folder name",
            )
        })?;
    let parent_raw = pb.parent().ok_or_else(|| {
        api_err(
            "invalid_path",
            "Local destination path must include a parent directory",
        )
    })?;
    let parent = sanitize_path_follow(&parent_raw.to_string_lossy(), false)
        .map_err(|e| api_err("invalid_path", e.to_string()))?;
    Ok(parent.join(file_name))
}

async fn validate_local_to_cloud_route(
    sources: Vec<String>,
    dest_dir: String,
) -> ApiResult<MixedTransferRoute> {
    let dest = CloudPath::parse(&dest_dir).map_err(|e| {
        api_err(
            "invalid_path",
            format!("Invalid cloud destination path: {e}"),
        )
    })?;

    if !dest.is_root() {
        match cloud::stat_cloud_entry(dest.to_string()).await? {
            Some(entry) if matches!(entry.kind, CloudEntryKind::Dir) => {}
            Some(_) => {
                return Err(api_err(
                    "invalid_path",
                    "Cloud destination must be a directory",
                ))
            }
            None => {
                return Err(api_err(
                    "not_found",
                    "Cloud destination directory was not found",
                ))
            }
        }
    }

    let mut local_sources = Vec::with_capacity(sources.len());
    for raw in sources {
        let path =
            sanitize_path_follow(&raw, true).map_err(|e| api_err("invalid_path", e.to_string()))?;
        let meta = fs::symlink_metadata(&path)
            .map_err(|e| api_err("io_error", format!("Failed to read source metadata: {e}")))?;
        if meta.file_type().is_symlink() {
            return Err(api_err(
                "symlink_unsupported",
                "Symlinks are not supported for mixed local/cloud transfers yet",
            ));
        }
        local_sources.push(path);
    }

    Ok(MixedTransferRoute::LocalToCloud {
        sources: local_sources,
        dest_dir: dest,
    })
}

async fn validate_cloud_to_local_route(
    sources: Vec<String>,
    dest_dir: String,
) -> ApiResult<MixedTransferRoute> {
    let dest = sanitize_path_follow(&dest_dir, false)
        .map_err(|e| api_err("invalid_path", e.to_string()))?;
    let dest_meta = fs::symlink_metadata(&dest).map_err(|e| {
        api_err(
            "io_error",
            format!("Failed to read destination metadata: {e}"),
        )
    })?;
    if !dest_meta.is_dir() {
        return Err(api_err(
            "invalid_path",
            "Local destination must be a directory",
        ));
    }

    let mut cloud_sources = Vec::with_capacity(sources.len());
    for raw in sources {
        let path = CloudPath::parse(&raw)
            .map_err(|e| api_err("invalid_path", format!("Invalid cloud source path: {e}")))?;
        match cloud::stat_cloud_entry(path.to_string()).await? {
            Some(entry) if matches!(entry.kind, CloudEntryKind::File | CloudEntryKind::Dir) => {
                cloud_sources.push(path)
            }
            None => return Err(api_err("not_found", "Cloud source was not found")),
            Some(_) => return Err(api_err("unsupported", "Unsupported cloud entry type")),
        }
    }

    Ok(MixedTransferRoute::CloudToLocal {
        sources: cloud_sources,
        dest_dir: dest,
    })
}

fn execute_mixed_entries_blocking(
    op: MixedTransferOp,
    route: MixedTransferRoute,
    options: MixedTransferWriteOptions,
    cancel: Option<Arc<AtomicBool>>,
) -> ApiResult<Vec<String>> {
    let cli = RcloneCli::default();
    execute_mixed_entries_blocking_with_cli(&cli, op, route, options, cancel)
}

fn execute_mixed_entry_to_blocking(
    op: MixedTransferOp,
    pair: MixedTransferPair,
    options: MixedTransferWriteOptions,
    cancel: Option<Arc<AtomicBool>>,
) -> ApiResult<String> {
    let cli = RcloneCli::default();
    execute_mixed_entry_to_blocking_with_cli(&cli, op, pair, options, cancel)
}

fn execute_mixed_entry_to_blocking_with_cli(
    cli: &RcloneCli,
    op: MixedTransferOp,
    pair: MixedTransferPair,
    options: MixedTransferWriteOptions,
    cancel: Option<Arc<AtomicBool>>,
) -> ApiResult<String> {
    if transfer_cancelled(cancel.as_deref()) {
        return Err(api_err("cancelled", "Transfer cancelled"));
    }
    let MixedTransferPair {
        src,
        dst,
        cloud_remote_for_error_mapping,
    } = pair;
    let out = match &dst {
        LocalOrCloudArg::Local(path) => path.to_string_lossy().to_string(),
        LocalOrCloudArg::Cloud(path) => path.to_string(),
    };
    execute_rclone_transfer(
        cli,
        op,
        src,
        dst,
        options,
        cloud_remote_for_error_mapping.as_deref(),
        cancel.as_deref(),
    )?;
    Ok(out)
}

fn execute_mixed_entries_blocking_with_cli(
    cli: &RcloneCli,
    op: MixedTransferOp,
    route: MixedTransferRoute,
    options: MixedTransferWriteOptions,
    cancel: Option<Arc<AtomicBool>>,
) -> ApiResult<Vec<String>> {
    let mut created = Vec::new();
    match route {
        MixedTransferRoute::LocalToCloud { sources, dest_dir } => {
            for src in sources {
                if transfer_cancelled(cancel.as_deref()) {
                    return Err(api_err("cancelled", "Transfer cancelled"));
                }
                let leaf = local_leaf_name(&src)?;
                let target = dest_dir.child_path(leaf).map_err(|e| {
                    api_err("invalid_path", format!("Invalid cloud target path: {e}"))
                })?;
                execute_rclone_transfer(
                    cli,
                    op,
                    LocalOrCloudArg::Local(src.clone()),
                    LocalOrCloudArg::Cloud(target.clone()),
                    options,
                    Some(dest_dir.remote()),
                    cancel.as_deref(),
                )?;
                created.push(target.to_string());
            }
        }
        MixedTransferRoute::CloudToLocal { sources, dest_dir } => {
            for src in sources {
                if transfer_cancelled(cancel.as_deref()) {
                    return Err(api_err("cancelled", "Transfer cancelled"));
                }
                let leaf = src.leaf_name().map_err(|e| {
                    api_err("invalid_path", format!("Invalid cloud source path: {e}"))
                })?;
                let target = dest_dir.join(leaf);
                execute_rclone_transfer(
                    cli,
                    op,
                    LocalOrCloudArg::Cloud(src.clone()),
                    LocalOrCloudArg::Local(target.clone()),
                    options,
                    Some(src.remote()),
                    cancel.as_deref(),
                )?;
                created.push(target.to_string_lossy().to_string());
            }
        }
    }
    Ok(created)
}

#[derive(Debug, Clone)]
enum LocalOrCloudArg {
    Local(PathBuf),
    Cloud(CloudPath),
}

impl LocalOrCloudArg {
    fn to_os_arg(&self) -> OsString {
        match self {
            Self::Local(path) => path.as_os_str().to_os_string(),
            Self::Cloud(path) => OsString::from(path.to_rclone_remote_spec()),
        }
    }

    fn local_path(&self) -> Option<&Path> {
        match self {
            Self::Local(path) => Some(path.as_path()),
            Self::Cloud(_) => None,
        }
    }

    fn cloud_path(&self) -> Option<&CloudPath> {
        match self {
            Self::Cloud(path) => Some(path),
            Self::Local(_) => None,
        }
    }
}

fn execute_rclone_transfer(
    cli: &RcloneCli,
    op: MixedTransferOp,
    src: LocalOrCloudArg,
    dst: LocalOrCloudArg,
    options: MixedTransferWriteOptions,
    cloud_remote_for_error_mapping: Option<&str>,
    cancel: Option<&AtomicBool>,
) -> ApiResult<()> {
    if transfer_cancelled(cancel) {
        return Err(api_err("cancelled", "Transfer cancelled"));
    }
    if !options.overwrite
        && !options.prechecked
        && mixed_target_exists(cli, &dst, cloud_remote_for_error_mapping, cancel)?
    {
        return Err(api_err(
            "destination_exists",
            "A file or folder with the same name already exists",
        ));
    }

    let subcommand = match op {
        MixedTransferOp::Copy => RcloneSubcommand::CopyTo,
        MixedTransferOp::Move => RcloneSubcommand::MoveTo,
    };

    let spec = RcloneCommandSpec::new(subcommand)
        .arg(src.to_os_arg())
        .arg(dst.to_os_arg());

    cli.run_capture_text_with_cancel(spec, cancel)
        .map_err(|error| map_rclone_cli_error(error, cloud_remote_for_error_mapping))?;
    Ok(())
}

fn mixed_target_exists(
    cli: &RcloneCli,
    dst: &LocalOrCloudArg,
    cloud_remote_for_error_mapping: Option<&str>,
    cancel: Option<&AtomicBool>,
) -> ApiResult<bool> {
    if transfer_cancelled(cancel) {
        return Err(api_err("cancelled", "Transfer cancelled"));
    }
    if let Some(path) = dst.local_path() {
        return match fs::symlink_metadata(path) {
            Ok(_) => Ok(true),
            Err(e) if e.kind() == ErrorKind::NotFound => Ok(false),
            Err(e) => Err(api_err(
                "io_error",
                format!("Failed to read destination metadata: {e}"),
            )),
        };
    }

    let Some(cloud_path) = dst.cloud_path() else {
        return Ok(false);
    };
    let spec = RcloneCommandSpec::new(RcloneSubcommand::LsJson)
        .arg("--stat")
        .arg(cloud_path.to_rclone_remote_spec());
    match cli.run_capture_text_with_cancel(spec, cancel) {
        Ok(_) => Ok(true),
        Err(RcloneCliError::NonZero { stderr, stdout, .. })
            if is_rclone_not_found_text(&stderr, &stdout) =>
        {
            Ok(false)
        }
        Err(error) => Err(map_rclone_cli_error(error, cloud_remote_for_error_mapping)),
    }
}

fn map_rclone_cli_error(error: RcloneCliError, cloud_remote: Option<&str>) -> ApiError {
    match error {
        RcloneCliError::Io(io) if io.kind() == std::io::ErrorKind::NotFound => {
            api_err("binary_missing", "rclone not found in PATH")
        }
        RcloneCliError::Io(io) => api_err("network_error", format!("Failed to run rclone: {io}")),
        RcloneCliError::Shutdown { .. } => api_err(
            "task_failed",
            "Application is shutting down; transfer was cancelled",
        ),
        RcloneCliError::Cancelled { .. } => api_err("cancelled", "Transfer cancelled"),
        RcloneCliError::AsyncJobStateUnknown {
            operation,
            job_id,
            reason,
            ..
        } => api_err(
            "task_failed",
            format!(
                "Transfer status is unknown after rclone rc {operation} job {job_id}; Browsey did not retry automatically to avoid duplicate operations. Refresh and verify destination state before retrying. Cause: {}",
                reason.trim()
            ),
        ),
        RcloneCliError::Timeout {
            subcommand,
            timeout,
            ..
        } => api_err(
            "timeout",
            format!(
                "rclone {} timed out after {}s",
                subcommand.as_str(),
                timeout.as_secs()
            ),
        ),
        RcloneCliError::NonZero { stderr, stdout, .. } => {
            let msg_ref = if !stderr.trim().is_empty() {
                stderr.as_str()
            } else {
                stdout.as_str()
            };
            let lower = msg_ref.to_ascii_lowercase();
            let not_found = is_rclone_not_found_text(&stderr, &stdout);
            let provider = cloud_remote.and_then(cloud::cloud_provider_kind_for_remote);
            let provider_code = provider_specific_rclone_code(provider, &lower);
            let code = if lower.contains("quota exceeded")
                || lower.contains("rate_limit_exceeded")
                || lower.contains("too many requests")
            {
                "rate_limited"
            } else if lower.contains("unauthorized")
                || lower.contains("invalid_grant")
                || lower.contains("token") && lower.contains("expired")
            {
                "auth_required"
            } else if lower.contains("permission denied") || lower.contains("access denied") {
                "permission_denied"
            } else if lower.contains("already exists")
                || lower.contains("destination exists")
                || lower.contains("file exists")
            {
                "destination_exists"
            } else if not_found {
                "not_found"
            } else if lower.contains("x509") || lower.contains("certificate") {
                "tls_certificate_error"
            } else {
                provider_code.unwrap_or("unknown_error")
            };
            api_err(code, msg_ref.trim())
        }
    }
}

fn register_mixed_cancel(
    cancel_state: &CancelState,
    progress_event: &Option<String>,
) -> ApiResult<Option<CancelGuard>> {
    progress_event
        .as_ref()
        .map(|event| cancel_state.register(event.clone()))
        .transpose()
        .map_err(|error| {
            api_err(
                "task_failed",
                format!("Failed to register cancel token: {error}"),
            )
        })
}

fn transfer_cancelled(cancel: Option<&AtomicBool>) -> bool {
    cancel
        .map(|token| token.load(Ordering::SeqCst))
        .unwrap_or(false)
}

fn provider_specific_rclone_code(
    provider: Option<CloudProviderKind>,
    lower_message: &str,
) -> Option<&'static str> {
    match provider {
        Some(CloudProviderKind::Onedrive) => {
            if lower_message.contains("activitylimitreached") {
                return Some("rate_limited");
            }
            None
        }
        Some(CloudProviderKind::Gdrive) => {
            if lower_message.contains("userratelimitexceeded")
                || lower_message.contains("ratelimitexceeded")
            {
                return Some("rate_limited");
            }
            None
        }
        Some(CloudProviderKind::Nextcloud) | None => None,
    }
}

fn is_rclone_not_found_text(stderr: &str, stdout: &str) -> bool {
    let combined = if !stderr.trim().is_empty() {
        stderr
    } else {
        stdout
    };
    let lower = combined.to_lowercase();
    lower.contains("not found")
        || lower.contains("object not found")
        || lower.contains("directory not found")
        || lower.contains("file not found")
        || lower.contains("404")
}

async fn preview_local_to_cloud_conflicts(
    sources: Vec<String>,
    dest_dir: String,
) -> ApiResult<Vec<MixedTransferConflictInfo>> {
    let dest = CloudPath::parse(&dest_dir).map_err(|e| {
        api_err(
            "invalid_path",
            format!("Invalid cloud destination path: {e}"),
        )
    })?;

    let local_sources = sources
        .into_iter()
        .map(|raw| {
            sanitize_path_follow(&raw, true).map_err(|e| api_err("invalid_path", e.to_string()))
        })
        .collect::<ApiResult<Vec<PathBuf>>>()?;

    let provider = cloud::cloud_provider_kind_for_remote(dest.remote());
    let dest_entries = cloud::list_cloud_entries(dest.to_string()).await?;
    build_local_to_cloud_conflicts_from_entries(local_sources, &dest, provider, &dest_entries)
}

fn build_local_to_cloud_conflicts_from_entries(
    local_sources: Vec<PathBuf>,
    dest: &CloudPath,
    provider: Option<CloudProviderKind>,
    dest_entries: &[crate::commands::cloud::types::CloudEntry],
) -> ApiResult<Vec<MixedTransferConflictInfo>> {
    let mut name_to_is_dir: HashMap<String, bool> = HashMap::with_capacity(dest_entries.len());
    for entry in dest_entries {
        let key = cloud::cloud_conflict_name_key(provider, &entry.name);
        name_to_is_dir
            .entry(key)
            .or_insert(matches!(entry.kind, CloudEntryKind::Dir));
    }

    let mut conflicts = Vec::new();
    for src in local_sources {
        let name = local_leaf_name(&src)?;
        let key = cloud::cloud_conflict_name_key(provider, name);
        let Some(is_dir) = name_to_is_dir.get(&key).copied() else {
            continue;
        };
        let target = dest
            .child_path(name)
            .map_err(|e| api_err("invalid_path", format!("Invalid cloud target path: {e}")))?;
        conflicts.push(MixedTransferConflictInfo {
            src: src.to_string_lossy().to_string(),
            target: target.to_string(),
            exists: true,
            is_dir,
        });
    }

    Ok(conflicts)
}

fn preview_cloud_to_local_conflicts(
    sources: Vec<String>,
    dest_dir: String,
) -> ApiResult<Vec<MixedTransferConflictInfo>> {
    let dest = sanitize_path_follow(&dest_dir, false)
        .map_err(|e| api_err("invalid_path", e.to_string()))?;
    let cloud_sources = sources
        .into_iter()
        .map(|raw| {
            CloudPath::parse(&raw)
                .map_err(|e| api_err("invalid_path", format!("Invalid cloud source path: {e}")))
        })
        .collect::<ApiResult<Vec<CloudPath>>>()?;

    let dest_entries = list_local_dir_entries(&dest)?;
    let mut name_to_is_dir: HashMap<String, bool> = HashMap::with_capacity(dest_entries.len());
    for (name, is_dir) in dest_entries {
        name_to_is_dir.entry(name).or_insert(is_dir);
    }

    let mut conflicts = Vec::new();
    for src in cloud_sources {
        let name = src
            .leaf_name()
            .map_err(|e| api_err("invalid_path", format!("Invalid cloud source path: {e}")))?;
        let Some(is_dir) = name_to_is_dir.get(name).copied() else {
            continue;
        };
        let target = dest.join(name);
        conflicts.push(MixedTransferConflictInfo {
            src: src.to_string(),
            target: target.to_string_lossy().to_string(),
            exists: true,
            is_dir,
        });
    }

    Ok(conflicts)
}

fn local_leaf_name(path: &Path) -> ApiResult<&str> {
    path.file_name()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            api_err(
                "invalid_path",
                "Local source path does not have a file name",
            )
        })
}

fn list_local_dir_entries(dest: &Path) -> ApiResult<Vec<(String, bool)>> {
    let mut out = Vec::new();
    let rd = fs::read_dir(dest).map_err(|e| {
        api_err(
            "io_error",
            format!("Failed to read destination directory: {e}"),
        )
    })?;
    for item in rd {
        let item =
            item.map_err(|e| api_err("io_error", format!("Failed to read directory entry: {e}")))?;
        let name = item.file_name();
        let Some(name) = name.to_str().map(|s| s.to_string()) else {
            continue;
        };
        let is_dir = match fs::symlink_metadata(item.path()) {
            Ok(meta) => meta.file_type().is_dir(),
            Err(e) if e.kind() == ErrorKind::NotFound => continue,
            Err(e) => {
                return Err(api_err(
                    "io_error",
                    format!("Failed to read destination entry metadata: {e}"),
                ))
            }
        };
        out.push((name, is_dir));
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::cloud::types::{CloudCapabilities, CloudEntry};
    #[cfg(unix)]
    use std::sync::atomic::{AtomicU64, Ordering};
    #[cfg(unix)]
    use std::sync::Mutex;
    #[cfg(unix)]
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn local_leaf_name_rejects_root_like_path() {
        let err = local_leaf_name(Path::new("/")).expect_err("should fail");
        assert_eq!(err.code, "invalid_path");
    }

    #[test]
    fn local_dir_entries_lists_files_and_dirs() {
        let base = std::env::temp_dir().join(format!(
            "browsey-transfer-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("epoch")
                .as_nanos()
        ));
        fs::create_dir(&base).expect("create temp base");
        let file_path = base.join("a.txt");
        let dir_path = base.join("folder");
        fs::write(&file_path, b"x").expect("write");
        fs::create_dir(&dir_path).expect("mkdir");

        let mut rows = list_local_dir_entries(&base).expect("list");
        rows.sort_by(|a, b| a.0.cmp(&b.0));

        assert_eq!(rows, vec![("a.txt".into(), false), ("folder".into(), true)]);

        fs::remove_file(&file_path).ok();
        fs::remove_dir(&dir_path).ok();
        fs::remove_dir(&base).ok();
    }

    #[test]
    fn sanitize_local_target_path_allow_missing_accepts_missing_leaf_when_parent_exists() {
        let base = std::env::temp_dir().join(format!(
            "browsey-transfer-target-sanitize-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("epoch")
                .as_nanos()
        ));
        fs::create_dir_all(&base).expect("create temp dir");
        let target = base.join("new-file.txt");

        let out = sanitize_local_target_path_allow_missing(&target.to_string_lossy())
            .expect("should allow missing leaf");
        assert_eq!(out, target);

        fs::remove_dir_all(&base).ok();
    }

    fn cloud_entry(name: &str, kind: CloudEntryKind) -> CloudEntry {
        let provider = CloudProviderKind::Onedrive;
        CloudEntry {
            name: name.to_string(),
            path: format!("rclone://work/{name}"),
            kind,
            size: None,
            modified: None,
            capabilities: CloudCapabilities::v1_for_provider(provider),
        }
    }

    #[cfg(unix)]
    struct FakeRcloneSandbox {
        root: PathBuf,
        script_path: PathBuf,
        state_root: PathBuf,
        local_root: PathBuf,
    }

    #[cfg(unix)]
    impl FakeRcloneSandbox {
        fn new() -> Self {
            static NEXT_ID: AtomicU64 = AtomicU64::new(1);
            let unique = format!(
                "browsey-transfer-fake-rclone-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .expect("time")
                    .as_nanos()
                    + u128::from(NEXT_ID.fetch_add(1, Ordering::Relaxed))
            );
            let root = std::env::temp_dir().join(unique);
            let state_root = root.join("state");
            let local_root = root.join("local");
            let script_path = root.join("rclone");
            fs::create_dir_all(&state_root).expect("create state root");
            fs::create_dir_all(&local_root).expect("create local root");
            let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/support/fake-rclone.sh");
            fs::copy(&source, &script_path).expect("copy fake rclone script");
            let mut perms = fs::metadata(&script_path)
                .expect("script metadata")
                .permissions();
            use std::os::unix::fs::PermissionsExt;
            perms.set_mode(0o755);
            fs::set_permissions(&script_path, perms).expect("chmod fake rclone");
            Self {
                root,
                script_path,
                state_root,
                local_root,
            }
        }

        fn cli(&self) -> RcloneCli {
            RcloneCli::new(self.script_path.as_os_str())
        }

        fn cloud_path(&self, raw: &str) -> CloudPath {
            CloudPath::parse(raw).expect("valid cloud path")
        }

        fn remote_path(&self, remote: &str, rel: &str) -> PathBuf {
            let base = self.state_root.join(remote);
            if rel.is_empty() {
                base
            } else {
                base.join(rel)
            }
        }

        fn local_path(&self, rel: &str) -> PathBuf {
            self.local_root.join(rel)
        }

        fn mkdir_remote(&self, remote: &str, rel: &str) {
            fs::create_dir_all(self.remote_path(remote, rel)).expect("mkdir remote");
        }

        fn write_remote_file(&self, remote: &str, rel: &str, content: &str) {
            let path = self.remote_path(remote, rel);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("mkdir remote parent");
            }
            fs::write(path, content).expect("write remote file");
        }

        fn write_local_file(&self, rel: &str, content: &str) -> PathBuf {
            let path = self.local_path(rel);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("mkdir local parent");
            }
            fs::write(&path, content).expect("write local file");
            path
        }
    }

    #[cfg(unix)]
    impl Drop for FakeRcloneSandbox {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[cfg(unix)]
    fn fake_rclone_test_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: Mutex<()> = Mutex::new(());
        LOCK.lock().expect("lock fake rclone test")
    }

    #[cfg(unix)]
    #[test]
    fn mixed_execute_local_to_cloud_file_copy_and_move_via_fake_rclone() {
        let _guard = fake_rclone_test_lock();
        let sandbox = FakeRcloneSandbox::new();
        sandbox.mkdir_remote("work", "dest");
        let cli = sandbox.cli();

        let copy_src = sandbox.write_local_file("src/copy.txt", "copy-payload");
        let copy_route = MixedTransferRoute::LocalToCloud {
            sources: vec![copy_src.clone()],
            dest_dir: sandbox.cloud_path("rclone://work/dest"),
        };
        let copy_out = execute_mixed_entries_blocking_with_cli(
            &cli,
            MixedTransferOp::Copy,
            copy_route,
            MixedTransferWriteOptions {
                overwrite: false,
                prechecked: true,
            },
            None,
        )
        .expect("copy local->cloud");
        assert_eq!(copy_out, vec!["rclone://work/dest/copy.txt".to_string()]);
        assert!(copy_src.exists(), "copy should preserve local source");
        assert_eq!(
            fs::read_to_string(sandbox.remote_path("work", "dest/copy.txt")).expect("read remote"),
            "copy-payload"
        );

        let move_src = sandbox.write_local_file("src/move.txt", "move-payload");
        let move_route = MixedTransferRoute::LocalToCloud {
            sources: vec![move_src.clone()],
            dest_dir: sandbox.cloud_path("rclone://work/dest"),
        };
        let move_out = execute_mixed_entries_blocking_with_cli(
            &cli,
            MixedTransferOp::Move,
            move_route,
            MixedTransferWriteOptions {
                overwrite: false,
                prechecked: true,
            },
            None,
        )
        .expect("move local->cloud");
        assert_eq!(move_out, vec!["rclone://work/dest/move.txt".to_string()]);
        assert!(!move_src.exists(), "move should remove local source");
        assert_eq!(
            fs::read_to_string(sandbox.remote_path("work", "dest/move.txt")).expect("read remote"),
            "move-payload"
        );
    }

    #[cfg(unix)]
    #[test]
    fn mixed_execute_local_to_cloud_directory_copy_and_move_via_fake_rclone() {
        let _guard = fake_rclone_test_lock();
        let sandbox = FakeRcloneSandbox::new();
        sandbox.mkdir_remote("work", "dest");
        let cli = sandbox.cli();

        let copy_dir = sandbox.local_path("src/folder-copy");
        fs::create_dir_all(copy_dir.join("nested")).expect("mkdir local copy dir");
        fs::write(copy_dir.join("nested/file.txt"), b"copy-dir").expect("write local nested");
        let copy_route = MixedTransferRoute::LocalToCloud {
            sources: vec![copy_dir.clone()],
            dest_dir: sandbox.cloud_path("rclone://work/dest"),
        };
        let copy_out = execute_mixed_entries_blocking_with_cli(
            &cli,
            MixedTransferOp::Copy,
            copy_route,
            MixedTransferWriteOptions {
                overwrite: false,
                prechecked: true,
            },
            None,
        )
        .expect("copy dir local->cloud");
        assert_eq!(copy_out, vec!["rclone://work/dest/folder-copy".to_string()]);
        assert!(copy_dir.exists(), "copy should preserve local source dir");
        assert_eq!(
            fs::read_to_string(sandbox.remote_path("work", "dest/folder-copy/nested/file.txt"))
                .expect("read remote nested"),
            "copy-dir"
        );

        let move_dir = sandbox.local_path("src/folder-move");
        fs::create_dir_all(move_dir.join("nested")).expect("mkdir local move dir");
        fs::write(move_dir.join("nested/file.txt"), b"move-dir").expect("write local nested move");
        let move_route = MixedTransferRoute::LocalToCloud {
            sources: vec![move_dir.clone()],
            dest_dir: sandbox.cloud_path("rclone://work/dest"),
        };
        let move_out = execute_mixed_entries_blocking_with_cli(
            &cli,
            MixedTransferOp::Move,
            move_route,
            MixedTransferWriteOptions {
                overwrite: false,
                prechecked: true,
            },
            None,
        )
        .expect("move dir local->cloud");
        assert_eq!(move_out, vec!["rclone://work/dest/folder-move".to_string()]);
        assert!(!move_dir.exists(), "move should remove local source dir");
        assert_eq!(
            fs::read_to_string(sandbox.remote_path("work", "dest/folder-move/nested/file.txt"))
                .expect("read moved remote nested"),
            "move-dir"
        );
    }

    #[cfg(unix)]
    #[test]
    fn mixed_execute_cloud_to_local_file_copy_and_move_via_fake_rclone() {
        let _guard = fake_rclone_test_lock();
        let sandbox = FakeRcloneSandbox::new();
        sandbox.write_remote_file("work", "src/copy.txt", "copy-payload");
        sandbox.write_remote_file("work", "src/move.txt", "move-payload");
        let cli = sandbox.cli();
        let local_dest = sandbox.local_path("dest");
        fs::create_dir_all(&local_dest).expect("mkdir local dest");

        let copy_route = MixedTransferRoute::CloudToLocal {
            sources: vec![sandbox.cloud_path("rclone://work/src/copy.txt")],
            dest_dir: local_dest.clone(),
        };
        let copy_out = execute_mixed_entries_blocking_with_cli(
            &cli,
            MixedTransferOp::Copy,
            copy_route,
            MixedTransferWriteOptions {
                overwrite: false,
                prechecked: true,
            },
            None,
        )
        .expect("copy cloud->local");
        assert_eq!(
            copy_out,
            vec![local_dest.join("copy.txt").to_string_lossy().to_string()]
        );
        assert_eq!(
            fs::read_to_string(local_dest.join("copy.txt")).expect("read local copy"),
            "copy-payload"
        );
        assert!(
            sandbox.remote_path("work", "src/copy.txt").exists(),
            "copy should preserve remote source"
        );

        let move_route = MixedTransferRoute::CloudToLocal {
            sources: vec![sandbox.cloud_path("rclone://work/src/move.txt")],
            dest_dir: local_dest.clone(),
        };
        let move_out = execute_mixed_entries_blocking_with_cli(
            &cli,
            MixedTransferOp::Move,
            move_route,
            MixedTransferWriteOptions {
                overwrite: false,
                prechecked: true,
            },
            None,
        )
        .expect("move cloud->local");
        assert_eq!(
            move_out,
            vec![local_dest.join("move.txt").to_string_lossy().to_string()]
        );
        assert_eq!(
            fs::read_to_string(local_dest.join("move.txt")).expect("read local move"),
            "move-payload"
        );
        assert!(
            !sandbox.remote_path("work", "src/move.txt").exists(),
            "move should remove remote source"
        );
    }

    #[test]
    fn validate_local_to_cloud_route_allows_directory_source() {
        let base = std::env::temp_dir().join(format!(
            "browsey-transfer-dir-reject-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("epoch")
                .as_nanos()
        ));
        let src_dir = base.join("folder");
        fs::create_dir_all(&src_dir).expect("create dir source");
        let result = tauri::async_runtime::block_on(validate_local_to_cloud_route(
            vec![src_dir.to_string_lossy().to_string()],
            "rclone://work".to_string(),
        ));
        let route = result.expect("directory mixed local->cloud should be accepted");
        match route {
            MixedTransferRoute::LocalToCloud { sources, .. } => {
                assert_eq!(sources, vec![src_dir.clone()]);
            }
            _ => panic!("expected local->cloud route"),
        }
        fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn mixed_preview_local_to_cloud_matches_onedrive_case_insensitive_and_preserves_kind() {
        let base = std::env::temp_dir().join(format!(
            "browsey-transfer-preview-local-cloud-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("epoch")
                .as_nanos()
        ));
        fs::create_dir_all(&base).expect("create temp base");
        let local_file = base.join("Report.txt");
        let local_dir = base.join("FolderA");
        fs::write(&local_file, b"x").expect("write local file");
        fs::create_dir_all(&local_dir).expect("create local dir");

        let dest = CloudPath::parse("rclone://work/dest").expect("cloud path");
        let conflicts = build_local_to_cloud_conflicts_from_entries(
            vec![local_file.clone(), local_dir.clone()],
            &dest,
            Some(CloudProviderKind::Onedrive),
            &[
                cloud_entry("report.txt", CloudEntryKind::File),
                cloud_entry("FolderA", CloudEntryKind::Dir),
            ],
        )
        .expect("build conflicts");

        assert_eq!(conflicts.len(), 2);
        assert!(conflicts
            .iter()
            .any(|c| c.src == local_file.to_string_lossy() && !c.is_dir));
        assert!(conflicts
            .iter()
            .any(|c| c.src == local_dir.to_string_lossy() && c.is_dir));

        fs::remove_file(&local_file).ok();
        fs::remove_dir_all(&local_dir).ok();
        fs::remove_dir_all(&base).ok();
    }

    #[test]
    fn mixed_preview_cloud_to_local_reports_file_and_dir_conflicts() {
        let base = std::env::temp_dir().join(format!(
            "browsey-transfer-preview-cloud-local-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("epoch")
                .as_nanos()
        ));
        fs::create_dir_all(&base).expect("create temp base");
        let local_file = base.join("report.txt");
        let local_dir = base.join("FolderA");
        fs::write(&local_file, b"x").expect("write local file");
        fs::create_dir_all(&local_dir).expect("mkdir local dir");

        let conflicts = preview_cloud_to_local_conflicts(
            vec![
                "rclone://work/src/report.txt".to_string(),
                "rclone://work/src/FolderA".to_string(),
                "rclone://work/src/other.txt".to_string(),
            ],
            base.to_string_lossy().to_string(),
        )
        .expect("preview cloud->local conflicts");

        assert_eq!(conflicts.len(), 2);
        assert!(conflicts
            .iter()
            .any(|c| c.src.ends_with("/report.txt") && !c.is_dir));
        assert!(conflicts
            .iter()
            .any(|c| c.src.ends_with("/FolderA") && c.is_dir));

        fs::remove_file(&local_file).ok();
        fs::remove_dir_all(&local_dir).ok();
        fs::remove_dir_all(&base).ok();
    }

    #[cfg(unix)]
    #[test]
    fn mixed_execute_cloud_to_local_directory_copy_and_move_via_fake_rclone() {
        let _guard = fake_rclone_test_lock();
        let sandbox = FakeRcloneSandbox::new();
        sandbox.write_remote_file("work", "src/folder-copy/nested/file.txt", "copy-dir");
        sandbox.write_remote_file("work", "src/folder-move/nested/file.txt", "move-dir");
        let cli = sandbox.cli();
        let local_dest = sandbox.local_path("dest");
        fs::create_dir_all(&local_dest).expect("mkdir local dest");

        let copy_route = MixedTransferRoute::CloudToLocal {
            sources: vec![sandbox.cloud_path("rclone://work/src/folder-copy")],
            dest_dir: local_dest.clone(),
        };
        let copy_out = execute_mixed_entries_blocking_with_cli(
            &cli,
            MixedTransferOp::Copy,
            copy_route,
            MixedTransferWriteOptions {
                overwrite: false,
                prechecked: true,
            },
            None,
        )
        .expect("copy dir cloud->local");
        assert_eq!(
            copy_out,
            vec![local_dest.join("folder-copy").to_string_lossy().to_string()]
        );
        assert_eq!(
            fs::read_to_string(local_dest.join("folder-copy/nested/file.txt"))
                .expect("read local copied dir"),
            "copy-dir"
        );
        assert!(
            sandbox.remote_path("work", "src/folder-copy").exists(),
            "copy should preserve remote source dir"
        );

        let move_route = MixedTransferRoute::CloudToLocal {
            sources: vec![sandbox.cloud_path("rclone://work/src/folder-move")],
            dest_dir: local_dest.clone(),
        };
        let move_out = execute_mixed_entries_blocking_with_cli(
            &cli,
            MixedTransferOp::Move,
            move_route,
            MixedTransferWriteOptions {
                overwrite: false,
                prechecked: true,
            },
            None,
        )
        .expect("move dir cloud->local");
        assert_eq!(
            move_out,
            vec![local_dest.join("folder-move").to_string_lossy().to_string()]
        );
        assert_eq!(
            fs::read_to_string(local_dest.join("folder-move/nested/file.txt"))
                .expect("read local moved dir"),
            "move-dir"
        );
        assert!(
            !sandbox.remote_path("work", "src/folder-move").exists(),
            "move should remove remote source dir"
        );
    }

    #[test]
    fn provider_specific_error_mapping_handles_onedrive_activity_limit() {
        assert_eq!(
            provider_specific_rclone_code(
                Some(CloudProviderKind::Onedrive),
                "activitylimitreached"
            ),
            Some("rate_limited")
        );
        assert_eq!(
            provider_specific_rclone_code(Some(CloudProviderKind::Gdrive), "userratelimitexceeded"),
            Some("rate_limited")
        );
        assert_eq!(
            provider_specific_rclone_code(
                Some(CloudProviderKind::Nextcloud),
                "activitylimitreached"
            ),
            None
        );
    }
}
