use super::*;
use crate::commands::cloud::set_rclone_path_override_for_tests;
#[cfg(unix)]
use std::fs;
#[cfg(unix)]
use std::os::unix::process::ExitStatusExt;
#[cfg(unix)]
use std::path::{Path, PathBuf};
#[cfg(unix)]
use std::sync::{
    atomic::{AtomicBool, AtomicU64, Ordering},
    Arc, Mutex,
};
#[cfg(unix)]
use std::thread;
#[cfg(unix)]
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

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
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time")
            .as_nanos();
        let seq = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let unique = format!(
            "browsey-transfer-fake-rclone-{}-{}-{}",
            std::process::id(),
            nanos,
            seq
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

    fn set_subcommand_delay(&self, subcommand: &str, invocation: usize, delay_ms: u64) {
        fs::write(
            self.root.join(format!("{subcommand}-delay-invocation")),
            invocation.to_string(),
        )
        .expect("write delay invocation");
        fs::write(
            self.root.join(format!("{subcommand}-delay-ms")),
            delay_ms.to_string(),
        )
        .expect("write delay ms");
        let _ = fs::remove_file(self.root.join(format!("{subcommand}-delay-notify")));
        let _ = fs::remove_file(self.root.join(format!("{subcommand}-count")));
    }

    fn wait_for_subcommand_delay(&self, subcommand: &str, timeout: Duration) {
        let notify = self.root.join(format!("{subcommand}-delay-notify"));
        let started = Instant::now();
        while !notify.exists() {
            assert!(
                started.elapsed() < timeout,
                "timed out waiting for fake-rclone {subcommand} delay"
            );
            thread::sleep(Duration::from_millis(10));
        }
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

#[cfg(unix)]
#[test]
fn mixed_execute_local_to_cloud_multi_file_copy_without_progress_event_succeeds() {
    let _guard = fake_rclone_test_lock();
    let sandbox = FakeRcloneSandbox::new();
    sandbox.mkdir_remote("work", "dest");
    let cli = sandbox.cli();

    let src_a = sandbox.write_local_file("src/a.txt", "alpha");
    let src_b = sandbox.write_local_file("src/b.txt", "beta");
    let route = MixedTransferRoute::LocalToCloud {
        sources: vec![src_a.clone(), src_b.clone()],
        dest_dir: sandbox.cloud_path("rclone://work/dest"),
    };
    let out = execute_mixed_entries_blocking_with_cli(
        &cli,
        MixedTransferOp::Copy,
        route,
        MixedTransferWriteOptions {
            overwrite: false,
            prechecked: true,
        },
        None,
        None,
    )
    .expect("multi-file local->cloud copy");

    assert_eq!(
        out,
        vec![
            "rclone://work/dest/a.txt".to_string(),
            "rclone://work/dest/b.txt".to_string(),
        ]
    );
    assert_eq!(
        fs::read_to_string(sandbox.remote_path("work", "dest/a.txt")).expect("read remote a"),
        "alpha"
    );
    assert_eq!(
        fs::read_to_string(sandbox.remote_path("work", "dest/b.txt")).expect("read remote b"),
        "beta"
    );
}

fn sample_cloud_cache_entry(path: &str, name: &str) -> crate::commands::cloud::types::CloudEntry {
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
fn mixed_execute_local_to_cloud_partial_copy_keeps_source_and_invalidates_cache() {
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

    let first_src = sandbox.write_local_file("src/first.txt", "first");
    let missing_src = sandbox.local_path("src/missing.txt");
    let route = MixedTransferRoute::LocalToCloud {
        sources: vec![first_src.clone(), missing_src],
        dest_dir: dest_dir.clone(),
    };

    let err = execute_mixed_entries_blocking_with_cli(
        &cli,
        MixedTransferOp::Copy,
        route,
        MixedTransferWriteOptions {
            overwrite: false,
            prechecked: true,
        },
        None,
        None,
    )
    .expect_err("second source should fail and produce partial completion");

    assert!(
        err.code_str() == "not_found" || err.code_str() == "unknown_error",
        "unexpected error code: {} ({err})",
        err.code_str()
    );
    assert!(
        first_src.exists(),
        "copy semantics should keep first local source"
    );
    assert_eq!(
        fs::read_to_string(sandbox.remote_path("work", "dest/first.txt")).expect("read remote"),
        "first"
    );
    assert!(
        !crate::commands::cloud::cloud_dir_listing_cache_contains_for_tests(&dest_dir),
        "partial cloud write should still invalidate destination cache"
    );
}

#[cfg(unix)]
#[test]
fn mixed_execute_local_to_cloud_partial_move_removes_first_source() {
    let _guard = fake_rclone_test_lock();
    let sandbox = FakeRcloneSandbox::new();
    sandbox.mkdir_remote("work", "dest");
    let cli = sandbox.cli();

    let first_src = sandbox.write_local_file("src/first-move.txt", "first-move");
    let missing_src = sandbox.local_path("src/missing-move.txt");
    let route = MixedTransferRoute::LocalToCloud {
        sources: vec![first_src.clone(), missing_src],
        dest_dir: sandbox.cloud_path("rclone://work/dest"),
    };

    let err = execute_mixed_entries_blocking_with_cli(
        &cli,
        MixedTransferOp::Move,
        route,
        MixedTransferWriteOptions {
            overwrite: false,
            prechecked: true,
        },
        None,
        None,
    )
    .expect_err("second source should fail and keep partial move state");

    assert!(
        err.code_str() == "not_found" || err.code_str() == "unknown_error",
        "unexpected error code: {} ({err})",
        err.code_str()
    );
    assert!(
        !first_src.exists(),
        "move semantics should remove first local source after successful transfer"
    );
    assert_eq!(
        fs::read_to_string(sandbox.remote_path("work", "dest/first-move.txt"))
            .expect("read remote moved file"),
        "first-move"
    );
}

#[cfg(unix)]
#[test]
fn mixed_execute_local_to_cloud_partial_directory_move_invalidates_cache_and_keeps_partial_state() {
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

    sandbox.write_local_file("src/dir-ok/nested/file.txt", "dir-payload");
    let first_dir = sandbox.local_path("src/dir-ok");
    let missing_dir = sandbox.local_path("src/dir-missing");
    let route = MixedTransferRoute::LocalToCloud {
        sources: vec![first_dir.clone(), missing_dir],
        dest_dir: dest_dir.clone(),
    };

    let err = execute_mixed_entries_blocking_with_cli(
        &cli,
        MixedTransferOp::Move,
        route,
        MixedTransferWriteOptions {
            overwrite: false,
            prechecked: true,
        },
        None,
        None,
    )
    .expect_err("later missing directory should fail after first success");

    assert!(
        err.code_str() == "not_found" || err.code_str() == "unknown_error",
        "unexpected error code: {} ({err})",
        err.code_str()
    );
    assert!(
        !first_dir.exists(),
        "move semantics should remove first directory source after successful transfer"
    );
    assert_eq!(
        fs::read_to_string(sandbox.remote_path("work", "dest/dir-ok/nested/file.txt"))
            .expect("read moved dir file"),
        "dir-payload"
    );
    assert!(
        !crate::commands::cloud::cloud_dir_listing_cache_contains_for_tests(&dest_dir),
        "partial directory move should still invalidate destination cache"
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
fn mixed_execute_cloud_to_local_multi_file_copy_without_progress_event_succeeds() {
    let _guard = fake_rclone_test_lock();
    let sandbox = FakeRcloneSandbox::new();
    sandbox.write_remote_file("work", "src/a.txt", "alpha");
    sandbox.write_remote_file("work", "src/b.txt", "beta");
    let cli = sandbox.cli();
    let local_dest = sandbox.local_path("dest");
    fs::create_dir_all(&local_dest).expect("mkdir local dest");

    let route = MixedTransferRoute::CloudToLocal {
        sources: vec![
            sandbox.cloud_path("rclone://work/src/a.txt"),
            sandbox.cloud_path("rclone://work/src/b.txt"),
        ],
        dest_dir: local_dest.clone(),
    };
    let out = execute_mixed_entries_blocking_with_cli(
        &cli,
        MixedTransferOp::Copy,
        route,
        MixedTransferWriteOptions {
            overwrite: false,
            prechecked: true,
        },
        None,
        None,
    )
    .expect("multi-file cloud->local copy");

    assert_eq!(
        out,
        vec![
            local_dest.join("a.txt").to_string_lossy().to_string(),
            local_dest.join("b.txt").to_string_lossy().to_string(),
        ]
    );
    assert_eq!(
        fs::read_to_string(local_dest.join("a.txt")).expect("read local a"),
        "alpha"
    );
    assert_eq!(
        fs::read_to_string(local_dest.join("b.txt")).expect("read local b"),
        "beta"
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

#[cfg(unix)]
#[test]
fn mixed_execute_local_to_cloud_returns_cancelled_when_token_is_set_before_start() {
    let _guard = fake_rclone_test_lock();
    let sandbox = FakeRcloneSandbox::new();
    sandbox.mkdir_remote("work", "dest");
    let cli = sandbox.cli();
    let src = sandbox.write_local_file("src/cancel-me.txt", "payload");
    let route = MixedTransferRoute::LocalToCloud {
        sources: vec![src.clone()],
        dest_dir: sandbox.cloud_path("rclone://work/dest"),
    };
    let cancel = Arc::new(AtomicBool::new(true));

    let err = execute_mixed_entries_blocking_with_cli(
        &cli,
        MixedTransferOp::Copy,
        route,
        MixedTransferWriteOptions {
            overwrite: false,
            prechecked: true,
        },
        Some(cancel),
        None,
    )
    .expect_err("transfer should abort before work starts");

    assert_eq!(err.code_str(), "cancelled");
    assert!(
        src.exists(),
        "source should remain unchanged when transfer is cancelled early"
    );
    assert!(
        !sandbox.remote_path("work", "dest/cancel-me.txt").exists(),
        "destination should not be created"
    );
}

#[cfg(unix)]
#[test]
fn mixed_execute_local_to_cloud_copy_cancels_during_second_active_transfer() {
    let _guard = fake_rclone_test_lock();
    let sandbox = FakeRcloneSandbox::new();
    sandbox.mkdir_remote("work", "dest");
    sandbox.set_subcommand_delay("copyto", 2, 1500);
    let cli = sandbox.cli();
    let src_a = sandbox.write_local_file("src/a.txt", "alpha");
    let src_b = sandbox.write_local_file("src/b.txt", "beta");
    let route = MixedTransferRoute::LocalToCloud {
        sources: vec![src_a.clone(), src_b.clone()],
        dest_dir: sandbox.cloud_path("rclone://work/dest"),
    };
    let cancel = Arc::new(AtomicBool::new(false));
    let cancel_for_task = cancel.clone();

    let worker = thread::spawn(move || {
        execute_mixed_entries_blocking_with_cli(
            &cli,
            MixedTransferOp::Copy,
            route,
            MixedTransferWriteOptions {
                overwrite: false,
                prechecked: true,
            },
            Some(cancel_for_task),
            None,
        )
    });

    sandbox.wait_for_subcommand_delay("copyto", Duration::from_secs(3));
    cancel.store(true, Ordering::SeqCst);
    let err = worker
        .join()
        .expect("mixed local->cloud worker thread")
        .expect_err("second transfer should be cancelled");

    assert_eq!(err.code_str(), "cancelled");
    assert_eq!(
        fs::read_to_string(sandbox.remote_path("work", "dest/a.txt")).expect("read first remote"),
        "alpha"
    );
    assert!(
        !sandbox.remote_path("work", "dest/b.txt").exists(),
        "second destination should not be created after cancellation"
    );
    assert!(
        src_a.exists(),
        "copy cancellation should keep first local source"
    );
    assert!(
        src_b.exists(),
        "copy cancellation should keep second local source"
    );
}

#[cfg(unix)]
#[test]
fn mixed_execute_cloud_to_local_copy_cancels_during_second_active_transfer() {
    let _guard = fake_rclone_test_lock();
    let sandbox = FakeRcloneSandbox::new();
    sandbox.write_remote_file("work", "src/a.txt", "alpha");
    sandbox.write_remote_file("work", "src/b.txt", "beta");
    sandbox.set_subcommand_delay("copyto", 2, 1500);
    let cli = sandbox.cli();
    let local_dest = sandbox.local_path("dest");
    fs::create_dir_all(&local_dest).expect("mkdir local dest");
    let route = MixedTransferRoute::CloudToLocal {
        sources: vec![
            sandbox.cloud_path("rclone://work/src/a.txt"),
            sandbox.cloud_path("rclone://work/src/b.txt"),
        ],
        dest_dir: local_dest.clone(),
    };
    let cancel = Arc::new(AtomicBool::new(false));
    let cancel_for_task = cancel.clone();

    let worker = thread::spawn(move || {
        execute_mixed_entries_blocking_with_cli(
            &cli,
            MixedTransferOp::Copy,
            route,
            MixedTransferWriteOptions {
                overwrite: false,
                prechecked: true,
            },
            Some(cancel_for_task),
            None,
        )
    });

    sandbox.wait_for_subcommand_delay("copyto", Duration::from_secs(3));
    cancel.store(true, Ordering::SeqCst);
    let err = worker
        .join()
        .expect("mixed cloud->local worker thread")
        .expect_err("second transfer should be cancelled");

    assert_eq!(err.code_str(), "cancelled");
    assert_eq!(
        fs::read_to_string(local_dest.join("a.txt")).expect("read first local"),
        "alpha"
    );
    assert!(
        !local_dest.join("b.txt").exists(),
        "second local destination should not be created after cancellation"
    );
    assert!(
        sandbox.remote_path("work", "src/a.txt").exists(),
        "copy cancellation should keep first remote source"
    );
    assert!(
        sandbox.remote_path("work", "src/b.txt").exists(),
        "copy cancellation should keep second remote source"
    );
}

#[cfg(unix)]
#[test]
fn mixed_execute_local_to_cloud_move_cancels_during_second_active_transfer() {
    let _guard = fake_rclone_test_lock();
    let sandbox = FakeRcloneSandbox::new();
    sandbox.mkdir_remote("work", "dest");
    sandbox.set_subcommand_delay("moveto", 2, 1500);
    let cli = sandbox.cli();
    let src_a = sandbox.write_local_file("src/a.txt", "alpha");
    let src_b = sandbox.write_local_file("src/b.txt", "beta");
    let route = MixedTransferRoute::LocalToCloud {
        sources: vec![src_a.clone(), src_b.clone()],
        dest_dir: sandbox.cloud_path("rclone://work/dest"),
    };
    let cancel = Arc::new(AtomicBool::new(false));
    let cancel_for_task = cancel.clone();

    let worker = thread::spawn(move || {
        execute_mixed_entries_blocking_with_cli(
            &cli,
            MixedTransferOp::Move,
            route,
            MixedTransferWriteOptions {
                overwrite: false,
                prechecked: true,
            },
            Some(cancel_for_task),
            None,
        )
    });

    sandbox.wait_for_subcommand_delay("moveto", Duration::from_secs(3));
    cancel.store(true, Ordering::SeqCst);
    let err = worker
        .join()
        .expect("mixed local->cloud move worker thread")
        .expect_err("second transfer should be cancelled");

    assert_eq!(err.code_str(), "cancelled");
    assert_eq!(
        fs::read_to_string(sandbox.remote_path("work", "dest/a.txt")).expect("read first remote"),
        "alpha"
    );
    assert!(
        !sandbox.remote_path("work", "dest/b.txt").exists(),
        "second destination should not be created after cancellation"
    );
    assert!(
        !src_a.exists(),
        "move cancellation should keep first completed move in partial state"
    );
    assert!(
        src_b.exists(),
        "second source should remain after cancellation"
    );
}

#[cfg(unix)]
#[test]
fn mixed_execute_cloud_to_local_move_cancels_during_second_active_transfer() {
    let _guard = fake_rclone_test_lock();
    let sandbox = FakeRcloneSandbox::new();
    sandbox.write_remote_file("work", "src/a.txt", "alpha");
    sandbox.write_remote_file("work", "src/b.txt", "beta");
    sandbox.set_subcommand_delay("moveto", 2, 1500);
    let cli = sandbox.cli();
    let local_dest = sandbox.local_path("dest");
    fs::create_dir_all(&local_dest).expect("mkdir local dest");
    let route = MixedTransferRoute::CloudToLocal {
        sources: vec![
            sandbox.cloud_path("rclone://work/src/a.txt"),
            sandbox.cloud_path("rclone://work/src/b.txt"),
        ],
        dest_dir: local_dest.clone(),
    };
    let cancel = Arc::new(AtomicBool::new(false));
    let cancel_for_task = cancel.clone();

    let worker = thread::spawn(move || {
        execute_mixed_entries_blocking_with_cli(
            &cli,
            MixedTransferOp::Move,
            route,
            MixedTransferWriteOptions {
                overwrite: false,
                prechecked: true,
            },
            Some(cancel_for_task),
            None,
        )
    });

    sandbox.wait_for_subcommand_delay("moveto", Duration::from_secs(3));
    cancel.store(true, Ordering::SeqCst);
    let err = worker
        .join()
        .expect("mixed cloud->local move worker thread")
        .expect_err("second transfer should be cancelled");

    assert_eq!(err.code_str(), "cancelled");
    assert_eq!(
        fs::read_to_string(local_dest.join("a.txt")).expect("read first local"),
        "alpha"
    );
    assert!(
        !local_dest.join("b.txt").exists(),
        "second local destination should not be created after cancellation"
    );
    assert!(
        !sandbox.remote_path("work", "src/a.txt").exists(),
        "first remote source should be removed after completed move"
    );
    assert!(
        sandbox.remote_path("work", "src/b.txt").exists(),
        "second remote source should remain after cancellation"
    );
}

#[cfg(unix)]
#[test]
fn mixed_execute_local_to_cloud_progress_batch_copy_cancels_during_second_active_transfer() {
    let _guard = fake_rclone_test_lock();
    let sandbox = FakeRcloneSandbox::new();
    sandbox.mkdir_remote("work", "dest");
    sandbox.set_subcommand_delay("copyto", 2, 1500);
    let cli = sandbox.cli();
    let src_a = sandbox.write_local_file("src/a.bin", &"a".repeat(1024));
    let src_b = sandbox.write_local_file("src/b.bin", &"b".repeat(1024));
    let route = MixedTransferRoute::LocalToCloud {
        sources: vec![src_a.clone(), src_b.clone()],
        dest_dir: sandbox.cloud_path("rclone://work/dest"),
    };
    let cancel = Arc::new(AtomicBool::new(false));
    let cancel_for_task = cancel.clone();

    let worker = thread::spawn(move || {
        execute_mixed_entries_blocking_with_cli(
            &cli,
            MixedTransferOp::Copy,
            route,
            MixedTransferWriteOptions {
                overwrite: false,
                prechecked: true,
            },
            Some(cancel_for_task),
            Some(TransferProgressContext {
                app: None,
                event_name: "mixed-progress-local-cloud".to_string(),
            }),
        )
    });

    sandbox.wait_for_subcommand_delay("copyto", Duration::from_secs(3));
    cancel.store(true, Ordering::SeqCst);
    let err = worker
        .join()
        .expect("mixed local->cloud progress worker thread")
        .expect_err("second transfer should be cancelled");

    assert_eq!(err.code_str(), "cancelled");
    assert!(sandbox.remote_path("work", "dest/a.bin").exists());
    assert!(!sandbox.remote_path("work", "dest/b.bin").exists());
    assert!(src_a.exists());
    assert!(src_b.exists());
}

#[cfg(unix)]
#[test]
fn mixed_execute_cloud_to_local_progress_batch_copy_cancels_during_second_active_transfer() {
    let _guard = fake_rclone_test_lock();
    let sandbox = FakeRcloneSandbox::new();
    sandbox.write_remote_file("work", "src/a.bin", &"a".repeat(1024));
    sandbox.write_remote_file("work", "src/b.bin", &"b".repeat(1024));
    sandbox.set_subcommand_delay("copyto", 2, 1500);
    let cli = sandbox.cli();
    let local_dest = sandbox.local_path("dest");
    fs::create_dir_all(&local_dest).expect("mkdir local dest");
    let route = MixedTransferRoute::CloudToLocal {
        sources: vec![
            sandbox.cloud_path("rclone://work/src/a.bin"),
            sandbox.cloud_path("rclone://work/src/b.bin"),
        ],
        dest_dir: local_dest.clone(),
    };
    let cancel = Arc::new(AtomicBool::new(false));
    let cancel_for_task = cancel.clone();

    let worker = thread::spawn(move || {
        execute_mixed_entries_blocking_with_cli(
            &cli,
            MixedTransferOp::Copy,
            route,
            MixedTransferWriteOptions {
                overwrite: false,
                prechecked: true,
            },
            Some(cancel_for_task),
            Some(TransferProgressContext {
                app: None,
                event_name: "mixed-progress-cloud-local".to_string(),
            }),
        )
    });

    sandbox.wait_for_subcommand_delay("copyto", Duration::from_secs(3));
    cancel.store(true, Ordering::SeqCst);
    let err = worker
        .join()
        .expect("mixed cloud->local progress worker thread")
        .expect_err("second transfer should be cancelled");

    assert_eq!(err.code_str(), "cancelled");
    assert!(local_dest.join("a.bin").exists());
    assert!(!local_dest.join("b.bin").exists());
    assert!(sandbox.remote_path("work", "src/a.bin").exists());
    assert!(sandbox.remote_path("work", "src/b.bin").exists());
}

#[cfg(unix)]
#[test]
fn mixed_execute_local_to_cloud_progress_batch_move_cancels_during_second_active_transfer() {
    let _guard = fake_rclone_test_lock();
    let sandbox = FakeRcloneSandbox::new();
    sandbox.mkdir_remote("work", "dest");
    sandbox.set_subcommand_delay("copyto", 2, 1500);
    let cli = sandbox.cli();
    let src_a = sandbox.write_local_file("src/a.bin", &"a".repeat(1024));
    let src_b = sandbox.write_local_file("src/b.bin", &"b".repeat(1024));
    let route = MixedTransferRoute::LocalToCloud {
        sources: vec![src_a.clone(), src_b.clone()],
        dest_dir: sandbox.cloud_path("rclone://work/dest"),
    };
    let cancel = Arc::new(AtomicBool::new(false));
    let cancel_for_task = cancel.clone();

    let worker = thread::spawn(move || {
        execute_mixed_entries_blocking_with_cli(
            &cli,
            MixedTransferOp::Move,
            route,
            MixedTransferWriteOptions {
                overwrite: false,
                prechecked: true,
            },
            Some(cancel_for_task),
            Some(TransferProgressContext {
                app: None,
                event_name: "mixed-progress-local-cloud-move".to_string(),
            }),
        )
    });

    sandbox.wait_for_subcommand_delay("copyto", Duration::from_secs(3));
    cancel.store(true, Ordering::SeqCst);
    let err = worker
        .join()
        .expect("mixed local->cloud move progress worker thread")
        .expect_err("second transfer should be cancelled");

    assert_eq!(err.code_str(), "cancelled");
    assert!(sandbox.remote_path("work", "dest/a.bin").exists());
    assert!(!sandbox.remote_path("work", "dest/b.bin").exists());
    assert!(!src_a.exists());
    assert!(src_b.exists());
}

#[cfg(unix)]
#[test]
fn mixed_execute_cloud_to_local_progress_batch_move_cancels_during_second_active_transfer() {
    let _guard = fake_rclone_test_lock();
    let sandbox = FakeRcloneSandbox::new();
    sandbox.write_remote_file("work", "src/a.bin", &"a".repeat(1024));
    sandbox.write_remote_file("work", "src/b.bin", &"b".repeat(1024));
    sandbox.set_subcommand_delay("copyto", 2, 1500);
    let cli = sandbox.cli();
    let local_dest = sandbox.local_path("dest");
    fs::create_dir_all(&local_dest).expect("mkdir local dest");
    let route = MixedTransferRoute::CloudToLocal {
        sources: vec![
            sandbox.cloud_path("rclone://work/src/a.bin"),
            sandbox.cloud_path("rclone://work/src/b.bin"),
        ],
        dest_dir: local_dest.clone(),
    };
    let cancel = Arc::new(AtomicBool::new(false));
    let cancel_for_task = cancel.clone();

    let worker = thread::spawn(move || {
        execute_mixed_entries_blocking_with_cli(
            &cli,
            MixedTransferOp::Move,
            route,
            MixedTransferWriteOptions {
                overwrite: false,
                prechecked: true,
            },
            Some(cancel_for_task),
            Some(TransferProgressContext {
                app: None,
                event_name: "mixed-progress-cloud-local-move".to_string(),
            }),
        )
    });

    sandbox.wait_for_subcommand_delay("copyto", Duration::from_secs(3));
    cancel.store(true, Ordering::SeqCst);
    let err = worker
        .join()
        .expect("mixed cloud->local move progress worker thread")
        .expect_err("second transfer should be cancelled");

    assert_eq!(err.code_str(), "cancelled");
    assert!(local_dest.join("a.bin").exists());
    assert!(!local_dest.join("b.bin").exists());
    assert!(!sandbox.remote_path("work", "src/a.bin").exists());
    assert!(sandbox.remote_path("work", "src/b.bin").exists());
}

#[cfg(unix)]
#[test]
fn mixed_execute_local_to_cloud_reports_destination_exists_when_not_prechecked() {
    let _guard = fake_rclone_test_lock();
    let sandbox = FakeRcloneSandbox::new();
    sandbox.mkdir_remote("work", "dest");
    sandbox.write_remote_file("work", "dest/conflict.txt", "remote-existing");
    let cli = sandbox.cli();
    let src = sandbox.write_local_file("src/conflict.txt", "local-payload");
    let route = MixedTransferRoute::LocalToCloud {
        sources: vec![src.clone()],
        dest_dir: sandbox.cloud_path("rclone://work/dest"),
    };

    let err = execute_mixed_entries_blocking_with_cli(
        &cli,
        MixedTransferOp::Copy,
        route,
        MixedTransferWriteOptions {
            overwrite: false,
            prechecked: false,
        },
        None,
        None,
    )
    .expect_err("copy should fail when destination exists and prechecked is false");

    assert_eq!(err.code_str(), "destination_exists");
    assert!(
        src.exists(),
        "source should remain unchanged on rejected copy"
    );
    assert_eq!(
        fs::read_to_string(sandbox.remote_path("work", "dest/conflict.txt"))
            .expect("read existing destination"),
        "remote-existing",
        "existing destination content should remain unchanged"
    );
}

#[cfg(unix)]
#[test]
fn register_mixed_cancel_returns_none_without_progress_event() {
    let cancel_state = CancelState::default();
    let guard = register_mixed_cancel(&cancel_state, &None).expect("register cancel");
    assert!(guard.is_none());
}

#[cfg(unix)]
#[test]
fn register_mixed_cancel_progress_event_sets_token_on_cancel() {
    let cancel_state = CancelState::default();
    let progress_event = Some("mixed-transfer-progress-event".to_string());
    let guard =
        register_mixed_cancel(&cancel_state, &progress_event).expect("register cancel guard");
    let token = guard
        .as_ref()
        .expect("progress event should register a cancel guard")
        .token();

    assert!(!token.load(Ordering::Relaxed));
    assert!(
        cancel_state
            .cancel("mixed-transfer-progress-event")
            .expect("cancel event"),
        "cancel state should find registered progress event"
    );
    assert!(
        token.load(Ordering::Relaxed),
        "registered token should flip when cancellation is triggered"
    );
}

#[test]
fn provider_specific_error_mapping_handles_onedrive_activity_limit() {
    assert_eq!(
        provider_specific_rclone_code(Some(CloudProviderKind::Onedrive), "activitylimitreached"),
        Some("rate_limited")
    );
    assert_eq!(
        provider_specific_rclone_code(Some(CloudProviderKind::Gdrive), "userratelimitexceeded"),
        Some("rate_limited")
    );
    assert_eq!(
        provider_specific_rclone_code(Some(CloudProviderKind::Nextcloud), "activitylimitreached"),
        None
    );
}

#[cfg(unix)]
#[test]
fn maps_rclone_nonzero_errors_to_consistent_transfer_codes() {
    let destination_exists = map_rclone_cli_error(
        RcloneCliError::NonZero {
            status: std::process::ExitStatus::from_raw(256),
            stdout: "destination exists".to_string(),
            stderr: String::new(),
        },
        Some("work"),
    );
    assert_eq!(destination_exists.code_str(), "destination_exists");

    let permission_denied = map_rclone_cli_error(
        RcloneCliError::NonZero {
            status: std::process::ExitStatus::from_raw(256),
            stdout: String::new(),
            stderr: "permission denied".to_string(),
        },
        Some("work"),
    );
    assert_eq!(permission_denied.code_str(), "permission_denied");
}

#[cfg(unix)]
#[test]
fn mixed_execute_uses_invalid_config_for_bad_rclone_path() {
    let _guard = fake_rclone_test_lock();
    let source_root = std::env::temp_dir().join(format!(
        "browsey-transfer-invalid-rclone-path-{}",
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&source_root);
    fs::create_dir_all(&source_root).expect("create source root");
    set_rclone_path_override_for_tests(Some("/usr/bin/rclone-does-not-exist"));
    let source = source_root.join("copy-source.txt");
    fs::write(&source, b"payload").expect("write local source");

    let route = MixedTransferRoute::LocalToCloud {
        sources: vec![source],
        dest_dir: CloudPath::parse("rclone://work/dest").expect("cloud path"),
    };
    let error = execute_mixed_entries_blocking(
        MixedTransferOp::Copy,
        route,
        MixedTransferWriteOptions {
            overwrite: false,
            prechecked: true,
        },
        None,
        None,
    )
    .expect_err("invalid configured rclone path should fail");

    assert_eq!(error.code_str(), "invalid_config");
    assert!(
        error
            .to_string()
            .contains("Configured Rclone path is invalid or not executable"),
        "unexpected error: {error}"
    );
    set_rclone_path_override_for_tests(None);
    let _ = fs::remove_dir_all(&source_root);
}
