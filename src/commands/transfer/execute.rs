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
use crate::commands::cloud::providers::rclone::{
    RcloneCloudProvider, RcloneReadBackend, RcloneReadOptions,
};
use crate::commands::cloud::rclone_cli::{
    RcloneCli, RcloneCliError, RcloneCommandSpec, RcloneSubcommand,
};
use crate::commands::cloud::types::CloudEntryKind;
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

mod flow;
mod progress;

pub(super) fn copy_staged_entry(
    source: String,
    destination: String,
    app: tauri::AppHandle,
    cancel: Option<Arc<AtomicBool>>,
    event: Option<String>,
) -> TransferResult<String> {
    // The caller owns a newly allocated private staging directory and validated
    // CloudPaths. Still run normal route validation and destination preflight.
    let pair = tauri::async_runtime::block_on(validate_mixed_transfer_pair(source, destination))?;
    // Acquire only after route validation's async stat has released its own
    // permits; holding a permit across that nested stat can deadlock two jobs.
    let remotes = [&pair.src, &pair.dst]
        .into_iter()
        .filter_map(|arg| arg.cloud_path().map(|path| path.remote().to_owned()))
        .collect::<Vec<_>>();
    let _permit = cloud::limits_for_staging(remotes);
    execute_mixed_entry_to_blocking(
        MixedTransferOp::Copy,
        pair,
        MixedTransferWriteOptions::default(),
        cancel,
        event.map(|event_name| TransferProgressContext {
            app: Some(app),
            event_name,
        }),
    )
}

#[derive(Clone)]
struct TransferProgressContext {
    app: Option<tauri::AppHandle>,
    event_name: String,
}

#[derive(Serialize, Clone)]
struct TransferProgressPayload {
    bytes: u64,
    total: u64,
    finished: bool,
}

struct RcloneTransferContext<'a> {
    cli: &'a RcloneCli,
    cloud_remote_for_error_mapping: Option<&'a str>,
    cancel: Option<&'a AtomicBool>,
    progress: Option<&'a TransferProgressContext>,
}

#[cfg(feature = "native-test")]
fn native_probe_path(arg: &LocalOrCloudArg) -> String {
    match arg {
        LocalOrCloudArg::Local(path) => path.to_string_lossy().into_owned(),
        LocalOrCloudArg::Cloud(path) => path.to_string(),
    }
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
    let cancel_guard = register_mixed_cancel(&cancel_state, &progress_event)?;
    let route = match validate_mixed_transfer_route(sources, dest_dir).await {
        Ok(route) => route,
        Err(err) => {
            let result = Err(err);
            log_mixed_execute_result(op, &result, route_hint, source_count, started);
            return result;
        }
    };
    let cancel_token = cancel_guard.as_ref().map(|guard| guard.token());
    let progress = progress_event
        .clone()
        .map(|event_name| TransferProgressContext {
            app: Some(app),
            event_name,
        });
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
    let cancel_guard = register_mixed_cancel(&cancel_state, &progress_event)?;
    let cancel_token = cancel_guard.as_ref().map(|guard| guard.token());
    #[cfg(feature = "native-test")]
    {
        let source = src.clone();
        let target = dst.clone();
        let token = cancel_token.clone();
        tauri::async_runtime::spawn_blocking(move || {
            crate::native_test::probes::checkpoint(&source, &target, "validation", 0, || {
                transfer_cancelled(token.as_deref())
            });
        })
        .await
        .map_err(|error| {
            api_err(
                "task_failed",
                format!("Transfer checkpoint failed: {error}"),
            )
        })?;
    }
    if transfer_cancelled(cancel_token.as_deref()) {
        return Err(transfer_err(
            TransferErrorCode::Cancelled,
            "Transfer cancelled",
        ));
    }
    let pair = match validate_mixed_transfer_pair(src, dst).await {
        Ok(pair) => pair,
        Err(err) => {
            let result = Err(err);
            log_mixed_single_execute_result(op, &result, route_hint, started);
            return result;
        }
    };
    let progress = progress_event
        .clone()
        .map(|event_name| TransferProgressContext {
            app: Some(app),
            event_name,
        });
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
    let cli = cloud::configured_rclone_cli().map_err(|error| {
        let code = match error.code() {
            cloud::RclonePathErrorCode::BinaryMissing => TransferErrorCode::BinaryMissing,
            cloud::RclonePathErrorCode::InvalidBinaryPath => TransferErrorCode::InvalidConfig,
            cloud::RclonePathErrorCode::DbOpenFailed | cloud::RclonePathErrorCode::DbReadFailed => {
                TransferErrorCode::TaskFailed
            }
        };
        transfer_err(code, error.message())
    })?;
    execute_mixed_entries_blocking_with_cli(&cli, op, route, options, cancel, progress)
}

