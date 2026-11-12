//! Release backups that no undo or redo entry can reach any more.
//!
//! History is limited to this session and 50 actions, so a backup outside it
//! can only be recovered manually. Delete it once its action leaves history,
//! and release the remaining history on a normal exit. Interrupted operations
//! keep their recovery markers and backups; so do actions whose undo or redo
//! failed, because their on-disk state is uncertain.
use std::{
    collections::{HashSet, VecDeque},
    ffi::OsString,
    fs, io,
    path::{Component, Path, PathBuf},
    sync::{Condvar, Mutex, OnceLock},
    time::{Duration, Instant},
};

use tracing::warn;

use super::{backup::RECOVERY_SUFFIX, Action, UndoError, UndoResult};

/// Backups whose data only this history entry could restore.
pub(super) fn releasable_backups(action: &Action, out: &mut Vec<PathBuf>) {
    match action {
        Action::Delete {
            backup, protection, ..
        } => {
            if !protection.as_ref().is_some_and(|p| p.is_active()) {
                out.push(backup.clone());
            }
        }
        Action::Create { backup, .. } => out.push(backup.clone()),
        Action::Copy { receipt, .. } => {
            if let Some(path) = receipt.releasable_backup() {
                out.push(path.to_path_buf());
            }
        }
        Action::Batch(actions) => actions.iter().for_each(|a| releasable_backups(a, out)),
        _ => {}
    }
}

fn retained() -> &'static Mutex<HashSet<PathBuf>> {
    static RETAINED: OnceLock<Mutex<HashSet<PathBuf>>> = OnceLock::new();
    RETAINED.get_or_init(Mutex::default)
}

/// An undo/redo that failed may leave data only in its backups. Never release
/// them automatically; they remain available through manual recovery.
pub(super) fn retain(action: &Action) {
    let mut paths = Vec::new();
    releasable_backups(action, &mut paths);
    if let Ok(mut retained) = retained().lock() {
        retained.extend(paths);
    }
}

#[derive(Default)]
struct Queue {
    pending: VecDeque<PathBuf>,
    active: bool,
    worker: bool,
}

fn queue() -> &'static (Mutex<Queue>, Condvar) {
    static QUEUE: OnceLock<(Mutex<Queue>, Condvar)> = OnceLock::new();
    QUEUE.get_or_init(Default::default)
}

/// Release the backups of actions that have left history.
pub(super) fn release(actions: Vec<Action>) {
    let mut paths = Vec::new();
    for action in &actions {
        releasable_backups(action, &mut paths);
    }
    if paths.is_empty() {
        return;
    }
    // Tests observe the result deterministically; the app never deletes large
    // trees while the history lock is held.
    if cfg!(test) {
        for path in paths {
            release_logged(&path, true);
        }
        return;
    }
    let (lock, changed) = queue();
    let Ok(mut state) = lock.lock() else {
        return;
    };
    state.pending.extend(paths);
    if !state.worker {
        let spawned = std::thread::Builder::new()
            .name("undo-backup-release".into())
            .spawn(worker);
        match spawned {
            Ok(_) => state.worker = true,
            Err(error) => warn!(%error, "Undo backup release worker unavailable"),
        }
    }
    changed.notify_all();
}

fn worker() {
    let (lock, changed) = queue();
    loop {
        let path = {
            let Ok(mut state) = lock.lock() else {
                return;
            };
            loop {
                if let Some(path) = state.pending.pop_front() {
                    state.active = true;
                    break path;
                }
                state = match changed.wait(state) {
                    Ok(state) => state,
                    Err(_) => return,
                };
            }
        };
        release_logged(&path, true);
        if let Ok(mut state) = lock.lock() {
            state.active = false;
        }
        changed.notify_all();
    }
}

