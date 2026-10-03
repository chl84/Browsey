//! Opt-in measurements using real actions, never a user's undo directory.
use super::*;
use std::os::unix::fs::MetadataExt;
use std::time::Instant;

struct Fixture(PathBuf);
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn inventory(path: &Path) -> (u64, u64, u64) {
    let mut stack = vec![path.to_path_buf()];
    let (mut logical, mut allocated, mut files) = (0, 0, 0);
    while let Some(path) = stack.pop() {
        let metadata = fs::symlink_metadata(&path).unwrap();
        assert!(
            !metadata.is_symlink(),
            "measurement fixtures must not follow links"
        );
        allocated += metadata.blocks() * 512;
        if metadata.is_dir() {
            stack.extend(
                fs::read_dir(path)
                    .unwrap()
                    .map(|entry| entry.unwrap().path()),
            );
        } else {
            logical += metadata.len();
            files += 1;
        }
    }
    (logical, allocated, files)
}

#[test]
#[ignore = "Disposable 200 MiB recovery measurement; run alone with --ignored --exact --nocapture"]
fn recovery_storage_workloads() {
    let base = test_undo_dir();
    let fixture = Fixture(uniq_path("storage-measurement"));
    fs::create_dir(&fixture.0).unwrap();
    let mut results = Vec::new();
    for shape in ["large", "small", "nested", "sparse"] {
        let root = fixture.0.join(shape);
        fs::create_dir(&root).unwrap();
        let source = root.join("source");
        match shape {
            "large" => write_file(&source, &vec![0x51; 32 * 1024 * 1024]),
            "small" => {
                for index in 0..1024 {
                    write_file(&source.join(format!("{index}.txt")), b"fixture\n");
                }
            }
            "nested" => {
                for index in 0..128 {
                    write_file(
                        &source.join(format!("{}/deep/{index}.bin", index % 16)),
                        &vec![0x52; 8192],
                    );
                }
            }
            "sparse" => {
                write_file(&source, b"fixture");
                OpenOptions::new()
                    .write(true)
                    .open(&source)
                    .unwrap()
                    .set_len(32 * 1024 * 1024)
                    .unwrap();
            }
            _ => unreachable!(),
        }
        let source_bytes = inventory(&source);
        let mut action = Action::Copy {
            from: source,
            to: root.join("target"),
            receipt: CopyReceipt::default(),
        };
        super::super::engine::execute_action(&mut action, Direction::Forward).unwrap();
        let start = Instant::now();
        super::super::engine::execute_action(&mut action, Direction::Backward).unwrap();
        let undo_ms = start.elapsed().as_secs_f64() * 1000.0;
        let backup = match &action {
            Action::Copy { receipt, .. } => receipt.recovery_path().unwrap().to_path_buf(),
            _ => unreachable!(),
        };
        super::super::engine::finalize_action(&mut action);
        let before_roundtrips = inventory(&backup);
        for _ in 0..3 {
            super::super::engine::execute_action(&mut action, Direction::Forward).unwrap();
            super::super::engine::finalize_action(&mut action);
            super::super::engine::execute_action(&mut action, Direction::Backward).unwrap();
            super::super::engine::finalize_action(&mut action);
        }
        assert_eq!(
            inventory(&backup),
            before_roundtrips,
            "redo must reuse its backup"
        );
        let mut manager = UndoManager::new();
        manager.record_applied(action);
        manager.clear();
        assert!(backup.exists(), "history clearing is not disk reclamation");
        results.push(serde_json::json!({
            "shape": shape, "sourceLogicalBytes": source_bytes.0,
            "sourceAllocatedBytes": source_bytes.1, "files": source_bytes.2,
            "backupLogicalBytes": before_roundtrips.0,
            "backupAllocatedBytes": before_roundtrips.1, "firstUndoMs": undo_ms,
            "roundtrips": 3, "retainedAfterHistoryClear": true,
        }));
    }
    let mut manager = UndoManager::new();
    let mut backups = Vec::new();
    for index in 0..51 {
        let original = fixture.0.join(format!("history-{index}"));
        write_file(&original, &vec![0x53; 128 * 1024]);
        let backup = temp_backup_path(&original).unwrap();
        manager
            .apply(Action::Delete {
                path: original,
                backup: backup.clone(),
                protection: None,
            })
            .unwrap();
        backups.push(backup);
    }
    for _ in 0..50 {
        manager.undo().unwrap();
    }
    assert!(!manager.can_undo(), "the oldest action was evicted");
    assert!(
        backups[0].exists(),
        "evicting history must not be assumed to free its backup"
    );
    let protection = BackupProtection::create(&backups[0], &fixture.0.join("recovered")).unwrap();
    drop(protection);
    drop(manager);
    let summary = super::super::storage::inspect_storage().unwrap();
    assert_eq!(summary.marked_sessions, 1);
    println!(
        "BROWSEY_RECOVERY_MEASUREMENT={}",
        serde_json::json!({
            "schema": 1, "workloads": results, "historyActions": 51,
            "availableUndoActions": 50, "evictedBackupRetained": true,
            "protectedSessions": summary.marked_sessions,
            "inventoryIncomplete": summary.incomplete,
            "inventoryLogicalBytes": summary.logical_bytes,
        })
    );
    // Only this standalone test process's uniquely generated storage is removed.
    fs::remove_dir_all(base).unwrap();
}