fn execute_mixed_entry_to_blocking(
    op: MixedTransferOp,
    pair: MixedTransferPair,
    options: MixedTransferWriteOptions,
    cancel: Option<Arc<AtomicBool>>,
    progress: Option<TransferProgressContext>,
) -> TransferResult<String> {
    let cli = cloud::configured_rclone_cli().map_err(|error| {
        let code = match error.code() {
            cloud::RclonePathErrorCode::BinaryMissing => TransferErrorCode::BinaryMissing,
            cloud::RclonePathErrorCode::InvalidBinaryPath => TransferErrorCode::InvalidConfig,
            cloud::RclonePathErrorCode::DbOpenFailed | cloud::RclonePathErrorCode::DbReadFailed => {
                TransferErrorCode::TaskFailed
            }
        };
        transfer_err(code, error.message())
    })?;
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
    flow::execute_mixed_entry_to_blocking_with_cli(cli, op, pair, options, cancel, progress)
}

fn execute_mixed_entries_blocking_with_cli(
    cli: &RcloneCli,
    op: MixedTransferOp,
    route: MixedTransferRoute,
    options: MixedTransferWriteOptions,
    cancel: Option<Arc<AtomicBool>>,
    progress: Option<TransferProgressContext>,
) -> TransferResult<Vec<String>> {
    flow::execute_mixed_entries_blocking_with_cli(cli, op, route, options, cancel, progress)
}

