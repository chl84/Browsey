use super::{
    configured_rclone_provider, ensure_cloud_enabled,
    error::{CloudCommandError, CloudCommandErrorCode, CloudCommandResult},
    map_spawn_result,
    provider::CloudProvider,
    providers::rclone::{RcloneReadBackend, RcloneReadOptions},
    register_cloud_cancel,
    types::{
        CloudProbePathStatus, CloudProbeRecommendation, CloudProbeState, CloudRemote,
        CloudRemoteProbeStatus,
    },
    CancelState,
};
use crate::errors::domain::ErrorCode;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};
use tracing::warn;

#[cfg(not(test))]
const CLOUD_PROBE_RC_TIMEOUT: Duration = Duration::from_secs(10);
#[cfg(test)]
const CLOUD_PROBE_RC_TIMEOUT: Duration = Duration::from_millis(100);
#[cfg(not(test))]
const CLOUD_PROBE_CLI_TIMEOUT: Duration = Duration::from_secs(20);
#[cfg(test)]
const CLOUD_PROBE_CLI_TIMEOUT: Duration = Duration::from_millis(200);

pub(super) async fn probe_cloud_remote_impl(
    remote_id: String,
    cancel_state: CancelState,
    progress_event: Option<String>,
) -> CloudCommandResult<CloudRemoteProbeStatus> {
    ensure_cloud_enabled()?;
    let cancel_guard = register_cloud_cancel(&cancel_state, &progress_event)?;
    let cancel_token = cancel_guard.as_ref().map(|guard| guard.token());
    let remote_id_for_log = remote_id.clone();
    let task = tauri::async_runtime::spawn_blocking(move || {
        probe_cloud_remote_sync(&remote_id, cancel_token.as_deref())
    });
    let result = map_spawn_result(task.await, "cloud probe task failed");
    if let Err(error) = &result {
        warn!(
            remote_id = %remote_id_for_log,
            code = error.code().as_code_str(),
            error = %error,
            "cloud remote probe command failed"
        );
    }
    result
}

fn probe_cloud_remote_sync(
    remote_id: &str,
    cancel: Option<&AtomicBool>,
) -> CloudCommandResult<CloudRemoteProbeStatus> {
    let provider = configured_rclone_provider().map_err(CloudCommandError::from)?;
    let remote = provider
        .list_remotes()?
        .into_iter()
        .find(|remote| remote.id == remote_id)
        .ok_or_else(|| {
            CloudCommandError::new(
                CloudCommandErrorCode::InvalidConfig,
                format!("Cloud remote is not configured or unsupported: {remote_id}"),
            )
        })?;
    let root_path = super::path::CloudPath::parse(&remote.root_path).map_err(|error| {
        CloudCommandError::new(
            CloudCommandErrorCode::InvalidPath,
            format!("Invalid cloud remote root path: {error}"),
        )
    })?;

    let rc = probe_backend(
        &provider,
        &remote,
        &root_path,
        cancel,
        RcloneReadBackend::RcOnly,
        CLOUD_PROBE_RC_TIMEOUT,
    );
    let cli = probe_backend(
        &provider,
        &remote,
        &root_path,
        cancel,
        RcloneReadBackend::CliOnly,
        CLOUD_PROBE_CLI_TIMEOUT,
    );

    Ok(CloudRemoteProbeStatus {
        remote,
        recommendation: probe_recommendation(&rc, &cli),
        rc,
        cli,
    })
}

fn probe_backend(
    provider: &super::providers::rclone::RcloneCloudProvider,
    remote: &CloudRemote,
    root_path: &super::path::CloudPath,
    cancel: Option<&AtomicBool>,
    backend: RcloneReadBackend,
    timeout: Duration,
) -> CloudProbePathStatus {
    let started = Instant::now();
    let result = provider.list_dir_with_read_options(
        root_path,
        RcloneReadOptions {
            cancel,
            rc_timeout: Some(timeout),
            cli_timeout: Some(timeout),
            backend,
        },
    );
    let elapsed_ms = started.elapsed().as_millis() as u64;
    match result {
        Ok(entries) => CloudProbePathStatus {
            ok: true,
            state: CloudProbeState::Ok,
            message: format!(
                "{} root listed successfully ({} item{})",
                remote.label,
                entries.len(),
                if entries.len() == 1 { "" } else { "s" }
            ),
            elapsed_ms,
        },
        Err(error) => {
            let state = probe_state_from_error_code(error.code());
            warn!(
                remote_id = %remote.id,
                backend = ?backend,
                elapsed_ms,
                code = error.code().as_code_str(),
                error = %error,
                "cloud remote probe backend failed"
            );
            CloudProbePathStatus {
                ok: false,
                state,
                message: probe_user_message(state, error.message()),
                elapsed_ms,
            }
        }
    }
}

