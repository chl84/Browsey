use super::*;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "browsey-storage-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn inspect(&self, limit: usize) -> UndoStorageSummary {
        inspect_directory(
            &self.0,
            ScanBudget {
                remaining: limit,
                deadline: Instant::now() + Duration::from_secs(30),
            },
        )
        .unwrap()
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[test]
fn summary_serialization_matches_the_settings_contract() {
    let summary = UndoStorageSummary {
        directory: "/fixture/undo-sessions".into(),
        exists: true,
        sessions: 3,
        marked_sessions: 1,
        files: 4,
        logical_bytes: 8192,
        incomplete: false,
    };
    assert_eq!(
        serde_json::to_value(summary).unwrap(),
        serde_json::json!({
            "directory": "/fixture/undo-sessions", "exists": true, "sessions": 3,
            "markedSessions": 1, "files": 4, "logicalBytes": 8192, "incomplete": false,
        })
    );
}

#[test]
fn inventory_counts_regular_backups_and_marked_sessions_without_modifying_them() {
    let fixture = Fixture::new();
    let session = fixture.0.join("session-fixture");
    fs::create_dir_all(session.join("bucket/deep")).unwrap();
    fs::write(session.join("bucket/deep/file"), b"original").unwrap();
    fs::write(session.join("bucket.recovery-required"), b"marker").unwrap();
    fs::write(fixture.0.join("session-fixture.lock"), b"lock").unwrap();
    fs::create_dir_all(fixture.0.join("legacy")).unwrap();
    fs::write(fixture.0.join("legacy/ignored"), b"not in the inventory").unwrap();
    let result = fixture.inspect(100);
    assert!(result.exists);
    assert!(!result.incomplete);
    assert_eq!(result.sessions, 1);
    assert_eq!(result.marked_sessions, 1);
    assert_eq!(result.files, 2);
    assert_eq!(result.logical_bytes, 14);
    assert_eq!(
        fs::read(session.join("bucket/deep/file")).unwrap(),
        b"original"
    );
    assert_eq!(
        fs::read(session.join("bucket.recovery-required")).unwrap(),
        b"marker"
    );
    assert_eq!(
        fs::read(fixture.0.join("session-fixture.lock")).unwrap(),
        b"lock"
    );
    assert_eq!(fixture.inspect(100).logical_bytes, result.logical_bytes);
}

#[test]
fn missing_storage_is_empty_and_not_created_by_inspection() {
    let fixture = Fixture::new();
    let missing = fixture.0.join("missing");
    let result = inspect_directory(
        &missing,
        ScanBudget {
            remaining: 100,
            deadline: Instant::now() + Duration::from_secs(30),
        },
    )
    .unwrap();
    assert!(!result.exists);
    assert!(!result.incomplete);
    assert_eq!(result.logical_bytes, 0);
    assert!(!missing.exists());
}

#[test]
fn entry_and_time_limits_report_incomplete_not_zero_as_a_complete_total() {
    let fixture = Fixture::new();
    fs::create_dir(fixture.0.join("session-fixture")).unwrap();
    for index in 0..10 {
        fs::write(fixture.0.join(format!("session-fixture/{index}")), b"bytes").unwrap();
    }
    let limited = fixture.inspect(3);
    assert!(limited.incomplete);
    assert!(limited.files < 10);
    assert!(limited.logical_bytes < 50);
    let timed = inspect_directory(
        &fixture.0,
        ScanBudget {
            remaining: 100,
            deadline: Instant::now(),
        },
    )
    .unwrap();
    assert!(timed.incomplete);
    assert_eq!(timed.files, 0);
    assert!(!fixture.inspect(100).incomplete);
}

#[test]
fn suspicious_directory_marker_is_counted_but_not_traversed() {
    let fixture = Fixture::new();
    let marker = fixture.0.join("session-fixture/bucket.recovery-required");
    fs::create_dir_all(&marker).unwrap();
    fs::write(marker.join("not-a-backup"), b"outside inventory").unwrap();
    let result = fixture.inspect(100);
    assert_eq!(result.marked_sessions, 1);
    assert_eq!(result.logical_bytes, 0);
    assert!(result.incomplete);
    assert!(marker.join("not-a-backup").exists());
}

#[cfg(unix)]
#[test]
fn symlinked_backups_and_session_paths_are_not_followed() {
    use std::os::unix::fs::symlink;
    let fixture = Fixture::new();
    fs::create_dir(fixture.0.join("outside")).unwrap();
    fs::write(fixture.0.join("outside/file"), b"not a backup").unwrap();
    fs::create_dir(fixture.0.join("session-fixture")).unwrap();
    symlink(
        fixture.0.join("outside"),
        fixture.0.join("session-fixture/link"),
    )
    .unwrap();
    symlink(
        fixture.0.join("outside/file"),
        fixture.0.join("session-fixture/file-link"),
    )
    .unwrap();
    symlink(fixture.0.join("outside"), fixture.0.join("session-link")).unwrap();
    symlink(
        fixture.0.join("outside/file"),
        fixture.0.join("session-fixture/bucket.recovery-required"),
    )
    .unwrap();
    let result = fixture.inspect(100);
    assert_eq!(result.sessions, 1);
    assert_eq!(result.marked_sessions, 1);
    assert_eq!(result.files, 0);
    assert_eq!(result.logical_bytes, 0);
    assert!(result.incomplete);
    assert!(inspect_directory(
        &fixture.0.join("session-link"),
        ScanBudget {
            remaining: 100,
            deadline: Instant::now() + Duration::from_secs(30)
        },
    )
    .is_err());
    assert_eq!(
        fs::read(fixture.0.join("outside/file")).unwrap(),
        b"not a backup"
    );
}

#[cfg(unix)]
#[test]
fn inventory_counts_native_filename_bytes_but_rejects_lossy_root_paths() {
    use std::os::unix::ffi::OsStringExt;
    let fixture = Fixture::new();
    let name = std::ffi::OsString::from_vec(b"file-\xff".to_vec());
    fs::create_dir(fixture.0.join("session-fixture")).unwrap();
    fs::write(fixture.0.join("session-fixture").join(&name), b"bytes").unwrap();
    assert_eq!(fixture.inspect(100).logical_bytes, 5);
    let bad_root = fixture.0.join(name);
    fs::create_dir(&bad_root).unwrap();
    let error = inspect_directory(
        &bad_root,
        ScanBudget {
            remaining: 100,
            deadline: Instant::now() + Duration::from_secs(30),
        },
    )
    .unwrap_err();
    assert_eq!(error.code(), super::super::UndoErrorCode::InvalidInput);
}
