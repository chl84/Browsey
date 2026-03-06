use super::{
    error::{map_api_result, CloudCommandError, CloudCommandErrorCode, CloudCommandResult},
    providers::rclone::RcloneCloudProvider,
    rclone_cli::RcloneCli,
    rclone_path::{
        load_rclone_path_setting, resolve_configured_rclone_binary, RclonePathError,
        RclonePathErrorCode,
    },
    types::{CloudSetupState, CloudSetupStatus},
};

fn empty_status(
    state: CloudSetupState,
    configured_path: Option<String>,
    resolved_binary_path: Option<String>,
) -> CloudSetupStatus {
    CloudSetupStatus {
        state,
        configured_path,
        resolved_binary_path,
        detected_remote_count: 0,
        supported_remote_count: 0,
        unsupported_remote_count: 0,
        supported_remotes: Vec::new(),
    }
}

fn map_rclone_path_error(
    error: &RclonePathError,
    configured_path: Option<String>,
) -> CloudSetupStatus {
    match error.code() {
        RclonePathErrorCode::BinaryMissing => {
            empty_status(CloudSetupState::BinaryMissing, configured_path, None)
        }
        RclonePathErrorCode::InvalidBinaryPath => {
            empty_status(CloudSetupState::InvalidBinaryPath, configured_path, None)
        }
        RclonePathErrorCode::DbOpenFailed | RclonePathErrorCode::DbReadFailed => {
            empty_status(CloudSetupState::ConfigReadFailed, None, None)
        }
    }
}

fn inspect_cloud_setup_sync() -> CloudSetupStatus {
    let configured_path = match load_rclone_path_setting() {
        Ok(value) => value,
        Err(error) => return map_rclone_path_error(&error, None),
    };

    let resolved = match resolve_configured_rclone_binary() {
        Ok(value) => value,
        Err(error) => return map_rclone_path_error(&error, configured_path),
    };

    let resolved_binary_path = Some(
        resolved
            .resolved_binary_path()
            .to_string_lossy()
            .into_owned(),
    );
    let configured_path = resolved.configured_path().map(ToOwned::to_owned);
    let provider = RcloneCloudProvider::from_cli(RcloneCli::with_binary(
        resolved.resolved_binary_path().clone(),
    ));

    if provider.ensure_runtime_ready().is_err() {
        return empty_status(
            CloudSetupState::RuntimeUnusable,
            configured_path,
            resolved_binary_path,
        );
    }

    match provider.inspect_remote_inventory_impl() {
        Ok(inventory) => {
            let supported_remote_count = inventory.supported_remotes.len();
            let state = if supported_remote_count == 0 {
                CloudSetupState::NoSupportedRemotes
            } else {
                CloudSetupState::Ready
            };
            CloudSetupStatus {
                state,
                configured_path,
                resolved_binary_path,
                detected_remote_count: inventory.detected_remote_count,
                supported_remote_count,
                unsupported_remote_count: inventory.unsupported_remote_count,
                supported_remotes: inventory.supported_remotes,
            }
        }
        Err(_) => empty_status(
            CloudSetupState::DiscoveryFailed,
            configured_path,
            resolved_binary_path,
        ),
    }
}

pub(super) async fn cloud_setup_status() -> crate::errors::api_error::ApiResult<CloudSetupStatus> {
    map_api_result(cloud_setup_status_impl().await)
}

