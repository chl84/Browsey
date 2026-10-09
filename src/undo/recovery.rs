//! Copy retained backups to a user-selected local folder. Diagnostic marker
//! text is never interpreted as a restore plan, and markers are never cleared.
use std::{
    fs::{self, File, OpenOptions},
    io,
    path::{Component, Path, PathBuf},
    sync::atomic::{AtomicBool, Ordering},
    time::{Duration, Instant, UNIX_EPOCH},
};

use super::{
    backup::{base_undo_dir, validate_undo_dir},
    error::map_api_result,
    UndoError, UndoResult,
};
use crate::{
    errors::api_error::{ApiError, ApiResult},
    fs_utils::{FileIdentity, TreeSnapshot},
    tasks::CancelState,
};
use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryBackup {
    id: String,
    version: String,
    name: String,
    kind: &'static str,
    bytes: Option<u64>,
    modified_at: Option<u64>,
    blocked_reason: Option<String>,
}

#[derive(Debug, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryBackups {
    entries: Vec<RecoveryBackup>,
    incomplete: bool,
}

struct ScanBudget {
    remaining: usize,
    deadline: Instant,
}
impl ScanBudget {
    fn take(&mut self) -> bool {
        if self.remaining == 0 || Instant::now() >= self.deadline {
            return false;
        }
        self.remaining -= 1;
        true
    }
}

// Keep the sibling lock open and locked through the whole copy. Startup cleanup
// in any Browsey process must acquire this same lock before removing a session.
struct SessionLock {
    _file: Option<File>,
    current: Option<super::backup::OwnedSession>,
    directory: PathBuf,
    identity: FileIdentity,
    lock_path: PathBuf,
    lock_identity: FileIdentity,
}
impl SessionLock {
    fn verify(&self) -> UndoResult<()> {
        if self.identity.matches(&self.directory) && self.lock_identity.matches(&self.lock_path) {
            Ok(())
        } else {
            Err(UndoError::snapshot_mismatch(&self.directory))
        }
    }

    fn read_backup(
        &self,
        source: &Path,
    ) -> UndoResult<Option<super::backup_access::BackupReadGuard>> {
        let Some(current) = &self.current else {
            return Ok(None);
        };
        current
            .access
            .try_read(current.bucket(source)?)?
            .map(Some)
            .ok_or_else(|| {
                UndoError::lock_failed(
                    "A file operation is using this backup. Wait for it to finish, then refresh.",
                )
            })
    }
}

fn lock_session(base: &Path, name: &str) -> UndoResult<Option<SessionLock>> {
    let directory = base.join(name);
    super::path_checks::ensure_existing_dir_nonsymlink(&directory)?;
    let identity = FileIdentity::capture(&directory)
        .ok_or_else(|| UndoError::invalid_input("Cannot verify backup session"))?;
    let path = base.join(format!("{name}.lock"));
    crate::fs_utils::check_no_symlink_components(&path)?;
    let mut options = OpenOptions::new();
    options.read(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.custom_flags(windows_sys::Win32::Storage::FileSystem::FILE_FLAG_OPEN_REPARSE_POINT);
    }
    let file = options
        .open(&path)
        .map_err(|error| UndoError::from_io_error("Open backup session lock", error))?;
    let lock_identity = FileIdentity::from_file(&file)
        .ok_or_else(|| UndoError::invalid_input("Cannot verify backup session lock"))?;
    if !file.metadata().is_ok_and(|meta| meta.is_file()) || !lock_identity.matches(&path) {
        return Err(UndoError::invalid_input("Unsafe backup session lock"));
    }
    match file.try_lock() {
        Ok(()) => {
            let lock = SessionLock {
                _file: Some(file),
                current: None,
                directory,
                identity,
                lock_path: path,
                lock_identity,
            };
            lock.verify()?;
            Ok(Some(lock))
        }
        Err(std::fs::TryLockError::WouldBlock) => {
            let Some(current) = super::backup::owned_session(&directory)? else {
                return Ok(None);
            };
            // The original session descriptor stays locked in the registry.
            // Do not unlock it or replace it with this newly opened descriptor.
            if current.identity != identity || current.lock_identity != lock_identity {
                return Err(UndoError::snapshot_mismatch(&directory));
            }
            let lock = SessionLock {
                _file: None,
                current: Some(current),
                directory,
                identity,
                lock_path: path,
                lock_identity,
            };
            lock.verify()?;
            Ok(Some(lock))
        }
        Err(error) => Err(UndoError::lock_failed(format!(
            "Lock backup session: {error}"
        ))),
    }
}

