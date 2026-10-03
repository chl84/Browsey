use std::collections::{hash_map::DefaultHasher, HashMap};
use std::fs::{self, File, OpenOptions};
use std::hash::{Hash, Hasher};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tracing::warn;

use crate::undo::{UndoError, UndoResult};

pub(super) const RECOVERY_SUFFIX: &str = ".recovery-required";

#[derive(Debug, Clone)]
pub(super) struct RecoveryMarker {
    path: PathBuf,
    identity: crate::fs_utils::FileIdentity,
}

impl RecoveryMarker {
    pub(super) fn create(backup: &Path, destination: &Path) -> UndoResult<Self> {
        let bucket = backup
            .parent()
            .ok_or_else(|| UndoError::invalid_input("Invalid backup path"))?;
        let session = bucket
            .parent()
            .ok_or_else(|| UndoError::invalid_input("Invalid backup bucket"))?;
        let mut name = bucket
            .file_name()
            .ok_or_else(|| UndoError::invalid_input("Invalid backup bucket name"))?
            .to_os_string();
        name.push(RECOVERY_SUFFIX);
        let path = session.join(name);
        crate::path_guard::ensure_no_symlink_components_existing_prefix(&path).map_err(
            |error| UndoError::invalid_input(format!("Unsafe recovery marker: {error}")),
        )?;
        #[cfg(test)]
        crate::fs_utils::copy_test_hooks::hit(
            destination,
            backup,
            crate::fs_utils::copy_test_hooks::Phase::RecoveryMarker,
            0,
        )
        .map_err(|error| UndoError::from_io_error("Create copy recovery marker", error))?;
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options
            .open(&path)
            .map_err(|error| UndoError::from_io_error("Create copy recovery marker", error))?;
        writeln!(file, "Browsey interrupted/failed copy undo or redo. Recover this backup manually before deleting the session.\nDestination: {destination:?}\nBackup: {backup:?}")
            .and_then(|()| file.sync_all())
            .map_err(|error| UndoError::from_io_error("Finalize copy recovery marker", error))?;
        let identity = crate::fs_utils::FileIdentity::from_file(&file)
            .ok_or_else(|| UndoError::invalid_input("Cannot verify recovery marker ownership"))?;
        if !identity.matches(&path) {
            return Err(UndoError::snapshot_mismatch(&path));
        }
        Ok(Self { path, identity })
    }

    pub(super) fn verify(&self) -> UndoResult<()> {
        if self.identity.matches(&self.path) {
            Ok(())
        } else {
            Err(UndoError::snapshot_mismatch(&self.path))
        }
    }

    pub(super) fn clear(&self) -> UndoResult<()> {
        self.verify()?;
        fs::remove_file(&self.path).map_err(|error| {
            UndoError::from_io_error("Remove completed copy recovery marker", error)
        })
    }
}

struct BackupSession {
    directory: PathBuf,
    // Held until process exit. Cleanup in other instances must acquire this lock.
    _lock: File,
}

