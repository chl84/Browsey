//! Durable working copies are user data, not an evictable preview cache.
//! Edited cloud copies use version-checked writeback; retained user data is never evicted.
use super::{
    error::{map_api_result, CloudCommandError, CloudCommandErrorCode, CloudCommandResult},
    path::CloudPath,
};
use crate::errors::api_error::ApiResult;
use serde::{Deserialize, Serialize};
use std::{
    fs, io,
    path::{Path, PathBuf},
    sync::{Mutex, MutexGuard},
    time::{SystemTime, UNIX_EPOCH},
};

static WORKSPACE_LOCK: Mutex<()> = Mutex::new(());
type StatusCallback = std::sync::Arc<dyn Fn(&CloudWorkingCopy, u64, u64) + Send + Sync>;
mod monitor;
mod sync;
pub use monitor::cloud_writeback_statuses;
pub(crate) use monitor::{start_cloud_writeback, stop_cloud_writeback};
pub(super) use sync::enable_for_open;
pub use sync::{save_cloud_working_copy, set_cloud_working_copy_auto_save};
pub(super) fn register_open_copy(
    app: &tauri::AppHandle,
    copy: &mut CloudWorkingCopy,
) -> CloudCommandResult<()> {
    if let Err(error) = monitor::register(app, copy) {
        copy.auto_save = false;
        copy.save_status = CloudSaveStatus::Error;
        copy.save_message = Some(error.message().into());
        sync::persist(&root()?, copy)?;
        monitor::callback(app)(copy, 0, 0);
    }
    Ok(())
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CloudSaveStatus {
    #[default]
    Manual,
    Saved,
    Pending,
    Uploading,
    Conflict,
    Error,
    Paused,
    Unsupported,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PendingSave {
    relative_path: String,
    hash: String,
    version: super::providers::rclone::CloudWriteVersion,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudWorkingCopy {
    pub id: String,
    pub source_path: String,
    pub local_path: String,
    pub original_size: Option<u64>,
    pub original_modified: Option<String>,
    pub original_hash: String,
    pub created_at: u64,
    #[serde(default)]
    pub dirty: bool,
    #[serde(default)]
    pub uploaded_path: Option<String>,
    // Old manifests stay manual: opening an earlier copy must not silently
    // publish edits that were made under the previous save-as-new contract.
    #[serde(default)]
    pub auto_save: bool,
    #[serde(default)]
    pub save_status: CloudSaveStatus,
    #[serde(default)]
    pub save_message: Option<String>,
    #[serde(default)]
    pub(crate) write_version: Option<super::providers::rclone::CloudWriteVersion>,
    #[serde(default)]
    pub pending_save: Option<PendingSave>,
}

pub(super) fn lock() -> CloudCommandResult<MutexGuard<'static, ()>> {
    WORKSPACE_LOCK.lock().map_err(|_| {
        CloudCommandError::new(
            CloudCommandErrorCode::TaskFailed,
            "Cloud working-copy registry is unavailable",
        )
    })
}

pub(super) fn root() -> CloudCommandResult<PathBuf> {
    let base = dirs_next::data_local_dir().ok_or_else(|| {
        CloudCommandError::new(
            CloudCommandErrorCode::TaskFailed,
            "No persistent user data directory is available",
        )
    })?;
    Ok(base.join("browsey").join("cloud-workspaces"))
}

pub(super) fn io_error(error: io::Error) -> CloudCommandError {
    let code = match error.kind() {
        io::ErrorKind::NotFound => CloudCommandErrorCode::NotFound,
        io::ErrorKind::PermissionDenied => CloudCommandErrorCode::PermissionDenied,
        io::ErrorKind::AlreadyExists => CloudCommandErrorCode::DestinationExists,
        _ => CloudCommandErrorCode::TaskFailed,
    };
    CloudCommandError::new(code, format!("Cloud working copy: {error}"))
}

pub(super) fn private_dir(path: &Path) -> CloudCommandResult<()> {
    fs::create_dir_all(path).map_err(io_error)?;
    let metadata = fs::symlink_metadata(path).map_err(io_error)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(CloudCommandError::new(
            CloudCommandErrorCode::InvalidPath,
            "Working-copy directory must not be a symlink",
        ));
    }
    private_permissions(path, true)
}

pub(super) fn operation_dir(purpose: &str) -> CloudCommandResult<PathBuf> {
    let base = root()?;
    operation_dir_at(&base, purpose)
}

fn operation_dir_at(base: &Path, purpose: &str) -> CloudCommandResult<PathBuf> {
    private_dir(base)?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let path = base.join(format!("{purpose}-{:x}-{stamp:x}", std::process::id()));
    fs::create_dir(&path).map_err(io_error)?;
    private_permissions(&path, true)?;
    Ok(path)
}

pub(super) fn private_permissions(path: &Path, directory: bool) -> CloudCommandResult<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(
            path,
            fs::Permissions::from_mode(if directory { 0o700 } else { 0o600 }),
        )
        .map_err(io_error)?;
    }
    #[cfg(not(unix))]
    let _ = (path, directory);
    Ok(())
}