fn version(source: &Path) -> UndoResult<String> {
    let mut hash = blake3::Hasher::new();
    // Bind a listing row to the root entry and its bucket/session, rather than
    // silently accepting an old row after the same path was replaced.
    for (index, path) in source.ancestors().take(3).enumerate() {
        crate::fs_utils::check_no_symlink_components(path)?;
        let identity = FileIdentity::capture(path)
            .ok_or_else(|| UndoError::invalid_input("Cannot verify backup identity"))?;
        let meta = fs::symlink_metadata(path)
            .map_err(|error| UndoError::from_io_error("Inspect backup version", error))?;
        hash.update(format!("{identity:?}").as_bytes());
        // Another backup/marker in this live session must not invalidate a
        // completed file's listing row. Bind ancestors by identity only.
        if index != 0 {
            continue;
        }
        hash.update(&meta.len().to_le_bytes());
        if let Ok(modified) = meta
            .modified()
            .and_then(|time| time.duration_since(UNIX_EPOCH).map_err(io::Error::other))
        {
            hash.update(&modified.as_nanos().to_le_bytes());
        }
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            hash.update(&meta.ctime().to_le_bytes());
            hash.update(&meta.ctime_nsec().to_le_bytes());
        }
    }
    Ok(hash.finalize().to_hex().to_string())
}

struct Measurement {
    bytes: u64,
    fingerprint: [u8; 32],
}

fn measure(source: &Path, budget: &mut ScanBudget) -> UndoResult<Option<Measurement>> {
    let mut bytes = 0u64;
    let mut snapshot = TreeSnapshot::default();
    for entry in walkdir::WalkDir::new(source).follow_links(false) {
        if !budget.take() {
            return Ok(None);
        }
        let entry =
            entry.map_err(|error| UndoError::invalid_input(format!("Inspect backup: {error}")))?;
        crate::fs_utils::check_no_symlink_components(entry.path())?;
        let meta = fs::symlink_metadata(entry.path())
            .map_err(|error| UndoError::from_io_error("Inspect backup size", error))?;
        if meta.is_file() {
            bytes = bytes
                .checked_add(meta.len())
                .ok_or_else(|| UndoError::invalid_input("Backup size overflow"))?;
        } else if !meta.is_dir() {
            return Err(UndoError::symlink_unsupported(entry.path()));
        }
        snapshot
            .record_existing(
                entry.path().strip_prefix(source).unwrap().into(),
                entry.path(),
                &meta,
            )
            .map_err(|error| UndoError::from_io_error("Inspect backup version", error))?;
    }
    Ok(Some(Measurement {
        bytes,
        fingerprint: super::recovery_state::fingerprint(&snapshot),
    }))
}