impl BackupSession {
    fn create(base: &Path) -> UndoResult<Self> {
        validate_undo_dir(base)?;
        crate::path_guard::ensure_no_symlink_components_existing_prefix(base)
            .map_err(|e| UndoError::invalid_input(format!("Unsafe undo directory: {e}")))?;
        let mut builder = fs::DirBuilder::new();
        builder.recursive(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder
            .create(base)
            .map_err(|e| UndoError::from_io_error("Create undo directory", e))?;
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos();
        for attempt in 0..100 {
            let name = format!("session-{}-{stamp}-{attempt}", std::process::id());
            let directory = base.join(&name);
            let lock_path = base.join(format!("{name}.lock"));
            let mut options = OpenOptions::new();
            options.read(true).write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let lock = match options.open(&lock_path) {
                Ok(lock) => lock,
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(UndoError::from_io_error("Create undo session lock", e)),
            };
            // Publish the directory only after locking, so cleanup cannot mistake
            // a session still being initialized for an abandoned one.
            lock.try_lock()
                .map_err(|e| UndoError::lock_failed(format!("Lock undo session: {e}")))?;
            builder
                .create(&directory)
                .map_err(|e| UndoError::from_io_error("Create undo session", e))?;
            return Ok(Self {
                directory,
                _lock: lock,
            });
        }
        Err(UndoError::invalid_input(
            "Unable to allocate a unique undo session",
        ))
    }
}

/// Remove only abandoned sessions, never another running instance's backups.
/// Legacy `undo/` backups are deliberately left intact: old versions have no
/// ownership locks, and may still be running while the new version is installed.
pub fn cleanup_stale_backups(max_age: Option<Duration>) {
    let base = base_undo_dir();
    if let Err(e) = validate_undo_dir(&base) {
        warn!("Skip cleanup; unsafe undo dir {:?}: {}", base, e);
        return;
    }
    cleanup_sessions(&base, max_age);
}

fn cleanup_sessions(base: &Path, max_age: Option<Duration>) {
    if crate::path_guard::ensure_no_symlink_components_existing_prefix(base).is_err() {
        return;
    }
    let Ok(entries) = fs::read_dir(base) else {
        return;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str().filter(|name| name.starts_with("session-")) else {
            continue;
        };
        if !entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            continue;
        }
        if let Some(age) = max_age {
            if !entry
                .metadata()
                .ok()
                .and_then(|meta| meta.modified().ok())
                .and_then(|modified| modified.elapsed().ok())
                .is_some_and(|elapsed| elapsed >= age)
            {
                continue;
            }
        }
        let lock_path = base.join(format!("{name}.lock"));
        if !fs::symlink_metadata(&lock_path).is_ok_and(|meta| meta.file_type().is_file()) {
            continue;
        }
        let Ok(lock) = OpenOptions::new().read(true).write(true).open(&lock_path) else {
            continue;
        };
        if lock.try_lock().is_err() {
            continue;
        }
        // Recovery markers pin the entire abandoned session. Fail closed on
        // scan errors or suspicious marker types; these are not persistent undo.
        if session_requires_recovery(&entry.path()) {
            warn!(
                "Retain undo session requiring manual recovery: {:?}",
                entry.path()
            );
            continue;
        }
        if let Err(e) = fs::remove_dir_all(entry.path()) {
            warn!(
                "Failed to remove abandoned undo session {:?}: {}",
                entry.path(),
                e
            );
            continue;
        }
        drop(lock);
        let _ = fs::remove_file(lock_path);
    }
}

fn session_requires_recovery(directory: &Path) -> bool {
    let Ok(entries) = fs::read_dir(directory) else {
        return true;
    };
    entries.into_iter().any(|entry| match entry {
        Ok(entry) => entry
            .file_name()
            .to_str()
            .is_some_and(|name| name.ends_with(RECOVERY_SUFFIX)),
        Err(_) => true,
    })
}

pub fn temp_backup_path(original: &Path) -> UndoResult<PathBuf> {
    static SESSIONS: OnceLock<Mutex<HashMap<PathBuf, BackupSession>>> = OnceLock::new();
    let base = base_undo_dir();
    let mut sessions = SESSIONS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .map_err(|_| UndoError::lock_failed("Undo session registry lock poisoned"))?;
    let session = match sessions.entry(base.clone()) {
        std::collections::hash_map::Entry::Occupied(entry) => entry.into_mut(),
        std::collections::hash_map::Entry::Vacant(entry) => {
            entry.insert(BackupSession::create(&base)?)
        }
    };
    let mut hasher = DefaultHasher::new();
    original.hash(&mut hasher);
    static NEXT_BACKUP: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let sequence = NEXT_BACKUP.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let bucket = format!("{:016x}-{sequence}", hasher.finish());
    let name = original
        .file_name()
        .unwrap_or_else(|| std::ffi::OsStr::new("item"));
    let mut candidate = session.directory.join(&bucket).join(name);
    let mut idx = 1u32;
    while fs::symlink_metadata(&candidate).is_ok() {
        let mut alternate = name.to_os_string();
        alternate.push(format!("-{idx}"));
        candidate = session.directory.join(&bucket).join(alternate);
        idx += 1;
    }
    Ok(candidate)
}

