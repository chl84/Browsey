use super::*;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{Duration, SystemTime};

fn uniq_path(label: &str) -> PathBuf {
    let ts = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or(Duration::from_secs(0))
        .as_nanos();
    std::env::temp_dir().join(format!("browsey-undo-test-{label}-{ts}"))
}

fn test_undo_dir() -> PathBuf {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| {
        let dir = uniq_path("undo-base");
        let _ = fs::remove_dir_all(&dir);
        std::env::set_var("BROWSEY_UNDO_DIR", &dir);
        dir
    })
    .clone()
}

fn write_file(path: &Path, content: &[u8]) {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(path)
        .unwrap();
    file.write_all(content).unwrap();
}

#[test]
fn undo_copy_and_move_faults_keep_source_and_report_partial_output() {
    use crate::fs_utils::copy_test_hooks::{Phase, Scope};
    use std::io::{Error, ErrorKind};
    for moving in [false, true] {
        for (phase, kind) in [
            (Phase::Write, ErrorKind::StorageFull),
            (Phase::Read, ErrorKind::NotFound),
            (Phase::Write, ErrorKind::BrokenPipe),
            (Phase::Sync, ErrorKind::StorageFull),
            (Phase::Sync, ErrorKind::Other),
        ] {
            let root = uniq_path("undo-copy-fault");
            let source = root.join("source.bin");
            let target = root.join("target.bin");
            let unrelated = root.join("unrelated.bin");
            let data = vec![0x5a; 64 * 1024];
            write_file(&source, &data);
            write_file(&unrelated, b"unrelated-data");
            let reached = std::rc::Rc::new(std::cell::Cell::new(false));
            let observed = reached.clone();
            let scope = Scope::new(move |_, _, current, bytes| {
                if moving && current == Phase::Rename {
                    return Err(Error::new(ErrorKind::Unsupported, "force copy fallback"));
                }
                if current == phase && bytes > 0 {
                    observed.set(true);
                    return Err(Error::new(kind, "injected undo copy fault"));
                }
                Ok(())
            });
            let result = if moving {
                move_with_fallback(&source, &target)
            } else {
                copy_entry(&source, &target)
            };
            drop(scope);
            let source_data = fs::read(&source).unwrap();
            let partial = fs::read(&target).unwrap();
            let unrelated_data = fs::read(&unrelated).unwrap();
            fs::remove_dir_all(root).unwrap();
            let error = result.expect_err("fault must prevent source deletion");
            assert!(reached.get());
            assert!(error.to_string().contains("injected undo copy fault"));
            assert!(error.to_string().contains("source retained"));
            assert!(error.to_string().contains("destination may remain"));
            assert_eq!(
                error.code(),
                if kind == ErrorKind::NotFound {
                    UndoErrorCode::NotFound
                } else {
                    UndoErrorCode::IoError
                }
            );
            assert_eq!(source_data, data);
            assert_eq!(unrelated_data, b"unrelated-data");
            if phase == Phase::Sync {
                assert_eq!(partial, data);
            } else {
                assert!(!partial.is_empty() && partial.len() < data.len());
                assert_eq!(partial, data[..partial.len()]);
            }
        }
    }
}

#[cfg(unix)]
#[test]
fn undo_move_with_unlinked_open_target_must_not_delete_source() {
    use crate::fs_utils::copy_test_hooks::{Phase, Scope};
    let root = uniq_path("undo-move-unlinked-target");
    let source = root.join("source.bin");
    let target = root.join("target.bin");
    let data = vec![0x44; 32 * 1024];
    write_file(&source, &data);
    let scope = Scope::new(move |_, dst, phase, bytes| {
        if phase == Phase::Rename {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "force copy fallback",
            ));
        }
        if phase == Phase::Write && bytes == 8192 {
            fs::remove_file(dst)?;
        }
        Ok(())
    });
    let result = move_with_fallback(&source, &target);
    drop(scope);
    let source_data = fs::read(&source);
    let target_exists = target.exists();
    fs::remove_dir_all(root).unwrap();
    assert!(
        result.is_err(),
        "sync of an unlinked inode is not successful delivery"
    );
    assert_eq!(source_data.unwrap(), data);
    assert!(!target_exists);
}