pub(super) fn hash_file(path: &Path) -> CloudCommandResult<String> {
    let mut file = crate::fs_utils::open_regular_file_nofollow(path).map_err(io_error)?;
    let mut hasher = blake3::Hasher::new();
    io::copy(&mut file, &mut hasher).map_err(io_error)?;
    Ok(hasher.finalize().to_hex().to_string())
}

pub(super) fn create_at(
    base: &Path,
    source: &CloudPath,
    cached: &Path,
    size: Option<u64>,
    modified: Option<String>,
) -> CloudCommandResult<CloudWorkingCopy> {
    private_dir(base)?;
    // create_dir provides collision exclusion even across multiple Browsey processes.
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let mut sequence = 0u64;
    let (id, dir) = loop {
        let id = format!("{:x}-{stamp:x}-{sequence:x}", std::process::id());
        let dir = base.join(&id);
        match fs::create_dir(&dir) {
            Ok(()) => break (id, dir),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => sequence += 1,
            Err(error) => return Err(io_error(error)),
        }
    };
    private_permissions(&dir, true)?;
    let name = source.leaf_name().map_err(super::map_cloud_path_error)?;
    // child_path validates separators and relative names before creating local paths.
    source
        .child_path(name)
        .map_err(super::map_cloud_path_error)?;
    let files = dir.join("files");
    private_dir(&files)?;
    let local = files.join(name);
    let source_hash = hash_file(cached)?;
    let mut input = crate::fs_utils::open_regular_file_nofollow(cached).map_err(io_error)?;
    let mut output = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&local)
        .map_err(io_error)?;
    private_permissions(&local, false)?;
    io::copy(&mut input, &mut output).map_err(io_error)?;
    output.sync_all().map_err(io_error)?;
    let copied_hash = hash_file(&local)?;
    if copied_hash != source_hash || hash_file(cached)? != source_hash {
        return Err(CloudCommandError::new(CloudCommandErrorCode::TaskFailed,
            format!("The local source changed while preparing a working copy. Close other writers and retry. Output retained at {}", local.display())));
    }
    let copy = CloudWorkingCopy {
        id,
        source_path: source.to_string(),
        local_path: local.to_string_lossy().into_owned(),
        original_size: size,
        original_modified: modified,
        original_hash: copied_hash,
        created_at: stamp.checked_div(1_000_000_000).unwrap_or_default() as u64,
        dirty: false,
        uploaded_path: None,
        auto_save: false,
        save_status: CloudSaveStatus::Manual,
        save_message: None,
        write_version: None,
        pending_save: None,
    };
    save_at(base, &copy)?;
    Ok(copy)
}

pub(super) fn save_at(base: &Path, copy: &CloudWorkingCopy) -> CloudCommandResult<()> {
    let dir = session_dir(base, &copy.id)?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let temporary = dir.join(format!("manifest-{stamp:x}.part"));
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(io_error)?;
    private_permissions(&temporary, false)?;
    serde_json::to_writer(&mut file, copy).map_err(|error| {
        CloudCommandError::new(
            CloudCommandErrorCode::TaskFailed,
            format!("Cannot save working-copy manifest: {error}"),
        )
    })?;
    file.sync_all().map_err(io_error)?;
    fs::rename(temporary, dir.join("manifest.json")).map_err(io_error)?;
    #[cfg(unix)]
    fs::File::open(&dir)
        .and_then(|dir| dir.sync_all())
        .map_err(io_error)?;
    Ok(())
}

fn session_dir(base: &Path, id: &str) -> CloudCommandResult<PathBuf> {
    if id.is_empty()
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() || byte == b'-')
    {
        return Err(CloudCommandError::new(
            CloudCommandErrorCode::InvalidPath,
            "Invalid working-copy id",
        ));
    }
    let dir = base.join(id);
    let meta = fs::symlink_metadata(&dir).map_err(io_error)?;
    if !meta.is_dir() || meta.file_type().is_symlink() {
        return Err(CloudCommandError::new(
            CloudCommandErrorCode::InvalidPath,
            "Invalid working-copy directory",
        ));
    }
    Ok(dir)
}