pub(super) fn base_undo_dir() -> PathBuf {
    if let Ok(custom) = std::env::var("BROWSEY_UNDO_DIR") {
        return PathBuf::from(custom);
    }
    default_undo_dir()
}

fn default_undo_dir() -> PathBuf {
    dirs_next::data_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("browsey")
        .join("undo-sessions")
}

pub(super) fn validate_undo_dir(path: &Path) -> UndoResult<()> {
    if cfg!(test) {
        return Ok(());
    }
    if !path.is_absolute() {
        return Err(UndoError::invalid_input(
            "Undo directory must be an absolute path",
        ));
    }
    if path.parent().is_none() {
        return Err(UndoError::invalid_input(
            "Undo directory cannot be the filesystem root",
        ));
    }
    let default_parent = default_undo_dir()
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("/"));
    if !path.starts_with(&default_parent) {
        return Err(UndoError::invalid_input(format!(
            "Undo directory must reside under {}",
            default_parent.display()
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unique_base() -> PathBuf {
        std::env::temp_dir().join(format!(
            "browsey-session-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ))
    }

    #[test]
    fn cleanup_child() {
        if let Some(base) = std::env::var_os("BROWSEY_TEST_CLEANUP_ROOT") {
            cleanup_sessions(Path::new(&base), None);
        }
    }

    fn cleanup_in_child(base: &Path) {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "undo::backup::tests::cleanup_child"])
            .env("BROWSEY_TEST_CLEANUP_ROOT", base)
            .status()
            .unwrap();
        assert!(status.success());
    }

    #[test]
    fn cleanup_keeps_live_sessions_and_prunes_abandoned_sessions_across_processes() {
        let base = unique_base();
        let session = BackupSession::create(&base).unwrap();
        let backup = session.directory.join("original.txt");
        fs::write(&backup, b"original").unwrap();
        cleanup_in_child(&base);
        assert_eq!(fs::read(&backup).unwrap(), b"original");
        drop(session);
        cleanup_in_child(&base);
        assert!(!backup.exists(), "released session should be pruned");
        assert_eq!(fs::read_dir(&base).unwrap().count(), 0);
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn cleanup_respects_age_and_preserves_unknown_directories() {
        let base = unique_base();
        let session = BackupSession::create(&base).unwrap();
        let directory = session.directory.clone();
        drop(session);
        fs::create_dir(base.join("legacy")).unwrap();
        cleanup_sessions(&base, Some(Duration::from_secs(3600)));
        assert!(directory.exists());
        cleanup_sessions(&base, Some(Duration::ZERO));
        assert!(!directory.exists());
        assert!(base.join("legacy").exists());
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn cleanup_preserves_abandoned_recovery_sessions_across_processes() {
        let base = unique_base();
        let recovery = BackupSession::create(&base).unwrap();
        let ordinary = BackupSession::create(&base).unwrap();
        let backup = recovery.directory.join("bucket/file.txt");
        fs::create_dir(backup.parent().unwrap()).unwrap();
        fs::write(&backup, b"recoverable").unwrap();
        let marker = RecoveryMarker::create(&backup, &base.join("destination.txt")).unwrap();
        let ordinary_directory = ordinary.directory.clone();
        drop(recovery);
        drop(ordinary);
        cleanup_in_child(&base);
        assert_eq!(fs::read(&backup).unwrap(), b"recoverable");
        assert!(!ordinary_directory.exists());
        marker.clear().unwrap();
        cleanup_in_child(&base);
        assert!(!backup.exists());
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn recovery_marker_cleanup_preserves_a_foreign_replacement() {
        let base = unique_base();
        let session = BackupSession::create(&base).unwrap();
        let backup = session.directory.join("bucket/file.txt");
        let marker = RecoveryMarker::create(&backup, &base.join("destination.txt")).unwrap();
        fs::rename(&marker.path, session.directory.join("parked-marker")).unwrap();
        fs::write(&marker.path, b"foreign marker").unwrap();
        assert!(marker.clear().is_err());
        assert_eq!(fs::read(&marker.path).unwrap(), b"foreign marker");
        drop(session);
        cleanup_in_child(&base);
        assert!(
            marker.path.exists(),
            "uncertain marker must pin the session"
        );
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn held_session_child() {
        if let Some(base) = std::env::var_os("BROWSEY_TEST_HELD_SESSION_ROOT") {
            use std::io::{Read, Write};
            let session = BackupSession::create(Path::new(&base)).unwrap();
            let protected = std::env::var_os("BROWSEY_TEST_HELD_COPY_RECOVERY").is_some();
            let backup = session.directory.join(if protected {
                "bucket/document.txt"
            } else {
                "document.txt"
            });
            fs::create_dir_all(backup.parent().unwrap()).unwrap();
            fs::write(&backup, b"owned-backup").unwrap();
            let _marker = if protected {
                Some(
                    RecoveryMarker::create(&backup, &session.directory.join("original.txt"))
                        .unwrap(),
                )
            } else {
                None
            };
            println!("BROWSEY_HELD_SESSION_READY");
            std::io::stdout().flush().unwrap();
            let _ = std::io::stdin().read(&mut [0_u8]);
            drop(session);
        }
    }

    #[test]
    fn cleanup_after_killed_process_keeps_the_other_live_session() {
        use std::io::{BufRead, BufReader};
        use std::process::{Command, Stdio};

        let base = unique_base();
        let live = BackupSession::create(&base).unwrap();
        fs::write(live.directory.join("document.txt"), b"live-backup").unwrap();
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "undo::backup::tests::held_session_child",
                "--nocapture",
            ])
            .env("BROWSEY_TEST_HELD_SESSION_ROOT", &base)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let ready = BufReader::new(child.stdout.take().unwrap())
            .lines()
            .map_while(Result::ok)
            .any(|line| line.contains("BROWSEY_HELD_SESSION_READY"));
        if !ready {
            let _ = child.kill();
            let _ = child.wait();
            panic!("child session was not initialized");
        }
        cleanup_sessions(&base, None);
        assert_eq!(
            fs::read_dir(&base)
                .unwrap()
                .filter_map(Result::ok)
                .filter(|entry| entry.file_type().unwrap().is_dir())
                .count(),
            2
        );
        child.kill().unwrap();
        assert!(!child.wait().unwrap().success());
        cleanup_sessions(&base, None);
        assert_eq!(
            fs::read(live.directory.join("document.txt")).unwrap(),
            b"live-backup"
        );
        assert_eq!(
            fs::read_dir(&base)
                .unwrap()
                .filter_map(Result::ok)
                .filter(|entry| entry.file_type().unwrap().is_dir())
                .count(),
            1
        );
        drop(live);
        cleanup_sessions(&base, None);
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn cleanup_after_killed_recovery_process_keeps_the_protected_copy() {
        use std::io::{BufRead, BufReader};
        use std::process::{Command, Stdio};
        let base = unique_base();
        let mut child = Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "undo::backup::tests::held_session_child",
                "--nocapture",
            ])
            .env("BROWSEY_TEST_HELD_SESSION_ROOT", &base)
            .env("BROWSEY_TEST_HELD_COPY_RECOVERY", "1")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let ready = BufReader::new(child.stdout.take().unwrap())
            .lines()
            .map_while(Result::ok)
            .any(|line| line.contains("BROWSEY_HELD_SESSION_READY"));
        if !ready {
            let _ = child.kill();
            let _ = child.wait();
            panic!("child recovery session was not initialized");
        }
        let session = fs::read_dir(&base)
            .unwrap()
            .filter_map(Result::ok)
            .find(|entry| entry.file_type().unwrap().is_dir())
            .unwrap()
            .path();
        child.kill().unwrap();
        assert!(!child.wait().unwrap().success());
        cleanup_in_child(&base);
        assert_eq!(
            fs::read(session.join("bucket/document.txt")).unwrap(),
            b"owned-backup"
        );
        assert!(session.join("bucket.recovery-required").exists());
        fs::remove_dir_all(base).unwrap();
    }
}