#[cfg(unix)]
#[test]
fn undo_move_with_unlinked_open_source_retains_completed_destination() {
    use crate::fs_utils::copy_test_hooks::{Phase, Scope};
    let root = uniq_path("undo-move-unlinked-source");
    let source = root.join("source.bin");
    let target = root.join("target.bin");
    let data = vec![0x46; 32 * 1024];
    write_file(&source, &data);
    let scope = Scope::new(move |src, _, phase, bytes| {
        if phase == Phase::Rename {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "force copy fallback",
            ));
        }
        if phase == Phase::Read && bytes == 8192 {
            fs::remove_file(src)?;
        }
        Ok(())
    });
    let result = move_with_fallback(&source, &target);
    drop(scope);
    let target_data = fs::read(&target).unwrap();
    let source_exists = source.exists();
    fs::remove_dir_all(root).unwrap();
    assert_eq!(result.unwrap_err().code(), UndoErrorCode::NotFound);
    assert!(!source_exists);
    assert_eq!(target_data, data);
}

#[cfg(unix)]
#[test]
fn undo_move_with_replaced_target_preserves_source_and_competing_file() {
    use crate::fs_utils::copy_test_hooks::{Phase, Scope};
    let root = uniq_path("undo-move-replaced-target");
    let source = root.join("source.bin");
    let target = root.join("target.bin");
    let retained = root.join("retained-copy.bin");
    let data = vec![0x47; 32 * 1024];
    write_file(&source, &data);
    let saved_copy = retained.clone();
    let scope = Scope::new(move |_, dst, phase, _| {
        if phase == Phase::Rename {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "force copy fallback",
            ));
        }
        if phase == Phase::Synced {
            fs::rename(dst, &saved_copy)?;
            fs::write(dst, b"competing-data")?;
        }
        Ok(())
    });
    let result = move_with_fallback(&source, &target);
    drop(scope);
    let source_data = fs::read(&source).unwrap();
    let target_data = fs::read(&target).unwrap();
    let retained_data = fs::read(&retained).unwrap();
    fs::remove_dir_all(root).unwrap();
    assert_eq!(result.unwrap_err().code(), UndoErrorCode::IoError);
    assert_eq!(source_data, data);
    assert_eq!(target_data, b"competing-data");
    assert_eq!(retained_data, data);
}

#[test]
fn undo_fallback_move_keeps_sources_edited_after_copying() {
    use crate::fs_utils::copy_test_hooks::{Phase, Scope};
    for directory in [false, true] {
        let root = uniq_path("undo-move-edited-source");
        let source = root.join("source");
        let target = root.join("target");
        let source_file = if directory {
            source.join("deep/file.bin")
        } else {
            source.clone()
        };
        let target_file = if directory {
            target.join("deep/file.bin")
        } else {
            target.clone()
        };
        let data = vec![0x46; 32 * 1024];
        write_file(&source_file, &data);
        let scope = Scope::new(|src, _, phase, _| {
            if phase == Phase::Rename {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Unsupported,
                    "force fallback",
                ));
            }
            if phase == Phase::Synced {
                fs::write(src, b"edited-source-content")?;
            }
            Ok(())
        });
        let result = move_with_fallback(&source, &target);
        drop(scope);
        let source_data = fs::read(&source_file).unwrap();
        let target_data = fs::read(&target_file).unwrap();
        fs::remove_dir_all(root).unwrap();
        assert!(result.is_err());
        assert_eq!(source_data, b"edited-source-content");
        assert_eq!(target_data, data);
    }
}

