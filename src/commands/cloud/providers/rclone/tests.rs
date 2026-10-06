use super::{
    error::{
        classify_rclone_message_code, is_rclone_not_found_text, map_rclone_error,
        map_rclone_error_for_provider,
    },
    parse::{
        classify_provider_kind, classify_provider_kind_from_config, parse_config_dump_summaries,
        parse_config_dump_summaries_value, parse_listremotes_plain, parse_lsjson_items,
        parse_lsjson_items_value, parse_lsjson_stat_item, parse_lsjson_stat_item_value,
        parse_rclone_version_stdout, parse_rclone_version_triplet,
    },
    read::normalize_cloud_modified_time_value,
    remotes::{remote_allowed_by_policy_with, RcloneRemotePolicy},
    runtime::{reset_runtime_probe_cache_for_tests, RCLONE_RUNTIME_PROBE_FAILURE_RETRY_BACKOFF},
    write::should_fallback_to_cli_after_rc_error,
    RcloneCloudProvider, RcloneReadOptions,
};
use crate::{
    commands::cloud::{
        clear_cloud_provider_kind_overrides_for_tests,
        error::CloudCommandErrorCode,
        path::CloudPath,
        provider::CloudProvider,
        rclone_cli::RcloneCli,
        rclone_cli::RcloneCliError,
        rclone_cli::RcloneSubcommand,
        rclone_rc::RcloneRcClient,
        set_cloud_provider_kind_override_for_tests,
        types::{CloudEntryKind, CloudProviderKind},
    },
    errors::domain::{DomainError, ErrorCode},
};
use std::{
    process::ExitStatus,
    time::{Duration, Instant},
};

#[cfg(unix)]
use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    thread,
    time::{SystemTime, UNIX_EPOCH},
};

#[cfg(unix)]
fn fake_exit_status(code: i32) -> ExitStatus {
    use std::os::unix::process::ExitStatusExt;
    ExitStatus::from_raw(code << 8)
}

#[cfg(windows)]
fn fake_exit_status(code: u32) -> ExitStatus {
    use std::os::windows::process::ExitStatusExt;
    ExitStatus::from_raw(code)
}

#[test]
fn parses_listremotes_plain_output() {
    let out = parse_listremotes_plain("work:\npersonal:\n\n").expect("parse");
    assert_eq!(out, vec!["work".to_string(), "personal".to_string()]);
}

#[test]
fn parses_config_dump_config_summaries() {
    let json = r#"{
          "work": {"type":"onedrive","token":"secret"},
          "photos": {"type":"drive"},
          "nc": {"type":"webdav","vendor":"nextcloud","url":"https://cloud.example/remote.php/dav/files/user","pass":"***"},
          "misc": {"provider":"something"}
        }"#;
    let map = parse_config_dump_summaries(json).expect("parse json");
    assert_eq!(
        map.get("work").map(|c| c.backend_type.as_str()),
        Some("onedrive")
    );
    assert_eq!(
        map.get("photos").map(|c| c.backend_type.as_str()),
        Some("drive")
    );
    assert_eq!(
        map.get("nc").and_then(|c| c.vendor.as_deref()),
        Some("nextcloud")
    );
    assert!(map.get("nc").map(|c| c.has_password).unwrap_or(false));
    assert!(!map.contains_key("misc"));
}

#[test]
fn parses_config_dump_config_summaries_from_value() {
    let value = serde_json::json!({
        "work": {"type":"onedrive","token":"secret"},
        "photos": {"type":"drive"},
        "nc": {"type":"webdav","vendor":"nextcloud","url":"https://cloud.example/remote.php/dav/files/user","pass":"***"},
        "misc": {"provider":"something"}
    });
    let map = parse_config_dump_summaries_value(value).expect("parse value");
    assert_eq!(
        map.get("work").map(|c| c.backend_type.as_str()),
        Some("onedrive")
    );
    assert_eq!(
        map.get("photos").map(|c| c.backend_type.as_str()),
        Some("drive")
    );
    assert_eq!(
        map.get("nc").and_then(|c| c.vendor.as_deref()),
        Some("nextcloud")
    );
    assert!(map.get("nc").map(|c| c.has_password).unwrap_or(false));
    assert!(!map.contains_key("misc"));
}

#[test]
fn classifies_supported_provider_types() {
    assert_eq!(
        classify_provider_kind("onedrive"),
        Some(CloudProviderKind::Onedrive)
    );
    assert_eq!(
        classify_provider_kind("drive"),
        Some(CloudProviderKind::Gdrive)
    );
    assert_eq!(classify_provider_kind("webdav"), None);
}

#[test]
fn classifies_nextcloud_from_webdav_config_metadata() {
    let map = parse_config_dump_summaries(
            r#"{
              "nc-vendor": {"type":"webdav","vendor":"nextcloud","url":"https://cloud.example/remote.php/dav/files/user"},
              "nc-url": {"type":"webdav","url":"https://nextcloud.example/remote.php/dav/files/user"},
              "plain-webdav": {"type":"webdav","url":"https://dav.example/remote.php/webdav"}
            }"#,
        )
        .expect("parse config dump");
    assert_eq!(
        classify_provider_kind_from_config(map.get("nc-vendor").expect("nc-vendor")),
        Some(CloudProviderKind::Nextcloud)
    );
    assert_eq!(
        classify_provider_kind_from_config(map.get("nc-url").expect("nc-url")),
        Some(CloudProviderKind::Nextcloud)
    );
    assert_eq!(
        classify_provider_kind_from_config(map.get("plain-webdav").expect("plain-webdav")),
        None
    );
}

#[test]
fn parses_lsjson_items() {
    let json = r#"[
          {"Name":"Folder","IsDir":true,"Size":0,"ModTime":"2026-02-25T10:00:00Z"},
          {"Name":"note.txt","IsDir":false,"Size":12,"ModTime":"2026-02-25T10:01:00Z"}
        ]"#;
    let items = parse_lsjson_items(json).expect("parse lsjson");
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].name, "Folder");
    assert!(items[0].is_dir);
    assert_eq!(items[1].name, "note.txt");
    assert_eq!(items[1].size, Some(12));
}

#[test]
fn parses_lsjson_items_from_value() {
    let value = serde_json::json!([
        {"Name":"Folder","IsDir":true,"Size":0,"ModTime":"2026-02-25T10:00:00Z"},
        {"Name":"note.txt","IsDir":false,"Size":12,"ModTime":"2026-02-25T10:01:00Z"}
    ]);
    let items = parse_lsjson_items_value(value).expect("parse lsjson value");
    assert_eq!(items.len(), 2);
    assert_eq!(items[0].name, "Folder");
    assert_eq!(items[1].name, "note.txt");
    assert_eq!(items[1].size, Some(12));
}

#[test]
fn parses_lsjson_items_with_negative_directory_size() {
    let json = r#"[
          {"Name":"Folder","IsDir":true,"Size":-1,"ModTime":"2026-02-25T10:00:00Z"}
        ]"#;
    let items = parse_lsjson_items(json).expect("parse lsjson with -1 dir size");
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].name, "Folder");
    assert!(items[0].is_dir);
    assert_eq!(items[0].size, None);
}

#[test]
fn parses_lsjson_stat_item() {
    let json = r#"{"Name":"note.txt","IsDir":false,"Size":12,"ModTime":"2026-02-25T10:01:00Z"}"#;
    let item = parse_lsjson_stat_item(json).expect("parse lsjson stat");
    assert_eq!(item.name, "note.txt");
    assert!(!item.is_dir);
    assert_eq!(item.size, Some(12));
}

#[test]
fn parses_lsjson_stat_item_from_value() {
    let value = serde_json::json!({"Name":"note.txt","IsDir":false,"Size":12,"ModTime":"2026-02-25T10:01:00Z"});
    let item = parse_lsjson_stat_item_value(value).expect("parse lsjson stat value");
    assert_eq!(item.name, "note.txt");
    assert!(!item.is_dir);
    assert_eq!(item.size, Some(12));
}

#[test]
fn normalizes_rclone_rfc3339_mod_time_to_browsey_format() {
    let out = normalize_cloud_modified_time_value("2026-02-25T10:01:45Z");
    assert!(out.len() == 16, "expected YYYY-MM-DD HH:MM, got {out}");
    assert!(
        out.contains(' '),
        "expected local Browsey time format, got {out}"
    );
    assert!(!out.contains('T'), "expected normalized format, got {out}");
}

#[test]
fn detects_not_found_rclone_messages() {
    assert!(is_rclone_not_found_text(
        "Failed to lsjson: object not found",
        ""
    ));
    assert!(is_rclone_not_found_text("", "directory not found"));
    assert!(!is_rclone_not_found_text("permission denied", ""));
}

#[test]
fn classifies_common_rclone_error_messages() {
    assert_eq!(
        classify_rclone_message_code(
            CloudProviderKind::Onedrive,
            "Failed to move: destination exists"
        ),
        CloudCommandErrorCode::DestinationExists
    );
    assert_eq!(
        classify_rclone_message_code(CloudProviderKind::Onedrive, "Permission denied"),
        CloudCommandErrorCode::PermissionDenied
    );
    assert_eq!(
        classify_rclone_message_code(CloudProviderKind::Onedrive, "object not found"),
        CloudCommandErrorCode::NotFound
    );
    assert_eq!(
        classify_rclone_message_code(
            CloudProviderKind::Onedrive,
            "HTTP error 429: too many requests"
        ),
        CloudCommandErrorCode::RateLimited
    );
    assert_eq!(
        classify_rclone_message_code(
            CloudProviderKind::Onedrive,
            "authentication failed: token expired"
        ),
        CloudCommandErrorCode::AuthRequired
    );
    assert_eq!(
        classify_rclone_message_code(
            CloudProviderKind::Nextcloud,
            "x509: certificate signed by unknown authority"
        ),
        CloudCommandErrorCode::TlsCertificateError
    );
    assert_eq!(
        classify_rclone_message_code(CloudProviderKind::Onedrive, "dial tcp: i/o timeout"),
        CloudCommandErrorCode::Timeout
    );
}

#[test]
fn parses_rclone_version_output() {
    let out = "rclone v1.69.1\n- os/version: fedora 41\n";
    assert_eq!(parse_rclone_version_stdout(out).as_deref(), Some("1.69.1"));
    assert_eq!(parse_rclone_version_stdout("not-rclone\n"), None);
}

#[test]
fn parses_rclone_version_triplet_with_suffixes() {
    assert_eq!(parse_rclone_version_triplet("1.69.1"), Some((1, 69, 1)));
    assert_eq!(
        parse_rclone_version_triplet("1.68.0-beta.1"),
        Some((1, 68, 0))
    );
    assert_eq!(parse_rclone_version_triplet("v1.69.1"), None);
    assert_eq!(parse_rclone_version_triplet("1.69"), None);
}

