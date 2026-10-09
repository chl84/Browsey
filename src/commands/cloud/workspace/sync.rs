//! Frozen snapshots and a durable upload journal separate local editor saves
//! from network writes. Registry locks never cover network or file hashing.
use super::*;
use crate::commands::cloud::providers::rclone::{CloudWriteVersion, RcloneCloudProvider};
use std::{
    collections::HashMap,
    sync::{atomic::AtomicBool, Arc, OnceLock, Weak},
};

type CopyLocks = Mutex<HashMap<String, Weak<Mutex<()>>>>;
static COPY_LOCKS: OnceLock<CopyLocks> = OnceLock::new();
pub(super) fn with_copy_lock<T>(
    id: &str,
    f: impl FnOnce() -> CloudCommandResult<T>,
) -> CloudCommandResult<T> {
    let mutex = copy_mutex(id)?;
    let _guard = mutex.lock().map_err(|_| registry_error())?;
    f()
}
pub(super) fn try_with_copy_lock<T>(
    id: &str,
    f: impl FnOnce() -> CloudCommandResult<T>,
) -> CloudCommandResult<T> {
    let mutex = copy_mutex(id)?;
    let _guard = mutex.try_lock().map_err(|_| {
        CloudCommandError::new(
            CloudCommandErrorCode::Conflict,
            "This working copy is busy saving; try cleanup after it finishes",
        )
    })?;
    f()
}
fn copy_mutex(id: &str) -> CloudCommandResult<Arc<Mutex<()>>> {
    Ok({
        let mut locks = COPY_LOCKS
            .get_or_init(Default::default)
            .lock()
            .map_err(|_| registry_error())?;
        locks.retain(|_, v| v.strong_count() > 0);
        if let Some(lock) = locks.get(id).and_then(Weak::upgrade) {
            lock
        } else {
            let mutex = Arc::new(Mutex::new(()));
            locks.insert(id.into(), Arc::downgrade(&mutex));
            mutex
        }
    })
}
fn registry_error() -> CloudCommandError {
    CloudCommandError::new(
        CloudCommandErrorCode::TaskFailed,
        "Cloud saving registry is unavailable; local edits are kept",
    )
}
pub(super) fn persist(base: &Path, copy: &CloudWorkingCopy) -> CloudCommandResult<()> {
    let _guard = lock()?;
    save_at(base, copy)
}

