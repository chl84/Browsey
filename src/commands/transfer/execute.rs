use super::error::{transfer_err, transfer_err_code as api_err, TransferErrorCode, TransferResult};
use super::logging::{log_mixed_execute_result, log_mixed_single_execute_result};
use super::route::{
    local_leaf_name, mixed_route_hint, validate_mixed_transfer_pair, validate_mixed_transfer_route,
    LocalOrCloudArg, MixedTransferPair, MixedTransferRoute,
};
use super::{MixedTransferOp, MixedTransferWriteOptions};
use crate::commands::cloud;
use crate::commands::cloud::path::CloudPath;
use crate::commands::cloud::provider::CloudProvider;
use crate::commands::cloud::providers::rclone::RcloneCloudProvider;
use crate::commands::cloud::rclone_cli::{
    RcloneCli, RcloneCliError, RcloneCommandSpec, RcloneSubcommand,
};
use crate::commands::cloud::types::{CloudEntryKind, CloudProviderKind};
use crate::errors::domain::DomainError;
use crate::runtime_lifecycle;
use crate::tasks::{CancelGuard, CancelState};
use serde::Serialize;
use std::fs;
use std::io::ErrorKind;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::Instant;

#[derive(Clone)]
struct TransferProgressContext {
    app: tauri::AppHandle,
    event_name: String,
}

#[derive(Serialize, Clone)]
struct TransferProgressPayload {
    bytes: u64,
    total: u64,
    finished: bool,
}

struct CloudToLocalBatchProgressPlan {
    total_bytes: u64,
    file_sizes: Vec<u64>,
}