/// On a normal exit, history ends. Release it and any queued backups without
/// waiting for active recovery copies; those backups are simply kept.
pub(crate) fn release_on_exit(state: &super::UndoState) {
    let actions = match state.clone_inner().try_lock() {
        Ok(mut manager) => manager.take_all(),
        // An operation still holds history; keep everything it may reference.
        Err(_) => Vec::new(),
    };
    let mut paths = Vec::new();
    for action in &actions {
        releasable_backups(action, &mut paths);
    }
    let (lock, changed) = queue();
    if let Ok(mut queued) = lock.lock() {
        paths.extend(queued.pending.drain(..));
        // Let a deletion in progress finish instead of stopping it midway.
        let deadline = Instant::now() + Duration::from_secs(2);
        while queued.active {
            let Some(timeout) = deadline.checked_duration_since(Instant::now()) else {
                break;
            };
            queued = match changed.wait_timeout(queued, timeout) {
                Ok((queued, _)) => queued,
                Err(_) => break,
            };
        }
    }
    for path in paths {
        release_logged(&path, false);
    }
}

fn release_logged(path: &Path, wait_for_readers: bool) {
    if let Err(error) = release_backup(path, wait_for_readers) {
        warn!(%error, backup = %path.display(), "Retain unreachable undo backup");
    }
}

pub(super) fn release_backup(path: &Path, wait_for_readers: bool) -> UndoResult<()> {
    if retained().lock().is_ok_and(|set| set.contains(path)) {
        return Ok(());
    }
    // "Delete all" holds this exclusively and removes the same data itself.
    let _operation = super::backup_operation()?;
    // Only this instance's registered, verified session; never another
    // instance's backups or an older session left after a crash.
    let Some(session) = super::backup::owned_session(path)? else {
        return Ok(());
    };
    let relative = path
        .strip_prefix(&session.directory)
        .map_err(|_| UndoError::invalid_input("Backup is outside the owned session"))?;
    let mut parts = relative.components();
    let (Some(Component::Normal(bucket)), Some(Component::Normal(_)), None) =
        (parts.next(), parts.next(), parts.next())
    else {
        return Err(UndoError::invalid_input("Unexpected backup location"));
    };
    let bucket = bucket.to_os_string();
    crate::fs_utils::check_no_symlink_components(&session.directory)?;
    let marker = session.directory.join(sidecar(&bucket, RECOVERY_SUFFIX));
    match fs::symlink_metadata(&marker) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        // A recovery marker, or one we cannot inspect: keep the backup.
        _ => return Ok(()),
    }
    let _guard = if wait_for_readers {
        super::write_backups(&[path])?
    } else {
        match super::backup_access::try_write_backup(path)? {
            Some(guard) => guard,
            None => return Ok(()),
        }
    };
    match fs::symlink_metadata(path) {
        Ok(_) => super::nofollow::delete_backup_tree_nofollow(path)
            .map_err(|error| UndoError::from_io_error("Release undo backup", error))?,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(UndoError::from_io_error("Inspect undo backup", error)),
    }
    // Each bucket belongs to one backup. Never remove it recursively: anything
    // else found there is kept together with its descriptive sidecars.
    let bucket_dir = session.directory.join(&bucket);
    match fs::remove_dir(&bucket_dir) {
        Ok(()) => {}
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(_) => return Ok(()),
    }
    remove_sidecars(&session.directory, &bucket)
}

fn sidecar(bucket: &OsString, suffix: &str) -> OsString {
    let mut name = bucket.clone();
    name.push(suffix);
    name
}