pub(crate) fn enable_for_open(
    base: &Path,
    copy: &mut CloudWorkingCopy,
    version: Option<CloudWriteVersion>,
    message: Option<String>,
) -> CloudCommandResult<()> {
    copy.auto_save = version.is_some();
    copy.save_status = if version.is_some() {
        CloudSaveStatus::Saved
    } else {
        CloudSaveStatus::Unsupported
    };
    copy.write_version = version;
    copy.save_message = message;
    persist(base, copy)
}
fn pending_path(
    base: &Path,
    copy: &CloudWorkingCopy,
    pending: &PendingSave,
) -> CloudCommandResult<PathBuf> {
    // Journal paths are one generated file name under this exact session.
    let name = &pending.relative_path;
    if !name.starts_with("save-")
        || !name.ends_with(".snapshot")
        || name.contains(['/', '\\'])
        || !name
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'.')
    {
        return Err(CloudCommandError::new(
            CloudCommandErrorCode::InvalidPath,
            "Invalid cloud save journal path",
        ));
    }
    let file = session_dir(base, &copy.id)?.join(name);
    if hash_file(&file)? != pending.hash {
        return Err(registry_error());
    }
    Ok(file)
}
fn freeze(
    base: &Path,
    copy: &CloudWorkingCopy,
    version: CloudWriteVersion,
) -> CloudCommandResult<PendingSave> {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let name = format!("save-{stamp:x}.snapshot");
    let destination = session_dir(base, &copy.id)?.join(&name);
    let before = hash_file(Path::new(&copy.local_path))?;
    let mut input = crate::fs_utils::open_regular_file_nofollow(Path::new(&copy.local_path))
        .map_err(io_error)?;
    let mut output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&destination)
        .map_err(io_error)?;
    private_permissions(&destination, false)?;
    io::copy(&mut input, &mut output).map_err(io_error)?;
    output.sync_all().map_err(io_error)?;
    let hash = hash_file(&destination)?;
    if before != hash || hash_file(Path::new(&copy.local_path))? != hash {
        // Unsent partial snapshots are our own temporary data. Retry after
        // another debounce; never upload an editor's half-written document.
        let _ = fs::remove_file(&destination);
        return Err(CloudCommandError::new(
            CloudCommandErrorCode::Cancelled,
            "The editor is still saving; waiting for a stable file",
        ));
    }
    Ok(PendingSave {
        relative_path: name,
        hash,
        version,
    })
}
fn verify_remote(
    base: &Path,
    copy: &CloudWorkingCopy,
    provider: &RcloneCloudProvider,
    version: &CloudWriteVersion,
    expected_hash: &str,
    cancel: &AtomicBool,
) -> CloudCommandResult<bool> {
    let source =
        CloudPath::parse(&version.source_path).map_err(super::super::map_cloud_path_error)?;
    let dir = operation_dir_at(&session_dir(base, &copy.id)?, "verify")?;
    let path = dir.join("remote");
    provider.download_cloud_write_version(version, &path, Some(cancel))?;
    private_permissions(&path, false)?;
    let actual = hash_file(&path)?;
    let after = provider.cloud_write_version(&source, Some(version), Some(cancel))?;
    let same = after == *version && actual == expected_hash;
    if same {
        let _ = fs::remove_file(path);
        let _ = fs::remove_dir(dir);
    }
    Ok(same)
}
fn commit(
    base: &Path,
    copy: &mut CloudWorkingCopy,
    pending: &PendingSave,
    version: CloudWriteVersion,
) -> CloudCommandResult<()> {
    let path = pending_path(base, copy, pending)?;
    copy.original_hash = pending.hash.clone();
    copy.original_size = Some(fs::metadata(&path).map_err(io_error)?.len());
    copy.original_modified = None;
    copy.write_version = Some(version);
    copy.uploaded_path = Some(copy.source_path.clone());
    copy.pending_save = None;
    copy.dirty = hash_file(Path::new(&copy.local_path))? != copy.original_hash;
    copy.save_status = if copy.dirty {
        CloudSaveStatus::Pending
    } else {
        CloudSaveStatus::Saved
    };
    copy.save_message = None;
    // Commit the baseline before removing the journal's byte snapshot.
    persist(base, copy)?;
    if let Ok(source) = CloudPath::parse(&copy.source_path) {
        super::super::invalidate_cloud_write_paths(&[source]);
    }
    let _ = fs::remove_file(path);
    Ok(())
}
pub(super) fn save_original_at(
    base: &Path,
    id: &str,
    provider: &RcloneCloudProvider,
    stop: Arc<AtomicBool>,
    on_status: StatusCallback,
) -> CloudCommandResult<CloudWorkingCopy> {
    with_copy_lock(id, || {
        let mut copy = load_at(base, id)?;
        let result = save_inner(base, &mut copy, provider, stop, on_status.clone());
        if let Err(ref error) = result {
            copy.save_status = match error.code() {
                CloudCommandErrorCode::Conflict | CloudCommandErrorCode::NotFound => {
                    CloudSaveStatus::Conflict
                }
                CloudCommandErrorCode::Unsupported => CloudSaveStatus::Unsupported,
                CloudCommandErrorCode::Cancelled => CloudSaveStatus::Pending,
                _ => CloudSaveStatus::Error,
            };
            copy.save_message = Some(error.message().into());
            persist(base, &copy)?;
            on_status(&copy, 0, 0, false);
        }
        result.map(|()| copy)
    })
}
fn save_inner(
    base: &Path,
    copy: &mut CloudWorkingCopy,
    provider: &RcloneCloudProvider,
    stop: Arc<AtomicBool>,
    on_status: StatusCallback,
) -> CloudCommandResult<()> {
    let source = CloudPath::parse(&copy.source_path).map_err(super::super::map_cloud_path_error)?;
    // Old/manual copies only gain a write validator after their original bytes
    // have been verified. We never rebase unsent edits onto somebody else's save.
    if copy.write_version.is_none() {
        let version = provider.cloud_write_version(&source, None, Some(&stop))?;
        if !verify_remote(base, copy, provider, &version, &copy.original_hash, &stop)? {
            return Err(CloudCommandError::new(
                CloudCommandErrorCode::Conflict,
                "The cloud original changed; your local edits are kept",
            ));
        }
        if version.source_path != copy.source_path {
            copy.source_path = version.source_path.clone();
        }
        copy.write_version = Some(version);
        persist(base, copy)?;
    }
    let mut save_completed = false;
    if let Some(pending) = copy.pending_save.clone() {
        pending_path(base, copy, &pending)?;
        let current = provider.cloud_write_version(&source, Some(&pending.version), Some(&stop))?;
        if current != pending.version {
            // A lost response or crash after commit is resolved by checking the
            // exact journal bytes, with version checks around the download.
            if !verify_remote(base, copy, provider, &current, &pending.hash, &stop)? {
                return Err(CloudCommandError::new(CloudCommandErrorCode::Conflict,"The cloud file changed while saving; your local edits and upload snapshot are kept"));
            }
            commit(base, copy, &pending, current)?;
            save_completed = true;
        }
    }
    if !copy.dirty && copy.pending_save.is_none() {
        copy.save_status = CloudSaveStatus::Saved;
        copy.save_message = None;
        persist(base, copy)?;
        on_status(copy, 0, 0, save_completed);
        return Ok(());
    }
    if copy
        .write_version
        .as_ref()
        .is_some_and(|v| v.provider == super::super::types::CloudProviderKind::Onedrive)
        && fs::metadata(&copy.local_path).map_err(io_error)?.len() > 250_000_000
    {
        return Err(CloudCommandError::new(CloudCommandErrorCode::Unsupported,"Automatic OneDrive saving supports files up to 250 MB. Your local edits are kept; use Save as new file."));
    }
    let pending = match &copy.pending_save {
        Some(pending) => pending.clone(),
        None => freeze(
            base,
            copy,
            copy.write_version.clone().ok_or_else(registry_error)?,
        )?,
    };
    let path = pending_path(base, copy, &pending)?;
    copy.pending_save = Some(pending.clone());
    copy.save_status = CloudSaveStatus::Uploading;
    copy.save_message = None;
    persist(base, copy)?;
    on_status(copy, 0, fs::metadata(&path).map_err(io_error)?.len(), false);
    let event_copy = copy.clone();
    let callback = on_status.clone();
    let mut last = std::time::Instant::now();
    // The HTTP reader only emits progress, never acquires workspace locks.
    let tick = Arc::new(Mutex::new(move |bytes, total| {
        if last.elapsed() >= std::time::Duration::from_millis(200) || bytes == total {
            last = std::time::Instant::now();
            callback(&event_copy, bytes, total, false);
        }
    }));
    let mut version = provider.replace_cloud_file(
        &path,
        &pending.version,
        stop.clone(),
        Arc::new(move |bytes, total| {
            if let Ok(mut tick) = tick.lock() {
                tick(bytes, total);
            }
        }),
    )?;
    if version.etag.is_empty() {
        version = provider.cloud_write_version(&source, Some(&pending.version), Some(&stop))?;
        if !verify_remote(base, copy, provider, &version, &pending.hash, &stop)? {
            return Err(CloudCommandError::new(
                CloudCommandErrorCode::Conflict,
                "The saved cloud version could not be confirmed; your local edits are kept",
            ));
        }
    }
    commit(base, copy, &pending, version)?;
    on_status(copy, 0, 0, true);
    Ok(())
}

