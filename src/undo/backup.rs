use std::collections::{hash_map::DefaultHasher, HashMap};
use std::fs::{self, File, OpenOptions};
use std::hash::{Hash, Hasher};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};
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
        writeln!(file, "Browsey interrupted/failed file operation. Recover this backup manually before deleting the session.\nDestination: {destination:?}\nBackup: {backup:?}")
            .and_then(|()| {
                #[cfg(test)]
                crate::fs_utils::copy_test_hooks::hit(destination, backup,
                    crate::fs_utils::copy_test_hooks::Phase::RecoveryMarkerSync, 0)?;
                file.sync_all()
            })
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

/// Opaque protection carried with overwrite history; never cleared by Drop.
#[derive(Debug, Clone)]
pub struct BackupProtection {
    marker: Option<RecoveryMarker>,
}

impl BackupProtection {
    pub(crate) fn create(backup: &Path, destination: &Path) -> UndoResult<Self> {
        Ok(Self {
            marker: Some(RecoveryMarker::create(backup, destination)?),
        })
    }

    pub(crate) fn ensure(&mut self, backup: &Path, destination: &Path) -> UndoResult<()> {
        let result = if let Some(marker) = &self.marker {
            marker.verify()
        } else {
            RecoveryMarker::create(backup, destination).map(|marker| self.marker = Some(marker))
        };
        result.map_err(|error| {
            error.with_context(format!(
                "Original backup protection failed at {}; inspect destination {} before retrying",
                backup.display(),
                destination.display()
            ))
        })
    }

    pub(super) fn is_active(&self) -> bool {
        self.marker.is_some()
    }

    pub(super) fn finalize(&mut self) {
        if let Some(marker) = &self.marker {
            match marker.clear() {
                Ok(()) => self.marker = None,
                Err(error) => {
                    warn!(%error, "Keep original backup marker after completed operation")
                }
            }
        }
    }
}

struct BackupSession {
    directory: PathBuf,
    // Held until process exit. Cleanup in other instances must acquire this lock.
    _lock: File,
    identity: crate::fs_utils::FileIdentity,
    access: Arc<super::backup_access::BackupAccess>,
}

static SESSIONS: OnceLock<Mutex<HashMap<PathBuf, BackupSession>>> = OnceLock::new();

pub(super) struct OwnedSession {
    pub directory: PathBuf,
    pub identity: crate::fs_utils::FileIdentity,
    pub lock_identity: crate::fs_utils::FileIdentity,
    pub access: Arc<super::backup_access::BackupAccess>,
}

impl OwnedSession {
    pub fn bucket(&self, path: &Path) -> UndoResult<PathBuf> {
        let relative = path
            .strip_prefix(&self.directory)
            .map_err(|_| UndoError::invalid_input("Backup is outside the owned session"))?;
        if relative
            .components()
            .any(|component| !matches!(component, std::path::Component::Normal(_)))
        {
            return Err(UndoError::invalid_input("Invalid backup path"));
        }
        Ok(relative
            .components()
            .next()
            .map(|component| PathBuf::from(component.as_os_str()))
            .unwrap_or_default())
    }
}

/// Ownership comes from the in-process registry and verified filesystem
/// identities, never a PID parsed from a directory name or a borrowed file lock.
pub(super) fn owned_session(path: &Path) -> UndoResult<Option<OwnedSession>> {
    let Some(sessions) = SESSIONS.get() else {
        return Ok(None);
    };
    let sessions = sessions
        .lock()
        .map_err(|_| UndoError::lock_failed("Undo session registry poisoned"))?;
    let Some(session) = sessions
        .values()
        .find(|session| path.starts_with(&session.directory))
    else {
        return Ok(None);
    };
    let name = session
        .directory
        .file_name()
        .ok_or_else(|| UndoError::invalid_input("Invalid session directory"))?;
    let mut lock_name = name.to_os_string();
    lock_name.push(".lock");
    let lock_path = session.directory.with_file_name(lock_name);
    let lock_identity = crate::fs_utils::FileIdentity::from_file(&session._lock)
        .ok_or_else(|| UndoError::invalid_input("Cannot verify owned session lock"))?;
    if !session.identity.matches(&session.directory) || !lock_identity.matches(&lock_path) {
        return Err(UndoError::snapshot_mismatch(&session.directory));
    }
    Ok(Some(OwnedSession {
        directory: session.directory.clone(),
        identity: session.identity.clone(),
        lock_identity,
        access: session.access.clone(),
    }))
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
            let identity = crate::fs_utils::FileIdentity::capture(&directory)
                .ok_or_else(|| UndoError::invalid_input("Cannot verify undo session identity"))?;
            return Ok(Self {
                directory,
                _lock: lock,
                identity,
                access: Arc::default(),
            });
        }
        Err(UndoError::invalid_input(
            "Unable to allocate a unique undo session",
        ))
    }
}