fn list_at(base: &Path, mut budget: ScanBudget) -> UndoResult<RecoveryBackups> {
    crate::path_guard::ensure_no_symlink_components_existing_prefix(base)
        .map_err(|error| UndoError::invalid_input(format!("Unsafe backup storage: {error}")))?;
    let mut result = RecoveryBackups::default();
    let sessions = match fs::read_dir(base) {
        Ok(entries) => entries,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(result),
        Err(error) => return Err(UndoError::from_io_error("List recovery backups", error)),
    };
    for session in sessions {
        if !budget.take() {
            result.incomplete = true;
            break;
        }
        let Ok(session) = session else {
            result.incomplete = true;
            continue;
        };
        let Some(name) = session
            .file_name()
            .to_str()
            .filter(|name| name.starts_with("session-"))
            .map(str::to_owned)
        else {
            continue;
        };
        match session.file_type() {
            Ok(kind) if kind.is_dir() => {}
            Ok(kind) if kind.is_file() => continue,
            _ => {
                result.incomplete = true;
                continue;
            }
        }
        let lock = match lock_session(base, &name) {
            Ok(lock) => lock,
            Err(_) => {
                result.incomplete = true;
                continue;
            }
        };
        let Ok(buckets) = fs::read_dir(session.path()) else {
            result.incomplete = true;
            continue;
        };
        for bucket in buckets {
            if !budget.take() {
                result.incomplete = true;
                break;
            }
            let Ok(bucket) = bucket else {
                result.incomplete = true;
                continue;
            };
            match bucket.file_type() {
                Ok(kind) if kind.is_dir() => {}
                Ok(kind) if kind.is_file() => continue,
                _ => {
                    result.incomplete = true;
                    continue;
                }
            }
            if crate::path_guard::ensure_existing_dir_nonsymlink(&bucket.path()).is_err() {
                result.incomplete = true;
                continue;
            }
            let Ok(entries) = fs::read_dir(bucket.path()) else {
                result.incomplete = true;
                continue;
            };
            for entry in entries {
                if !budget.take() {
                    result.incomplete = true;
                    break;
                }
                let Ok(entry) = entry else {
                    result.incomplete = true;
                    continue;
                };
                let source = entry.path();
                let Some(id) = source
                    .strip_prefix(base)
                    .ok()
                    .and_then(Path::to_str)
                    .map(str::to_owned)
                else {
                    result.incomplete = true;
                    continue;
                };
                let Some(file_name) = entry.file_name().to_str().map(str::to_owned) else {
                    result.incomplete = true;
                    continue;
                };
                let read = lock.as_ref().map(|lock| lock.read_backup(&source));
                let busy = read.as_ref().is_some_and(|read| read.is_err());
                let Ok(meta) = fs::symlink_metadata(&source) else {
                    result.incomplete = true;
                    continue;
                };
                if !meta.is_file() && !meta.is_dir() {
                    result.incomplete = true;
                    continue;
                }
                let Ok(version) = version(&source) else {
                    result.incomplete = true;
                    continue;
                };
                let size = if busy {
                    Ok(None)
                } else {
                    measure(&source, &mut budget)
                };
                if !busy && !matches!(size, Ok(Some(_))) {
                    result.incomplete = true;
                }
                // A successful recovery is remembered independently of history.
                // Fully measured mutations become pending again. With a partial
                // scan, keep the last known status for this same root version.
                if !busy && size.is_ok() {
                    if let Some(recovered) = super::recovery_state::read(&source, &version) {
                        if size
                            .as_ref()
                            .ok()
                            .and_then(Option::as_ref)
                            .is_none_or(|measurement| measurement.fingerprint == recovered)
                        {
                            continue;
                        }
                    }
                }
                let blocked_reason = if lock.is_none() {
                    Some("In use by another Browsey instance.".into())
                } else if busy {
                    Some("File operation in progress. Refresh when it finishes.".into())
                } else if size.is_err() {
                    Some("This backup could not be fully inspected. Refresh to try again.".into())
                } else {
                    None
                };
                result.entries.push(RecoveryBackup {
                    id,
                    version,
                    name: file_name,
                    kind: if meta.is_dir() { "dir" } else { "file" },
                    bytes: size.ok().flatten().map(|measurement| measurement.bytes),
                    modified_at: meta
                        .modified()
                        .ok()
                        .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                        .map(|duration| duration.as_secs()),
                    blocked_reason,
                });
            }
        }
        if lock.as_ref().is_some_and(|lock| lock.verify().is_err()) {
            result.incomplete = true;
            result
                .entries
                .retain(|entry| !Path::new(&entry.id).starts_with(&name));
        }
    }
    result
        .entries
        .sort_by(|a, b| b.modified_at.cmp(&a.modified_at).then(a.id.cmp(&b.id)));
    Ok(result)
}