fn execute_rclone_transfer(
    ctx: RcloneTransferContext<'_>,
    op: MixedTransferOp,
    src: LocalOrCloudArg,
    dst: LocalOrCloudArg,
    options: MixedTransferWriteOptions,
) -> TransferResult<()> {
    let RcloneTransferContext {
        cli,
        cloud_remote_for_error_mapping,
        cancel,
        progress,
    } = ctx;
    #[cfg(feature = "native-test")]
    crate::native_test::probes::checkpoint(
        &native_probe_path(&src),
        &native_probe_path(&dst),
        "start",
        0,
        || transfer_cancelled(cancel),
    );
    #[cfg(feature = "native-test")]
    if let Some(error) = crate::native_test::probes::fault(
        &native_probe_path(&src),
        &native_probe_path(&dst),
        "start",
        0,
    ) {
        return Err(transfer_err(
            TransferErrorCode::IoError,
            format!("Owned transfer I/O fault: {error}"),
        ));
    }
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

    // Without a recovery receipt, replacing a directory with a file (or vice
    // versa) must not recursively delete the original destination. Even an
    // explicitly prechecked name still needs this current kind check.
    if options.overwrite
        && mixed_target_exists(cli, &dst, cloud_remote_for_error_mapping, cancel)?
        && transfer_source_is_directory(cli, &src, cancel)?
            != transfer_source_is_directory(cli, &dst, cancel)?
    {
        return Err(transfer_err(TransferErrorCode::Unsupported,
            "Cannot overwrite a file with a folder or a folder with a file on this route; use Auto-rename or Skip"));
    }

    if let Some(result) = progress::try_execute_cloud_to_local_file_transfer_with_progress(
        cli, op, &src, &dst, cancel, progress,
    )? {
        return result;
    }

    if let Some(result) = progress::try_execute_local_to_cloud_file_transfer_with_progress(
        cli, op, &src, &dst, cancel, progress, options,
    )? {
        return result;
    }

    let directory = transfer_source_is_directory(cli, &src, cancel)?;
    if let Some(progress) = progress {
        emit_transfer_progress(progress, 0, 0, false);
    }
    let source_identity = if directory && op == MixedTransferOp::Move {
        src.local_path()
            .map(|path| {
                crate::fs_utils::FileIdentity::capture(path).ok_or_else(|| {
                    transfer_err(
                        TransferErrorCode::TaskFailed,
                        "Cannot identify source directory; move has not started",
                    )
                })
            })
            .transpose()?
    } else {
        None
    };
    let subcommand = match (op, directory) {
        (MixedTransferOp::Copy, true) => RcloneSubcommand::Copy,
        (MixedTransferOp::Copy, false) => RcloneSubcommand::CopyTo,
        (MixedTransferOp::Move, true) => RcloneSubcommand::Move,
        (MixedTransferOp::Move, false) => RcloneSubcommand::MoveTo,
    };

    let mut spec = RcloneCommandSpec::new(subcommand)
        .arg(src.to_os_arg())
        .arg(dst.to_os_arg());
    if let Some(destination) = dst.local_path() {
        spec = spec.local_destination_options(destination);
    }
    if op == MixedTransferOp::Copy && dst.cloud_path().is_some() && !options.overwrite {
        // A destination can appear after our preflight. This is an additional
        // rclone guard, not an atomic provider compare-and-swap transaction.
        spec = spec.arg("--immutable").arg("--checksum");
        if subcommand == RcloneSubcommand::CopyTo {
            spec = spec.arg("--ignore-existing").arg("--error-on-no-transfer");
        }
    }
    if subcommand == RcloneSubcommand::Copy {
        // Archive staging must not silently lose empty directories on upload.
        // This flag belongs to `copy`, not `copyto` (even for directory sources).
        spec = spec.arg("--create-empty-src-dirs");
    }
    if subcommand == RcloneSubcommand::Move {
        // Cross-backend moveto moves files but leaves empty source directories.
        // Only move exposes these flags: preserve empty destinations and let
        // rclone remove empty source directories, never recursively delete data.
        spec = spec
            .arg("--create-empty-src-dirs")
            .arg("--delete-empty-src-dirs");
    }

    tracing::info!(
        op = if op == MixedTransferOp::Copy {
            "copy"
        } else {
            "move"
        },
        backend = "rclone-cli",
        kind = if directory { "directory" } else { "file" },
        staging = "direct",
        "transfer dispatch"
    );
    cli.run_capture_text_with_cancel(spec, cancel)
        .map_err(|error| map_rclone_cli_error(error, cloud_remote_for_error_mapping))?;
    #[cfg(feature = "native-test")]
    crate::native_test::probes::checkpoint(
        &native_probe_path(&src),
        &native_probe_path(&dst),
        "finalize",
        0,
        || transfer_cancelled(cancel),
    );
    if directory {
        // rclone transfers directory contents; even --create-empty-src-dirs does
        // not create the destination root when the source itself is empty.
        ensure_transferred_directory_root(cli, &dst, cloud_remote_for_error_mapping, cancel)?;
        if op == MixedTransferOp::Move {
            remove_moved_source_root(
                cli,
                &src,
                source_identity.as_ref(),
                cloud_remote_for_error_mapping,
                cancel,
            )?;
        }
    }
    Ok(())
}

