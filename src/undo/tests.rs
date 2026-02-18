use super::*;
use std::fs::{self, OpenOptions};
use std::io::Write;
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
    let backup = temp_backup_path(&path);

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

    let backup = temp_backup_path(&path);
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
    let backup = temp_backup_path(&dir);

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
    assert!(err.contains("Batch action 2 failed"));
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
    assert!(
        err.contains("File exists") || err.contains("already exists") || err.contains("rename"),
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
    let backup = temp_backup_path(&path);

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
    assert!(
        err.contains("Backup")
            || err.contains("rename")
            || err.contains("metadata")
            || err.contains("does not exist")
    );
    assert!(mgr.can_undo());
    assert!(!mgr.can_redo());

    let _ = fs::remove_dir_all(&dir);
    let _ = fs::remove_dir_all(backup.parent().unwrap_or_else(|| Path::new(".")));
}

#[test]
fn cleanup_prunes_stale_backup_dirs() {
    let base = test_undo_dir();
    let target = base.join("dummy");
    fs::create_dir_all(&target).unwrap();

    cleanup_stale_backups(Some(Duration::from_secs(0)));

    assert!(
        !target.exists(),
        "backup base contents should be removed during cleanup"
    );
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
fn path_snapshot_detects_replaced_path() {
    let dir = uniq_path("snapshot-replaced");
    let _ = fs::create_dir_all(&dir);
    let path = dir.join("file.txt");
    write_file(&path, b"first");

    let snapshot = snapshot_existing_path(&path).expect("snapshot should succeed");
    let _ = fs::remove_file(&path);
    write_file(&path, b"second");

    let err = assert_path_snapshot(&path, &snapshot).expect_err("snapshot mismatch expected");
    assert!(err.contains("Path changed during operation"));

    let _ = fs::remove_dir_all(&dir);
}