#[tauri::command]
pub async fn list_recovery_backups() -> ApiResult<RecoveryBackups> {
    let result = tauri::async_runtime::spawn_blocking(|| {
        let base = base_undo_dir();
        validate_undo_dir(&base)?;
        list_at(
            &base,
            ScanBudget {
                remaining: 10_000,
                deadline: Instant::now() + Duration::from_millis(250),
            },
        )
    })
    .await
    .map_err(|error| {
        ApiError::new(
            "task_failed",
            format!("Backup listing worker failed: {error}"),
        )
    })?;
    map_api_result(result)
}

fn source_from_id(base: &Path, id: &str) -> UndoResult<PathBuf> {
    let parts = Path::new(id).components().collect::<Vec<_>>();
    if parts.len() != 3
        || parts
            .iter()
            .any(|part| !matches!(part, Component::Normal(_)))
        || !parts[0]
            .as_os_str()
            .to_str()
            .is_some_and(|name| name.starts_with("session-"))
    {
        return Err(UndoError::invalid_input(
            "Invalid recovery backup reference",
        ));
    }
    Ok(base.join(id))
}

fn destination_path(directory: &Path, source: &Path) -> UndoResult<PathBuf> {
    let name = source
        .file_name()
        .ok_or_else(|| UndoError::invalid_input("Missing backup name"))?;
    let folder = fs::symlink_metadata(source).is_ok_and(|meta| meta.is_dir());
    for index in 0..10_000 {
        let target = if index == 0 {
            directory.join(name)
        } else {
            let mut name = if folder {
                name.to_os_string()
            } else {
                source.file_stem().unwrap_or(name).to_os_string()
            };
            name.push(format!("-{index}"));
            if !folder {
                if let Some(extension) = source.extension() {
                    name.push(".");
                    name.push(extension);
                }
            }
            directory.join(name)
        };
        match fs::symlink_metadata(&target) {
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(target),
            Err(error) => {
                return Err(UndoError::from_io_error(
                    "Inspect recovery destination",
                    error,
                ))
            }
            Ok(_) => {}
        }
    }
    Err(UndoError::target_exists("No free recovery filename found"))
}

fn validate_destination(base: &Path, directory: &Path) -> UndoResult<()> {
    if !directory.is_absolute()
        || directory
            .components()
            .any(|part| matches!(part, Component::ParentDir))
    {
        return Err(UndoError::invalid_input(
            "Choose an absolute local destination folder",
        ));
    }
    super::path_checks::ensure_existing_dir_nonsymlink(directory)?;
    if directory.starts_with(base) {
        return Err(UndoError::invalid_input(
            "Choose a folder outside backup storage",
        ));
    }
    Ok(())
}

fn original_destination(base: &Path, source: &Path) -> UndoResult<PathBuf> {
    let target = super::backup_origin::read(source)?;
    let directory = target
        .parent()
        .ok_or_else(|| UndoError::invalid_input("Original folder is unknown"))?;
    validate_destination(base, directory)?;
    match fs::symlink_metadata(&target) {
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(target),
        Err(error) => Err(UndoError::from_io_error("Inspect original location", error)),
        Ok(_) => Err(UndoError::target_exists(
            "The original location already contains a file or folder",
        )),
    }
}

fn destination_unavailable(error: impl std::fmt::Display) -> ApiError {
    ApiError::new(
        "recovery_destination_unavailable",
        format!("Could not recover to the original location. Choose another folder. {error}"),
    )
}