async fn cloud_setup_status_impl() -> CloudCommandResult<CloudSetupStatus> {
    let task = tauri::async_runtime::spawn_blocking(inspect_cloud_setup_sync);
    match task.await {
        Ok(status) => Ok(status),
        Err(error) => Err(CloudCommandError::new(
            CloudCommandErrorCode::TaskFailed,
            format!("cloud setup status task failed: {error}"),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::cloud::rclone_path::{
        set_rclone_path_override_for_tests, set_rclone_resolution_override_for_tests,
    };
    use once_cell::sync::Lazy;
    use std::{
        fs,
        os::unix::fs::PermissionsExt,
        path::{Path, PathBuf},
        sync::{
            atomic::{AtomicU64, Ordering},
            Mutex,
        },
    };

    static TEST_LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

    struct OverrideGuard;

    impl Drop for OverrideGuard {
        fn drop(&mut self) {
            set_rclone_path_override_for_tests(None);
            set_rclone_resolution_override_for_tests(None);
        }
    }

    fn override_guard() -> OverrideGuard {
        set_rclone_path_override_for_tests(None);
        set_rclone_resolution_override_for_tests(None);
        OverrideGuard
    }

    struct TempScript {
        root: PathBuf,
        path: PathBuf,
    }

    impl TempScript {
        fn new(contents: &str) -> Self {
            static NEXT_ID: AtomicU64 = AtomicU64::new(1);
            let root = std::env::temp_dir().join(format!(
                "browsey-cloud-setup-status-{}-{}",
                std::process::id(),
                NEXT_ID.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&root).expect("create temp root");
            let path = root.join("rclone");
            fs::write(&path, contents).expect("write script");
            let mut perms = fs::metadata(&path).expect("script metadata").permissions();
            perms.set_mode(0o755);
            fs::set_permissions(&path, perms).expect("chmod script");
            Self { root, path }
        }

        fn path_str(&self) -> &str {
            self.path.to_str().expect("utf-8 path")
        }
    }

    impl Drop for TempScript {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
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
                "browsey-cloud-setup-fake-rclone-{}-{}",
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
    }

    impl Drop for FakeRcloneSandbox {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    #[test]
    fn cloud_setup_status_reports_binary_missing_for_failed_autodetect() {
        let _lock = TEST_LOCK.lock().expect("test lock");
        let _guard = override_guard();
        set_rclone_resolution_override_for_tests(Some(Err(RclonePathError::new(
            RclonePathErrorCode::BinaryMissing,
            "Unable to auto-detect rclone; install it or set Rclone path in Settings.",
        ))));

        let status = inspect_cloud_setup_sync();

        assert_eq!(status.state, CloudSetupState::BinaryMissing);
        assert_eq!(status.configured_path, None);
        assert_eq!(status.resolved_binary_path, None);
    }

    #[test]
    fn cloud_setup_status_reports_invalid_binary_path() {
        let _lock = TEST_LOCK.lock().expect("test lock");
        let _guard = override_guard();
        set_rclone_path_override_for_tests(Some("/usr/bin/rclone-does-not-exist"));

        let status = inspect_cloud_setup_sync();

        assert_eq!(status.state, CloudSetupState::InvalidBinaryPath);
        assert_eq!(
            status.configured_path.as_deref(),
            Some("/usr/bin/rclone-does-not-exist")
        );
        assert_eq!(status.resolved_binary_path, None);
    }

    #[test]
    fn cloud_setup_status_reports_runtime_unusable_for_old_rclone() {
        let _lock = TEST_LOCK.lock().expect("test lock");
        let _guard = override_guard();
        let script = TempScript::new(
            "#!/usr/bin/env bash\nset -euo pipefail\nif [[ \"$1\" == \"version\" ]]; then\necho \"rclone v1.66.0\"\nexit 0\nfi\necho \"unexpected command\" >&2\nexit 2\n",
        );
        set_rclone_path_override_for_tests(Some(script.path_str()));

        let status = inspect_cloud_setup_sync();

        assert_eq!(status.state, CloudSetupState::RuntimeUnusable);
        assert_eq!(
            status.resolved_binary_path.as_deref(),
            Some(script.path_str())
        );
    }

    #[test]
    fn cloud_setup_status_reports_no_supported_remotes_when_only_unsupported_exist() {
        let _lock = TEST_LOCK.lock().expect("test lock");
        let _guard = override_guard();
        let sandbox = FakeRcloneSandbox::new();
        sandbox.mkdir_remote("dropbox-work");
        sandbox.set_remote_provider_type("dropbox-work", "dropbox");
        set_rclone_path_override_for_tests(Some(sandbox.path_str()));

        let status = inspect_cloud_setup_sync();

        assert_eq!(status.state, CloudSetupState::NoSupportedRemotes);
        assert_eq!(status.detected_remote_count, 1);
        assert_eq!(status.supported_remote_count, 0);
        assert_eq!(status.unsupported_remote_count, 1);
        assert!(status.supported_remotes.is_empty());
    }

    #[test]
    fn cloud_setup_status_reports_ready_with_supported_remotes() {
        let _lock = TEST_LOCK.lock().expect("test lock");
        let _guard = override_guard();
        let sandbox = FakeRcloneSandbox::new();
        sandbox.mkdir_remote("browsey-gdrive");
        sandbox.set_remote_provider_type("browsey-gdrive", "drive");
        set_rclone_path_override_for_tests(Some(sandbox.path_str()));

        let status = inspect_cloud_setup_sync();

        assert_eq!(status.state, CloudSetupState::Ready);
        assert_eq!(status.detected_remote_count, 1);
        assert_eq!(status.supported_remote_count, 1);
        assert_eq!(status.unsupported_remote_count, 0);
        assert_eq!(status.supported_remotes.len(), 1);
        assert_eq!(status.supported_remotes[0].id, "browsey-gdrive");
    }

    #[test]
    fn cloud_setup_status_counts_unsupported_remotes_without_exposing_them() {
        let _lock = TEST_LOCK.lock().expect("test lock");
        let _guard = override_guard();
        let sandbox = FakeRcloneSandbox::new();
        sandbox.mkdir_remote("browsey-gdrive");
        sandbox.set_remote_provider_type("browsey-gdrive", "drive");
        sandbox.mkdir_remote("dropbox-work");
        sandbox.set_remote_provider_type("dropbox-work", "dropbox");
        set_rclone_path_override_for_tests(Some(sandbox.path_str()));

        let status = inspect_cloud_setup_sync();

        assert_eq!(status.state, CloudSetupState::Ready);
        assert_eq!(status.detected_remote_count, 2);
        assert_eq!(status.supported_remote_count, 1);
        assert_eq!(status.unsupported_remote_count, 1);
        assert_eq!(status.supported_remotes.len(), 1);
        assert_eq!(status.supported_remotes[0].id, "browsey-gdrive");
    }
}