#[test]
fn rename_and_undo_redo() {
    let dir = uniq_path("rename");
    let _ = fs::create_dir_all(&dir);
    let from = dir.join("a.txt");
    let to = dir.join("b.txt");
    write_file(&from, b"hello");

    let mut mgr = UndoManager::new();
    mgr.apply(Action::Rename {
        from: from.clone(),
        to: to.clone(),
    })
    .unwrap();
    assert!(!from.exists());
    assert!(to.exists());

    mgr.undo().unwrap();
    assert!(from.exists());
    assert!(!to.exists());

    mgr.redo().unwrap();
    assert!(!from.exists());
    assert!(to.exists());

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn delete_and_restore() {
    let _ = test_undo_dir();
    let dir = uniq_path("delete");
    let _ = fs::create_dir_all(&dir);
    let path = dir.join("file.txt");
    write_file(&path, b"bye");
    let backup = temp_backup_path(&path).unwrap();

    let mut mgr = UndoManager::new();
    mgr.apply(Action::Delete {
        path: path.clone(),
        backup: backup.clone(),
    })
    .unwrap();
    assert!(!path.exists());
    assert!(backup.exists());

    mgr.undo().unwrap();
    assert!(path.exists());
    assert!(!backup.exists());

    let _ = fs::remove_dir_all(&dir);
    let _ = fs::remove_dir_all(backup.parent().unwrap_or_else(|| Path::new(".")));
}

#[test]
fn create_folder_and_undo() {
    let path = uniq_path("mkdir");
    let mut mgr = UndoManager::new();
    mgr.apply(Action::CreateFolder { path: path.clone() })
        .unwrap();
    assert!(path.is_dir());

    mgr.undo().unwrap();
    assert!(!path.exists());

    let _ = fs::remove_dir_all(&path);
}

#[test]
fn create_file_action_undo_redo() {
    let path = uniq_path("create-file").join("file.txt");
    write_file(&path, b"hello");
    assert!(path.exists());

    let backup = temp_backup_path(&path).unwrap();
    let mut mgr = UndoManager::new();
    mgr.record_applied(Action::Create {
        path: path.clone(),
        backup: backup.clone(),
    });

    mgr.undo().unwrap();
    assert!(!path.exists());
    assert!(backup.exists());

    mgr.redo().unwrap();
    assert!(path.exists());
    assert!(!backup.exists());

    let _ = fs::remove_dir_all(path.parent().unwrap_or_else(|| Path::new(".")));
}

#[test]
fn create_dir_action_undo_redo() {
    let dir = uniq_path("create-dir");
    fs::create_dir_all(&dir).unwrap();
    let backup = temp_backup_path(&dir).unwrap();

    let mut mgr = UndoManager::new();
    mgr.record_applied(Action::Create {
        path: dir.clone(),
        backup: backup.clone(),
    });

    mgr.undo().unwrap();
    assert!(!dir.exists());
    assert!(backup.exists());

    mgr.redo().unwrap();
    assert!(dir.exists());
    assert!(!backup.exists());

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn batch_apply_and_undo_redo() {
    let dir = uniq_path("batch");
    let _ = fs::create_dir_all(&dir);
    let source = dir.join("a.txt");
    let subdir = dir.join("nested");
    let moved = subdir.join("a.txt");
    let copied = dir.join("b.txt");
    write_file(&source, b"hello");

    let mut mgr = UndoManager::new();
    mgr.apply(Action::Batch(vec![
        Action::CreateFolder {
            path: subdir.clone(),
        },
        Action::Move {
            from: source.clone(),
            to: moved.clone(),
        },
        Action::Copy {
            from: moved.clone(),
            to: copied.clone(),
        },
    ]))
    .unwrap();

    assert!(!source.exists());
    assert!(moved.exists());
    assert!(copied.exists());
    assert!(subdir.exists());

    mgr.undo().unwrap();
    assert!(source.exists());
    assert!(!moved.exists());
    assert!(!copied.exists());
    assert!(!subdir.exists());

    mgr.redo().unwrap();
    assert!(!source.exists());
    assert!(moved.exists());
    assert!(copied.exists());

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn batch_rolls_back_on_failure() {
    let dir = uniq_path("batch-fail");
    let _ = fs::create_dir_all(&dir);
    let source = dir.join("source.txt");
    let existing = dir.join("existing.txt");
    let new_dir = dir.join("new-dir");
    write_file(&source, b"hello");
    write_file(&existing, b"keep");

    let mut mgr = UndoManager::new();
    let err = mgr
        .apply(Action::Batch(vec![
            Action::CreateFolder {
                path: new_dir.clone(),
            },
            Action::Copy {
                from: source.clone(),
                to: existing.clone(),
            },
        ]))
        .unwrap_err();
    assert!(err.to_string().contains("Batch action 2 failed"));
    assert!(source.exists());
    assert!(existing.exists());
    assert!(!new_dir.exists());
    assert!(!mgr.can_undo());
    assert!(!mgr.can_redo());

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn move_with_fallback_refuses_existing_destination() {
    let dir = uniq_path("move-no-overwrite");
    let _ = fs::create_dir_all(&dir);
    let source = dir.join("source.txt");
    let dest = dir.join("dest.txt");
    write_file(&source, b"source-data");
    write_file(&dest, b"dest-data");

    let err = move_with_fallback(&source, &dest).expect_err("existing destination should fail");
    let err_msg = err.to_string();
    assert!(
        err_msg.contains("File exists")
            || err_msg.contains("already exists")
            || err_msg.contains("rename"),
        "unexpected error: {err}"
    );
    assert!(source.exists(), "source should remain when move fails");
    assert_eq!(fs::read(&dest).unwrap_or_default(), b"dest-data");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn copy_delete_fallback_moves_file_without_overwrite() {
    let dir = uniq_path("move-copy-delete");
    let _ = fs::create_dir_all(&dir);
    let source = dir.join("source.txt");
    let dest = dir.join("dest.txt");
    write_file(&source, b"source-data");
    let src_snapshot = snapshot_existing_path(&source).expect("snapshot");

    move_by_copy_delete_noreplace(&source, &dest, &src_snapshot).expect("fallback move");
    assert!(
        !source.exists(),
        "source should be deleted after fallback move"
    );
    assert_eq!(fs::read(&dest).unwrap_or_default(), b"source-data");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn copy_delete_fallback_refuses_existing_destination() {
    let dir = uniq_path("move-copy-delete-exists");
    let _ = fs::create_dir_all(&dir);
    let source = dir.join("source.txt");
    let dest = dir.join("dest.txt");
    write_file(&source, b"source-data");
    write_file(&dest, b"dest-data");
    let src_snapshot = snapshot_existing_path(&source).expect("snapshot");

    let err = move_by_copy_delete_noreplace(&source, &dest, &src_snapshot)
        .expect_err("fallback move should fail when destination exists");
    assert!(is_destination_exists_error(&err), "unexpected error: {err}");
    assert!(
        source.exists(),
        "source should remain when destination exists"
    );
    assert_eq!(fs::read(&dest).unwrap_or_default(), b"dest-data");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn fallback_copy_reports_final_sync_failure_and_keeps_source() {
    let root = uniq_path("copy-sync-failure");
    let source = root.join("source.txt");
    let destination = root.join("destination.txt");
    write_file(&source, b"original-data");
    let error = super::path_ops::copy_file_noreplace_with_sync(&source, &destination, |file| {
        assert_eq!(
            file.metadata().unwrap().len(),
            13,
            "all writes precede sync"
        );
        Err(std::io::Error::other("injected delayed writeback failure"))
    })
    .unwrap_err();
    assert_eq!(error.code(), UndoErrorCode::IoError);
    assert!(error
        .to_string()
        .contains("injected delayed writeback failure"));
    assert_eq!(fs::read(&source).unwrap(), b"original-data");
    assert_eq!(fs::read(&destination).unwrap(), b"original-data");
    fs::remove_dir_all(root).unwrap();
}

#[test]
#[cfg(unix)]
fn fallback_move_rejects_fifo_without_deleting_source_or_creating_target() {
    use std::os::unix::ffi::OsStrExt;
    let root = uniq_path("fallback-fifo");
    fs::create_dir(&root).unwrap();
    let source = root.join("pipe");
    let destination = root.join("destination");
    let cpath = std::ffi::CString::new(source.as_os_str().as_bytes()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(cpath.as_ptr(), 0o600) }, 0);
    let snapshot = snapshot_existing_path(&source).unwrap();
    assert!(move_by_copy_delete_noreplace(&source, &destination, &snapshot).is_err());
    assert!(fs::symlink_metadata(&source).is_ok());
    assert!(!destination.exists());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn fallback_copy_checks_sync_only_after_exclusive_destination_creation() {
    let root = uniq_path("copy-sync-existing");
    let source = root.join("source.txt");
    let destination = root.join("destination.txt");
    write_file(&source, b"original-data");
    write_file(&destination, b"other-process-data");
    let error = super::path_ops::copy_file_noreplace_with_sync(&source, &destination, |_| {
        panic!("an existing destination must never be opened for finalization");
    })
    .unwrap_err();
    assert_eq!(error.code(), UndoErrorCode::TargetExists);
    assert_eq!(fs::read(&source).unwrap(), b"original-data");
    assert_eq!(fs::read(&destination).unwrap(), b"other-process-data");
    fs::remove_dir_all(root).unwrap();
}

#[test]
#[cfg(unix)]
fn copy_delete_fallback_keeps_complete_destination_after_partial_source_deletion() {
    use std::os::unix::fs::PermissionsExt;
    if unsafe { libc::geteuid() } == 0 {
        return;
    }
    let root = uniq_path("fallback-partial-delete");
    let parent = root.join("source-parent");
    let source = parent.join("tree");
    let dest = root.join("destination");
    write_file(&source.join("document.txt"), b"irreplaceable-data");
    let snapshot = snapshot_existing_path(&source).unwrap();
    // Child removal is allowed, but removing the source root from its parent
    // fails only after the directory's contents have already been deleted.
    fs::set_permissions(&parent, fs::Permissions::from_mode(0o500)).unwrap();
    let result = move_by_copy_delete_noreplace(&source, &dest, &snapshot);
    fs::set_permissions(&parent, fs::Permissions::from_mode(0o700)).unwrap();
    let original_exists = source.join("document.txt").exists();
    let copied = fs::read(dest.join("document.txt")).ok();
    fs::remove_dir_all(root).unwrap();
    assert!(result.is_err());
    assert!(
        !original_exists,
        "source deletion must actually have progressed"
    );
    assert_eq!(
        copied.as_deref(),
        Some(b"irreplaceable-data".as_slice()),
        "failed source deletion must never destroy the only complete copy"
    );
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("destination retained"));
}

#[test]
fn delete_entry_path_removes_non_empty_directory() {
    let dir = uniq_path("delete-dir-recursive");
    let nested = dir.join("nested");
    let deep_file = nested.join("child.txt");
    let _ = fs::create_dir_all(&nested);
    write_file(&deep_file, b"child");

    delete_entry_path(&dir).expect("recursive delete should succeed");
    assert!(!dir.exists(), "directory should be removed recursively");
}

#[test]
fn undo_failure_restores_stack() {
    let _ = test_undo_dir();
    let dir = uniq_path("undo-fail");
    let _ = fs::create_dir_all(&dir);
    let path = dir.join("file.txt");
    write_file(&path, b"bye");
    let backup = temp_backup_path(&path).unwrap();

    let mut mgr = UndoManager::new();
    mgr.apply(Action::Delete {
        path: path.clone(),
        backup: backup.clone(),
    })
    .unwrap();
    assert!(!path.exists());
    assert!(backup.exists());

    let _ = fs::remove_file(&backup);
    let err = mgr.undo().unwrap_err();
    let err_msg = err.to_string();
    assert!(
        err_msg.contains("Backup")
            || err_msg.contains("rename")
            || err_msg.contains("metadata")
            || err_msg.contains("does not exist")
    );
    assert!(mgr.can_undo());
    assert!(!mgr.can_redo());

    let _ = fs::remove_dir_all(&dir);
    let _ = fs::remove_dir_all(backup.parent().unwrap_or_else(|| Path::new(".")));
}

#[test]
fn path_snapshot_accepts_unchanged_path() {
    let dir = uniq_path("snapshot-unchanged");
    let _ = fs::create_dir_all(&dir);
    let path = dir.join("file.txt");
    write_file(&path, b"one");

    let snapshot = snapshot_existing_path(&path).expect("snapshot should succeed");
    assert!(
        assert_path_snapshot(&path, &snapshot).is_ok(),
        "unchanged path should pass snapshot check"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn history_keeps_only_the_last_fifty_actions() {
    let root = uniq_path("history-cap");
    fs::create_dir(&root).unwrap();
    let mut manager = UndoManager::new();
    for index in 0..51 {
        manager
            .apply(Action::CreateFolder {
                path: root.join(index.to_string()),
            })
            .unwrap();
    }
    for _ in 0..50 {
        manager.undo().unwrap();
    }
    assert!(!manager.can_undo());
    assert!(manager.can_redo());
    assert!(
        root.join("0").is_dir(),
        "oldest action is outside retained history"
    );
    assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn new_action_clears_redo_and_a_new_manager_has_no_history() {
    let root = uniq_path("history-lifetime");
    fs::create_dir(&root).unwrap();
    let mut manager = UndoManager::new();
    manager
        .apply(Action::CreateFolder {
            path: root.join("first"),
        })
        .unwrap();
    manager.undo().unwrap();
    assert!(manager.can_redo());
    manager
        .apply(Action::CreateFolder {
            path: root.join("second"),
        })
        .unwrap();
    assert!(!manager.can_redo());
    assert_eq!(
        manager.redo().unwrap_err().code(),
        UndoErrorCode::RedoUnavailable
    );
    let mut fresh = UndoManager::new();
    assert!(!fresh.can_undo());
    assert_eq!(
        fresh.undo().unwrap_err().code(),
        UndoErrorCode::UndoUnavailable
    );
    assert!(root.join("second").exists());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn redo_failure_keeps_history_and_preserves_conflicting_destination() {
    let root = uniq_path("redo-conflict");
    let source = root.join("source.txt");
    let destination = root.join("destination.txt");
    write_file(&source, b"original");
    let mut manager = UndoManager::new();
    manager
        .apply(Action::Move {
            from: source.clone(),
            to: destination.clone(),
        })
        .unwrap();
    manager.undo().unwrap();
    write_file(&destination, b"other-process-data");
    assert_eq!(
        manager.redo().unwrap_err().code(),
        UndoErrorCode::TargetExists
    );
    assert!(manager.can_redo());
    assert!(!manager.can_undo());
    assert_eq!(fs::read(&source).unwrap(), b"original");
    assert_eq!(fs::read(&destination).unwrap(), b"other-process-data");
    fs::remove_file(&destination).unwrap();
    manager.redo().unwrap();
    assert!(manager.can_undo());
    assert!(!source.exists());
    assert_eq!(fs::read(&destination).unwrap(), b"original");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn path_snapshot_detects_replaced_path() {
    let dir = uniq_path("snapshot-replaced");
    let _ = fs::create_dir_all(&dir);
    let path = dir.join("file.txt");
    write_file(&path, b"first");

    let snapshot = snapshot_existing_path(&path).expect("snapshot should succeed");
    let _ = fs::remove_file(&path);
    // Replace with a different path kind to avoid inode-reuse flakiness on CI filesystems.
    let _ = fs::create_dir(&path);

    let err = assert_path_snapshot(&path, &snapshot).expect_err("snapshot mismatch expected");
    assert!(err.to_string().contains("Path changed during operation"));

    let _ = fs::remove_dir_all(&dir);
}
