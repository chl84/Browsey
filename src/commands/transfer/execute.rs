use super::logging::{log_mixed_execute_result, log_mixed_single_execute_result};
use super::route::{
    api_err, local_leaf_name, mixed_route_hint, validate_mixed_transfer_pair,
    validate_mixed_transfer_route, LocalOrCloudArg, MixedTransferPair, MixedTransferRoute,
};
use super::{MixedTransferOp, MixedTransferWriteOptions};
use crate::commands::cloud;
use crate::commands::cloud::rclone_cli::{
    RcloneCli, RcloneCliError, RcloneCommandSpec, RcloneSubcommand,
};
use crate::commands::cloud::types::CloudProviderKind;
use crate::errors::api_error::{ApiError, ApiResult};
use crate::tasks::{CancelGuard, CancelState};
use std::fs;
use std::io::ErrorKind;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::time::Instant;

pub(super) async fn execute_mixed_entries(
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
    let cancel_guard = register_mixed_cancel(&cancel_state, &progress_event)?;
    let cancel_token = cancel_guard.as_ref().map(|guard| guard.token());
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

pub(super) async fn execute_mixed_entry_to(
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
    let cancel_guard = register_mixed_cancel(&cancel_state, &progress_event)?;
    let cancel_token = cancel_guard.as_ref().map(|guard| guard.token());
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