#[test]
fn maps_rclone_timeout_to_cloud_timeout_error_code() {
    let err = map_rclone_error(RcloneCliError::Timeout {
        subcommand: RcloneSubcommand::CopyTo,
        timeout: Duration::from_secs(10),
        stdout: String::new(),
        stderr: "timed out".to_string(),
    });
    assert_eq!(err.code_str(), CloudCommandErrorCode::Timeout.as_code_str());
    assert!(err.to_string().contains("timed out"));
}

#[test]
fn maps_rclone_nonzero_stderr_to_cloud_error_code() {
    let err = map_rclone_error(RcloneCliError::NonZero {
        status: fake_exit_status(1),
        stdout: String::new(),
        stderr: "HTTP error 429: too many requests".to_string(),
    });
    assert_eq!(
        err.code_str(),
        CloudCommandErrorCode::RateLimited.as_code_str()
    );
}

#[test]
fn maps_async_job_unknown_to_task_failed_with_guidance() {
    let err = map_rclone_error(RcloneCliError::AsyncJobStateUnknown {
        subcommand: RcloneSubcommand::Rc,
        operation: "operations/copyfile".to_string(),
        job_id: 42,
        reason: "job/status failed: connection reset https://example.invalid/upload/PRIVATE?tempauth=PRIVATE".to_string(),
    });
    assert_eq!(
        err.code_str(),
        CloudCommandErrorCode::TaskFailed.as_code_str()
    );
    let msg = err.to_string();
    assert!(
        msg.contains("status is unknown"),
        "unexpected message: {msg}"
    );
    assert!(
        msg.contains("did not retry automatically"),
        "unexpected message: {msg}"
    );
    assert!(msg.contains("job 42"), "unexpected message: {msg}");
    assert!(!msg.contains("PRIVATE"));
}

#[test]
fn async_job_unknown_error_is_not_cli_fallback_safe() {
    let unknown = RcloneCliError::AsyncJobStateUnknown {
        subcommand: RcloneSubcommand::Rc,
        operation: "operations/deletefile".to_string(),
        job_id: 7,
        reason: "job/status timed out".to_string(),
    };
    assert!(!should_fallback_to_cli_after_rc_error(&unknown));

    let timeout = RcloneCliError::Timeout {
        subcommand: RcloneSubcommand::Rc,
        timeout: Duration::from_secs(5),
        stdout: String::new(),
        stderr: "timed out".to_string(),
    };
    assert!(should_fallback_to_cli_after_rc_error(&timeout));
}

#[test]
fn oversized_output_cannot_trigger_a_second_cloud_write() {
    assert!(!should_fallback_to_cli_after_rc_error(
        &RcloneCliError::OutputLimit {
            subcommand: RcloneSubcommand::Rc,
            stream: "response",
            limit: 128,
        }
    ));
}

#[test]
fn submitted_and_failed_jobs_are_not_retried_or_laundered_into_io_errors() {
    let failed = RcloneCliError::AsyncJobFailed {
        operation: "operations/copyfile".into(),
        job_id: 42,
        message: "permission denied https://example.invalid/uploadSession('ID')?tempauth=PRIVATE"
            .into(),
    };
    assert!(!should_fallback_to_cli_after_rc_error(&failed));
    let error = map_rclone_error(failed);
    assert_eq!(error.code_str(), "permission_denied");
    assert!(!error.to_string().contains("PRIVATE"));
    let unknown = RcloneCliError::WriteStateUnknown {
        operation: "operations/copyfile".into(),
        cause: Box::new(RcloneCliError::Io(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "permission denied",
        ))),
    };
    assert!(!should_fallback_to_cli_after_rc_error(&unknown));
    assert_eq!(map_rclone_error(unknown).code_str(), "task_failed");
}

#[test]
fn io_feedback_and_debug_redact_signed_odata_upload_urls() {
    let error = RcloneCliError::Io(std::io::Error::other(
        "Put https://example.invalid/uploadSession('ID')?tempauth=PRIVATE: connection reset",
    ));
    assert!(!format!("{error:?}").contains("PRIVATE"));
    assert!(!error.to_string().contains("PRIVATE"));
    assert!(!map_rclone_error(error).to_string().contains("PRIVATE"));
}

#[test]
fn provider_specific_error_mapping_does_not_leak_between_providers() {
    // Policy unit tests own the hint rules; exercise the complete adapter here,
    // including output selection, common-error precedence and message trimming.
    for (provider, hint_code) in [
        (
            CloudProviderKind::Onedrive,
            CloudCommandErrorCode::RateLimited,
        ),
        (
            CloudProviderKind::Gdrive,
            CloudCommandErrorCode::UnknownError,
        ),
        (
            CloudProviderKind::Nextcloud,
            CloudCommandErrorCode::UnknownError,
        ),
    ] {
        for (stderr, stdout, expected_code, expected_message) in [
            (
                " \nActivityLimitReached\t ",
                "Permission denied",
                hint_code,
                "ActivityLimitReached",
            ),
            (
                " \t\n ",
                " \nActivityLimitReached\t ",
                hint_code,
                "ActivityLimitReached",
            ),
            (
                "Permission denied",
                "ActivityLimitReached",
                CloudCommandErrorCode::PermissionDenied,
                "Permission denied",
            ),
            (
                "ActivityLimitReached: Permission denied",
                "",
                CloudCommandErrorCode::PermissionDenied,
                "ActivityLimitReached: Permission denied",
            ),
        ] {
            let error = map_rclone_error_for_provider(
                provider,
                RcloneCliError::NonZero {
                    status: fake_exit_status(1),
                    stdout: stdout.to_string(),
                    stderr: stderr.to_string(),
                },
            );
            assert_eq!(
                error.code_str(),
                expected_code.as_code_str(),
                "provider={provider:?}, stderr={stderr:?}, stdout={stdout:?}"
            );
            assert_eq!(
                error.message(),
                expected_message,
                "provider={provider:?}, stderr={stderr:?}, stdout={stdout:?}"
            );
        }
    }
}

#[test]
fn remote_policy_filters_by_allowlist_and_prefix() {
    let policy = RcloneRemotePolicy {
        allowlist: Some(
            ["browsey-work", "browsey-personal"]
                .into_iter()
                .map(ToOwned::to_owned)
                .collect(),
        ),
        prefix: Some("browsey-".to_string()),
    };
    assert!(remote_allowed_by_policy_with(&policy, "browsey-work"));
    assert!(!remote_allowed_by_policy_with(&policy, "work"));
    assert!(!remote_allowed_by_policy_with(&policy, "browsey-other"));
}

#[cfg(unix)]
#[test]
fn runtime_probe_recovers_after_initial_failure_for_same_binary_path() {
    use std::os::unix::fs::PermissionsExt;

    reset_runtime_probe_cache_for_tests();

    static NEXT_ID: AtomicU64 = AtomicU64::new(1);
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("time")
        .as_nanos();
    let seq = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let unique = format!(
        "browsey-rclone-runtime-probe-{}-{}-{}",
        std::process::id(),
        nanos,
        seq
    );
    let root = std::env::temp_dir().join(unique);
    fs::create_dir_all(&root).expect("create temp root");

    let binary_path = root.join("rclone");
    let provider = RcloneCloudProvider::new(RcloneCli::new(binary_path.as_os_str()));

    let first = provider
        .ensure_runtime_ready()
        .expect_err("initial runtime probe should fail with missing binary");
    assert_eq!(
        first.code_str(),
        CloudCommandErrorCode::BinaryMissing.as_code_str()
    );

    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/support/fake-rclone.sh");
    fs::copy(&source, &binary_path).expect("copy fake rclone script");
    let mut perms = fs::metadata(&binary_path)
        .expect("script metadata")
        .permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&binary_path, perms).expect("chmod fake rclone");

    thread::sleep(RCLONE_RUNTIME_PROBE_FAILURE_RETRY_BACKOFF + Duration::from_millis(3));
    provider
        .ensure_runtime_ready()
        .expect("runtime probe should recover after binary appears");

    let _ = fs::remove_dir_all(&root);
    reset_runtime_probe_cache_for_tests();
}

#[cfg(unix)]
struct FakeRcloneSandbox {
    root: PathBuf,
    script_path: PathBuf,
    state_root: PathBuf,
    log_path: PathBuf,
}

#[cfg(unix)]
impl FakeRcloneSandbox {
    fn new() -> Self {
        static NEXT_ID: AtomicU64 = AtomicU64::new(1);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let seq = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let unique = format!(
            "browsey-fake-rclone-{}-{}-{}",
            std::process::id(),
            nanos,
            seq
        );
        let root = std::env::temp_dir().join(unique);
        let state_root = root.join("state");
        let script_path = root.join("rclone");
        let log_path = root.join("fake-rclone.log");
        fs::create_dir_all(&state_root).expect("create state root");
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
            log_path,
        }
    }

    fn provider(&self) -> RcloneCloudProvider {
        RcloneCloudProvider::new(RcloneCli::new(self.script_path.as_os_str()))
    }

    fn provider_with_forced_rc(&self) -> RcloneCloudProvider {
        crate::commands::cloud::rclone_rc::reset_state_for_tests();
        let cli = RcloneCli::new(self.script_path.as_os_str());
        let rc =
            RcloneRcClient::new(self.script_path.as_os_str()).with_enabled_override_for_tests(true);
        RcloneCloudProvider { cli, rc }
    }

    fn provider_with_forced_rc_async_status_error_for_delete(&self) -> RcloneCloudProvider {
        crate::commands::cloud::rclone_rc::reset_state_for_tests();
        let cli = RcloneCli::new(self.script_path.as_os_str());
        let rc = RcloneRcClient::new(self.script_path.as_os_str())
            .with_enabled_override_for_tests(true)
            .with_forced_async_status_error_on_delete_for_tests(
                std::io::ErrorKind::ConnectionReset,
            );
        RcloneCloudProvider { cli, rc }
    }

    fn provider_with_forced_rc_async_status_error_for_copy(&self) -> RcloneCloudProvider {
        crate::commands::cloud::rclone_rc::reset_state_for_tests();
        let cli = RcloneCli::new(self.script_path.as_os_str());
        let rc = RcloneRcClient::new(self.script_path.as_os_str())
            .with_enabled_override_for_tests(true)
            .with_forced_async_status_error_on_copy_for_tests(std::io::ErrorKind::ConnectionReset);
        RcloneCloudProvider { cli, rc }
    }

    fn remote_path(&self, remote: &str, rel: &str) -> PathBuf {
        let base = self.state_root.join(remote);
        if rel.is_empty() {
            base
        } else {
            base.join(rel)
        }
    }

    fn mkdir_remote(&self, remote: &str, rel: &str) {
        fs::create_dir_all(self.remote_path(remote, rel)).expect("mkdir remote path");
    }

    fn write_remote_file(&self, remote: &str, rel: &str, content: &str) {
        let path = self.remote_path(remote, rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("mkdir parent");
        }
        fs::write(path, content).expect("write remote file");
    }

    fn read_log(&self) -> String {
        fs::read_to_string(&self.log_path).unwrap_or_default()
    }

    fn mark_mkdir_destination_exists_once(&self) {
        fs::write(self.root.join("mkdir-destination-exists-once"), "1")
            .expect("mark mkdir destination exists once");
    }

    fn mark_mkdir_destination_exists_always(&self) {
        fs::write(self.root.join("mkdir-destination-exists-always"), "1")
            .expect("mark mkdir destination exists always");
    }

    fn set_remote_provider_type(&self, remote: &str, backend_type: &str) {
        let provider_root = self.root.join("provider-types");
        fs::create_dir_all(&provider_root).expect("create provider type root");
        fs::write(provider_root.join(remote), backend_type).expect("set remote provider type");
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

    fn subcommand_delay_notify_path(&self, subcommand: &str) -> PathBuf {
        self.root.join(format!("{subcommand}-delay-notify"))
    }

    fn mark_config_dump_failure(&self) {
        fs::write(self.root.join("config-dump-fail"), "1").expect("mark config dump failure");
    }
}