/// Remove only completely empty abandoned sessions. Stored backups have no
/// automatic expiry, whether or not a recovery marker is present. `max_age`
/// restricts empty-session cleanup; it never authorizes deleting backup data.
/// Never remove another running instance's session.
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
    let mut retained = 0;
    let mut changed = 0;
    let mut uncertain = 0;
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name) = name.to_str().filter(|name| name.starts_with("session-")) else {
            continue;
        };
        if !entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            continue;
        }
        if let Some(age) = max_age.filter(|age| !age.is_zero()) {
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
        // Markers describe interrupted operations and retain their diagnostics.
        // Unmarked backups must also survive restart for manual recovery.
        match super::recovery_notice::scan(&entry.path()) {
            super::recovery_notice::RecoveryScan::Clean => {}
            super::recovery_notice::RecoveryScan::Marked(fingerprint) => {
                retained += 1;
                changed += usize::from(super::recovery_notice::notify_changed(
                    &entry.path(),
                    &fingerprint,
                ));
                tracing::debug!(session = %entry.path().display(), "Retain protected undo recovery session");
                continue;
            }
            super::recovery_notice::RecoveryScan::Uncertain => {
                retained += 1;
                uncertain += 1;
                continue;
            }
        }
        // This must stay nonrecursive: even a zero-byte file, an empty backed-up
        // folder or an entry added after the scan prevents session deletion.
        if let Err(e) = fs::remove_dir(entry.path()) {
            if e.kind() == std::io::ErrorKind::DirectoryNotEmpty {
                continue;
            }
            warn!(
                "Failed to remove empty abandoned undo session {:?}: {}",
                entry.path(),
                e
            );
            continue;
        }
        drop(lock);
        let _ = fs::remove_file(lock_path);
        super::recovery_notice::remove(&entry.path());
    }
    if changed > 0 || uncertain > 0 {
        warn!(
            directory = %base.display(),
            retained_sessions = retained,
            new_or_changed_sessions = changed,
            uncertain_sessions = uncertain,
            "Undo recovery sessions retained; inspect backup details in Settings before removing backups"
        );
    } else if retained > 0 {
        tracing::debug!(
            retained_sessions = retained,
            "Unchanged undo recovery sessions retained"
        );
    }
}

#[cfg(test)]
fn session_requires_recovery(directory: &Path) -> bool {
    !matches!(
        super::recovery_notice::scan(directory),
        super::recovery_notice::RecoveryScan::Clean
    )
}

pub fn temp_backup_path(original: &Path) -> UndoResult<PathBuf> {
    let base = base_undo_dir();
    temp_backup_path_at(&base, original)
}