pub(super) fn load_at(base: &Path, id: &str) -> CloudCommandResult<CloudWorkingCopy> {
    load_manifest_at(base, id, true)
}

fn load_manifest_at(
    base: &Path,
    id: &str,
    check_contents: bool,
) -> CloudCommandResult<CloudWorkingCopy> {
    let dir = session_dir(base, id)?;
    let file = crate::fs_utils::open_regular_file_nofollow(&dir.join("manifest.json"))
        .map_err(io_error)?;
    let mut copy: CloudWorkingCopy = serde_json::from_reader(file).map_err(|error| {
        CloudCommandError::new(
            CloudCommandErrorCode::TaskFailed,
            format!("Cannot read working-copy manifest: {error}"),
        )
    })?;
    let source = CloudPath::parse(&copy.source_path).map_err(super::map_cloud_path_error)?;
    let name = source.leaf_name().map_err(super::map_cloud_path_error)?;
    source
        .child_path(name)
        .map_err(super::map_cloud_path_error)?;
    let files = dir.join("files");
    let meta = fs::symlink_metadata(&files).map_err(io_error)?;
    if !meta.is_dir() || meta.file_type().is_symlink() {
        return Err(CloudCommandError::new(
            CloudCommandErrorCode::InvalidPath,
            "Invalid working-copy files directory",
        ));
    }
    let expected = files.join(name);
    if copy.id != id || Path::new(&copy.local_path) != expected {
        return Err(CloudCommandError::new(
            CloudCommandErrorCode::InvalidPath,
            "Working-copy manifest path mismatch",
        ));
    }
    let file = crate::fs_utils::open_regular_file_nofollow(&expected).map_err(io_error)?;
    if check_contents {
        drop(file);
        copy.dirty = hash_file(&expected)? != copy.original_hash;
        if copy.dirty && copy.save_status == CloudSaveStatus::Saved {
            copy.save_status = if copy.auto_save {
                CloudSaveStatus::Pending
            } else {
                CloudSaveStatus::Manual
            };
        }
    }
    if copy
        .write_version
        .as_ref()
        .is_some_and(|v| v.source_path != copy.source_path)
        || copy
            .pending_save
            .as_ref()
            .is_some_and(|p| p.version.source_path != copy.source_path)
    {
        return Err(CloudCommandError::new(
            CloudCommandErrorCode::InvalidPath,
            "Working-copy source identity mismatch",
        ));
    }
    Ok(copy)
}