#[tauri::command]
pub async fn save_cloud_working_copy(
    id: String,
    app: tauri::AppHandle,
    cancel: tauri::State<'_, crate::tasks::CancelState>,
    progress_event: Option<String>,
) -> crate::errors::api_error::ApiResult<CloudWorkingCopy> {
    let cancel_state = cancel.inner().clone();
    map_api_result(
        tauri::async_runtime::spawn_blocking(move || {
            super::super::ensure_cloud_enabled()?;
            let guard = super::super::register_cloud_cancel(&cancel_state, &progress_event)?;
            let stop = guard
                .as_ref()
                .map(|g| g.token())
                .unwrap_or_else(|| Arc::new(AtomicBool::new(false)));
            let base = root()?;
            let copy = load_at(&base, &id)?;
            let source =
                CloudPath::parse(&copy.source_path).map_err(super::super::map_cloud_path_error)?;
            let saved = super::super::limits::with_cloud_remote_permits(
                vec![source.remote().into()],
                || {
                    let provider = super::super::configured_rclone_provider()
                        .map_err(CloudCommandError::from)?;
                    let publish = super::monitor::callback(&app);
                    let progress_app = app.clone();
                    let event = progress_event.clone();
                    let status_callback: StatusCallback = Arc::new(move |copy, bytes, total, saved| {
                        publish(copy, bytes, total, saved);
                        if let Some(event) = event.as_deref() {
                            let phase = if saved { "Cloud file saved" }
                                else if total > 0 && bytes >= total { "Confirming cloud save…" }
                                else if matches!(copy.save_status, CloudSaveStatus::Uploading) { "Uploading edited file…" }
                                else { "Checking edited cloud file…" };
                            crate::runtime_lifecycle::emit_if_running(&progress_app, event,
                                serde_json::json!({"bytes": bytes, "total": total, "finished": false, "phase": phase}));
                        }
                    });
                    save_original_at(&base, &id, &provider, stop, status_callback)
                },
            )?;
            super::monitor::register(&app, &saved)?;
            Ok(saved)
        })
        .await
        .map_err(|_| registry_error())
        .and_then(|r| r),
    )
}