fn probe_recommendation(
    rc: &CloudProbePathStatus,
    cli: &CloudProbePathStatus,
) -> CloudProbeRecommendation {
    if rc.ok {
        CloudProbeRecommendation::HealthyRc
    } else if cli.ok {
        CloudProbeRecommendation::HealthyCliOnly
    } else {
        CloudProbeRecommendation::ProbeFailed
    }
}

fn probe_state_from_error_code(code: CloudCommandErrorCode) -> CloudProbeState {
    match code {
        CloudCommandErrorCode::BinaryMissing => CloudProbeState::BinaryMissing,
        CloudCommandErrorCode::InvalidConfig | CloudCommandErrorCode::InvalidPath => {
            CloudProbeState::InvalidConfig
        }
        CloudCommandErrorCode::AuthRequired => CloudProbeState::AuthRequired,
        CloudCommandErrorCode::Timeout => CloudProbeState::Timeout,
        CloudCommandErrorCode::NetworkError | CloudCommandErrorCode::TlsCertificateError => {
            CloudProbeState::NetworkError
        }
        CloudCommandErrorCode::RateLimited => CloudProbeState::RateLimited,
        CloudCommandErrorCode::PermissionDenied => CloudProbeState::PermissionDenied,
        CloudCommandErrorCode::Cancelled => CloudProbeState::Cancelled,
        CloudCommandErrorCode::TaskFailed | CloudCommandErrorCode::CloudDisabled => {
            CloudProbeState::TaskFailed
        }
        CloudCommandErrorCode::DestinationExists
        | CloudCommandErrorCode::Unsupported
        | CloudCommandErrorCode::NotFound
        | CloudCommandErrorCode::UnknownError => CloudProbeState::UnknownError,
    }
}