#[tauri::command]
pub async fn list_cloud_working_copies() -> ApiResult<Vec<CloudWorkingCopy>> {
    let result = tauri::async_runtime::spawn_blocking(|| {
        let base = root()?;
        {
            let _guard = lock()?;
            private_dir(&base)?;
        }
        let mut copies = Vec::new();
        for entry in fs::read_dir(&base).map_err(io_error)? {
            let entry = entry.map_err(io_error)?;
            if let Some(id) = entry.file_name().to_str() {
                // Keep incomplete/corrupt sessions on disk for manual recovery, never prune them.
                if let Ok(copy) = load_at(&base, id) {
                    copies.push(copy);
                }
            }
        }
        copies.sort_by_key(|copy| std::cmp::Reverse(copy.created_at));
        Ok(copies)
    })
    .await;
    map_api_result(super::map_spawn_result(
        result,
        "Working-copy listing task failed",
    ))
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudUploadResult {
    pub path: String,
    pub source_changed: bool,
}

#[tauri::command]
pub async fn upload_cloud_working_copy(
    id: String,
    cancel: tauri::State<'_, crate::tasks::CancelState>,
    progress_event: Option<String>,
) -> ApiResult<CloudUploadResult> {
    map_api_result(upload_working_copy_impl(id, cancel.inner().clone(), progress_event).await)
}

async fn upload_working_copy_impl(
    id: String,
    cancel: crate::tasks::CancelState,
    progress_event: Option<String>,
) -> CloudCommandResult<CloudUploadResult> {
    super::ensure_cloud_enabled()?;
    let guard = super::register_cloud_cancel(&cancel, &progress_event)?;
    let token = guard.as_ref().map(|guard| guard.token());
    let result = tauri::async_runtime::spawn_blocking(move || {
        let base = root()?;
        let copy = load_manifest_at(&base, &id, false)?;
        let source = CloudPath::parse(&copy.source_path).map_err(super::map_cloud_path_error)?;
        super::limits::with_cloud_remote_permits(vec![source.remote().to_owned()], || {
            let provider = super::configured_rclone_provider().map_err(CloudCommandError::from)?;
            sync::with_copy_lock(&id, || upload_at(&base, &id, &provider, token.as_deref()))
        })
    })
    .await;
    super::map_spawn_result(result, "Working-copy upload task failed")
}

// The command and acceptance tests use this same upload path. The injected
// storage root keeps tests out of the user's working-copy registry.
pub(super) fn upload_at(
    base: &Path,
    id: &str,
    provider: &super::providers::rclone::RcloneCloudProvider,
    token: Option<&std::sync::atomic::AtomicBool>,
) -> CloudCommandResult<CloudUploadResult> {
    use super::provider::CloudProvider;
    let mut copy = load_at(base, id)?;
    let source = CloudPath::parse(&copy.source_path).map_err(super::map_cloud_path_error)?;
    let current = provider.stat_path(&source)?;
    let mut source_changed = current.as_ref().is_none_or(|entry| {
        entry.size != copy.original_size || entry.modified != copy.original_modified
    });
    if !source_changed {
        let check_dir = operation_dir_at(base, "source-check")?;
        let check_file = check_dir.join("source");
        provider.download_file(&source, &check_file, token)?;
        private_permissions(&check_file, false)?;
        source_changed = hash_file(&check_file)? != copy.original_hash;
    }
    // Stop other writers before invoking: upload a frozen private snapshot,
    // never the live file held open by an editor.
    let snapshot = create_at(
        base,
        &source,
        Path::new(&copy.local_path),
        copy.original_size,
        copy.original_modified.clone(),
    )?;
    let destination = save_as_new_path(&source, &snapshot.id)?;
    let result = provider.upload_new_file(Path::new(&snapshot.local_path), &destination, token);
    super::invalidate_cloud_write_paths(std::slice::from_ref(&destination));
    result?;
    copy.uploaded_path = Some(destination.to_string());
    {
        let _guard = lock()?;
        save_at(base, &copy)?;
    }
    Ok(CloudUploadResult {
        path: destination.to_string(),
        source_changed,
    })
}

#[tauri::command]
pub async fn cloud_working_copy_storage_path() -> ApiResult<String> {
    map_api_result(root().and_then(|path| {
        private_dir(&path)?;
        Ok(path.to_string_lossy().into_owned())
    }))
}

#[tauri::command]
pub async fn create_cloud_file(
    path: String,
    cancel: tauri::State<'_, crate::tasks::CancelState>,
    progress_event: Option<String>,
) -> ApiResult<String> {
    map_api_result(create_cloud_file_impl(path, cancel.inner().clone(), progress_event).await)
}

async fn create_cloud_file_impl(
    path: String,
    cancel: crate::tasks::CancelState,
    progress_event: Option<String>,
) -> CloudCommandResult<String> {
    super::ensure_cloud_enabled()?;
    let path = super::parse_cloud_path_arg(path)?;
    let guard = super::register_cloud_cancel(&cancel, &progress_event)?;
    let token = guard.as_ref().map(|guard| guard.token());
    let result = tauri::async_runtime::spawn_blocking(move || {
        super::limits::with_cloud_remote_permits(vec![path.remote().to_owned()], || {
            let stage = operation_dir("new-file")?;
            let local = stage.join("empty");
            let file = fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&local)
                .map_err(io_error)?;
            private_permissions(&local, false)?;
            file.sync_all().map_err(io_error)?;
            let provider = super::configured_rclone_provider().map_err(CloudCommandError::from)?;
            let result = provider.upload_new_file(&local, &path, token.as_deref());
            super::invalidate_cloud_write_paths(std::slice::from_ref(&path));
            result?;
            Ok(path.to_string())
        })
    })
    .await;
    super::map_spawn_result(result, "Cloud new-file task failed")
}

fn save_as_new_path(source: &CloudPath, id: &str) -> CloudCommandResult<CloudPath> {
    let name = source.leaf_name().map_err(super::map_cloud_path_error)?;
    let path = Path::new(name);
    let stem = path
        .file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or(name);
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| format!(".{value}"))
        .unwrap_or_default();
    let name = format!("{stem}-edited-{id}{extension}");
    source
        .parent_dir_path()
        .ok_or_else(|| {
            CloudCommandError::new(
                CloudCommandErrorCode::InvalidPath,
                "Cannot edit a cloud root",
            )
        })?
        .child_path(&name)
        .map_err(super::map_cloud_path_error)
}

#[cfg(test)]
mod tests;