pub(super) fn temp_backup_path_at(base: &Path, original: &Path) -> UndoResult<PathBuf> {
    let _backup_operation = super::backup_operation()?;
    let mut sessions = SESSIONS
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .map_err(|_| UndoError::lock_failed("Undo session registry lock poisoned"))?;
    let session = match sessions.entry(base.to_path_buf()) {
        std::collections::hash_map::Entry::Occupied(entry) => entry.into_mut(),
        std::collections::hash_map::Entry::Vacant(entry) => {
            entry.insert(BackupSession::create(base)?)
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
    super::backup_origin::record(&candidate, original)?;
    Ok(candidate)
}

#[cfg(test)]
pub(super) fn forget_test_session(base: &Path) {
    if let Some(sessions) = SESSIONS.get() {
        sessions.lock().unwrap().remove(base);
    }
}

pub(super) fn base_undo_dir() -> PathBuf {
    if let Ok(custom) = std::env::var("BROWSEY_UNDO_DIR") {
        return PathBuf::from(custom);
    }
    default_undo_dir()
}

#[cfg(not(test))]
fn default_undo_dir() -> PathBuf {
    dirs_next::data_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("browsey")
        .join("undo-sessions")
}

#[cfg(test)]
fn default_undo_dir() -> PathBuf {
    // Filtered tests may never call the helpers setting BROWSEY_UNDO_DIR.
    // Their recovery backups must still never enter the user's app storage.
    static DIRECTORY: OnceLock<PathBuf> = OnceLock::new();
    DIRECTORY
        .get_or_init(|| {
            std::env::temp_dir().join(format!(
                "browsey-undo-default-test-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_nanos()
            ))
        })
        .clone()
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
            let subscriber = tracing_subscriber::fmt()
                .without_time()
                .with_ansi(false)
                .with_max_level(tracing::Level::WARN)
                .finish();
            tracing::subscriber::with_default(subscriber, || {
                cleanup_sessions(Path::new(&base), None);
            });
        }
    }

    fn cleanup_log_in_child(base: &Path) -> String {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "undo::backup::tests::cleanup_child",
                "--nocapture",
            ])
            .env("BROWSEY_TEST_CLEANUP_ROOT", base)
            .output()
            .unwrap();
        assert!(output.status.success());
        String::from_utf8(output.stdout).unwrap()
    }

    #[test]
    fn recovery_warnings_survive_restarts_and_report_changes_once_without_clearing_markers() {
        let base = unique_base();
        let recovery = BackupSession::create(&base).unwrap();
        let backup = recovery.directory.join("bucket/file.txt");
        fs::create_dir(backup.parent().unwrap()).unwrap();
        fs::write(&backup, b"recoverable").unwrap();
        let marker = RecoveryMarker::create(&backup, &base.join("destination.txt")).unwrap();
        let marker_bytes = fs::read(&marker.path).unwrap();
        let directory = recovery.directory.clone();
        drop(recovery);
        let message = "Undo recovery sessions retained;";
        let first = cleanup_log_in_child(&base);
        assert_eq!(first.matches(message).count(), 1);
        assert!(first.contains("new_or_changed_sessions=1"));
        assert!(!cleanup_log_in_child(&base).contains(message));
        assert_eq!(fs::read(&marker.path).unwrap(), marker_bytes);
        assert_eq!(fs::read(&backup).unwrap(), b"recoverable");

        let additional_backup = directory.join("additional/file.txt");
        fs::create_dir(additional_backup.parent().unwrap()).unwrap();
        fs::write(&additional_backup, b"additional original").unwrap();
        let additional_marker =
            RecoveryMarker::create(&additional_backup, &base.join("additional.txt")).unwrap();
        assert!(cleanup_log_in_child(&base).contains("new_or_changed_sessions=1"));
        assert!(!cleanup_log_in_child(&base).contains(message));

        // Same marker name/content, but a different filesystem identity.
        let parked = directory.join("parked-marker");
        fs::rename(&marker.path, &parked).unwrap();
        fs::write(&marker.path, &marker_bytes).unwrap();
        let replaced = cleanup_log_in_child(&base);
        assert_eq!(replaced.matches(message).count(), 1);
        assert!(!cleanup_log_in_child(&base).contains(message));

        let second = BackupSession::create(&base).unwrap();
        let second_backup = second.directory.join("bucket/file.txt");
        fs::create_dir(second_backup.parent().unwrap()).unwrap();
        fs::write(&second_backup, b"second original").unwrap();
        let second_marker =
            RecoveryMarker::create(&second_backup, &base.join("second.txt")).unwrap();
        let second_directory = second.directory.clone();
        drop(second);
        // Concurrent startup processes must not both report the same new session.
        let children: Vec<_> = (0..2)
            .map(|_| {
                std::process::Command::new(std::env::current_exe().unwrap())
                    .args([
                        "--exact",
                        "undo::backup::tests::cleanup_child",
                        "--nocapture",
                    ])
                    .env("BROWSEY_TEST_CLEANUP_ROOT", &base)
                    .stdout(std::process::Stdio::piped())
                    .spawn()
                    .unwrap()
            })
            .collect();
        let added = children
            .into_iter()
            .map(|child| {
                let output = child.wait_with_output().unwrap();
                assert!(output.status.success());
                String::from_utf8(output.stdout).unwrap()
            })
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(added.matches(message).count(), 1);
        assert!(added.contains("new_or_changed_sessions=1"));
        assert!(!cleanup_log_in_child(&base).contains(message));

        // Diagnostic cache corruption must re-warn and recover, never unpin data.
        let notice = base.join(format!(
            "{}.recovery-notice",
            directory.file_name().unwrap().to_str().unwrap()
        ));
        fs::write(&notice, b"").unwrap();
        assert!(cleanup_log_in_child(&base).contains(message));
        assert!(!cleanup_log_in_child(&base).contains(message));
        assert_eq!(fs::read(&marker.path).unwrap(), marker_bytes);
        assert_eq!(fs::read(&backup).unwrap(), b"recoverable");

        // Clearing diagnostics never authorizes deleting stored backups.
        fs::remove_file(&marker.path).unwrap();
        additional_marker.clear().unwrap();
        second_marker.clear().unwrap();
        cleanup_in_child(&base);
        assert_eq!(fs::read(&backup).unwrap(), b"recoverable");
        assert_eq!(fs::read(&second_backup).unwrap(), b"second original");
        assert!(directory.exists() && second_directory.exists());
        assert!(notice.exists());
        fs::remove_dir_all(base).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn suspicious_recovery_entries_warn_and_notice_links_never_modify_foreign_data() {
        use std::os::unix::fs::symlink;
        let base = unique_base();
        let session = BackupSession::create(&base).unwrap();
        let directory = session.directory.clone();
        let foreign = base.join("foreign.txt");
        fs::write(&foreign, b"foreign document").unwrap();
        let marker = directory.join("bucket.recovery-required");
        symlink(&foreign, &marker).unwrap();
        drop(session);
        let uncertain = cleanup_log_in_child(&base);
        assert!(uncertain.contains("uncertain_sessions=1"));
        assert!(cleanup_log_in_child(&base).contains("uncertain_sessions=1"));
        assert!(marker.is_symlink());

        fs::remove_file(&marker).unwrap();
        fs::write(&marker, b"recovery protection").unwrap();
        let notice = base.join(format!(
            "{}.recovery-notice",
            directory.file_name().unwrap().to_str().unwrap()
        ));
        symlink(&foreign, &notice).unwrap();
        assert!(cleanup_log_in_child(&base).contains("new_or_changed_sessions=1"));
        assert!(cleanup_log_in_child(&base).contains("new_or_changed_sessions=1"));
        assert_eq!(fs::read(&foreign).unwrap(), b"foreign document");
        assert_eq!(fs::read(&marker).unwrap(), b"recovery protection");
        assert!(notice.is_symlink());
        fs::remove_dir_all(base).unwrap();
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
    fn cleanup_keeps_unmarked_backups_after_the_owner_exits() {
        let base = unique_base();
        let session = BackupSession::create(&base).unwrap();
        let backup = session.directory.join("original.txt");
        fs::write(&backup, b"original").unwrap();
        cleanup_in_child(&base);
        assert_eq!(fs::read(&backup).unwrap(), b"original");
        drop(session);
        cleanup_in_child(&base);
        assert_eq!(fs::read(&backup).unwrap(), b"original");
        // Only an explicitly emptied session can be pruned.
        fs::remove_file(&backup).unwrap();
        cleanup_in_child(&base);
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
    fn clipboard_rollback_pins_blocked_overwrite_backups_across_cleanup() {
        let base = unique_base();
        let session = BackupSession::create(&base).unwrap();
        let backup = session.directory.join("bucket/original.txt");
        let destination = base.join("destination.txt");
        fs::create_dir(backup.parent().unwrap()).unwrap();
        fs::write(&backup, b"original document").unwrap();
        fs::write(&destination, b"uncertain output").unwrap();
        let mut actions = [super::super::Action::Batch(vec![
            super::super::Action::Delete {
                path: destination.clone(),
                backup: backup.clone(),
                protection: None,
            },
        ])];
        let error = super::super::engine::run_rollback_actions(&mut actions).unwrap_err();
        assert!(error.to_string().contains(&backup.display().to_string()));
        assert!(error
            .to_string()
            .contains("Overwrite recovery backups retained"));
        drop(session);
        cleanup_in_child(&base);
        assert_eq!(fs::read(&backup).unwrap(), b"original document");
        assert_eq!(fs::read(&destination).unwrap(), b"uncertain output");
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn completed_clipboard_rollback_clears_its_protection_marker() {
        let base = unique_base();
        let session = BackupSession::create(&base).unwrap();
        let directory = session.directory.clone();
        let backup = directory.join("bucket/original.txt");
        let destination = base.join("destination.txt");
        fs::create_dir(backup.parent().unwrap()).unwrap();
        fs::write(&backup, b"original document").unwrap();
        let mut actions = [super::super::Action::Delete {
            path: destination.clone(),
            backup,
            protection: None,
        }];
        super::super::engine::run_rollback_actions(&mut actions).unwrap();
        assert!(!session_requires_recovery(&directory));
        drop(session);
        cleanup_in_child(&base);
        // Moving the restored file leaves an empty bucket. Cleanup does not
        // recursively erase even apparently empty trees.
        assert!(directory.join("bucket").is_dir());
        assert!(!directory.join("bucket/original.txt").exists());
        assert_eq!(fs::read(&destination).unwrap(), b"original document");
        fs::remove_dir_all(base).unwrap();
    }

    #[test]
    fn clipboard_rollback_does_not_start_if_backup_protection_fails() {
        use crate::fs_utils::copy_test_hooks::{Phase, Scope};
        let base = unique_base();
        let session = BackupSession::create(&base).unwrap();
        let backup = session.directory.join("bucket/original.txt");
        let destination = base.join("destination.txt");
        let second_backup = session.directory.join("bucket-2/original.txt");
        let second_destination = base.join("second-destination.txt");
        fs::create_dir(backup.parent().unwrap()).unwrap();
        fs::write(&backup, b"original document").unwrap();
        fs::create_dir(second_backup.parent().unwrap()).unwrap();
        fs::write(&second_backup, b"second original document").unwrap();
        let mut actions = [
            super::super::Action::Delete {
                path: destination.clone(),
                backup: backup.clone(),
                protection: None,
            },
            super::super::Action::Delete {
                path: second_destination.clone(),
                backup: second_backup.clone(),
                protection: None,
            },
        ];
        let mut markers = 0;
        let scope = Scope::new(move |_, _, phase, _| {
            if phase == Phase::RecoveryMarker {
                markers += 1;
            }
            if phase == Phase::RecoveryMarker && markers == 2 {
                return Err(std::io::Error::other("injected marker failure"));
            }
            Ok(())
        });
        let result = super::super::engine::run_rollback_actions(&mut actions);
        drop(scope);
        let error = result.unwrap_err();
        assert!(error.to_string().contains("Rollback not attempted"));
        assert!(error.to_string().contains(&backup.display().to_string()));
        assert!(error
            .to_string()
            .contains(&second_backup.display().to_string()));
        assert_eq!(fs::read(&backup).unwrap(), b"original document");
        assert_eq!(
            fs::read(&second_backup).unwrap(),
            b"second original document"
        );
        assert!(!destination.exists());
        assert!(!second_destination.exists());
        drop(session);
        cleanup_in_child(&base);
        assert_eq!(fs::read(&backup).unwrap(), b"original document");
        assert_eq!(
            fs::read(&second_backup).unwrap(),
            b"second original document"
        );
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
        assert_eq!(fs::read(&backup).unwrap(), b"recoverable");
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
    fn cleanup_after_killed_process_keeps_unmarked_backups_and_the_other_live_session() {
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
            2
        );
        let abandoned = fs::read_dir(&base)
            .unwrap()
            .filter_map(Result::ok)
            .find(|entry| entry.file_type().unwrap().is_dir() && entry.path() != live.directory)
            .unwrap()
            .path();
        assert_eq!(
            fs::read(abandoned.join("document.txt")).unwrap(),
            b"owned-backup"
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