pub(super) async fn execute_mixed_entries(
    op: MixedTransferOp,
    sources: Vec<String>,
    dest_dir: String,
    app: tauri::AppHandle,
    options: MixedTransferWriteOptions,
    cancel_state: CancelState,
    progress_event: Option<String>,
) -> TransferResult<Vec<String>> {
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
    let cancel_guard = register_mixed_cancel(&cancel_state, &progress_event)?;
    let cancel_token = cancel_guard.as_ref().map(|guard| guard.token());
    let progress = progress_event
        .clone()
        .map(|event_name| TransferProgressContext { app, event_name });
    let task = tauri::async_runtime::spawn_blocking(move || {
        execute_mixed_entries_blocking(op, route, options, cancel_token, progress)
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

pub(super) async fn execute_mixed_entry_to(
    op: MixedTransferOp,
    src: String,
    dst: String,
    app: tauri::AppHandle,
    options: MixedTransferWriteOptions,
    cancel_state: CancelState,
    progress_event: Option<String>,
) -> TransferResult<String> {
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
    let cancel_guard = register_mixed_cancel(&cancel_state, &progress_event)?;
    let cancel_token = cancel_guard.as_ref().map(|guard| guard.token());
    let progress = progress_event
        .clone()
        .map(|event_name| TransferProgressContext { app, event_name });
    let task = tauri::async_runtime::spawn_blocking(move || {
        execute_mixed_entry_to_blocking(op, pair, options, cancel_token, progress)
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

fn execute_mixed_entries_blocking(
    op: MixedTransferOp,
    route: MixedTransferRoute,
    options: MixedTransferWriteOptions,
    cancel: Option<Arc<AtomicBool>>,
    progress: Option<TransferProgressContext>,
) -> TransferResult<Vec<String>> {
    let cli = RcloneCli::default();
    execute_mixed_entries_blocking_with_cli(&cli, op, route, options, cancel, progress)
}

fn execute_mixed_entry_to_blocking(
    op: MixedTransferOp,
    pair: MixedTransferPair,
    options: MixedTransferWriteOptions,
    cancel: Option<Arc<AtomicBool>>,
    progress: Option<TransferProgressContext>,
) -> TransferResult<String> {
    let cli = RcloneCli::default();
    execute_mixed_entry_to_blocking_with_cli(&cli, op, pair, options, cancel, progress)
}

fn execute_mixed_entry_to_blocking_with_cli(
    cli: &RcloneCli,
    op: MixedTransferOp,
    pair: MixedTransferPair,
    options: MixedTransferWriteOptions,
    cancel: Option<Arc<AtomicBool>>,
    progress: Option<TransferProgressContext>,
) -> TransferResult<String> {
    if transfer_cancelled(cancel.as_deref()) {
        return Err(transfer_err(
            TransferErrorCode::Cancelled,
            "Transfer cancelled",
        ));
    }
    let MixedTransferPair {
        src,
        dst,
        cloud_remote_for_error_mapping,
    } = pair;
    let invalidate_source = match (&src, op) {
        (LocalOrCloudArg::Cloud(path), MixedTransferOp::Move) => Some(path.clone()),
        _ => None,
    };
    let invalidate_target = match &dst {
        LocalOrCloudArg::Cloud(path) => Some(path.clone()),
        LocalOrCloudArg::Local(_) => None,
    };
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
        progress.as_ref(),
    )?;
    if let Some(path) = invalidate_target.as_ref() {
        cloud::invalidate_cloud_write_paths(std::slice::from_ref(path));
    } else if let Some(path) = invalidate_source.as_ref() {
        cloud::invalidate_cloud_write_paths(std::slice::from_ref(path));
    }
    Ok(out)
}

fn execute_mixed_entries_blocking_with_cli(
    cli: &RcloneCli,
    op: MixedTransferOp,
    route: MixedTransferRoute,
    options: MixedTransferWriteOptions,
    cancel: Option<Arc<AtomicBool>>,
    progress: Option<TransferProgressContext>,
) -> TransferResult<Vec<String>> {
    let mut created = Vec::new();
    let mut cloud_write_targets = Vec::<CloudPath>::new();
    let mut moved_cloud_sources = Vec::<CloudPath>::new();
    match route {
        MixedTransferRoute::LocalToCloud { sources, dest_dir } => {
            for src in sources {
                if transfer_cancelled(cancel.as_deref()) {
                    return Err(transfer_err(
                        TransferErrorCode::Cancelled,
                        "Transfer cancelled",
                    ));
                }
                let leaf = local_leaf_name(&src)?;
                let target = dest_dir.child_path(leaf).map_err(|e| {
                    transfer_err(
                        TransferErrorCode::InvalidPath,
                        format!("Invalid cloud target path: {e}"),
                    )
                })?;
                execute_rclone_transfer(
                    cli,
                    op,
                    LocalOrCloudArg::Local(src.clone()),
                    LocalOrCloudArg::Cloud(target.clone()),
                    options,
                    Some(dest_dir.remote()),
                    cancel.as_deref(),
                    progress.as_ref(),
                )?;
                cloud_write_targets.push(target.clone());
                created.push(target.to_string());
            }
        }
        MixedTransferRoute::CloudToLocal { sources, dest_dir } => {
            let batch_source_count = sources.len();
            let progress_plan = if batch_source_count > 1 {
                build_cloud_to_local_batch_progress_plan(cli, &sources)?
            } else {
                None
            };
            let mut completed_bytes = 0_u64;
            for (index, src) in sources.into_iter().enumerate() {
                if transfer_cancelled(cancel.as_deref()) {
                    return Err(transfer_err(
                        TransferErrorCode::Cancelled,
                        "Transfer cancelled",
                    ));
                }
                let leaf = src.leaf_name().map_err(|e| {
                    transfer_err(
                        TransferErrorCode::InvalidPath,
                        format!("Invalid cloud source path: {e}"),
                    )
                })?;
                let target = dest_dir.join(leaf);
                if let Some(plan) = progress_plan.as_ref() {
                    execute_cloud_to_local_file_transfer_with_aggregate_progress(
                        cli,
                        op,
                        &src,
                        &target,
                        cancel.as_deref(),
                        progress.as_ref().expect("progress context for batch plan"),
                        completed_bytes,
                        plan.total_bytes,
                        plan.file_sizes[index],
                    )?;
                    completed_bytes = completed_bytes.saturating_add(plan.file_sizes[index]);
                } else {
                    execute_rclone_transfer(
                        cli,
                        op,
                        LocalOrCloudArg::Cloud(src.clone()),
                        LocalOrCloudArg::Local(target.clone()),
                        options,
                        Some(src.remote()),
                        cancel.as_deref(),
                        if batch_source_count == 1 {
                            progress.as_ref()
                        } else {
                            None
                        },
                    )?;
                }
                if op == MixedTransferOp::Move {
                    moved_cloud_sources.push(src.clone());
                }
                created.push(target.to_string_lossy().to_string());
            }
        }
    }
    if !cloud_write_targets.is_empty() {
        cloud::invalidate_cloud_write_paths(&cloud_write_targets);
    }
    if !moved_cloud_sources.is_empty() {
        cloud::invalidate_cloud_write_paths(&moved_cloud_sources);
    }
    Ok(created)
}

fn execute_rclone_transfer(
    cli: &RcloneCli,
    op: MixedTransferOp,
    src: LocalOrCloudArg,
    dst: LocalOrCloudArg,
    options: MixedTransferWriteOptions,
    cloud_remote_for_error_mapping: Option<&str>,
    cancel: Option<&AtomicBool>,
    progress: Option<&TransferProgressContext>,
) -> TransferResult<()> {
    if transfer_cancelled(cancel) {
        return Err(transfer_err(
            TransferErrorCode::Cancelled,
            "Transfer cancelled",
        ));
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

    if let Some(result) = try_execute_cloud_to_local_file_transfer_with_progress(
        cli, op, &src, &dst, cancel, progress,
    )? {
        return result;
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

fn try_execute_cloud_to_local_file_transfer_with_progress(
    cli: &RcloneCli,
    op: MixedTransferOp,
    src: &LocalOrCloudArg,
    dst: &LocalOrCloudArg,
    cancel: Option<&AtomicBool>,
    progress: Option<&TransferProgressContext>,
) -> TransferResult<Option<TransferResult<()>>> {
    let Some(progress) = progress else {
        return Ok(None);
    };
    let (Some(src_path), Some(dst_path)) = (src.cloud_path(), dst.local_path()) else {
        return Ok(None);
    };
    let provider = mixed_cloud_provider_for_cli(cli);
    let Some(entry) = provider
        .stat_path(src_path)
        .map_err(map_cloud_error_to_transfer)?
    else {
        return Err(transfer_err(
            TransferErrorCode::NotFound,
            "Cloud source was not found",
        ));
    };
    if !matches!(entry.kind, CloudEntryKind::File) {
        return Ok(None);
    }

    let total = entry.size.unwrap_or(1);
    let result = match op {
        MixedTransferOp::Copy => provider
            .download_file_with_progress(
                src_path,
                dst_path,
                &progress.event_name,
                cancel,
                |bytes, total| {
                    emit_transfer_progress(progress, bytes, total, false);
                },
            )
            .map_err(map_cloud_error_to_transfer),
        MixedTransferOp::Move => {
            provider
                .download_file_with_progress(
                    src_path,
                    dst_path,
                    &progress.event_name,
                    cancel,
                    |bytes, total| {
                        emit_transfer_progress(progress, bytes, total, false);
                    },
                )
                .map_err(map_cloud_error_to_transfer)?;
            provider
                .delete_file(src_path, cancel)
                .map_err(map_cloud_error_to_transfer)
        }
    }
    .map(|_| {
        emit_transfer_progress(progress, total, total, true);
    });

    Ok(Some(result))
}

fn build_cloud_to_local_batch_progress_plan(
    cli: &RcloneCli,
    sources: &[CloudPath],
) -> TransferResult<Option<CloudToLocalBatchProgressPlan>> {
    let provider = mixed_cloud_provider_for_cli(cli);
    let mut file_sizes = Vec::with_capacity(sources.len());
    let mut total_bytes = 0_u64;
    for src in sources {
        let Some(entry) = provider
            .stat_path(src)
            .map_err(map_cloud_error_to_transfer)?
        else {
            return Err(transfer_err(
                TransferErrorCode::NotFound,
                "Cloud source was not found",
            ));
        };
        if !matches!(entry.kind, CloudEntryKind::File) {
            return Ok(None);
        }
        let size = entry.size.unwrap_or(1);
        file_sizes.push(size);
        total_bytes = total_bytes.saturating_add(size);
    }
    if total_bytes == 0 {
        return Ok(None);
    }
    Ok(Some(CloudToLocalBatchProgressPlan {
        total_bytes,
        file_sizes,
    }))
}

fn execute_cloud_to_local_file_transfer_with_aggregate_progress(
    cli: &RcloneCli,
    op: MixedTransferOp,
    src: &CloudPath,
    dst: &std::path::Path,
    cancel: Option<&AtomicBool>,
    progress: &TransferProgressContext,
    completed_before: u64,
    total_bytes: u64,
    file_size: u64,
) -> TransferResult<()> {
    let provider = mixed_cloud_provider_for_cli(cli);
    provider
        .download_file_with_progress(src, dst, &progress.event_name, cancel, |bytes, _| {
            let aggregate = completed_before.saturating_add(bytes.min(file_size));
            emit_transfer_progress(progress, aggregate, total_bytes, false);
        })
        .map_err(map_cloud_error_to_transfer)?;
    if op == MixedTransferOp::Move {
        provider
            .delete_file(src, cancel)
            .map_err(map_cloud_error_to_transfer)?;
    }
    emit_transfer_progress(
        progress,
        completed_before.saturating_add(file_size),
        total_bytes,
        false,
    );
    Ok(())
}

fn mixed_target_exists(
    cli: &RcloneCli,
    dst: &LocalOrCloudArg,
    cloud_remote_for_error_mapping: Option<&str>,
    cancel: Option<&AtomicBool>,
) -> TransferResult<bool> {
    if transfer_cancelled(cancel) {
        return Err(transfer_err(
            TransferErrorCode::Cancelled,
            "Transfer cancelled",
        ));
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

fn map_rclone_cli_error(
    error: RcloneCliError,
    cloud_remote: Option<&str>,
) -> super::error::TransferError {
    match error {
        RcloneCliError::Io(io) if io.kind() == std::io::ErrorKind::NotFound => {
            transfer_err(TransferErrorCode::BinaryMissing, "rclone not found in PATH")
        }
        RcloneCliError::Io(io) => transfer_err(TransferErrorCode::NetworkError, format!("Failed to run rclone: {io}")),
        RcloneCliError::Shutdown { .. } => api_err(
            "task_failed",
            "Application is shutting down; transfer was cancelled",
        ),
        RcloneCliError::Cancelled { .. } => transfer_err(TransferErrorCode::Cancelled, "Transfer cancelled"),
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
) -> TransferResult<Option<CancelGuard>> {
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

fn emit_transfer_progress(
    progress: &TransferProgressContext,
    bytes: u64,
    total: u64,
    finished: bool,
) {
    if total == 0 {
        return;
    }
    let _ = runtime_lifecycle::emit_if_running(
        &progress.app,
        &progress.event_name,
        TransferProgressPayload {
            bytes,
            total,
            finished,
        },
    );
}

fn map_cloud_error_to_transfer(
    error: crate::commands::cloud::CloudCommandError,
) -> super::error::TransferError {
    api_err(error.code_str(), error.to_string())
}

fn mixed_cloud_provider_for_cli(cli: &RcloneCli) -> RcloneCloudProvider {
    #[cfg(test)]
    {
        return RcloneCloudProvider::new(cli.clone());
    }
    #[cfg(not(test))]
    {
        let _ = cli;
        RcloneCloudProvider::default()
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

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    use std::fs;
    #[cfg(unix)]
    use std::path::{Path, PathBuf};
    #[cfg(unix)]
    use std::sync::atomic::{AtomicU64, Ordering};
    #[cfg(unix)]
    use std::sync::Mutex;
    #[cfg(unix)]
    use std::time::{SystemTime, UNIX_EPOCH};

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

        fn cloud_path(&self, raw: &str) -> crate::commands::cloud::path::CloudPath {
            crate::commands::cloud::path::CloudPath::parse(raw).expect("valid cloud path")
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

    fn sample_cloud_cache_entry(
        path: &str,
        name: &str,
    ) -> crate::commands::cloud::types::CloudEntry {
        crate::commands::cloud::types::CloudEntry {
            name: name.to_string(),
            path: path.to_string(),
            kind: crate::commands::cloud::types::CloudEntryKind::File,
            size: Some(1),
            modified: None,
            capabilities: crate::commands::cloud::types::CloudCapabilities::v1_core_rw(),
        }
    }

    #[cfg(unix)]
    #[test]
    fn mixed_execute_local_to_cloud_batch_invalidates_destination_cloud_cache() {
        let _guard = fake_rclone_test_lock();
        let sandbox = FakeRcloneSandbox::new();
        sandbox.mkdir_remote("work", "dest");
        let cli = sandbox.cli();
        let dest_dir = sandbox.cloud_path("rclone://work/dest");
        crate::commands::cloud::store_cloud_dir_listing_cache_entry_for_tests(
            &dest_dir,
            vec![sample_cloud_cache_entry(
                "rclone://work/dest/stale.txt",
                "stale.txt",
            )],
        );
        assert!(crate::commands::cloud::cloud_dir_listing_cache_contains_for_tests(&dest_dir));

        let copy_src = sandbox.write_local_file("src/cache-batch.txt", "copy-payload");
        let copy_route = MixedTransferRoute::LocalToCloud {
            sources: vec![copy_src],
            dest_dir: dest_dir.clone(),
        };
        execute_mixed_entries_blocking_with_cli(
            &cli,
            MixedTransferOp::Copy,
            copy_route,
            MixedTransferWriteOptions {
                overwrite: false,
                prechecked: true,
            },
            None,
            None,
        )
        .expect("copy local->cloud should succeed");

        assert!(
            !crate::commands::cloud::cloud_dir_listing_cache_contains_for_tests(&dest_dir),
            "mixed local->cloud batch write should invalidate destination dir cache"
        );
    }

    #[cfg(unix)]
    #[test]
    fn mixed_execute_local_to_cloud_single_target_invalidates_destination_cloud_cache() {
        let _guard = fake_rclone_test_lock();
        let sandbox = FakeRcloneSandbox::new();
        sandbox.mkdir_remote("work", "dest");
        let cli = sandbox.cli();
        let dest_dir = sandbox.cloud_path("rclone://work/dest");
        crate::commands::cloud::store_cloud_dir_listing_cache_entry_for_tests(
            &dest_dir,
            vec![sample_cloud_cache_entry(
                "rclone://work/dest/stale.txt",
                "stale.txt",
            )],
        );
        assert!(crate::commands::cloud::cloud_dir_listing_cache_contains_for_tests(&dest_dir));

        let copy_src = sandbox.write_local_file("src/cache-single.txt", "copy-payload");
        let pair = MixedTransferPair {
            src: LocalOrCloudArg::Local(copy_src),
            dst: LocalOrCloudArg::Cloud(sandbox.cloud_path("rclone://work/dest/cache-single.txt")),
            cloud_remote_for_error_mapping: Some("work".to_string()),
        };

        execute_mixed_entry_to_blocking_with_cli(
            &cli,
            MixedTransferOp::Copy,
            pair,
            MixedTransferWriteOptions {
                overwrite: false,
                prechecked: true,
            },
            None,
            None,
        )
        .expect("single local->cloud copy should succeed");

        assert!(
            !crate::commands::cloud::cloud_dir_listing_cache_contains_for_tests(&dest_dir),
            "mixed local->cloud single write should invalidate destination dir cache"
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