/// Remove the origin record and recovered-state files of a released bucket.
fn remove_sidecars(directory: &Path, bucket: &OsString) -> UndoResult<()> {
    let origin = sidecar(bucket, ".origin.json");
    let recovered = sidecar(bucket, ".recovered-");
    let entries = fs::read_dir(directory)
        .map_err(|error| UndoError::from_io_error("Read undo session", error))?;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let matches = name == origin
            || name
                .as_encoded_bytes()
                .starts_with(recovered.as_encoded_bytes());
        if !matches || !entry.file_type().is_ok_and(|kind| kind.is_file()) {
            continue;
        }
        if let Err(error) = fs::remove_file(entry.path()) {
            if error.kind() != io::ErrorKind::NotFound {
                return Err(UndoError::from_io_error("Remove undo backup record", error));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::undo::{
        backup::{forget_test_session, temp_backup_path_at, BackupProtection, RecoveryMarker},
        UndoManager, UndoState,
    };
    use std::time::{SystemTime, UNIX_EPOCH};

    struct Fixture {
        base: PathBuf,
        root: PathBuf,
    }

    impl Fixture {
        fn new(label: &str) -> Self {
            let stamp = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let base = std::env::temp_dir().join(format!("browsey-release-base-{label}-{stamp}"));
            let root = std::env::temp_dir().join(format!("browsey-release-root-{label}-{stamp}"));
            fs::create_dir_all(&root).unwrap();
            Self { base, root }
        }

        fn backup(&self, original: &Path) -> PathBuf {
            temp_backup_path_at(&self.base, original).unwrap()
        }

        fn file(&self, name: &str) -> PathBuf {
            let path = self.root.join(name);
            fs::write(&path, name).unwrap();
            path
        }

        fn session_entries(&self) -> Vec<String> {
            let session = fs::read_dir(&self.base)
                .unwrap()
                .flatten()
                .find(|entry| entry.file_type().unwrap().is_dir())
                .unwrap()
                .path();
            let mut names: Vec<_> = fs::read_dir(session)
                .unwrap()
                .flatten()
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .collect();
            names.sort();
            names
        }

        /// Push 50 unrelated actions so the first one leaves history.
        fn evict_oldest(&self, manager: &mut UndoManager) {
            for index in 0..50 {
                manager
                    .apply(Action::CreateFolder {
                        path: self.root.join(format!("filler-{index}")),
                    })
                    .unwrap();
            }
        }
    }

    impl Drop for Fixture {
        fn drop(&mut self) {
            forget_test_session(&self.base);
            let _ = fs::remove_dir_all(&self.base);
            let _ = fs::remove_dir_all(&self.root);
        }
    }

    fn delete(path: &Path, backup: &Path) -> Action {
        Action::Delete {
            path: path.to_path_buf(),
            backup: backup.to_path_buf(),
            protection: None,
        }
    }

    #[test]
    fn evicted_delete_releases_backup_bucket_and_records() {
        let fx = Fixture::new("evict");
        let original = fx.file("deleted.txt");
        let backup = fx.backup(&original);
        let mut manager = UndoManager::new();
        manager.apply(delete(&original, &backup)).unwrap();
        assert!(backup.exists());
        assert!(!fx.session_entries().is_empty());

        fx.evict_oldest(&mut manager);
        assert!(!backup.exists());
        assert!(
            fx.session_entries().is_empty(),
            "{:?}",
            fx.session_entries()
        );
    }

    #[test]
    fn retained_history_keeps_its_backup() {
        let fx = Fixture::new("within-history");
        let original = fx.file("deleted.txt");
        let backup = fx.backup(&original);
        let mut manager = UndoManager::new();
        manager.apply(delete(&original, &backup)).unwrap();
        for index in 0..49 {
            manager
                .apply(Action::CreateFolder {
                    path: fx.root.join(format!("filler-{index}")),
                })
                .unwrap();
        }
        assert!(backup.exists(), "the 50th action can still be undone");
    }

    #[test]
    fn discarded_redo_releases_an_undone_creation() {
        let fx = Fixture::new("redo");
        let created = fx.file("archive.zip");
        let backup = fx.backup(&created);
        let mut manager = UndoManager::new();
        manager.record_applied(Action::Create {
            path: created.clone(),
            backup: backup.clone(),
        });
        manager.undo().unwrap();
        assert!(backup.exists() && !created.exists());

        manager
            .apply(Action::CreateFolder {
                path: fx.root.join("next"),
            })
            .unwrap();
        assert!(!manager.can_redo());
        assert!(!backup.exists());
        assert!(fx.session_entries().is_empty());
    }

    #[test]
    fn undo_and_redo_keep_backups_they_still_need() {
        let fx = Fixture::new("cycle");
        let original = fx.file("deleted.txt");
        let backup = fx.backup(&original);
        let mut manager = UndoManager::new();
        manager.apply(delete(&original, &backup)).unwrap();
        manager.undo().unwrap();
        manager.redo().unwrap();
        assert!(backup.exists() && !original.exists());
        manager.undo().unwrap();
        assert_eq!(fs::read(&original).unwrap(), b"deleted.txt");
    }

    #[test]
    fn active_protection_and_recovery_markers_retain_backups() {
        let fx = Fixture::new("markers");
        let protected = fx.file("protected.txt");
        let protected_backup = fx.backup(&protected);
        let protection = BackupProtection::create(&protected_backup, &protected).unwrap();
        let marked = fx.file("marked.txt");
        let marked_backup = fx.backup(&marked);
        let _marker = RecoveryMarker::create(&marked_backup, &marked).unwrap();
        for (path, backup) in [(&protected, &protected_backup), (&marked, &marked_backup)] {
            fs::create_dir_all(backup.parent().unwrap()).unwrap();
            fs::rename(path, backup).unwrap();
        }
        let mut manager = UndoManager::new();
        manager.record_applied(Action::Batch(vec![
            Action::Delete {
                path: protected.clone(),
                backup: protected_backup.clone(),
                protection: Some(protection),
            },
            delete(&marked, &marked_backup),
        ]));

        fx.evict_oldest(&mut manager);
        assert!(protected_backup.exists());
        assert!(marked_backup.exists());
    }

    #[test]
    fn failed_undo_retains_backup_after_eviction() {
        let fx = Fixture::new("failed-undo");
        let original = fx.file("deleted.txt");
        let backup = fx.backup(&original);
        let mut manager = UndoManager::new();
        manager.apply(delete(&original, &backup)).unwrap();
        fs::write(&original, b"newer file").unwrap();
        assert!(manager.undo().is_err());

        fx.evict_oldest(&mut manager);
        assert_eq!(fs::read(&backup).unwrap(), b"deleted.txt");
        assert_eq!(fs::read(&original).unwrap(), b"newer file");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn nested_symlinks_are_unlinked_without_touching_targets() {
        let fx = Fixture::new("symlink");
        let outside = fx.file("outside.txt");
        let folder = fx.root.join("project");
        fs::create_dir_all(folder.join("bin")).unwrap();
        fs::write(folder.join("a.txt"), b"a").unwrap();
        std::os::unix::fs::symlink(&outside, folder.join("bin/link")).unwrap();
        std::os::unix::fs::symlink(&fx.root, folder.join("dir-link")).unwrap();
        let backup = fx.backup(&folder);
        let mut manager = UndoManager::new();
        manager.apply(delete(&folder, &backup)).unwrap();

        fx.evict_oldest(&mut manager);
        assert!(!backup.exists());
        assert_eq!(fs::read(&outside).unwrap(), b"outside.txt");
        assert!(fx.root.join("filler-0").is_dir());
    }

    #[test]
    fn paths_outside_the_owned_session_are_never_deleted() {
        let fx = Fixture::new("foreign");
        let foreign = fx.file("not-a-backup.txt");
        release_backup(&foreign, true).unwrap();
        assert!(foreign.exists());

        let legacy = fx.base.join("session-1-1-0").join("bucket").join("item");
        fs::create_dir_all(legacy.parent().unwrap()).unwrap();
        fs::write(&legacy, b"other instance").unwrap();
        release_backup(&legacy, true).unwrap();
        assert!(legacy.exists());
    }

    #[test]
    fn exit_releases_history_but_keeps_backups_being_read() {
        let fx = Fixture::new("exit");
        let first = fx.file("first.txt");
        let first_backup = fx.backup(&first);
        let second = fx.file("second.txt");
        let second_backup = fx.backup(&second);
        let state = UndoState::default();
        state.record(delete(&first, &first_backup)).unwrap();
        state.record(delete(&second, &second_backup)).unwrap();
        let session = crate::undo::backup::owned_session(&second_backup)
            .unwrap()
            .unwrap();
        let reader = session
            .access
            .try_read(session.bucket(&second_backup).unwrap())
            .unwrap()
            .unwrap();

        release_on_exit(&state);
        drop(reader);
        assert!(!first_backup.exists());
        assert!(second_backup.exists(), "an active recovery copy keeps it");
        assert!(!state.clone_inner().lock().unwrap().can_undo());
    }
}
