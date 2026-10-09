use super::*;
use crate::{
    fs_utils::copy_test_hooks::{Phase, Scope},
    performance_fixture::Fixture,
};
use std::sync::Arc;

#[test]
fn restart_recovery_child() {
    let Some(root) = std::env::var_os("BROWSEY_TEST_RESTART_RECOVERY_ROOT") else {
        return;
    };
    let root = PathBuf::from(root);
    let base = root.join("backups");
    let names = ["photo.jpg", "zero.txt", "empty-folder"];
    if std::env::var_os("BROWSEY_TEST_RESTART_CREATE").is_some() {
        // Use the real allocator and move flow, with no failure marker. The
        // registry's lifetime locks are released only when this process exits.
        for name in names {
            let original = root.join(name);
            match name {
                "photo.jpg" => fs::write(&original, [9; 32 * 1024]).unwrap(),
                "zero.txt" => fs::write(&original, []).unwrap(),
                _ => fs::create_dir(&original).unwrap(),
            }
            let backup = super::super::temp_backup_path(&original).unwrap();
            fs::create_dir(backup.parent().unwrap()).unwrap();
            super::super::move_with_fallback(&original, &backup).unwrap();
            assert!(backup.starts_with(&base));
            assert!(!original.exists());
        }
        return;
    }

    // This is the same startup entry point used by main, in a fresh process
    // with no reconstructed registry or undo history.
    super::super::cleanup_stale_backups(None);
    let listing = list_at(
        &base,
        ScanBudget {
            remaining: 10_000,
            deadline: Instant::now() + Duration::from_secs(10),
        },
    )
    .unwrap();
    assert!(!listing.incomplete);
    if std::env::var_os("BROWSEY_TEST_RESTART_VERIFY_HIDDEN").is_some() {
        assert!(listing.entries.is_empty());
        assert_eq!(fs::read(root.join("photo.jpg")).unwrap(), [9; 32 * 1024]);
        assert!(root.join("empty-folder").is_dir());
        let retained = walkdir::WalkDir::new(&base)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|entry| names.iter().any(|name| entry.file_name() == *name))
            .count();
        assert_eq!(retained, names.len());
        return;
    }
    assert_eq!(listing.entries.len(), names.len());
    let destination = root.join("recovered");
    fs::create_dir_all(&destination).unwrap();
    for name in names {
        let row = listing.entries.iter().find(|row| row.name == name).unwrap();
        assert!(row.blocked_reason.is_none());
        let source = base.join(&row.id);
        let marker = source.parent().unwrap().with_extension("recovery-required");
        assert!(!marker.exists());
        let target = PathBuf::from(
            recover_at(&base, &row.id, &row.version, None, None, None, None).unwrap(),
        );
        assert_eq!(target, root.join(name));
        match name {
            "photo.jpg" => {
                assert_eq!(fs::read(&target).unwrap(), [9; 32 * 1024]);
                assert_eq!(fs::read(&source).unwrap(), [9; 32 * 1024]);
            }
            "zero.txt" => {
                assert_eq!(fs::metadata(&target).unwrap().len(), 0);
                assert!(source.is_file());
            }
            _ => {
                assert!(target.is_dir() && source.is_dir());
                assert_eq!(fs::read_dir(target).unwrap().count(), 0);
            }
        }
    }
}