fn remove_moved_source_root(
    cli: &RcloneCli,
    src: &LocalOrCloudArg,
    identity: Option<&crate::fs_utils::FileIdentity>,
    cloud_remote: Option<&str>,
    cancel: Option<&AtomicBool>,
) -> TransferResult<()> {
    if transfer_cancelled(cancel) {
        return Err(transfer_err(
            TransferErrorCode::Cancelled,
            "Transfer cancelled; source root and destination retained",
        ));
    }
    match src {
        LocalOrCloudArg::Local(path) => {
            match fs::symlink_metadata(path) {
                Err(error) if error.kind() == ErrorKind::NotFound => return Ok(()),
                Err(error) => {
                    return Err(transfer_err(
                        TransferErrorCode::IoError,
                        format!("Cannot inspect moved source root; destination retained: {error}"),
                    ))
                }
                Ok(_) => {}
            }
            if !identity.is_some_and(|identity| identity.matches(path)) {
                return Err(transfer_err(
                    TransferErrorCode::TaskFailed,
                    "Source directory changed; source and destination retained",
                ));
            }
            // Atomic empty-only removal. New contents, replaced directories and
            // uncertain failures are retained; never recursively delete here.
            match fs::remove_dir(path) {
                Ok(()) => Ok(()),
                Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
                Err(error) => Err(transfer_err(
                    TransferErrorCode::IoError,
                    format!("Cannot remove empty moved source root; destination retained: {error}"),
                )),
            }
        }
        LocalOrCloudArg::Cloud(path) => {
            // rclone's move flags clean descendants, leaving its filesystem root.
            // rmdir removes only an empty root; purge is never used for cleanup.
            match cli.run_capture_text_with_cancel(
                RcloneCommandSpec::new(RcloneSubcommand::Rmdir).arg(path.to_rclone_remote_spec()),
                cancel,
            ) {
                Ok(_) => Ok(()),
                Err(error) => {
                    let mapped = map_rclone_cli_error(error, cloud_remote);
                    if mapped.code_str() == "not_found" {
                        Ok(())
                    } else {
                        Err(mapped)
                    }
                }
            }
        }
    }
}

fn ensure_transferred_directory_root(
    cli: &RcloneCli,
    dst: &LocalOrCloudArg,
    cloud_remote: Option<&str>,
    cancel: Option<&AtomicBool>,
) -> TransferResult<()> {
    if transfer_cancelled(cancel) {
        return Err(transfer_err(
            TransferErrorCode::Cancelled,
            "Transfer cancelled",
        ));
    }
    match dst {
        LocalOrCloudArg::Local(path) => fs::create_dir_all(path).map_err(|error| {
            let code = match error.kind() {
                ErrorKind::PermissionDenied => TransferErrorCode::PermissionDenied,
                ErrorKind::AlreadyExists => TransferErrorCode::DestinationExists,
                _ => TransferErrorCode::IoError,
            };
            transfer_err(
                code,
                format!("Failed to create transferred directory: {error}"),
            )
        }),
        LocalOrCloudArg::Cloud(path) => cli
            .run_capture_text_with_cancel(
                RcloneCommandSpec::new(RcloneSubcommand::Mkdir).arg(path.to_rclone_remote_spec()),
                cancel,
            )
            .map(|_| ())
            .map_err(|error| map_rclone_cli_error(error, cloud_remote)),
    }
}