#[cfg(unix)]
impl Drop for FakeRcloneSandbox {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

#[cfg(unix)]
fn cloud_path(raw: &str) -> CloudPath {
    CloudPath::parse(raw).expect("valid cloud path")
}

#[cfg(unix)]
fn assert_create_delete_recreate_same_folder(sandbox: &FakeRcloneSandbox, remote: &str) {
    let provider = sandbox.provider_with_forced_rc();
    let path = cloud_path(&format!("rclone://{remote}/roundtrip/folder"));

    provider
        .mkdir(&path, None)
        .expect("initial create should succeed");
    assert!(
        sandbox.remote_path(remote, "roundtrip/folder").is_dir(),
        "folder should exist after initial create"
    );

    provider
        .delete_dir_empty(&path, None)
        .expect("delete should succeed");
    assert!(
        !sandbox.remote_path(remote, "roundtrip/folder").exists(),
        "folder should be removed after delete"
    );

    provider
        .mkdir(&path, None)
        .expect("recreate should succeed");
    assert!(
        sandbox.remote_path(remote, "roundtrip/folder").is_dir(),
        "folder should exist after recreate"
    );
}

#[cfg(unix)]
fn assert_copy_move_roundtrip_for_remote(sandbox: &FakeRcloneSandbox, remote: &str) {
    let provider = sandbox.provider_with_forced_rc();
    let src = cloud_path(&format!("rclone://{remote}/src/file.txt"));
    let copied = cloud_path(&format!("rclone://{remote}/dst/copied.txt"));
    let moved = cloud_path(&format!("rclone://{remote}/dst/moved.txt"));

    provider
        .copy_entry(&src, &copied, false, false, None)
        .expect("copy should succeed");
    assert!(
        sandbox.remote_path(remote, "dst/copied.txt").is_file(),
        "copied file should exist"
    );

    provider
        .move_entry(&copied, &moved, false, false, None)
        .expect("move should succeed");
    assert!(
        !sandbox.remote_path(remote, "dst/copied.txt").exists(),
        "copied source should be removed after move"
    );
    assert!(
        sandbox.remote_path(remote, "dst/moved.txt").is_file(),
        "moved file should exist"
    );
}

#[cfg(unix)]
#[test]
fn fake_rclone_shim_lists_remotes_and_directory_entries() {
    let sandbox = FakeRcloneSandbox::new();
    sandbox.mkdir_remote("work", "Docs");
    sandbox.write_remote_file("work", "note.txt", "hello cloud");
    let provider = sandbox.provider();

    let remotes = provider.list_remotes().expect("list remotes");
    assert_eq!(remotes.len(), 1);
    assert_eq!(remotes[0].id, "work");
    assert_eq!(remotes[0].provider, CloudProviderKind::Onedrive);
    assert_eq!(remotes[0].root_path, "rclone://work");

    let entries = provider
        .list_dir(&cloud_path("rclone://work"))
        .expect("list dir");
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].name, "Docs");
    assert_eq!(entries[0].kind, CloudEntryKind::Dir);
    assert_eq!(entries[1].name, "note.txt");
    assert_eq!(entries[1].kind, CloudEntryKind::File);
    assert_eq!(entries[1].size, Some("hello cloud".len() as u64));

    let log = sandbox.read_log();
    // `rclone version` may be skipped here because runtime probe is cached process-wide.
    assert!(log.contains("listremotes"));
    assert!(log.contains("config dump"));
    assert!(log.contains("lsjson work:"));
}

#[cfg(unix)]
#[test]
fn interactive_list_dir_falls_back_from_rc_to_cli() {
    let sandbox = FakeRcloneSandbox::new();
    sandbox.mkdir_remote("work", "Docs");
    sandbox.write_remote_file("work", "note.txt", "hello cloud");
    let provider = sandbox.provider_with_forced_rc();

    let entries = provider
        .list_dir_with_read_options(
            &cloud_path("rclone://work"),
            RcloneReadOptions {
                cancel: None,
                rc_timeout: Some(Duration::from_millis(40)),
                cli_timeout: Some(Duration::from_secs(1)),
                ..RcloneReadOptions::default()
            },
        )
        .expect("interactive list should fall back to cli");

    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].name, "Docs");
    assert_eq!(entries[1].name, "note.txt");
    let log = sandbox.read_log();
    assert!(
        log.contains("lsjson work:"),
        "expected cli lsjson fallback, log:\n{log}"
    );
}

#[cfg(unix)]
#[test]
fn interactive_list_dir_returns_cancelled_when_cli_fallback_is_cancelled() {
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };

    let sandbox = FakeRcloneSandbox::new();
    sandbox.mkdir_remote("work", "Docs");
    sandbox.configure_subcommand_delay("lsjson", 250, 1);
    let provider = sandbox.provider_with_forced_rc();
    let cancel = Arc::new(AtomicBool::new(false));
    let cancel_for_thread = cancel.clone();
    let notify_path = sandbox.subcommand_delay_notify_path("lsjson");
    let worker = thread::spawn(move || {
        let started = Instant::now();
        while !notify_path.exists() {
            assert!(
                started.elapsed() < Duration::from_secs(1),
                "timed out waiting for fake-rclone lsjson delay"
            );
            thread::sleep(Duration::from_millis(10));
        }
        cancel_for_thread.store(true, Ordering::SeqCst);
    });

    let err = provider
        .list_dir_with_read_options(
            &cloud_path("rclone://work"),
            RcloneReadOptions {
                cancel: Some(cancel.as_ref()),
                rc_timeout: Some(Duration::from_millis(40)),
                cli_timeout: Some(Duration::from_secs(2)),
                ..RcloneReadOptions::default()
            },
        )
        .expect_err("interactive list should cancel");
    worker.join().expect("join cancel worker");

    assert_eq!(
        err.code_str(),
        CloudCommandErrorCode::Cancelled.as_code_str()
    );
}

#[cfg(unix)]
#[test]
fn interactive_list_dir_timeout_is_bounded_to_custom_cli_budget() {
    let sandbox = FakeRcloneSandbox::new();
    sandbox.mkdir_remote("work", "Docs");
    sandbox.configure_subcommand_delay("lsjson", 220, 1);
    let provider = sandbox.provider_with_forced_rc();
    let started = std::time::Instant::now();

    let err = provider
        .list_dir_with_read_options(
            &cloud_path("rclone://work"),
            RcloneReadOptions {
                cancel: None,
                rc_timeout: Some(Duration::from_millis(40)),
                cli_timeout: Some(Duration::from_millis(60)),
                ..RcloneReadOptions::default()
            },
        )
        .expect_err("interactive list should respect short timeout");

    assert_eq!(err.code_str(), CloudCommandErrorCode::Timeout.as_code_str());
    assert!(
        started.elapsed() < Duration::from_secs(1),
        "interactive cloud list should fail fast, elapsed={:?}",
        started.elapsed()
    );
}

#[cfg(unix)]
#[test]
fn fake_rclone_shim_supports_copy_move_and_delete_operations() {
    let sandbox = FakeRcloneSandbox::new();
    sandbox.write_remote_file("work", "src/file.txt", "payload");
    sandbox.write_remote_file("work", "trash/sub/old.txt", "gone");
    let provider = sandbox.provider();

    provider
        .mkdir(&cloud_path("rclone://work/dst"), None)
        .expect("mkdir dst");
    provider
        .copy_entry(
            &cloud_path("rclone://work/src/file.txt"),
            &cloud_path("rclone://work/dst/copied.txt"),
            false,
            false,
            None,
        )
        .expect("copy file");
    assert!(sandbox.remote_path("work", "dst/copied.txt").is_file());

    let copied_stat = provider
        .stat_path(&cloud_path("rclone://work/dst/copied.txt"))
        .expect("stat copied")
        .expect("copied exists");
    assert_eq!(copied_stat.name, "copied.txt");
    assert_eq!(copied_stat.kind, CloudEntryKind::File);

    provider
        .move_entry(
            &cloud_path("rclone://work/dst/copied.txt"),
            &cloud_path("rclone://work/dst/moved.txt"),
            false,
            false,
            None,
        )
        .expect("move file");
    assert!(!sandbox.remote_path("work", "dst/copied.txt").exists());
    assert!(sandbox.remote_path("work", "dst/moved.txt").exists());

    provider
        .delete_file(&cloud_path("rclone://work/dst/moved.txt"), None)
        .expect("delete file");
    assert!(!sandbox.remote_path("work", "dst/moved.txt").exists());

    provider
        .delete_dir_empty(&cloud_path("rclone://work/dst"), None)
        .expect("delete empty dir");
    assert!(!sandbox.remote_path("work", "dst").exists());

    provider
        .delete_dir_recursive(&cloud_path("rclone://work/trash"), None)
        .expect("purge dir");
    assert!(!sandbox.remote_path("work", "trash").exists());

    let log = sandbox.read_log();
    assert!(log.contains("mkdir work:dst"));
    assert!(log.contains("copyto work:src/file.txt work:dst/copied.txt"));
    assert!(log.contains("moveto work:dst/copied.txt work:dst/moved.txt"));
    assert!(log.contains("deletefile --onedrive-hard-delete work:dst/moved.txt"));
    assert!(log.contains("rmdir --onedrive-hard-delete work:dst"));
    assert!(log.contains("purge --onedrive-hard-delete work:trash"));
}