#[tauri::command]
pub async fn set_cloud_working_copy_auto_save(
    id: String,
    enabled: bool,
    app: tauri::AppHandle,
) -> crate::errors::api_error::ApiResult<CloudWorkingCopy> {
    map_api_result(tauri::async_runtime::spawn_blocking(move || {
        let copy = with_copy_lock(&id,|| {
        let base = root()?;
        let mut copy = load_at(&base,&id)?;
        if enabled && copy.write_version.is_none() { return Err(CloudCommandError::new(CloudCommandErrorCode::Unsupported,"Use Save to original once to verify this working copy before enabling automatic saving")); }
        copy.auto_save = enabled;
        copy.save_status = if enabled { CloudSaveStatus::Pending } else { CloudSaveStatus::Paused };
        persist(&base,&copy)?;
        Ok(copy)
        })?;
        super::monitor::register(&app,&copy)?;
        Ok(copy)
    }).await.map_err(|_|registry_error()).and_then(|r|r))
}

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::commands::cloud::{providers::rclone::CloudWriteFixture, types::CloudProviderKind};
    fn copy(f: &CloudWriteFixture) -> CloudWorkingCopy {
        let base = f.base.join("workspaces");
        let mut copy =
            create_at(&base, &f.source, &f.base.join("fileA.bytes"), Some(8), None).unwrap();
        enable_for_open(&base, &mut copy, Some(f.version()), None).unwrap();
        fs::write(&copy.local_path, b"local edited").unwrap();
        copy
    }
    fn save(
        f: &CloudWriteFixture,
        copy: &CloudWorkingCopy,
    ) -> CloudCommandResult<CloudWorkingCopy> {
        save_with_status(f, copy, Arc::new(|_, _, _, _| {}))
    }
    fn save_with_status(
        f: &CloudWriteFixture,
        copy: &CloudWorkingCopy,
        on_status: StatusCallback,
    ) -> CloudCommandResult<CloudWorkingCopy> {
        save_original_at(
            &f.base.join("workspaces"),
            &copy.id,
            &f.provider,
            Arc::new(AtomicBool::new(false)),
            on_status,
        )
    }
    type CapturedStatuses = Arc<Mutex<Vec<(CloudSaveStatus, bool)>>>;
    fn capture_status() -> (CapturedStatuses, StatusCallback) {
        let events = Arc::new(Mutex::new(Vec::new()));
        let captured = events.clone();
        let callback: StatusCallback = Arc::new(move |copy, _, _, completed| {
            captured.lock().unwrap().push((copy.save_status, completed));
        });
        (events, callback)
    }
    #[test]
    fn unchanged_checks_are_silent_but_confirmed_writes_notify_including_empty_files() {
        for bytes in [b"local edited".as_slice(), b"".as_slice()] {
            let f = CloudWriteFixture::new(CloudProviderKind::Gdrive);
            let original = copy(&f);
            fs::write(&original.local_path, b"original").unwrap();
            let (events, callback) = capture_status();
            let clean = save_with_status(&f, &original, callback.clone()).unwrap();
            assert_eq!(
                events.lock().unwrap().as_slice(),
                &[(CloudSaveStatus::Saved, false)]
            );
            assert!(f.state.lock().unwrap().puts.is_empty());

            events.lock().unwrap().clear();
            fs::write(&original.local_path, bytes).unwrap();
            let saved = save_with_status(&f, &clean, callback.clone()).unwrap();
            let writes = events.lock().unwrap().clone();
            assert!(writes.contains(&(CloudSaveStatus::Uploading, false)));
            assert_eq!(writes.last(), Some(&(CloudSaveStatus::Saved, true)));
            assert_eq!(writes.iter().filter(|(_, completed)| *completed).count(), 1);
            assert_eq!(f.state.lock().unwrap().puts.len(), 1);

            events.lock().unwrap().clear();
            save_with_status(&f, &saved, callback).unwrap();
            assert_eq!(
                events.lock().unwrap().as_slice(),
                &[(CloudSaveStatus::Saved, false)]
            );
            assert_eq!(f.state.lock().unwrap().puts.len(), 1);
        }
    }
    #[test]
    fn a_lost_response_is_recovered_after_reloading_without_a_second_write() {
        for kind in [
            CloudProviderKind::Gdrive,
            CloudProviderKind::Onedrive,
            CloudProviderKind::Nextcloud,
        ] {
            let f = CloudWriteFixture::new(kind);
            let original = copy(&f);
            f.state.lock().unwrap().lose_response = true;
            assert!(save(&f, &original).is_err());
            let retained = load_at(&f.base.join("workspaces"), &original.id).unwrap();
            assert!(retained.pending_save.is_some());
            assert!(retained.dirty);
            let (events, callback) = capture_status();
            let saved = save_with_status(&f, &retained, callback).unwrap();
            assert_eq!(
                events.lock().unwrap().as_slice(),
                &[(CloudSaveStatus::Saved, true)]
            );
            assert_eq!(saved.save_status, CloudSaveStatus::Saved);
            assert!(!saved.dirty);
            assert!(saved.pending_save.is_none());
            assert_eq!(f.state.lock().unwrap().puts.len(), 1);
            assert_eq!(fs::read(&saved.local_path).unwrap(), b"local edited");
        }
    }
    #[test]
    fn conflicts_preserve_local_edits_and_the_frozen_upload_journal() {
        let f = CloudWriteFixture::new(CloudProviderKind::Gdrive);
        let original = copy(&f);
        f.state.lock().unwrap().race = true;
        let (events, callback) = capture_status();
        assert_eq!(
            save_with_status(&f, &original, callback)
                .unwrap_err()
                .code(),
            CloudCommandErrorCode::Conflict
        );
        assert!(events
            .lock()
            .unwrap()
            .iter()
            .all(|(_, completed)| !completed));
        let retained = load_at(&f.base.join("workspaces"), &original.id).unwrap();
        assert_eq!(retained.save_status, CloudSaveStatus::Conflict);
        assert_eq!(fs::read(&retained.local_path).unwrap(), b"local edited");
        let pending = retained.pending_save.as_ref().unwrap();
        assert_eq!(
            fs::read(pending_path(&f.base.join("workspaces"), &retained, pending).unwrap())
                .unwrap(),
            b"local edited"
        );
        assert_eq!(
            save(&f, &retained).unwrap_err().code(),
            CloudCommandErrorCode::Conflict
        );
        assert_eq!(f.state.lock().unwrap().puts.len(), 1);
    }
    #[test]
    fn edits_during_upload_are_not_mistaken_for_the_uploaded_snapshot() {
        let f = CloudWriteFixture::new(CloudProviderKind::Gdrive);
        let original = copy(&f);
        let local = original.local_path.clone();
        let saved = save_original_at(
            &f.base.join("workspaces"),
            &original.id,
            &f.provider,
            Arc::new(AtomicBool::new(false)),
            Arc::new(move |copy, bytes, _, _| {
                if copy.save_status == CloudSaveStatus::Uploading && bytes > 0 {
                    fs::write(&local, b"newer editor save").unwrap();
                }
            }),
        )
        .unwrap();
        assert_eq!(saved.save_status, CloudSaveStatus::Pending);
        assert!(saved.dirty);
        assert_eq!(
            fs::read(f.base.join("fileA.bytes")).unwrap(),
            b"local edited"
        );
        let next = save(&f, &saved).unwrap();
        assert_eq!(next.save_status, CloudSaveStatus::Saved);
        assert_eq!(
            fs::read(f.base.join("fileA.bytes")).unwrap(),
            b"newer editor save"
        );
    }
    #[test]
    fn a_journal_cannot_redirect_the_snapshot_outside_its_own_session() {
        let f = CloudWriteFixture::new(CloudProviderKind::Gdrive);
        let mut original = copy(&f);
        original.pending_save = Some(PendingSave {
            relative_path: "../../fileA.bytes".into(),
            hash: original.original_hash.clone(),
            version: f.version(),
        });
        persist(&f.base.join("workspaces"), &original).unwrap();
        assert_eq!(
            save(&f, &original).unwrap_err().code(),
            CloudCommandErrorCode::InvalidPath
        );
        assert!(f.state.lock().unwrap().puts.is_empty());
    }
    #[test]
    fn legacy_copies_verify_original_bytes_before_adopting_a_write_version() {
        let f = CloudWriteFixture::new(CloudProviderKind::Gdrive);
        let original = copy(&f);
        let base = f.base.join("workspaces");
        let mut legacy = original.clone();
        legacy.write_version = None;
        legacy.auto_save = false;
        persist(&base, &legacy).unwrap();
        fs::write(f.base.join("fileA.bytes"), b"changed original").unwrap();
        assert_eq!(
            save(&f, &legacy).unwrap_err().code(),
            CloudCommandErrorCode::Conflict
        );
        assert!(f.state.lock().unwrap().puts.is_empty());
        assert_eq!(fs::read(&legacy.local_path).unwrap(), b"local edited");
    }
}
