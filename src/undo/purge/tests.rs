use super::*;
use crate::{performance_fixture::Fixture, undo::Action};
use std::fs::{File, OpenOptions};

fn session(base: &Path, name: &str) -> std::path::PathBuf {
    let path = base.join(name);
    fs::create_dir_all(path.join("bucket/folder")).unwrap();
    fs::write(path.join("bucket/folder/photo.jpg"), b"backup").unwrap();
    fs::write(path.join("bucket.recovery-required"), b"diagnostic").unwrap();
    File::create(base.join(format!("{name}.lock"))).unwrap();
    path
}

#[test]
fn deletes_abandoned_backups_markers_and_both_history_stacks_but_keeps_originals_and_legacy() {
    let fixture = Fixture::new("purge");
    let base = fixture.0.join("backups");
    let old = session(&base, "session-abandoned");
    fs::create_dir(base.join("legacy")).unwrap();
    fs::write(base.join("legacy/photo.jpg"), b"legacy").unwrap();
    let original = fixture.0.join("original.jpg");
    fs::write(&original, b"original").unwrap();
    let mut manager = UndoManager::default();
    manager.record_applied(Action::Delete {
        path: original.clone(),
        backup: old.join("bucket/folder/photo.jpg"),
        protection: None,
    });
    let from = fixture.0.join("renamed-from");
    let to = fixture.0.join("renamed-to");
    fs::write(&to, b"renamed").unwrap();
    manager.record_applied(Action::Rename {
        from: from.clone(),
        to,
    });
    manager.undo().unwrap();
    assert!(manager.can_undo());
    assert!(manager.can_redo());
    let result = delete_at(&base, &mut manager).unwrap();
    assert_eq!(result.deleted_sessions, 1);
    assert_eq!(result.retained_sessions, 0);
    assert!(!manager.can_undo());
    assert!(!manager.can_redo());
    assert!(!old.exists());
    assert!(!base.join("session-abandoned.lock").exists());
    assert_eq!(fs::read(original).unwrap(), b"original");
    assert_eq!(fs::read(from).unwrap(), b"renamed");
    assert_eq!(fs::read(base.join("legacy/photo.jpg")).unwrap(), b"legacy");
}

#[test]
fn preserves_foreign_locked_sessions_and_deletes_unlocked_ones() {
    let fixture = Fixture::new("purge-lock");
    let base = fixture.0.join("backups");
    let live = session(&base, "session-live");
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .open(base.join("session-live.lock"))
        .unwrap();
    lock.try_lock().unwrap();
    let old = session(&base, "session-abandoned");
    let result = delete_at(&base, &mut UndoManager::default()).unwrap();
    assert_eq!(result.deleted_sessions, 1);
    assert_eq!(result.retained_sessions, 1);
    assert!(live.join("bucket/folder/photo.jpg").exists());
    assert!(!old.exists());
}

#[test]
fn clears_live_owned_backups_and_allows_new_allocations() {
    let fixture = Fixture::new("purge-owned");
    let base = fixture.0.join("backups");
    let original = fixture.0.join("photo.jpg");
    let path = backup::temp_backup_path_at(&base, &original).unwrap();
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(&path, b"backup").unwrap();
    let result = delete_at(&base, &mut UndoManager::default()).unwrap();
    assert_eq!(result.deleted_sessions, 1);
    assert!(!path.exists());
    let next = backup::temp_backup_path_at(&base, &original).unwrap();
    fs::create_dir_all(next.parent().unwrap()).unwrap();
    fs::write(&next, b"new backup").unwrap();
    assert!(backup::owned_session(&next).unwrap().is_some());
    backup::forget_test_session(&base);
}

#[cfg(unix)]
#[test]
fn unsafe_session_is_retained_without_touching_symlink_targets() {
    let fixture = Fixture::new("purge-symlink");
    let base = fixture.0.join("backups");
    let path = session(&base, "session-unsafe");
    let target = fixture.0.join("outside");
    fs::write(&target, b"keep").unwrap();
    std::os::unix::fs::symlink(&target, path.join("link")).unwrap();
    let result = delete_at(&base, &mut UndoManager::default()).unwrap();
    assert_eq!(result.retained_sessions, 1);
    assert_eq!(result.errors.len(), 1);
    assert!(path.join("bucket/folder/photo.jpg").exists());
    assert!(path.join("bucket.recovery-required").exists());
    assert_eq!(fs::read(target).unwrap(), b"keep");
}

#[test]
fn active_operation_rejects_exclusive_maintenance() {
    let _active = super::super::backup_operation().unwrap();
    assert!(super::super::maintenance::exclusive().is_err());
}