#[cfg(unix)]
#[test]
fn fake_rclone_shim_downloads_cloud_file_to_local_path() {
    let sandbox = FakeRcloneSandbox::new();
    sandbox.write_remote_file("work", "src/file.txt", "payload");
    let provider = sandbox.provider();
    let local_root = sandbox.root.join("local-downloads");
    let local_target = local_root.join("file.txt");

    provider
        .download_file(
            &cloud_path("rclone://work/src/file.txt"),
            &local_target,
            None,
        )
        .expect("download file");

    let downloaded = fs::read_to_string(&local_target).expect("read downloaded file");
    assert_eq!(downloaded, "payload");

    let log = sandbox.read_log();
    assert!(
        log.contains(&format!(
            "copyto work:src/file.txt {}",
            local_target.display()
        )),
        "expected CLI copyto call for download, log:\n{log}"
    );
}

#[cfg(unix)]
#[test]
fn fake_rclone_shim_uploads_local_file_to_cloud_path() {
    let sandbox = FakeRcloneSandbox::new();
    let provider = sandbox.provider();
    let local_root = sandbox.root.join("local-uploads");
    let local_source = local_root.join("file.txt");
    fs::create_dir_all(&local_root).expect("create local upload root");
    fs::write(&local_source, "payload").expect("write local upload source");

    provider
        .upload_file_with_progress(
            &local_source,
            &cloud_path("rclone://work/dst/file.txt"),
            "upload-progress-1",
            None,
            |_bytes, _total| {},
        )
        .expect("upload file");

    let uploaded = fs::read_to_string(sandbox.remote_path("work", "dst/file.txt"))
        .expect("read uploaded remote file");
    assert_eq!(uploaded, "payload");

    let log = sandbox.read_log();
    assert!(
        log.contains(&format!(
            "copyto {} work:dst/file.txt",
            local_source.display()
        )),
        "expected CLI copyto call for upload, log:\n{log}"
    );
}

#[cfg(unix)]
#[test]
fn cloud_trash_overrides_destructive_provider_flags_and_keeps_recoverable_content() {
    for (remote, backend, flag) in [
        ("work", "onedrive", "--onedrive-hard-delete=false"),
        ("drive-work", "drive", "--drive-use-trash=true"),
    ] {
        let sandbox = FakeRcloneSandbox::new();
        sandbox.set_remote_provider_type(remote, backend);
        sandbox.write_remote_file(remote, "photo.jpg", "keep photo");
        sandbox.write_remote_file(remote, "album/sub/photo.jpg", "keep album");
        let provider = sandbox.provider();
        provider
            .trash_entry(&cloud_path(&format!("rclone://{remote}/photo.jpg")), None)
            .unwrap();
        provider
            .trash_entry(&cloud_path(&format!("rclone://{remote}/album")), None)
            .unwrap();
        assert!(!sandbox.remote_path(remote, "photo.jpg").exists());
        assert_eq!(
            fs::read_to_string(sandbox.state_root.join(".trash/photo.jpg")).unwrap(),
            "keep photo"
        );
        assert_eq!(
            fs::read_to_string(sandbox.state_root.join(".trash/album/sub/photo.jpg")).unwrap(),
            "keep album"
        );
        let log = sandbox.read_log();
        assert!(
            log.contains(&format!("deletefile {flag} {remote}:photo.jpg")),
            "{log}"
        );
        assert!(
            log.contains(&format!("purge {flag} {remote}:album")),
            "{log}"
        );
    }
}

#[cfg(unix)]
#[test]
fn cloud_trash_refuses_nextcloud_and_remote_roots_without_deleting() {
    let sandbox = FakeRcloneSandbox::new();
    sandbox.set_remote_provider_type("nc", "nextcloud");
    set_cloud_provider_kind_override_for_tests("nc", CloudProviderKind::Nextcloud);
    sandbox.write_remote_file("nc", "photo.jpg", "keep");
    let provider = sandbox.provider();
    assert_eq!(
        provider
            .trash_entry(&cloud_path("rclone://nc/photo.jpg"), None)
            .unwrap_err()
            .code(),
        CloudCommandErrorCode::Unsupported
    );
    assert_eq!(
        provider
            .trash_entry(&cloud_path("rclone://nc"), None)
            .unwrap_err()
            .code(),
        CloudCommandErrorCode::InvalidPath
    );
    assert_eq!(
        provider
            .delete_dir_recursive(&cloud_path("rclone://nc"), None)
            .unwrap_err()
            .code(),
        CloudCommandErrorCode::InvalidPath
    );
    assert_eq!(
        fs::read_to_string(sandbox.remote_path("nc", "photo.jpg")).unwrap(),
        "keep"
    );
    assert!(!sandbox.read_log().contains("purge"));
    clear_cloud_provider_kind_overrides_for_tests();
}

#[cfg(unix)]
#[test]
fn new_cloud_upload_preserves_original_and_refuses_existing_destination() {
    let sandbox = FakeRcloneSandbox::new();
    sandbox.write_remote_file("work", "original.txt", "original");
    let local = sandbox.root.join("edited.txt");
    fs::write(&local, "edited").unwrap();
    let provider = sandbox.provider();
    let original = cloud_path("rclone://work/original.txt");
    assert_eq!(
        provider
            .upload_new_file(&local, &original, None)
            .unwrap_err()
            .code(),
        CloudCommandErrorCode::DestinationExists
    );
    provider
        .upload_new_file(&local, &cloud_path("rclone://work/new.txt"), None)
        .unwrap();
    assert_eq!(
        fs::read_to_string(sandbox.remote_path("work", "original.txt")).unwrap(),
        "original"
    );
    assert_eq!(
        fs::read_to_string(sandbox.remote_path("work", "new.txt")).unwrap(),
        "edited"
    );
    assert!(sandbox.read_log().contains("copyto --immutable --checksum"));
}

#[cfg(unix)]
#[test]
fn new_cloud_upload_cancellation_keeps_local_and_cloud_files() {
    let sandbox = FakeRcloneSandbox::new();
    sandbox.write_remote_file("work", "original.txt", "original");
    let local = sandbox.root.join("edited.txt");
    fs::write(&local, "edited").unwrap();
    let token = std::sync::atomic::AtomicBool::new(true);
    let result = sandbox.provider().upload_new_file(
        &local,
        &cloud_path("rclone://work/new.txt"),
        Some(&token),
    );
    assert_eq!(result.unwrap_err().code(), CloudCommandErrorCode::Cancelled);
    assert_eq!(fs::read_to_string(local).unwrap(), "edited");
    assert!(!sandbox.remote_path("work", "new.txt").exists());
    assert!(!sandbox.read_log().contains("copyto"));
}

#[cfg(unix)]
#[test]
fn edited_working_copy_detects_changed_source_and_preserves_both_versions() {
    use crate::commands::cloud::workspace::{create_at, load_at, upload_at};
    let sandbox = FakeRcloneSandbox::new();
    sandbox.write_remote_file("work", "report.txt", "original");
    let provider = sandbox.provider();
    let source = cloud_path("rclone://work/report.txt");
    let entry = provider.stat_path(&source).unwrap().unwrap();
    let cached = sandbox.root.join("download");
    provider.download_file(&source, &cached, None).unwrap();
    let base = sandbox.root.join("working-copies");
    let copy = create_at(&base, &source, &cached, entry.size, entry.modified).unwrap();
    fs::write(&copy.local_path, "my edits").unwrap();
    sandbox.write_remote_file("work", "report.txt", "new data");
    let result = upload_at(&base, &copy.id, &provider, None).unwrap();
    assert!(result.source_changed);
    let target = cloud_path(&result.path);
    assert_ne!(target, source);
    assert_eq!(
        fs::read_to_string(sandbox.remote_path("work", "report.txt")).unwrap(),
        "new data"
    );
    assert_eq!(
        fs::read_to_string(sandbox.remote_path("work", target.rel_path())).unwrap(),
        "my edits"
    );
    assert_eq!(fs::read_to_string(&copy.local_path).unwrap(), "my edits");
    assert_eq!(
        load_at(&base, &copy.id).unwrap().uploaded_path,
        Some(result.path)
    );
}

#[cfg(unix)]
#[test]
fn failed_working_copy_upload_retains_edits_and_manifest_for_retry() {
    use crate::commands::cloud::workspace::{create_at, load_at, upload_at};
    for failure in ["network connection reset", "quota exceeded"] {
        let sandbox = FakeRcloneSandbox::new();
        sandbox.write_remote_file("work", "report.txt", "original");
        let provider = sandbox.provider();
        let source = cloud_path("rclone://work/report.txt");
        let cached = sandbox.root.join("download");
        fs::write(&cached, "original").unwrap();
        let base = sandbox.root.join("working-copies");
        // Different metadata means source comparison does not need a download;
        // this failure is injected into the upload itself, not the initial read.
        let copy = create_at(&base, &source, &cached, None, None).unwrap();
        fs::write(&copy.local_path, "my edits").unwrap();
        fs::write(sandbox.root.join("transfer-failure"), failure).unwrap();
        assert!(upload_at(&base, &copy.id, &provider, None).is_err());
        assert!(load_at(&base, &copy.id).unwrap().dirty);
        assert_eq!(load_at(&base, &copy.id).unwrap().uploaded_path, None);
        assert_eq!(fs::read_to_string(&copy.local_path).unwrap(), "my edits");
        assert_eq!(
            fs::read_to_string(sandbox.remote_path("work", "report.txt")).unwrap(),
            "original"
        );
        fs::remove_file(sandbox.root.join("transfer-failure")).unwrap();
        assert!(upload_at(&base, &copy.id, &provider, None).is_ok());
    }
}