fn recover_at(
    base: &Path,
    id: &str,
    expected_version: &str,
    destination_dir: Option<&Path>,
    app: Option<&tauri::AppHandle>,
    event: Option<&str>,
    cancel: Option<&AtomicBool>,
) -> ApiResult<String> {
    let prepare = || -> UndoResult<_> {
        let source = source_from_id(base, id)?;
        let session = Path::new(id)
            .components()
            .next()
            .unwrap()
            .as_os_str()
            .to_str()
            .unwrap();
        let lock = lock_session(base, session)?.ok_or_else(|| {
            UndoError::lock_failed(
                "Backup is in use by another Browsey instance. Recover it in that instance instead.",
            )
        })?;
        let read = lock.read_backup(&source)?;
        if version(&source)? != expected_version {
            return Err(UndoError::snapshot_mismatch(&source));
        }
        Ok((source, lock, read))
    };
    let (source, lock, _read) = map_api_result(prepare())?;
    if cancel.is_some_and(|token| token.load(Ordering::Relaxed)) {
        return Err(ApiError::new("cancelled", "Recovery cancelled"));
    }
    let target = if let Some(directory) = destination_dir {
        map_api_result(validate_destination(base, directory))?;
        map_api_result(destination_path(directory, &source))?
    } else {
        let target = original_destination(base, &source);
        if cancel.is_some_and(|token| token.load(Ordering::Relaxed)) {
            return Err(ApiError::new("cancelled", "Recovery cancelled"));
        }
        target.map_err(destination_unavailable)?
    };
    let snapshot = crate::clipboard::copy_recovery_backup(&source, &target, app, event, cancel)
        .map_err(|error| {
            // Never redirect an interrupted copy automatically: the user sees the
            // diagnostic/partial output path and explicitly selects the next folder.
            if destination_dir.is_none() && error.code != "cancelled" {
                destination_unavailable(error.message)
            } else {
                error
            }
        })?;
    map_api_result(lock.verify())?;
    if cancel.is_some_and(|token| token.load(Ordering::Relaxed)) {
        return Err(ApiError::new("cancelled", "Recovery cancelled"));
    }
    super::recovery_state::record(&source, expected_version, &super::recovery_state::fingerprint(&snapshot))
        .map_err(|error| ApiError::new("recovery_record_failed", format!(
            "Recovered to {} but could not update backup status. The backup remains listed: {error}", target.display()
        )))?;
    map_api_result(lock.verify())?;
    Ok(target.to_string_lossy().into_owned())
}

#[cfg(test)]
fn restore_at(
    base: &Path,
    id: &str,
    expected_version: &str,
    destination_dir: &Path,
    app: Option<&tauri::AppHandle>,
    event: Option<&str>,
    cancel: Option<&AtomicBool>,
) -> ApiResult<String> {
    recover_at(
        base,
        id,
        expected_version,
        Some(destination_dir),
        app,
        event,
        cancel,
    )
}

#[tauri::command]
pub async fn restore_recovery_backup(
    id: String,
    version: String,
    destination_dir: Option<String>,
    app: tauri::AppHandle,
    cancel: tauri::State<'_, CancelState>,
    progress_event: String,
) -> ApiResult<String> {
    let guard = cancel
        .register(progress_event.clone())
        .map_err(|error| ApiError::new("task_failed", error.to_string()))?;
    tauri::async_runtime::spawn_blocking(move || {
        let base = base_undo_dir();
        map_api_result(validate_undo_dir(&base))?;
        let token = guard.token();
        let result = recover_at(
            &base,
            &id,
            &version,
            destination_dir.as_deref().map(Path::new),
            Some(&app),
            Some(&progress_event),
            Some(&token),
        );
        drop(guard);
        result
    })
    .await
    .map_err(|error| ApiError::new("task_failed", format!("Recovery worker failed: {error}")))?
}

#[cfg(test)]
mod tests;