#[test]
fn original_locations_and_recovered_status_survive_process_restarts() {
    let fixture = Fixture::new("recovery-restart");
    for stage in ["create", "recover", "verify-hidden"] {
        let mut child = std::process::Command::new(std::env::current_exe().unwrap());
        child
            .args([
                "--exact",
                "undo::recovery::tests::restart_recovery_child",
                "--nocapture",
            ])
            .env("BROWSEY_TEST_RESTART_RECOVERY_ROOT", &fixture.0)
            .env("BROWSEY_UNDO_DIR", fixture.0.join("backups"))
            .env_remove("BROWSEY_TEST_RESTART_CREATE")
            .env_remove("BROWSEY_TEST_RESTART_VERIFY_HIDDEN");
        if stage == "create" {
            child.env("BROWSEY_TEST_RESTART_CREATE", "1");
        } else if stage == "verify-hidden" {
            child.env("BROWSEY_TEST_RESTART_VERIFY_HIDDEN", "1");
        }
        let output = child.output().unwrap();
        assert!(
            output.status.success(),
            "Restart recovery failed: {}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

struct BackupFixture {
    _root: Fixture,
    base: PathBuf,
    source: PathBuf,
    destination: PathBuf,
}
impl BackupFixture {
    fn new(folder: bool) -> Self {
        let root = Fixture::new("recovery-modal");
        let base = root.0.join("backups");
        let source = base.join("session-fixture/bucket/report.txt");
        fs::create_dir_all(source.parent().unwrap()).unwrap();
        File::create(base.join("session-fixture.lock")).unwrap();
        if folder {
            fs::create_dir(&source).unwrap();
            fs::create_dir_all(source.join("empty/nested")).unwrap();
            fs::write(source.join("file.bin"), [7; 19]).unwrap();
        } else {
            fs::write(&source, [3; 33]).unwrap();
        }
        fs::write(
            base.join("session-fixture/bucket.recovery-required"),
            b"diagnostic only; never a restore plan",
        )
        .unwrap();
        let destination = root.0.join("recovered");
        fs::create_dir(&destination).unwrap();
        Self {
            _root: root,
            base,
            source,
            destination,
        }
    }
    fn list(&self) -> RecoveryBackups {
        list_at(
            &self.base,
            ScanBudget {
                remaining: 10_000,
                deadline: Instant::now() + Duration::from_secs(10),
            },
        )
        .unwrap()
    }
    fn restore(&self, row: &RecoveryBackup, token: Option<&AtomicBool>) -> ApiResult<String> {
        restore_at(
            &self.base,
            &row.id,
            &row.version,
            &self.destination,
            None,
            Some("test-recovery"),
            token,
        )
    }
}

#[test]
fn automatic_recovery_uses_the_original_file_or_folder_name_and_keeps_the_backup() {
    for folder in [false, true] {
        let fixture = BackupFixture::new(folder);
        let original = fixture._root.0.join("original-name");
        super::super::backup_origin::record(&fixture.source, &original).unwrap();
        let row = fixture.list().entries.remove(0);
        let path =
            recover_at(&fixture.base, &row.id, &row.version, None, None, None, None).unwrap();
        assert_eq!(Path::new(&path), original);
        assert!(fixture.list().entries.is_empty());
        assert!(fixture.source.exists());
        if folder {
            assert!(original.join("empty/nested").is_dir());
            assert_eq!(fs::read(original.join("file.bin")).unwrap(), [7; 19]);
        } else {
            assert_eq!(fs::read(original).unwrap(), [3; 33]);
        }
    }
}

#[test]
fn occupied_originals_and_missing_original_folders_require_a_chosen_destination() {
    for mode in ["file", "folder", "missing-parent"] {
        let fixture = BackupFixture::new(false);
        let directory = fixture._root.0.join("original-folder");
        fs::create_dir(&directory).unwrap();
        let original = directory.join("report.txt");
        super::super::backup_origin::record(&fixture.source, &original).unwrap();
        match mode {
            "file" => fs::write(&original, b"existing user data").unwrap(),
            "folder" => fs::create_dir(&original).unwrap(),
            _ => fs::remove_dir(&directory).unwrap(),
        }
        let row = fixture.list().entries.remove(0);
        let error =
            recover_at(&fixture.base, &row.id, &row.version, None, None, None, None).unwrap_err();
        assert_eq!(error.code, "recovery_destination_unavailable");
        if mode != "missing-parent" {
            assert_eq!(
                error.message,
                "Original location is occupied. Choose another folder."
            );
            assert_eq!(
                error.details,
                Some(serde_json::json!({ "reason": "occupied" }))
            );
        } else {
            assert!(error.details.is_none());
        }
        assert_eq!(fs::read(&fixture.source).unwrap(), [3; 33]);
        assert_eq!(fixture.list().entries.len(), 1);
        // Automatic recovery neither renames around a collision nor creates
        // missing ancestors; alternative recovery remains an explicit choice.
        if mode == "file" {
            assert_eq!(fs::read(&original).unwrap(), b"existing user data");
        }
        assert!(!directory.join("report-1.txt").exists());
        let restored = fixture.restore(&row, None).unwrap();
        assert_eq!(fs::read(restored).unwrap(), [3; 33]);
    }
}

#[test]
fn automatic_copy_failure_offers_a_folder_and_cancellation_does_not() {
    for cancelled in [false, true] {
        let fixture = BackupFixture::new(false);
        let original = fixture._root.0.join("original.txt");
        super::super::backup_origin::record(&fixture.source, &original).unwrap();
        let row = fixture.list().entries.remove(0);
        let token = Arc::new(AtomicBool::new(false));
        let cancel_token = token.clone();
        let scope = Scope::new(move |_, _, phase, _| {
            if phase == Phase::Write {
                if cancelled {
                    cancel_token.store(true, Ordering::Relaxed);
                } else {
                    return Err(io::Error::new(
                        io::ErrorKind::PermissionDenied,
                        "Injected write failure",
                    ));
                }
            }
            Ok(())
        });
        let error = recover_at(
            &fixture.base,
            &row.id,
            &row.version,
            None,
            None,
            None,
            Some(&token),
        )
        .unwrap_err();
        drop(scope);
        assert_eq!(
            error.code,
            if cancelled {
                "cancelled"
            } else {
                "recovery_destination_unavailable"
            }
        );
        assert!(
            original.is_file(),
            "uncertain output is kept for inspection"
        );
        assert_eq!(fs::read(&fixture.source).unwrap(), [3; 33]);
        assert_eq!(fixture.list().entries.len(), 1);
        if !cancelled {
            assert!(error.message.contains(&original.display().to_string()));
        }
        // The next copy starts only when the user chooses a folder.
        assert_eq!(fs::read_dir(&fixture.destination).unwrap().count(), 0);
        assert_eq!(
            fs::read(fixture.restore(&row, None).unwrap()).unwrap(),
            [3; 33]
        );
    }
}

#[cfg(unix)]
#[test]
fn automatic_recovery_refuses_symlink_originals_and_symlink_origin_records() {
    use std::os::unix::fs::symlink;
    for metadata_link in [false, true] {
        let fixture = BackupFixture::new(false);
        let original = fixture._root.0.join("original.txt");
        super::super::backup_origin::record(&fixture.source, &original).unwrap();
        let foreign = fixture._root.0.join("foreign.txt");
        fs::write(&foreign, b"foreign user data").unwrap();
        if metadata_link {
            let record = fixture
                .source
                .parent()
                .unwrap()
                .with_extension("origin.json");
            fs::remove_file(&record).unwrap();
            symlink(&foreign, record).unwrap();
        } else {
            symlink(&foreign, &original).unwrap();
        }
        let row = fixture.list().entries.remove(0);
        let error =
            recover_at(&fixture.base, &row.id, &row.version, None, None, None, None).unwrap_err();
        assert_eq!(error.code, "recovery_destination_unavailable");
        assert_eq!(fs::read(foreign).unwrap(), b"foreign user data");
        assert_eq!(fs::read(&fixture.source).unwrap(), [3; 33]);
    }
}

#[test]
fn lists_real_files_and_folders_and_recovers_without_changing_backups_or_markers() {
    for folder in [false, true] {
        let fixture = BackupFixture::new(folder);
        let marker = fixture
            .base
            .join("session-fixture/bucket.recovery-required");
        let before = fs::read(&marker).unwrap();
        let listing = fixture.list();
        assert!(!listing.incomplete);
        assert_eq!(listing.entries.len(), 1);
        let row = &listing.entries[0];
        assert_eq!(row.bytes, Some(if folder { 19 } else { 33 }));
        assert!(row.blocked_reason.is_none());
        let target = PathBuf::from(fixture.restore(row, None).unwrap());
        if folder {
            assert!(target.join("empty/nested").is_dir());
            assert_eq!(fs::read(target.join("file.bin")).unwrap(), [7; 19]);
            assert_eq!(fs::read(fixture.source.join("file.bin")).unwrap(), [7; 19]);
        } else {
            assert_eq!(
                fs::read(&target).unwrap(),
                fs::read(&fixture.source).unwrap()
            );
        }
        assert_eq!(fs::read(marker).unwrap(), before);
        assert!(fixture.base.join("session-fixture.lock").is_file());
        assert!(fixture.list().entries.is_empty());
        // Repeated recovery is an independent copy, never an overwrite/merge.
        let again = PathBuf::from(fixture.restore(row, None).unwrap());
        assert_ne!(target, again);
        assert_eq!(
            again.file_name().unwrap(),
            if folder {
                "report.txt-1"
            } else {
                "report-1.txt"
            }
        );
    }
}

#[test]
fn changed_backup_contents_become_pending_again_and_recovery_updates_the_status() {
    let fixture = BackupFixture::new(true);
    let row = fixture.list().entries.remove(0);
    let first = PathBuf::from(fixture.restore(&row, None).unwrap());
    assert!(fixture.list().entries.is_empty());
    fs::write(fixture.source.join("file.bin"), b"new backup contents").unwrap();
    // A nested file edit need not change the root folder's metadata version.
    assert_eq!(version(&fixture.source).unwrap(), row.version);
    let changed = fixture.list();
    assert_eq!(changed.entries.len(), 1);
    let second = PathBuf::from(fixture.restore(&changed.entries[0], None).unwrap());
    assert!(fixture.list().entries.is_empty());
    assert_eq!(fs::read(first.join("file.bin")).unwrap(), [7; 19]);
    assert_eq!(
        fs::read(second.join("file.bin")).unwrap(),
        b"new backup contents"
    );
}

#[test]
fn invalid_recovered_status_keeps_the_backup_listed_and_reports_verified_output() {
    let fixture = BackupFixture::new(false);
    let row = fixture.list().entries.remove(0);
    fixture.restore(&row, None).unwrap();
    assert!(fixture.list().entries.is_empty());
    let record = fs::read_dir(fixture.source.parent().unwrap().parent().unwrap())
        .unwrap()
        .filter_map(Result::ok)
        .find(|entry| entry.file_name().to_string_lossy().contains(".recovered-"))
        .unwrap()
        .path();
    fs::write(&record, b"invalid recovery status").unwrap();
    assert_eq!(fixture.list().entries.len(), 1);
    let error = fixture.restore(&row, None).unwrap_err();
    assert_eq!(error.code, "recovery_record_failed");
    assert!(error.message.contains("Recovered to"));
    assert_eq!(fixture.list().entries.len(), 1);
    assert_eq!(fs::read(&fixture.source).unwrap(), [3; 33]);
    assert_eq!(
        fs::read(fixture.destination.join("report-1.txt")).unwrap(),
        [3; 33]
    );
}

#[cfg(unix)]
#[test]
fn symlink_recovered_status_cannot_hide_a_backup_or_modify_foreign_data() {
    use std::os::unix::fs::symlink;
    let fixture = BackupFixture::new(false);
    let row = fixture.list().entries.remove(0);
    fixture.restore(&row, None).unwrap();
    let record = fs::read_dir(fixture.source.parent().unwrap().parent().unwrap())
        .unwrap()
        .filter_map(Result::ok)
        .find(|entry| entry.file_name().to_string_lossy().contains(".recovered-"))
        .unwrap()
        .path();
    let bytes = fs::read(&record).unwrap();
    let foreign = fixture._root.0.join("foreign-status");
    fs::write(&foreign, &bytes).unwrap();
    fs::remove_file(&record).unwrap();
    symlink(&foreign, &record).unwrap();
    assert_eq!(fixture.list().entries.len(), 1);
    assert_eq!(
        fixture.restore(&row, None).unwrap_err().code,
        "recovery_record_failed"
    );
    assert_eq!(fs::read(&foreign).unwrap(), bytes);
    assert!(record.is_symlink());
}

#[test]
fn recovered_current_session_backup_remains_available_to_undo() {
    let fixture = CurrentFixture::new();
    let row = fixture.list().entries.remove(0);
    fixture.restore(&row).unwrap();
    assert!(fixture.list().entries.is_empty());
    let original = fixture._root.0.join("photo.jpg");
    let mut action = super::super::Action::Delete {
        path: original.clone(),
        backup: fixture.source.clone(),
        protection: None,
    };
    super::super::engine::execute_action(&mut action, super::super::Direction::Backward).unwrap();
    assert_eq!(fs::read(original).unwrap(), [9; 32 * 1024]);
}

#[test]
fn in_use_sessions_are_disabled_and_restore_rechecks_the_lock() {
    let fixture = BackupFixture::new(false);
    let stale_row = fixture.list().entries.remove(0);
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .open(fixture.base.join("session-fixture.lock"))
        .unwrap();
    lock.try_lock().unwrap();
    assert!(fixture.list().entries[0]
        .blocked_reason
        .as_deref()
        .unwrap()
        .contains("another Browsey instance"));
    assert_eq!(
        fixture.restore(&stale_row, None).unwrap_err().code,
        "lock_failed"
    );
    assert_eq!(fs::read_dir(&fixture.destination).unwrap().count(), 0);
}

#[test]
fn recovery_holds_session_lock_through_streaming_and_verification() {
    let fixture = BackupFixture::new(false);
    let row = fixture.list().entries.remove(0);
    let lock_path = fixture.base.join("session-fixture.lock");
    let _scope = Scope::new(move |_, _, phase, _| {
        if phase == Phase::Write || phase == Phase::Readback {
            let lock = OpenOptions::new().read(true).write(true).open(&lock_path)?;
            assert!(
                lock.try_lock().is_err(),
                "cleanup must be blocked until recovery finishes"
            );
        }
        Ok(())
    });
    fixture.restore(&row, None).unwrap();
}

#[test]
fn rejects_invalid_references_stale_rows_and_destinations_inside_backup_storage() {
    let fixture = BackupFixture::new(false);
    let row = fixture.list().entries.remove(0);
    for id in [
        "/etc/passwd",
        "session-fixture/../report.txt",
        "session-fixture/bucket/report.txt/extra",
        "bucket/report.txt",
    ] {
        assert!(restore_at(
            &fixture.base,
            id,
            &row.version,
            &fixture.destination,
            None,
            None,
            None
        )
        .is_err());
    }
    assert!(restore_at(
        &fixture.base,
        &row.id,
        &row.version,
        fixture.source.parent().unwrap(),
        None,
        None,
        None
    )
    .is_err());
    assert!(restore_at(
        &fixture.base,
        &row.id,
        &row.version,
        Path::new("relative"),
        None,
        None,
        None
    )
    .is_err());
    fs::rename(&fixture.source, fixture.source.with_extension("old")).unwrap();
    fs::write(&fixture.source, [4; 33]).unwrap();
    assert_eq!(
        fixture.restore(&row, None).unwrap_err().code,
        "snapshot_mismatch"
    );
    assert_eq!(fs::read_dir(&fixture.destination).unwrap().count(), 0);
}

#[cfg(unix)]
#[test]
fn symlink_inputs_locks_and_destinations_are_not_followed_and_dangling_conflicts_are_kept() {
    use std::os::unix::fs::symlink;
    let fixture = BackupFixture::new(true);
    let row = fixture.list().entries.remove(0);
    let link = fixture.source.join("external");
    symlink(&fixture.destination, &link).unwrap();
    assert!(fixture.list().entries[0].blocked_reason.is_some());
    assert!(fixture.restore(&row, None).is_err());
    assert_eq!(fs::read_dir(&fixture.destination).unwrap().count(), 0);
    fs::remove_file(link).unwrap();
    let row = fixture.list().entries.remove(0);
    let alias = fixture.destination.with_extension("link");
    symlink(&fixture.destination, &alias).unwrap();
    assert!(restore_at(
        &fixture.base,
        &row.id,
        &row.version,
        &alias,
        None,
        None,
        None
    )
    .is_err());
    let occupied = fixture.destination.join("report.txt");
    symlink(fixture.destination.join("missing"), &occupied).unwrap();
    assert!(fixture
        .restore(&row, None)
        .unwrap()
        .ends_with("report.txt-1"));
    assert!(fs::symlink_metadata(&occupied)
        .unwrap()
        .file_type()
        .is_symlink());
    let lock = fixture.base.join("session-fixture.lock");
    let old_lock = fixture.base.join("old.lock");
    fs::rename(&lock, &old_lock).unwrap();
    symlink(old_lock, &lock).unwrap();
    assert!(fixture.restore(&row, None).is_err());
}

#[test]
fn cancellation_before_copy_creates_no_output_and_midstream_keeps_backup_and_partial_copy() {
    let fixture = BackupFixture::new(false);
    fs::write(&fixture.source, [5; 64 * 1024]).unwrap();
    let row = fixture.list().entries.remove(0);
    let token = Arc::new(AtomicBool::new(true));
    assert_eq!(
        fixture.restore(&row, Some(&token)).unwrap_err().code,
        "cancelled"
    );
    assert_eq!(fs::read_dir(&fixture.destination).unwrap().count(), 0);
    token.store(false, Ordering::Relaxed);
    let hook_token = token.clone();
    let _scope = Scope::new(move |_, _, phase, bytes| {
        if phase == Phase::Write && bytes >= 8192 {
            hook_token.store(true, Ordering::Relaxed);
        }
        Ok(())
    });
    assert_eq!(
        fixture.restore(&row, Some(&token)).unwrap_err().code,
        "cancelled"
    );
    assert_eq!(fs::read(&fixture.source).unwrap(), [5; 64 * 1024]);
    let partial = fixture.destination.join("report.txt");
    assert!(partial.is_file());
    assert!(fs::metadata(partial).unwrap().len() < 64 * 1024);
    assert!(fixture
        .base
        .join("session-fixture/bucket.recovery-required")
        .is_file());
}

#[test]
fn verification_failure_is_not_a_success_and_preserves_both_paths() {
    let fixture = BackupFixture::new(false);
    let row = fixture.list().entries.remove(0);
    let _scope = Scope::new(move |_, _, phase, _| {
        if phase == Phase::Readback {
            return Err(io::Error::other("Injected readback failure"));
        }
        Ok(())
    });
    let error = fixture.restore(&row, None).unwrap_err();
    assert!(error.message.contains("Backup kept"));
    assert_eq!(fs::read(&fixture.source).unwrap(), [3; 33]);
    assert!(fixture.destination.join("report.txt").is_file());
}

#[test]
fn absent_and_partial_scans_never_create_storage_or_claim_complete_empty_results() {
    let fixture = BackupFixture::new(false);
    let missing = fixture.base.join("missing");
    let result = list_at(
        &missing,
        ScanBudget {
            remaining: 100,
            deadline: Instant::now() + Duration::from_secs(1),
        },
    )
    .unwrap();
    assert!(result.entries.is_empty());
    assert!(!result.incomplete);
    assert!(!missing.exists());
    let partial = list_at(
        &fixture.base,
        ScanBudget {
            remaining: 0,
            deadline: Instant::now() + Duration::from_secs(1),
        },
    )
    .unwrap();
    assert!(partial.entries.is_empty());
    assert!(partial.incomplete);
}

#[test]
fn bounded_size_measurement_does_not_disable_an_otherwise_recoverable_large_folder() {
    let fixture = BackupFixture::new(true);
    fs::remove_file(
        fixture
            .base
            .join("session-fixture/bucket.recovery-required"),
    )
    .unwrap();
    let mut listing = list_at(
        &fixture.base,
        ScanBudget {
            remaining: 4,
            deadline: Instant::now() + Duration::from_secs(10),
        },
    )
    .unwrap();
    assert!(listing.incomplete);
    let row = listing.entries.remove(0);
    assert_eq!(row.bytes, None);
    assert!(row.blocked_reason.is_none());
    // A partial inventory size is not a failed safety check. Recovery makes its
    // own full, cancellable snapshot and measures the actual transferred bytes.
    fixture.restore(&row, None).unwrap();
}

// Exercise the real registry and lifetime session lock, which the original
// abandoned-session fixtures did not cover.
struct CurrentFixture {
    _root: Fixture,
    base: PathBuf,
    source: PathBuf,
    destination: PathBuf,
}
impl CurrentFixture {
    fn new() -> Self {
        let root = Fixture::new("current-backup-recovery");
        let original = root.0.join("photo.jpg");
        fs::write(&original, [9; 32 * 1024]).unwrap();
        let base = root.0.join("backups");
        let source = super::super::backup::temp_backup_path_at(&base, &original).unwrap();
        fs::create_dir(source.parent().unwrap()).unwrap();
        super::super::move_with_fallback(&original, &source).unwrap();
        let destination = root.0.join("recovered");
        fs::create_dir(&destination).unwrap();
        Self {
            _root: root,
            base,
            source,
            destination,
        }
    }
    fn list(&self) -> RecoveryBackups {
        list_at(
            &self.base,
            ScanBudget {
                remaining: 10_000,
                deadline: Instant::now() + Duration::from_secs(10),
            },
        )
        .unwrap()
    }
    fn restore(&self, row: &RecoveryBackup) -> ApiResult<String> {
        restore_at(
            &self.base,
            &row.id,
            &row.version,
            &self.destination,
            None,
            None,
            None,
        )
    }
}
impl Drop for CurrentFixture {
    fn drop(&mut self) {
        super::super::backup::forget_test_session(&self.base);
    }
}

#[test]
fn completed_current_session_image_can_be_recovered_without_releasing_the_lifetime_lock() {
    let fixture = CurrentFixture::new();
    let listing = fixture.list();
    assert!(!listing.incomplete);
    let row = &listing.entries[0];
    assert_eq!(row.name, "photo.jpg");
    assert!(row.blocked_reason.is_none());
    let owner = super::super::backup::owned_session(&fixture.source)
        .unwrap()
        .unwrap();
    let lock_path = owner.directory.with_file_name(format!(
        "{}.lock",
        owner.directory.file_name().unwrap().to_str().unwrap()
    ));
    let _scope = Scope::new(move |_, _, phase, _| {
        if matches!(phase, Phase::Write | Phase::Readback) {
            let other = OpenOptions::new().read(true).write(true).open(&lock_path)?;
            assert!(
                other.try_lock().is_err(),
                "recovering must never release session retention"
            );
        }
        Ok(())
    });
    let target = fixture.restore(row).unwrap();
    assert_eq!(fs::read(&target).unwrap(), [9; 32 * 1024]);
    assert_eq!(fs::read(&fixture.source).unwrap(), [9; 32 * 1024]);
}

#[test]
fn only_the_backup_being_changed_is_blocked_and_it_becomes_available_after_the_write() {
    let fixture = CurrentFixture::new();
    let other =
        super::super::backup::temp_backup_path_at(&fixture.base, Path::new("other.txt")).unwrap();
    fs::create_dir(other.parent().unwrap()).unwrap();
    fs::write(&other, b"another finished backup").unwrap();
    let old_row = fixture
        .list()
        .entries
        .into_iter()
        .find(|row| row.name == "photo.jpg")
        .unwrap();
    {
        let _writer = super::super::write_backups(&[&fixture.source]).unwrap();
        let list = fixture.list();
        assert!(!list.incomplete);
        assert!(list
            .entries
            .iter()
            .find(|row| row.name == "photo.jpg")
            .unwrap()
            .blocked_reason
            .as_deref()
            .unwrap()
            .contains("operation in progress"));
        assert!(list
            .entries
            .iter()
            .find(|row| row.name == "other.txt")
            .unwrap()
            .blocked_reason
            .is_none());
        assert_eq!(fixture.restore(&old_row).unwrap_err().code, "lock_failed");
        assert_eq!(fs::read_dir(&fixture.destination).unwrap().count(), 0);
    }
    assert!(fixture
        .list()
        .entries
        .iter()
        .all(|row| row.blocked_reason.is_none()));
    fixture.restore(&old_row).unwrap();
}

#[test]
fn creating_another_backup_does_not_invalidate_an_existing_image_row() {
    let fixture = CurrentFixture::new();
    let row = fixture.list().entries.remove(0);
    let other =
        super::super::backup::temp_backup_path_at(&fixture.base, Path::new("new.txt")).unwrap();
    fs::create_dir(other.parent().unwrap()).unwrap();
    fs::write(other, b"later backup").unwrap();
    fixture.restore(&row).unwrap();
}

#[test]
fn actual_backup_writer_marks_partial_output_busy_and_releases_it_on_error() {
    let fixture = CurrentFixture::new();
    let other =
        super::super::backup::temp_backup_path_at(&fixture.base, Path::new("partial.txt")).unwrap();
    fs::create_dir(other.parent().unwrap()).unwrap();
    let base = fixture.base.clone();
    let _scope = Scope::new(move |_, _, phase, bytes| {
        if phase == Phase::Write && bytes > 0 {
            let listing = list_at(
                &base,
                ScanBudget {
                    remaining: 10_000,
                    deadline: Instant::now() + Duration::from_secs(10),
                },
            )
            .unwrap();
            assert!(listing
                .entries
                .iter()
                .find(|row| row.name == "partial.txt")
                .unwrap()
                .blocked_reason
                .is_some());
            assert!(listing
                .entries
                .iter()
                .find(|row| row.name == "photo.jpg")
                .unwrap()
                .blocked_reason
                .is_none());
            return Err(io::Error::other("Injected copy failure"));
        }
        Ok(())
    });
    assert!(super::super::path_ops::copy_entry_recorded(&fixture.source, &other).is_err());
    drop(_scope);
    assert!(fixture
        .list()
        .entries
        .iter()
        .all(|row| row.blocked_reason.is_none()));
}

#[test]
fn recovery_read_lease_delays_undo_move_until_the_copy_has_finished() {
    use std::{sync::mpsc, time::Duration};
    let fixture = CurrentFixture::new();
    let source = fixture.source.clone();
    let target = fixture.destination.join("undo-target.jpg");
    let lock = lock_session(
        &fixture.base,
        source
            .parent()
            .unwrap()
            .parent()
            .unwrap()
            .file_name()
            .unwrap()
            .to_str()
            .unwrap(),
    )
    .unwrap()
    .unwrap();
    let read = lock.read_backup(&source).unwrap();
    let (started_tx, started_rx) = mpsc::channel();
    let (finished_tx, finished_rx) = mpsc::channel();
    let writer = std::thread::spawn(move || {
        started_tx.send(()).unwrap();
        let result = super::super::move_with_fallback(&source, &target);
        finished_tx.send(result).unwrap();
    });
    started_rx.recv_timeout(Duration::from_secs(2)).unwrap();
    assert!(finished_rx.recv_timeout(Duration::from_millis(50)).is_err());
    assert!(fixture.source.is_file());
    drop(read);
    finished_rx
        .recv_timeout(Duration::from_secs(2))
        .unwrap()
        .unwrap();
    writer.join().unwrap();
    assert!(!fixture.source.exists());
    assert_eq!(
        fs::read(fixture.destination.join("undo-target.jpg")).unwrap(),
        [9; 32 * 1024]
    );
}