#[cfg(unix)]
#[test]
fn active_cloud_upload_cancellation_stops_child_without_removing_edits() {
    use std::sync::{atomic::AtomicBool, Arc};
    let sandbox = FakeRcloneSandbox::new();
    sandbox.mkdir_remote("work", "");
    sandbox.configure_subcommand_delay("copyto", 10_000, 1);
    let local = sandbox.root.join("edited.txt");
    fs::write(&local, "edited").unwrap();
    let token = Arc::new(AtomicBool::new(false));
    let worker_token = token.clone();
    let provider = sandbox.provider();
    let worker_local = local.clone();
    let worker = thread::spawn(move || {
        provider.upload_new_file(
            &worker_local,
            &cloud_path("rclone://work/new.txt"),
            Some(&worker_token),
        )
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    while !sandbox.subcommand_delay_notify_path("copyto").exists() {
        assert!(Instant::now() < deadline, "upload did not start");
        thread::sleep(Duration::from_millis(10));
    }
    token.store(true, Ordering::Relaxed);
    assert_eq!(
        worker.join().unwrap().unwrap_err().code(),
        CloudCommandErrorCode::Cancelled
    );
    assert_eq!(fs::read_to_string(local).unwrap(), "edited");
    assert!(!sandbox.remote_path("work", "new.txt").exists());
}

#[cfg(unix)]
#[test]
fn new_cloud_upload_does_not_recreate_a_deleted_destination_folder() {
    let sandbox = FakeRcloneSandbox::new();
    sandbox.mkdir_remote("work", "");
    let local = sandbox.root.join("edited.txt");
    fs::write(&local, "edited").unwrap();
    let result = sandbox.provider().upload_new_file(
        &local,
        &cloud_path("rclone://work/deleted/new.txt"),
        None,
    );
    assert_eq!(result.unwrap_err().code(), CloudCommandErrorCode::NotFound);
    assert!(!sandbox.remote_path("work", "deleted").exists());
    assert!(!sandbox.read_log().contains("copyto"));
}

#[cfg(unix)]
#[test]
fn new_cloud_upload_refuses_a_target_created_after_preflight() {
    let sandbox = FakeRcloneSandbox::new();
    sandbox.mkdir_remote("work", "");
    sandbox.configure_subcommand_delay("copyto", 200, 1);
    let local = sandbox.root.join("edited.txt");
    fs::write(&local, "my edits").unwrap();
    let provider = sandbox.provider();
    let worker_local = local.clone();
    let worker = thread::spawn(move || {
        provider.upload_new_file(&worker_local, &cloud_path("rclone://work/new.txt"), None)
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    while !sandbox.subcommand_delay_notify_path("copyto").exists() {
        assert!(Instant::now() < deadline, "upload did not start");
        thread::sleep(Duration::from_millis(5));
    }
    sandbox.write_remote_file("work", "new.txt", "someone else");
    assert!(worker.join().unwrap().is_err());
    assert_eq!(
        fs::read_to_string(sandbox.remote_path("work", "new.txt")).unwrap(),
        "someone else"
    );
    assert_eq!(fs::read_to_string(local).unwrap(), "my edits");
    assert!(sandbox.read_log().contains("copyto --immutable --checksum"));
}

#[cfg(unix)]
#[test]
fn cloud_batch_rename_preflights_all_targets_before_any_move() {
    use crate::commands::{cloud::batch_rename::rename_batch, rename::RenameEntryRequest};
    let sandbox = FakeRcloneSandbox::new();
    sandbox.write_remote_file("work", "a.txt", "a");
    sandbox.write_remote_file("work", "b.txt", "b");
    sandbox.write_remote_file("work", "exists.txt", "existing");
    let entries = vec![
        RenameEntryRequest {
            path: "rclone://work/a.txt".into(),
            new_name: "new-a.txt".into(),
        },
        RenameEntryRequest {
            path: "rclone://work/b.txt".into(),
            new_name: "exists.txt".into(),
        },
    ];
    assert!(rename_batch(&sandbox.provider(), entries).is_err());
    assert_eq!(
        fs::read_to_string(sandbox.remote_path("work", "a.txt")).unwrap(),
        "a"
    );
    assert!(!sandbox.read_log().contains("moveto"));
}

#[cfg(unix)]
#[test]
fn upload_with_progress_falls_back_to_cli_when_rc_startup_fails() {
    let sandbox = FakeRcloneSandbox::new();
    let provider = sandbox.provider_with_forced_rc();
    let local_root = sandbox.root.join("local-uploads");
    let local_source = local_root.join("file.txt");
    fs::create_dir_all(&local_root).expect("create local upload root");
    fs::write(&local_source, "payload").expect("write local upload source");

    provider
        .upload_file_with_progress(
            &local_source,
            &cloud_path("rclone://work/dst/file.txt"),
            "upload-progress-rc-fallback",
            None,
            |_bytes, _total| {},
        )
        .expect("upload file with rc fallback");

    let uploaded = fs::read_to_string(sandbox.remote_path("work", "dst/file.txt"))
        .expect("read uploaded remote file");
    assert_eq!(uploaded, "payload");

    let log = sandbox.read_log();
    assert!(
        log.contains("rcd --rc-no-auth"),
        "expected rc daemon startup attempt before fallback, log:\n{log}"
    );
    assert!(
        log.contains(&format!(
            "copyto {} work:dst/file.txt",
            local_source.display()
        )),
        "expected CLI upload fallback call after rc failure, log:\n{log}"
    );
}

#[cfg(unix)]
#[test]
fn fake_rclone_shim_supports_case_only_rename() {
    for directory in [false, true] {
        let sandbox = FakeRcloneSandbox::new();
        fs::write(sandbox.root.join("stat-case-insensitive"), "1").unwrap();
        let original = if directory {
            "docs/tree"
        } else {
            "docs/report.txt"
        };
        let renamed = if directory {
            "docs/TREE"
        } else {
            "docs/Report.txt"
        };
        let contents = if directory {
            "docs/tree/nested.txt"
        } else {
            original
        };
        sandbox.write_remote_file("work", contents, "payload");
        let provider = sandbox.provider();
        let src = cloud_path(&format!("rclone://work/{original}"));
        let dst = cloud_path(&format!("rclone://work/{renamed}"));
        assert!(
            provider.stat_path(&dst).unwrap().is_some(),
            "Destination metadata resolves the source alias"
        );
        provider
            .move_entry(&src, &dst, false, false, None)
            .expect("case-only rename");
        assert!(!sandbox.remote_path("work", original).exists());
        let result = if directory {
            "docs/TREE/nested.txt"
        } else {
            renamed
        };
        assert_eq!(
            fs::read_to_string(sandbox.remote_path("work", result)).unwrap(),
            "payload"
        );
    }
}

#[cfg(unix)]
#[test]
fn case_only_directory_rename_preserves_recovery_paths_and_does_not_retry_failed_stage() {
    for failed_stage in [1, 2] {
        let sandbox = FakeRcloneSandbox::new();
        fs::write(sandbox.root.join("stat-case-insensitive"), "1").unwrap();
        fs::write(
            sandbox.root.join("moveto-fail-invocation"),
            failed_stage.to_string(),
        )
        .unwrap();
        sandbox.write_remote_file("work", "docs/tree/nested.txt", "payload");
        sandbox.write_remote_file("work", "docs/unrelated.txt", "preserve");
        let error = sandbox
            .provider()
            .move_entry(
                &cloud_path("rclone://work/docs/tree"),
                &cloud_path("rclone://work/docs/TREE"),
                false,
                false,
                None,
            )
            .unwrap_err();
        assert!(error.to_string().contains("may be incomplete"));
        assert!(error.to_string().contains("No automatic retry or rollback"));
        let siblings = fs::read_dir(sandbox.remote_path("work", "docs"))
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .collect::<Vec<_>>();
        let recovery = siblings.iter().find(|path| path.is_dir()).unwrap();
        assert_eq!(
            fs::read_to_string(recovery.join("nested.txt")).unwrap(),
            "payload"
        );
        assert_eq!(
            fs::read_to_string(sandbox.remote_path("work", "docs/unrelated.txt")).unwrap(),
            "preserve"
        );
        if failed_stage == 1 {
            assert_eq!(recovery, &sandbox.remote_path("work", "docs/tree"));
        } else {
            assert!(recovery
                .file_name()
                .unwrap()
                .to_str()
                .unwrap()
                .starts_with(".browsey-rename-"));
            assert!(error
                .to_string()
                .contains(recovery.file_name().unwrap().to_str().unwrap()));
        }
        assert!(!sandbox.remote_path("work", "docs/TREE").exists());
        assert_eq!(
            siblings.len(),
            2,
            "Only the intact tree and unrelated file remain"
        );
        assert_eq!(
            sandbox
                .read_log()
                .lines()
                .filter(|line| line.contains(" moveto "))
                .count(),
            failed_stage
        );
    }
}

#[cfg(unix)]
#[test]
fn case_only_alias_does_not_bypass_copy_other_parent_or_case_sensitive_collisions() {
    for (copy, source, destination, provider_type) in [
        (true, "docs/report.txt", "docs/Report.txt", "onedrive"),
        (false, "docs/report.txt", "other/Report.txt", "onedrive"),
        (false, "docs/report.txt", "docs/Report.txt", "drive"),
    ] {
        let sandbox = FakeRcloneSandbox::new();
        fs::write(sandbox.root.join("stat-case-insensitive"), "1").unwrap();
        sandbox.set_remote_provider_type("work", provider_type);
        sandbox.write_remote_file("work", source, "source");
        if !copy {
            sandbox.write_remote_file("work", destination, "occupied");
        }
        let provider = sandbox.provider();
        let src = cloud_path(&format!("rclone://work/{source}"));
        let dst = cloud_path(&format!("rclone://work/{destination}"));
        let error = if copy {
            provider.copy_entry(&src, &dst, false, false, None)
        } else {
            provider.move_entry(&src, &dst, false, false, None)
        }
        .unwrap_err();
        assert_eq!(error.code(), CloudCommandErrorCode::DestinationExists);
        assert_eq!(
            fs::read_to_string(sandbox.remote_path("work", source)).unwrap(),
            "source"
        );
        if !copy {
            assert_eq!(
                fs::read_to_string(sandbox.remote_path("work", destination)).unwrap(),
                "occupied"
            );
        }
        assert!(!sandbox.read_log().contains("moveto"));
        assert!(!sandbox.read_log().contains("copyto"));
    }
}

#[cfg(unix)]
#[test]
fn read_path_falls_back_to_cli_when_rc_startup_fails() {
    let sandbox = FakeRcloneSandbox::new();
    sandbox.write_remote_file("work", "note.txt", "hello");
    let provider = sandbox.provider_with_forced_rc();

    let entries = provider
        .list_dir(&cloud_path("rclone://work"))
        .expect("list dir with fallback");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].name, "note.txt");

    let log = sandbox.read_log();
    assert!(
        log.contains("rcd --rc-no-auth"),
        "expected rc daemon to start without auth on private unix socket, log:\n{log}"
    );
    assert!(
        log.contains("--rc-addr"),
        "expected rc daemon startup attempt before fallback, log:\n{log}"
    );
    assert!(
        log.contains("unix://"),
        "expected unix socket rc endpoint (no TCP listener), log:\n{log}"
    );
    assert!(
        log.contains("lsjson work:"),
        "expected CLI fallback list call after rc failure, log:\n{log}"
    );
}

#[cfg(unix)]
#[test]
fn mkdir_uses_cli_path_without_rc_daemon() {
    let sandbox = FakeRcloneSandbox::new();
    let provider = sandbox.provider_with_forced_rc();

    provider
        .mkdir(&cloud_path("rclone://work/new-folder"), None)
        .expect("mkdir should succeed");
    assert!(sandbox.remote_path("work", "new-folder").is_dir());

    let log = sandbox.read_log();
    assert!(
        !log.contains("rcd --rc-no-auth"),
        "mkdir should not start rc daemon, log:\n{log}"
    );
    assert!(
        log.contains("mkdir work:new-folder"),
        "expected CLI mkdir call, log:\n{log}"
    );
}

#[cfg(unix)]
#[test]
fn interactive_folder_creation_rejects_existing_file_and_directory_without_mkdir() {
    for directory in [false, true] {
        let sandbox = FakeRcloneSandbox::new();
        if directory {
            sandbox.mkdir_remote("work", "existing");
            sandbox.write_remote_file("work", "existing/sentinel.txt", "preserved");
        } else {
            sandbox.write_remote_file("work", "existing", "preserved");
        }
        let err = crate::commands::cloud::write::create_new_cloud_folder(
            &sandbox.provider(),
            &cloud_path("rclone://work/existing"),
            None,
        )
        .expect_err("Interactive creation must reject an existing name");
        assert_eq!(
            err.code_str(),
            CloudCommandErrorCode::DestinationExists.as_code_str()
        );
        assert!(!sandbox.read_log().contains("mkdir work:existing"));
        let preserved = if directory {
            "existing/sentinel.txt"
        } else {
            "existing"
        };
        assert_eq!(
            std::fs::read_to_string(sandbox.remote_path("work", preserved)).unwrap(),
            "preserved"
        );
    }
}

#[cfg(unix)]
#[test]
fn interactive_folder_creation_creates_once_and_cancelled_creation_never_probes_or_writes() {
    let sandbox = FakeRcloneSandbox::new();
    let provider = sandbox.provider();
    let cancelled = std::sync::atomic::AtomicBool::new(true);
    let err = crate::commands::cloud::write::create_new_cloud_folder(
        &provider,
        &cloud_path("rclone://work/cancelled"),
        Some(&cancelled),
    )
    .expect_err("Already cancelled creation must stop before provider I/O");
    assert_eq!(
        err.code_str(),
        CloudCommandErrorCode::Cancelled.as_code_str()
    );
    assert!(sandbox.read_log().is_empty());
    crate::commands::cloud::write::create_new_cloud_folder(
        &provider,
        &cloud_path("rclone://work/new-folder"),
        None,
    )
    .expect("A new folder must be created");
    assert!(sandbox.remote_path("work", "new-folder").is_dir());
    assert_eq!(
        sandbox.read_log().matches("mkdir work:new-folder").count(),
        1
    );
}

#[cfg(unix)]
#[test]
fn mkdir_retries_destination_exists_when_probe_shows_missing_target() {
    let sandbox = FakeRcloneSandbox::new();
    sandbox.mark_mkdir_destination_exists_once();
    let provider = sandbox.provider();

    provider
        .mkdir(&cloud_path("rclone://work/new-folder"), None)
        .expect("mkdir should retry and succeed");
    assert!(sandbox.remote_path("work", "new-folder").is_dir());

    let log = sandbox.read_log();
    assert_eq!(
        log.matches("mkdir work:new-folder").count(),
        2,
        "expected mkdir retry after transient destination_exists, log:\n{log}"
    );
    assert!(
        log.contains("lsjson --stat work:new-folder"),
        "expected CLI stat probe before retry, log:\n{log}"
    );
}

#[cfg(unix)]
#[test]
fn mkdir_keeps_destination_exists_when_probe_confirms_existing_target() {
    let sandbox = FakeRcloneSandbox::new();
    sandbox.mkdir_remote("work", "existing-folder");
    sandbox.mark_mkdir_destination_exists_always();
    let provider = sandbox.provider();

    let err = provider
        .mkdir(&cloud_path("rclone://work/existing-folder"), None)
        .expect_err("mkdir should fail with destination_exists");
    assert_eq!(
        err.code_str(),
        CloudCommandErrorCode::DestinationExists.as_code_str()
    );

    let log = sandbox.read_log();
    assert_eq!(
        log.matches("mkdir work:existing-folder").count(),
        1,
        "mkdir must not retry when probe confirms destination exists, log:\n{log}"
    );
    assert!(
        log.contains("lsjson --stat work:existing-folder"),
        "expected CLI stat probe for destination_exists decision, log:\n{log}"
    );
}

#[cfg(unix)]
#[test]
fn create_delete_recreate_same_name_roundtrip_succeeds_for_onedrive() {
    let sandbox = FakeRcloneSandbox::new();
    assert_create_delete_recreate_same_folder(&sandbox, "work");
}

#[cfg(unix)]
#[test]
fn create_delete_recreate_same_name_roundtrip_succeeds_for_gdrive() {
    let sandbox = FakeRcloneSandbox::new();
    sandbox.set_remote_provider_type("drive-work", "drive");
    assert_create_delete_recreate_same_folder(&sandbox, "drive-work");
}

#[cfg(unix)]
#[test]
fn create_delete_recreate_same_name_roundtrip_succeeds_for_nextcloud() {
    let sandbox = FakeRcloneSandbox::new();
    sandbox.set_remote_provider_type("nc-work", "nextcloud");
    assert_create_delete_recreate_same_folder(&sandbox, "nc-work");
}

#[cfg(unix)]
#[test]
fn copy_move_roundtrip_succeeds_for_onedrive() {
    let sandbox = FakeRcloneSandbox::new();
    sandbox.write_remote_file("work", "src/file.txt", "payload");
    assert_copy_move_roundtrip_for_remote(&sandbox, "work");
}

#[cfg(unix)]
#[test]
fn copy_move_roundtrip_succeeds_for_gdrive() {
    let sandbox = FakeRcloneSandbox::new();
    sandbox.set_remote_provider_type("drive-work", "drive");
    sandbox.write_remote_file("drive-work", "src/file.txt", "payload");
    assert_copy_move_roundtrip_for_remote(&sandbox, "drive-work");
}

#[cfg(unix)]
#[test]
fn copy_move_roundtrip_succeeds_for_nextcloud() {
    let sandbox = FakeRcloneSandbox::new();
    sandbox.set_remote_provider_type("nc-work", "nextcloud");
    sandbox.write_remote_file("nc-work", "src/file.txt", "payload");
    assert_copy_move_roundtrip_for_remote(&sandbox, "nc-work");
}

#[cfg(unix)]
#[test]
fn delete_ops_use_cli_delete_policy_flags() {
    let sandbox = FakeRcloneSandbox::new();
    sandbox.write_remote_file("work", "trash/file.txt", "payload");
    sandbox.write_remote_file("work", "trash-deep/sub/old.txt", "payload");
    let provider = sandbox.provider_with_forced_rc();

    provider
        .delete_file(&cloud_path("rclone://work/trash/file.txt"), None)
        .expect("delete file");
    assert!(!sandbox.remote_path("work", "trash/file.txt").exists());

    provider
        .delete_dir_empty(&cloud_path("rclone://work/trash"), None)
        .expect("delete empty dir");
    assert!(!sandbox.remote_path("work", "trash").exists());

    provider
        .delete_dir_recursive(&cloud_path("rclone://work/trash-deep"), None)
        .expect("delete recursive dir");
    assert!(!sandbox.remote_path("work", "trash-deep").exists());

    let log = sandbox.read_log();
    assert!(
        !log.contains("rcd --rc-no-auth"),
        "delete commands should not start rc daemon, log:\n{log}"
    );
    assert!(
        log.contains("deletefile --onedrive-hard-delete work:trash/file.txt"),
        "expected OneDrive hard-delete file policy flag, log:\n{log}"
    );
    assert!(
        log.contains("rmdir --onedrive-hard-delete work:trash"),
        "expected OneDrive hard-delete dir-empty policy flag, log:\n{log}"
    );
    assert!(
        log.contains("purge --onedrive-hard-delete work:trash-deep"),
        "expected OneDrive hard-delete recursive-delete policy flag, log:\n{log}"
    );
}

#[cfg(unix)]
#[test]
fn delete_ops_use_gdrive_cli_delete_policy_flags() {
    let sandbox = FakeRcloneSandbox::new();
    sandbox.write_remote_file("drive-work", "trash/file.txt", "payload");
    sandbox.write_remote_file("drive-work", "trash-deep/sub/old.txt", "payload");
    sandbox.set_remote_provider_type("drive-work", "drive");
    let provider = sandbox.provider_with_forced_rc();

    provider
        .delete_file(&cloud_path("rclone://drive-work/trash/file.txt"), None)
        .expect("delete file");
    assert!(!sandbox.remote_path("drive-work", "trash/file.txt").exists());

    provider
        .delete_dir_empty(&cloud_path("rclone://drive-work/trash"), None)
        .expect("delete empty dir");
    assert!(!sandbox.remote_path("drive-work", "trash").exists());

    provider
        .delete_dir_recursive(&cloud_path("rclone://drive-work/trash-deep"), None)
        .expect("delete recursive dir");
    assert!(!sandbox.remote_path("drive-work", "trash-deep").exists());

    let log = sandbox.read_log();
    assert!(
        log.contains("deletefile --drive-use-trash=false drive-work:trash/file.txt"),
        "expected Google Drive delete file policy flag, log:\n{log}"
    );
    assert!(
        log.contains("rmdir --drive-use-trash=false drive-work:trash"),
        "expected Google Drive delete empty-dir policy flag, log:\n{log}"
    );
    assert!(
        log.contains("purge --drive-use-trash=false drive-work:trash-deep"),
        "expected Google Drive recursive-delete policy flag, log:\n{log}"
    );
}

#[cfg(unix)]
#[test]
fn delete_ops_use_nextcloud_default_policy_without_provider_flags() {
    let sandbox = FakeRcloneSandbox::new();
    sandbox.write_remote_file("nc-work", "trash/file.txt", "payload");
    sandbox.write_remote_file("nc-work", "trash-deep/sub/old.txt", "payload");
    sandbox.set_remote_provider_type("nc-work", "nextcloud");
    let provider = sandbox.provider_with_forced_rc();

    provider
        .delete_file(&cloud_path("rclone://nc-work/trash/file.txt"), None)
        .expect("delete file");
    assert!(!sandbox.remote_path("nc-work", "trash/file.txt").exists());

    provider
        .delete_dir_empty(&cloud_path("rclone://nc-work/trash"), None)
        .expect("delete empty dir");
    assert!(!sandbox.remote_path("nc-work", "trash").exists());

    provider
        .delete_dir_recursive(&cloud_path("rclone://nc-work/trash-deep"), None)
        .expect("delete recursive dir");
    assert!(!sandbox.remote_path("nc-work", "trash-deep").exists());

    let log = sandbox.read_log();
    assert!(
        log.contains("deletefile nc-work:trash/file.txt"),
        "expected Nextcloud delete command without provider delete policy flags, log:\n{log}"
    );
    assert!(
        log.contains("rmdir nc-work:trash"),
        "expected Nextcloud rmdir command without provider delete policy flags, log:\n{log}"
    );
    assert!(
        log.contains("purge nc-work:trash-deep"),
        "expected Nextcloud purge command without provider delete policy flags, log:\n{log}"
    );
    assert!(
        !log.contains("--onedrive-hard-delete"),
        "Nextcloud flow must not inherit OneDrive delete policy flags, log:\n{log}"
    );
    assert!(
        !log.contains("--drive-use-trash=false"),
        "Nextcloud flow must not inherit Google Drive delete policy flags, log:\n{log}"
    );
}

#[cfg(unix)]
#[test]
fn delete_fails_when_delete_policy_lookup_cannot_be_verified() {
    let sandbox = FakeRcloneSandbox::new();
    sandbox.write_remote_file("work", "trash/file.txt", "payload");
    sandbox.mark_config_dump_failure();
    let provider = sandbox.provider_with_forced_rc();

    let err = provider
        .delete_file(&cloud_path("rclone://work/trash/file.txt"), None)
        .expect_err("delete should fail when policy lookup cannot be verified");
    assert!(
        err.to_string()
            .contains("Cloud write policy lookup failed for remote `work`"),
        "unexpected error: {err}"
    );
    assert!(
        sandbox.remote_path("work", "trash/file.txt").exists(),
        "file should remain when policy lookup fails"
    );

    let log = sandbox.read_log();
    assert!(
        !log.contains("deletefile --onedrive-hard-delete work:trash/file.txt"),
        "deletefile must not run after policy lookup failure, log:\n{log}"
    );
}

#[cfg(unix)]
#[test]
fn delete_uses_cached_provider_policy_when_config_dump_fails() {
    clear_cloud_provider_kind_overrides_for_tests();
    let sandbox = FakeRcloneSandbox::new();
    let remote = format!("cache-delete-policy-{}", std::process::id());
    sandbox.write_remote_file(&remote, "trash/file.txt", "payload");
    sandbox.mark_config_dump_failure();
    set_cloud_provider_kind_override_for_tests(&remote, CloudProviderKind::Onedrive);

    let provider = sandbox.provider_with_forced_rc();
    provider
        .delete_file(
            &cloud_path(&format!("rclone://{remote}/trash/file.txt")),
            None,
        )
        .expect("delete should use cached provider policy");
    assert!(
        !sandbox.remote_path(&remote, "trash/file.txt").exists(),
        "file should be removed"
    );

    let log = sandbox.read_log();
    assert!(
        log.contains(&format!(
            "deletefile --onedrive-hard-delete {remote}:trash/file.txt"
        )),
        "expected cached OneDrive hard-delete policy flag, log:\n{log}"
    );
    assert!(
        !log.contains("config dump"),
        "cache-first delete policy lookup should avoid config dump, log:\n{log}"
    );
    clear_cloud_provider_kind_overrides_for_tests();
}

#[cfg(unix)]
#[test]
fn delete_file_missing_path_maps_to_not_found_code() {
    let sandbox = FakeRcloneSandbox::new();
    sandbox.mkdir_remote("work", "");
    let provider = sandbox.provider_with_forced_rc();

    let err = provider
        .delete_file(&cloud_path("rclone://work/missing/file.txt"), None)
        .expect_err("missing file delete should fail");
    assert_eq!(
        err.code_str(),
        CloudCommandErrorCode::NotFound.as_code_str()
    );
}

#[cfg(unix)]
#[test]
fn copy_move_ops_fall_back_to_cli_when_rc_startup_fails() {
    let sandbox = FakeRcloneSandbox::new();
    sandbox.write_remote_file("work", "src/file.txt", "payload");
    let provider = sandbox.provider_with_forced_rc();

    provider
        .copy_entry(
            &cloud_path("rclone://work/src/file.txt"),
            &cloud_path("rclone://work/dst/copied.txt"),
            false,
            false,
            None,
        )
        .expect("copy with fallback");
    assert!(sandbox.remote_path("work", "dst/copied.txt").exists());

    provider
        .move_entry(
            &cloud_path("rclone://work/dst/copied.txt"),
            &cloud_path("rclone://work/dst/moved.txt"),
            false,
            false,
            None,
        )
        .expect("move with fallback");
    assert!(!sandbox.remote_path("work", "dst/copied.txt").exists());
    assert!(sandbox.remote_path("work", "dst/moved.txt").exists());

    let log = sandbox.read_log();
    assert!(
        log.contains("rcd --rc-no-auth"),
        "expected rc daemon to start without auth on private unix socket, log:\n{log}"
    );
    assert!(
        log.contains("--rc-addr"),
        "expected rc daemon startup attempt before fallback, log:\n{log}"
    );
    assert!(
        log.contains("copyto work:src/file.txt work:dst/copied.txt"),
        "expected CLI copy fallback call, log:\n{log}"
    );
    assert!(
        log.contains("moveto work:dst/copied.txt work:dst/moved.txt"),
        "expected CLI move fallback call, log:\n{log}"
    );
}

#[cfg(unix)]
#[test]
fn download_missing_path_maps_to_not_found_code() {
    let sandbox = FakeRcloneSandbox::new();
    let provider = sandbox.provider();
    let local_target = sandbox.root.join("local-downloads").join("missing.txt");

    let err = provider
        .download_file(
            &cloud_path("rclone://work/missing.txt"),
            &local_target,
            None,
        )
        .expect_err("missing file download should fail");
    assert_eq!(
        err.code_str(),
        CloudCommandErrorCode::NotFound.as_code_str()
    );
}

#[cfg(unix)]
#[test]
fn delete_file_uses_cli_even_with_forced_rc_async_error_hooks() {
    let sandbox = FakeRcloneSandbox::new();
    sandbox.write_remote_file("work", "dst/moved.txt", "payload");
    let provider = sandbox.provider_with_forced_rc_async_status_error_for_delete();

    provider
        .delete_file(&cloud_path("rclone://work/dst/moved.txt"), None)
        .expect("delete should use CLI path");

    let log = sandbox.read_log();
    assert!(
        !log.contains("rcd --rc-no-auth"),
        "delete should not start rc daemon, log:\n{log}"
    );
    assert!(
        log.contains("deletefile --onedrive-hard-delete work:dst/moved.txt"),
        "expected OneDrive hard-delete flag on CLI path, log:\n{log}"
    );
}

#[cfg(unix)]
#[test]
fn copy_does_not_fallback_to_cli_when_rc_async_status_is_unknown() {
    let sandbox = FakeRcloneSandbox::new();
    sandbox.write_remote_file("work", "src/file.txt", "payload");
    let provider = sandbox.provider_with_forced_rc_async_status_error_for_copy();
    let cancel = std::sync::atomic::AtomicBool::new(false);

    let err = provider
        .copy_entry(
            &cloud_path("rclone://work/src/file.txt"),
            &cloud_path("rclone://work/dst/copied.txt"),
            false,
            true,
            Some(&cancel),
        )
        .expect_err("copy should fail with unknown async rc job state");
    assert_eq!(
        err.code_str(),
        CloudCommandErrorCode::TaskFailed.as_code_str()
    );
    let message = err.to_string();
    assert!(
        message.contains("status is unknown"),
        "unexpected message: {message}"
    );
    assert!(
        message.contains("did not retry automatically"),
        "unexpected message: {message}"
    );
    let log = sandbox.read_log();
    assert!(
        !log.contains("copyto work:src/file.txt work:dst/copied.txt"),
        "CLI copyto must not run after unknown async rc state, log:\n{log}"
    );
}

#[cfg(unix)]
#[test]
fn copy_preserves_destination_exists_conflict_policy() {
    let sandbox = FakeRcloneSandbox::new();
    sandbox.write_remote_file("work", "src/file.txt", "source");
    sandbox.write_remote_file("work", "dst/file.txt", "existing");
    let provider = sandbox.provider_with_forced_rc();

    let err = provider
        .copy_entry(
            &cloud_path("rclone://work/src/file.txt"),
            &cloud_path("rclone://work/dst/file.txt"),
            false,
            false,
            None,
        )
        .expect_err("copy should fail when destination exists");
    assert_eq!(
        err.code_str(),
        CloudCommandErrorCode::DestinationExists.as_code_str()
    );
    let existing = fs::read_to_string(sandbox.remote_path("work", "dst/file.txt"))
        .expect("read existing destination");
    assert_eq!(existing, "existing");
}

#[cfg(unix)]
#[test]
fn move_preserves_destination_exists_conflict_policy() {
    let sandbox = FakeRcloneSandbox::new();
    sandbox.write_remote_file("work", "src/file.txt", "source");
    sandbox.write_remote_file("work", "dst/file.txt", "existing");
    let provider = sandbox.provider_with_forced_rc();

    let err = provider
        .move_entry(
            &cloud_path("rclone://work/src/file.txt"),
            &cloud_path("rclone://work/dst/file.txt"),
            false,
            false,
            None,
        )
        .expect_err("move should fail when destination exists");
    assert_eq!(
        err.code_str(),
        CloudCommandErrorCode::DestinationExists.as_code_str()
    );
    assert!(
        sandbox.remote_path("work", "src/file.txt").exists(),
        "source must stay in place after rejected move"
    );
    let existing = fs::read_to_string(sandbox.remote_path("work", "dst/file.txt"))
        .expect("read existing destination");
    assert_eq!(existing, "existing");
}

#[cfg(unix)]
#[test]
fn copy_with_overwrite_true_replaces_destination() {
    let sandbox = FakeRcloneSandbox::new();
    sandbox.write_remote_file("work", "src/file.txt", "source");
    sandbox.write_remote_file("work", "dst/file.txt", "existing");
    let provider = sandbox.provider_with_forced_rc();

    provider
        .copy_entry(
            &cloud_path("rclone://work/src/file.txt"),
            &cloud_path("rclone://work/dst/file.txt"),
            true,
            false,
            None,
        )
        .expect("copy should overwrite destination");
    let copied = fs::read_to_string(sandbox.remote_path("work", "dst/file.txt"))
        .expect("read overwritten destination");
    assert_eq!(copied, "source");
}

#[cfg(unix)]
#[test]
fn fake_rclone_shim_skips_destination_stat_when_copy_is_prechecked() {
    let sandbox = FakeRcloneSandbox::new();
    sandbox.write_remote_file("work", "src/file.txt", "payload");
    let provider = sandbox.provider();

    provider
        .copy_entry(
            &cloud_path("rclone://work/src/file.txt"),
            &cloud_path("rclone://work/dst/copied.txt"),
            false,
            true,
            None,
        )
        .expect("copy file");

    let log = sandbox.read_log();
    assert!(log.contains("copyto work:src/file.txt work:dst/copied.txt"));
    assert!(!log.contains("lsjson --stat work:dst/copied.txt"));
}

#[cfg(unix)]
#[test]
fn fake_rclone_shim_skips_destination_stat_when_move_is_prechecked() {
    let sandbox = FakeRcloneSandbox::new();
    sandbox.write_remote_file("work", "src/file.txt", "payload");
    let provider = sandbox.provider();

    provider
        .move_entry(
            &cloud_path("rclone://work/src/file.txt"),
            &cloud_path("rclone://work/dst/moved.txt"),
            false,
            true,
            None,
        )
        .expect("move file");

    let log = sandbox.read_log();
    assert!(log.contains("moveto work:src/file.txt work:dst/moved.txt"));
    assert!(!log.contains("lsjson --stat work:dst/moved.txt"));
}

#[cfg(unix)]
#[test]
fn directory_copy_preserves_empty_root_and_nested_empty_directories() {
    for force_rc in [false, true] {
        let sandbox = FakeRcloneSandbox::new();
        sandbox.mkdir_remote("work", "source/empty");
        sandbox.mkdir_remote("work", "source/tree/deep/empty");
        sandbox.write_remote_file("work", "source/tree/deep/nested.txt", "nested payload");
        sandbox.write_remote_file("work", "source/unrelated.txt", "unrelated source");
        sandbox.write_remote_file("work", "target/unrelated.txt", "unrelated target");
        let provider = if force_rc {
            sandbox.provider_with_forced_rc()
        } else {
            sandbox.provider()
        };
        for name in ["empty", "tree"] {
            provider
                .copy_entry(
                    &cloud_path(&format!("rclone://work/source/{name}")),
                    &cloud_path(&format!("rclone://work/target/{name}")),
                    false,
                    false,
                    None,
                )
                .unwrap();
        }
        assert!(sandbox.remote_path("work", "target/empty").is_dir());
        assert!(sandbox
            .remote_path("work", "target/tree/deep/empty")
            .is_dir());
        assert!(sandbox.remote_path("work", "source/empty").is_dir());
        assert!(sandbox
            .remote_path("work", "source/tree/deep/empty")
            .is_dir());
        for name in ["source/tree/deep/nested.txt", "target/tree/deep/nested.txt"] {
            assert_eq!(
                fs::read(sandbox.remote_path("work", name)).unwrap(),
                b"nested payload"
            );
        }
        assert_eq!(
            fs::read(sandbox.remote_path("work", "source/unrelated.txt")).unwrap(),
            b"unrelated source"
        );
        assert_eq!(
            fs::read(sandbox.remote_path("work", "target/unrelated.txt")).unwrap(),
            b"unrelated target"
        );
        let log = sandbox.read_log();
        assert_eq!(
            log.lines()
                .filter(|line| line.contains("copy --create-empty-src-dirs "))
                .count(),
            2
        );
        assert!(!log.contains("operations/copyfile"));
    }
}

#[cfg(unix)]
#[test]
fn failed_directory_copy_preserves_source_without_finalizing_or_retrying() {
    let sandbox = FakeRcloneSandbox::new();
    sandbox.mkdir_remote("work", "source/tree/empty");
    sandbox.write_remote_file("work", "source/tree/nested.txt", "source payload");
    sandbox.write_remote_file("work", "target/unrelated.txt", "unrelated target");
    fs::write(
        sandbox.root.join("transfer-failure"),
        "forced directory-copy failure",
    )
    .unwrap();
    let provider = sandbox.provider();
    provider
        .copy_entry(
            &cloud_path("rclone://work/source/tree"),
            &cloud_path("rclone://work/target/tree"),
            false,
            false,
            None,
        )
        .expect_err("The failed write must remain a failure");
    assert_eq!(
        fs::read(sandbox.remote_path("work", "source/tree/nested.txt")).unwrap(),
        b"source payload"
    );
    assert!(sandbox.remote_path("work", "source/tree/empty").is_dir());
    assert!(!sandbox.remote_path("work", "target/tree").exists());
    assert_eq!(
        fs::read(sandbox.remote_path("work", "target/unrelated.txt")).unwrap(),
        b"unrelated target"
    );
    let log = sandbox.read_log();
    assert_eq!(
        log.lines()
            .filter(|line| line.contains("copy --create-empty-src-dirs "))
            .count(),
        1
    );
    assert!(!log.contains("mkdir work:target/tree"));
}

#[cfg(unix)]
#[test]
fn directories_bypass_file_only_rc_copy_and_move() {
    let sandbox = FakeRcloneSandbox::new();
    sandbox.write_remote_file("work", "source/tree/nested.txt", "generated payload");
    let provider = sandbox.provider_with_forced_rc();
    let src = cloud_path("rclone://work/source/tree");
    let copied = cloud_path("rclone://work/target/copied");
    let moved = cloud_path("rclone://work/target/moved");
    provider
        .copy_entry(&src, &copied, false, true, None)
        .unwrap();
    assert_eq!(
        fs::read(sandbox.remote_path("work", "target/copied/nested.txt")).unwrap(),
        b"generated payload"
    );
    assert!(sandbox
        .remote_path("work", "source/tree/nested.txt")
        .exists());
    provider
        .move_entry(&copied, &moved, false, true, None)
        .unwrap();
    assert!(!sandbox.remote_path("work", "target/copied").exists());
    assert_eq!(
        fs::read(sandbox.remote_path("work", "target/moved/nested.txt")).unwrap(),
        b"generated payload"
    );
    let log = sandbox.read_log();
    assert!(log.contains("copy --create-empty-src-dirs work:source/tree work:target/copied"));
    assert!(log.contains("moveto work:target/copied work:target/moved"));
    assert!(!log.contains("operations/copyfile"));
    assert!(!log.contains("operations/movefile"));
}

#[cfg(unix)]
#[test]
fn cross_kind_overwrite_refuses_before_copy_or_move_even_when_prechecked() {
    for source_directory in [false, true] {
        for move_source in [false, true] {
            for prechecked in [false, true] {
                let sandbox = FakeRcloneSandbox::new();
                if source_directory {
                    sandbox.write_remote_file("work", "source/nested.txt", "source bytes");
                    fs::create_dir_all(sandbox.remote_path("work", "source/empty")).unwrap();
                    sandbox.write_remote_file("work", "target", "original destination bytes");
                } else {
                    sandbox.write_remote_file("work", "source", "source bytes");
                    sandbox.write_remote_file(
                        "work",
                        "target/nested.txt",
                        "original destination bytes",
                    );
                    fs::create_dir_all(sandbox.remote_path("work", "target/empty")).unwrap();
                }
                let provider = sandbox.provider();
                let (source, destination) = (
                    cloud_path("rclone://work/source"),
                    cloud_path("rclone://work/target"),
                );
                let error = if move_source {
                    provider.move_entry(&source, &destination, true, prechecked, None)
                } else {
                    provider.copy_entry(&source, &destination, true, prechecked, None)
                }
                .unwrap_err();
                assert_eq!(
                    error.code_str(),
                    CloudCommandErrorCode::Unsupported.as_code_str()
                );
                let source_file = if source_directory {
                    "source/nested.txt"
                } else {
                    "source"
                };
                let destination_file = if source_directory {
                    "target"
                } else {
                    "target/nested.txt"
                };
                assert_eq!(
                    fs::read_to_string(sandbox.remote_path("work", source_file)).unwrap(),
                    "source bytes"
                );
                assert_eq!(
                    fs::read_to_string(sandbox.remote_path("work", destination_file)).unwrap(),
                    "original destination bytes"
                );
                assert!(sandbox
                    .remote_path(
                        "work",
                        if source_directory {
                            "source/empty"
                        } else {
                            "target/empty"
                        }
                    )
                    .is_dir());
                let log = sandbox.read_log();
                assert!(
                    !log.contains("copyto ")
                        && !log.contains("moveto ")
                        && !log.contains("copy work:")
                        && !log.contains("move work:")
                );
            }
        }
    }
}

#[cfg(unix)]
#[test]
fn overwrite_directory_move_merges_nested_bytes_preserves_empty_folders_and_removes_source_root() {
    for force_rc in [false, true] {
        let sandbox = FakeRcloneSandbox::new();
        sandbox.write_remote_file("work", "source/tree/nested.txt", "new nested bytes");
        fs::create_dir_all(sandbox.remote_path("work", "source/tree/empty")).unwrap();
        sandbox.write_remote_file("work", "source/unrelated.txt", "unrelated source");
        sandbox.write_remote_file("work", "target/tree/nested.txt", "old nested bytes");
        sandbox.write_remote_file(
            "work",
            "target/tree/target-only.txt",
            "unchanged destination",
        );
        sandbox.write_remote_file("work", "target/unrelated.txt", "unrelated target");
        let provider = if force_rc {
            sandbox.provider_with_forced_rc()
        } else {
            sandbox.provider()
        };
        provider
            .move_entry(
                &cloud_path("rclone://work/source/tree"),
                &cloud_path("rclone://work/target/tree"),
                true,
                true,
                None,
            )
            .unwrap();
        assert_eq!(
            fs::read_to_string(sandbox.remote_path("work", "target/tree/nested.txt")).unwrap(),
            "new nested bytes"
        );
        assert_eq!(
            fs::read_to_string(sandbox.remote_path("work", "target/tree/target-only.txt")).unwrap(),
            "unchanged destination"
        );
        assert!(sandbox.remote_path("work", "target/tree/empty").is_dir());
        assert!(!sandbox.remote_path("work", "source/tree").exists());
        assert_eq!(
            fs::read_to_string(sandbox.remote_path("work", "source/unrelated.txt")).unwrap(),
            "unrelated source"
        );
        assert_eq!(
            fs::read_to_string(sandbox.remote_path("work", "target/unrelated.txt")).unwrap(),
            "unrelated target"
        );
        let log = sandbox.read_log();
        assert_eq!(
            log.lines().filter(|line| line.contains(" move ")).count(),
            1
        );
        assert!(!log.contains(" moveto ") && !log.contains(" purge "));
    }
}

#[cfg(unix)]
#[test]
fn failed_directory_merge_move_never_retries_or_finalizes_source_or_destination() {
    let sandbox = FakeRcloneSandbox::new();
    sandbox.write_remote_file("work", "source/tree/nested.txt", "original source bytes");
    fs::create_dir_all(sandbox.remote_path("work", "source/tree/empty")).unwrap();
    sandbox.write_remote_file(
        "work",
        "target/tree/nested.txt",
        "original destination bytes",
    );
    fs::write(sandbox.root.join("move-fail-invocation"), "1").unwrap();
    let error = sandbox
        .provider()
        .move_entry(
            &cloud_path("rclone://work/source/tree"),
            &cloud_path("rclone://work/target/tree"),
            true,
            true,
            None,
        )
        .unwrap_err();
    assert!(error.to_string().contains("forced move failure"));
    assert_eq!(
        fs::read_to_string(sandbox.remote_path("work", "source/tree/nested.txt")).unwrap(),
        "original source bytes"
    );
    assert_eq!(
        fs::read_to_string(sandbox.remote_path("work", "target/tree/nested.txt")).unwrap(),
        "original destination bytes"
    );
    assert!(sandbox.remote_path("work", "source/tree/empty").is_dir());
    let log = sandbox.read_log();
    assert_eq!(
        log.lines().filter(|line| line.contains(" move ")).count(),
        1
    );
    assert!(!log.contains(" mkdir ") && !log.contains(" rmdir ") && !log.contains(" purge "));
}