fn transfer_source_is_directory(
    cli: &RcloneCli,
    src: &LocalOrCloudArg,
    cancel: Option<&AtomicBool>,
) -> TransferResult<bool> {
    match src {
        LocalOrCloudArg::Local(path) => fs::symlink_metadata(path)
            .map(|metadata| metadata.is_dir())
            .map_err(|error| {
                let code = match error.kind() {
                    ErrorKind::NotFound => TransferErrorCode::NotFound,
                    ErrorKind::PermissionDenied => TransferErrorCode::PermissionDenied,
                    _ => TransferErrorCode::IoError,
                };
                transfer_err(code, format!("Failed to read source metadata: {error}"))
            }),
        LocalOrCloudArg::Cloud(path) => {
            let entry = mixed_cloud_provider_for_cli(cli)
                .stat_path_with_read_options(
                    path,
                    RcloneReadOptions {
                        cancel,
                        backend: RcloneReadBackend::CliOnly,
                        ..Default::default()
                    },
                )
                .map_err(map_cloud_error_to_transfer)?
                .ok_or_else(|| {
                    transfer_err(TransferErrorCode::NotFound, "Cloud source was not found")
                })?;
            Ok(matches!(entry.kind, CloudEntryKind::Dir))
        }
    }
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
        RcloneCliError::WriteStateUnknown { operation, cause } => transfer_err(
            TransferErrorCode::TaskFailed, format!("Transfer write status is unknown after rclone rc {operation}; no automatic retry. Refresh and verify the destination. Cause: {cause}")),
        RcloneCliError::AsyncJobFailed { operation, job_id, message } => {
            let provider = cloud_remote.and_then(cloud::cloud_provider_kind_for_remote);
            let code = cloud::providers::rclone::classify_rclone_failure_code(provider, &message);
            let code = if code == cloud::CloudCommandErrorCode::UnknownError { cloud::CloudCommandErrorCode::TaskFailed } else { code };
            map_cloud_error_to_transfer(cloud::CloudCommandError::new(code, cloud::rclone_cli::failed_job_message(&operation, job_id, &message)))
        }
        RcloneCliError::OutputLimit { subcommand, stream, limit } => transfer_err(
            TransferErrorCode::TaskFailed,
            cloud::rclone_cli::output_limit_message(subcommand, stream, limit)),
        RcloneCliError::Io(io) if io.kind() == std::io::ErrorKind::NotFound => {
            transfer_err(TransferErrorCode::BinaryMissing, "rclone not found in PATH")
        }
        RcloneCliError::Io(io) => transfer_err(TransferErrorCode::NetworkError, cloud::rclone_cli::sanitize_failure_message(&format!("Failed to run rclone: {io}"))),
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
                cloud::rclone_cli::sanitize_failure_message(reason.trim())
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
        RcloneCliError::NonZero { status, stderr, stdout } => {
            if status.code() == Some(9) {
                return transfer_err(TransferErrorCode::TaskFailed,
                    cloud::rclone_cli::NO_TRANSFER_MESSAGE);
            }
            let msg_ref = if !stderr.trim().is_empty() {
                stderr.as_str()
            } else {
                stdout.as_str()
            };
            let provider = cloud_remote.and_then(cloud::cloud_provider_kind_for_remote);
            let code = cloud::providers::rclone::classify_rclone_failure_code(provider, msg_ref);
            map_cloud_error_to_transfer(cloud::CloudCommandError::new(code, cloud::rclone_cli::sanitize_failure_message(msg_ref.trim())))
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

fn emit_transfer_progress(
    progress: &TransferProgressContext,
    bytes: u64,
    total: u64,
    finished: bool,
) {
    let Some(app) = progress.app.as_ref() else {
        return;
    };
    let _ = runtime_lifecycle::emit_if_running(
        app,
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
    error.into()
}

fn snapshot_mixed_move_source(
    path: &std::path::Path,
) -> TransferResult<crate::fs_utils::FileState> {
    crate::fs_utils::open_regular_file_nofollow(path)
        .and_then(|file| crate::fs_utils::FileState::from_file(&file))
        .map_err(|error| {
            transfer_err(
                TransferErrorCode::IoError,
                format!("Cannot snapshot local source before cloud move: {error}"),
            )
        })
}

fn remove_local_source_after_mixed_file_move(
    path: &std::path::Path,
    snapshot: &crate::fs_utils::FileState,
    cancel: Option<&AtomicBool>,
) -> TransferResult<()> {
    if transfer_cancelled(cancel) {
        return Err(transfer_err(
            TransferErrorCode::Cancelled,
            "Move cancelled after upload; uploaded output retained and local source not removed",
        ));
    }
    if !snapshot.matches(path) {
        return Err(transfer_err(TransferErrorCode::TaskFailed,
            "Local source changed during cloud upload; uploaded output retained and source not removed; inspect affected paths before retrying"));
    }
    fs::remove_file(path).map_err(|error| {
        transfer_err(
            TransferErrorCode::IoError,
            format!("Failed to remove moved source file: {error}"),
        )
    })
}

fn mixed_cloud_provider_for_cli(cli: &RcloneCli) -> RcloneCloudProvider {
    RcloneCloudProvider::from_cli(cli.clone())
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
mod tests;