fn probe_user_message(state: CloudProbeState, fallback: &str) -> String {
    match state {
        CloudProbeState::Ok => "Remote probe succeeded".to_string(),
        CloudProbeState::BinaryMissing => {
            "rclone was not found. Install it or fix Rclone path in Settings > Cloud.".to_string()
        }
        CloudProbeState::InvalidConfig => {
            "The selected cloud remote is missing, unsupported, or misconfigured in rclone."
                .to_string()
        }
        CloudProbeState::AuthRequired => {
            "Cloud authentication is required or has expired. Reconnect the rclone remote and try again.".to_string()
        }
        CloudProbeState::Timeout => {
            "The cloud probe timed out. Check network/provider responsiveness and try again."
                .to_string()
        }
        CloudProbeState::NetworkError => {
            "The cloud probe failed because the network, TLS, or rclone backend is unavailable."
                .to_string()
        }
        CloudProbeState::RateLimited => {
            "The cloud provider is rate-limiting requests. Wait a bit and try again."
                .to_string()
        }
        CloudProbeState::PermissionDenied => {
            "The cloud probe was denied by the provider or remote permissions.".to_string()
        }
        CloudProbeState::Cancelled => "Cloud connection test cancelled.".to_string(),
        CloudProbeState::TaskFailed => fallback.to_string(),
        CloudProbeState::UnknownError => fallback.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::cloud::{
        clear_cloud_provider_kind_overrides_for_tests,
        rclone_path::set_rclone_resolution_override_for_tests,
        set_cloud_enabled_override_for_tests, set_rclone_path_override_for_tests,
    };
    use once_cell::sync::Lazy;
    use std::{
        fs,
        os::unix::fs::PermissionsExt,
        path::{Path, PathBuf},
        sync::{
            atomic::{AtomicBool, AtomicU64, Ordering},
            Mutex,
        },
    };

    static TEST_LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

    struct OverrideGuard;

    impl Drop for OverrideGuard {
        fn drop(&mut self) {
            set_cloud_enabled_override_for_tests(None);
            set_rclone_path_override_for_tests(None);
            set_rclone_resolution_override_for_tests(None);
            clear_cloud_provider_kind_overrides_for_tests();
        }
    }

    fn override_guard() -> OverrideGuard {
        set_cloud_enabled_override_for_tests(Some(true));
        set_rclone_path_override_for_tests(None);
        set_rclone_resolution_override_for_tests(None);
        clear_cloud_provider_kind_overrides_for_tests();
        OverrideGuard
    }

    struct FakeRcloneSandbox {
        root: PathBuf,
        script_path: PathBuf,
        state_root: PathBuf,
    }

    impl FakeRcloneSandbox {
        fn new() -> Self {
            static NEXT_ID: AtomicU64 = AtomicU64::new(1);
            let root = std::env::temp_dir().join(format!(
                "browsey-cloud-probe-fake-rclone-{}-{}",
                std::process::id(),
                NEXT_ID.fetch_add(1, Ordering::Relaxed)
            ));
            let state_root = root.join("state");
            let script_path = root.join("rclone");
            fs::create_dir_all(&state_root).expect("create state root");
            let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/support/fake-rclone.sh");
            fs::copy(&source, &script_path).expect("copy fake rclone script");
            let mut perms = fs::metadata(&script_path)
                .expect("script metadata")
                .permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&script_path, perms).expect("chmod fake rclone");
            Self {
                root,
                script_path,
                state_root,
            }
        }

        fn path_str(&self) -> &str {
            self.script_path.to_str().expect("utf-8 path")
        }

        fn mkdir_remote(&self, remote: &str) {
            fs::create_dir_all(self.state_root.join(remote)).expect("mkdir remote");
        }

        fn set_remote_provider_type(&self, remote: &str, backend_type: &str) {
            let provider_root = self.root.join("provider-types");
            fs::create_dir_all(&provider_root).expect("create provider type root");
            fs::write(provider_root.join(remote), backend_type).expect("set provider type");
        }

        fn configure_subcommand_delay(&self, subcommand: &str, delay_ms: u64, invocation: u64) {
            fs::write(
                self.root.join(format!("{subcommand}-delay-ms")),
                delay_ms.to_string(),
            )
            .expect("write delay ms");
            fs::write(
                self.root.join(format!("{subcommand}-delay-invocation")),
                invocation.to_string(),
            )
            .expect("write delay invocation");
        }
    }

    impl Drop for FakeRcloneSandbox {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn probe_cloud_remote_reports_cli_only_when_rc_reads_fail_but_cli_works() {
        let _lock = TEST_LOCK.lock().expect("test lock");
        let _guard = override_guard();
        let sandbox = FakeRcloneSandbox::new();
        sandbox.mkdir_remote("browsey-gdrive");
        sandbox.set_remote_provider_type("browsey-gdrive", "drive");
        set_rclone_path_override_for_tests(Some(sandbox.path_str()));

        let status =
            probe_cloud_remote_sync("browsey-gdrive", None).expect("probe should return status");

        assert!(!status.rc.ok);
        assert_ne!(status.rc.state, CloudProbeState::Ok);
        assert!(status.cli.ok);
        assert_eq!(
            status.recommendation,
            CloudProbeRecommendation::HealthyCliOnly
        );
    }

    #[test]
    fn probe_cloud_remote_reports_probe_failed_when_cli_listing_times_out() {
        let _lock = TEST_LOCK.lock().expect("test lock");
        let _guard = override_guard();
        let sandbox = FakeRcloneSandbox::new();
        sandbox.mkdir_remote("browsey-gdrive");
        sandbox.set_remote_provider_type("browsey-gdrive", "drive");
        sandbox.configure_subcommand_delay("lsjson", 21_000, 1);
        set_rclone_path_override_for_tests(Some(sandbox.path_str()));

        let status =
            probe_cloud_remote_sync("browsey-gdrive", None).expect("probe should return status");

        assert!(!status.cli.ok);
        assert_eq!(status.cli.state, CloudProbeState::Timeout);
        assert_eq!(status.recommendation, CloudProbeRecommendation::ProbeFailed);
    }

    #[test]
    fn probe_cloud_remote_reports_cancelled_when_cli_probe_is_cancelled() {
        let _lock = TEST_LOCK.lock().expect("test lock");
        let _guard = override_guard();
        let sandbox = FakeRcloneSandbox::new();
        sandbox.mkdir_remote("browsey-gdrive");
        sandbox.set_remote_provider_type("browsey-gdrive", "drive");
        sandbox.configure_subcommand_delay("lsjson", 1_000, 1);
        set_rclone_path_override_for_tests(Some(sandbox.path_str()));
        let cancel = AtomicBool::new(true);

        let status = probe_cloud_remote_sync("browsey-gdrive", Some(&cancel))
            .expect("probe should return status");

        assert_eq!(status.cli.state, CloudProbeState::Cancelled);
        assert_eq!(status.recommendation, CloudProbeRecommendation::ProbeFailed);
    }
}
