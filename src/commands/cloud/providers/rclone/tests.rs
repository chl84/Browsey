use super::{
    error::{
        classify_provider_rclone_message_code, classify_rclone_message_code,
        is_rclone_not_found_text, map_rclone_error, map_rclone_error_for_provider,
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
    RcloneCloudProvider,
};
use crate::{
    commands::cloud::{
        error::CloudCommandErrorCode,
        path::CloudPath,
        provider::CloudProvider,
        rclone_cli::RcloneCli,
        rclone_cli::RcloneCliError,
        rclone_cli::RcloneSubcommand,
        rclone_rc::RcloneRcClient,
        types::{CloudEntryKind, CloudProviderKind},
    },
    errors::domain::{DomainError, ErrorCode},
};
use std::{process::ExitStatus, time::Duration};

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
fn provider_specific_rclone_message_mapping_is_isolated() {
    assert_eq!(
        classify_provider_rclone_message_code(
            CloudProviderKind::Onedrive,
            "graph returned ActivityLimitReached"
        ),
        Some(CloudCommandErrorCode::RateLimited)
    );
    assert_eq!(
        classify_provider_rclone_message_code(
            CloudProviderKind::Gdrive,
            "graph returned ActivityLimitReached"
        ),
        None
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
        reason: "job/status failed: connection reset".to_string(),
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
fn provider_specific_error_mapping_does_not_leak_between_providers() {
    let onedrive_err = map_rclone_error_for_provider(
        CloudProviderKind::Onedrive,
        RcloneCliError::NonZero {
            status: fake_exit_status(1),
            stdout: String::new(),
            stderr: "ActivityLimitReached".to_string(),
        },
    );
    let gdrive_err = map_rclone_error_for_provider(
        CloudProviderKind::Gdrive,
        RcloneCliError::NonZero {
            status: fake_exit_status(1),
            stdout: String::new(),
            stderr: "ActivityLimitReached".to_string(),
        },
    );
    assert_eq!(
        onedrive_err.code_str(),
        CloudCommandErrorCode::RateLimited.as_code_str()
    );
    assert_eq!(
        gdrive_err.code_str(),
        CloudCommandErrorCode::UnknownError.as_code_str()
    );
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
    let unique = format!(
        "browsey-rclone-runtime-probe-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos()
            + u128::from(NEXT_ID.fetch_add(1, Ordering::Relaxed))
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
        let unique = format!(
            "browsey-fake-rclone-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time")
                .as_nanos()
                + u128::from(NEXT_ID.fetch_add(1, Ordering::Relaxed))
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
    assert!(log.contains("deletefile work:dst/moved.txt"));
    assert!(log.contains("rmdir work:dst"));
    assert!(log.contains("purge work:trash"));
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
    let sandbox = FakeRcloneSandbox::new();
    sandbox.write_remote_file("work", "docs/report.txt", "payload");
    let provider = sandbox.provider();

    provider
        .move_entry(
            &cloud_path("rclone://work/docs/report.txt"),
            &cloud_path("rclone://work/docs/Report.txt"),
            false,
            false,
            None,
        )
        .expect("case-only rename");

    assert!(!sandbox.remote_path("work", "docs/report.txt").exists());
    assert!(sandbox.remote_path("work", "docs/Report.txt").exists());
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
fn mkdir_falls_back_to_cli_when_rc_startup_fails() {
    let sandbox = FakeRcloneSandbox::new();
    let provider = sandbox.provider_with_forced_rc();

    provider
        .mkdir(&cloud_path("rclone://work/new-folder"), None)
        .expect("mkdir with fallback");
    assert!(sandbox.remote_path("work", "new-folder").is_dir());

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
        log.contains("mkdir work:new-folder"),
        "expected CLI mkdir fallback call after rc failure, log:\n{log}"
    );
}

#[cfg(unix)]
#[test]
fn delete_ops_fall_back_to_cli_when_rc_startup_fails() {
    let sandbox = FakeRcloneSandbox::new();
    sandbox.write_remote_file("work", "trash/file.txt", "payload");
    sandbox.write_remote_file("work", "trash-deep/sub/old.txt", "payload");
    let provider = sandbox.provider_with_forced_rc();

    provider
        .delete_file(&cloud_path("rclone://work/trash/file.txt"), None)
        .expect("delete file with fallback");
    assert!(!sandbox.remote_path("work", "trash/file.txt").exists());

    provider
        .delete_dir_empty(&cloud_path("rclone://work/trash"), None)
        .expect("delete empty dir with fallback");
    assert!(!sandbox.remote_path("work", "trash").exists());

    provider
        .delete_dir_recursive(&cloud_path("rclone://work/trash-deep"), None)
        .expect("delete recursive dir with fallback");
    assert!(!sandbox.remote_path("work", "trash-deep").exists());

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
        log.contains("deletefile work:trash/file.txt"),
        "expected CLI deletefile fallback call, log:\n{log}"
    );
    assert!(
        log.contains("rmdir work:trash"),
        "expected CLI rmdir fallback call, log:\n{log}"
    );
    assert!(
        log.contains("purge work:trash-deep"),
        "expected CLI purge fallback call, log:\n{log}"
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
fn delete_file_does_not_fallback_to_cli_when_rc_async_status_is_unknown() {
    let sandbox = FakeRcloneSandbox::new();
    sandbox.write_remote_file("work", "dst/moved.txt", "payload");
    let provider = sandbox.provider_with_forced_rc_async_status_error_for_delete();
    let cancel = std::sync::atomic::AtomicBool::new(false);

    let err = provider
        .delete_file(&cloud_path("rclone://work/dst/moved.txt"), Some(&cancel))
        .expect_err("delete should fail with unknown async rc job state");
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
        !log.contains("deletefile work:dst/moved.txt"),
        "CLI deletefile must not run after unknown async rc state, log:\n{log}"
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
