//! Cloud-provider Tauri commands (rclone-backed, CLI-first).

mod error;
pub mod path;
pub mod provider;
pub mod providers;
pub mod rclone_cli;
pub mod rclone_rc;
pub mod types;

use crate::errors::api_error::ApiResult;
use crate::runtime_lifecycle;
use crate::tasks::{CancelGuard, CancelState};
use error::{map_api_result, CloudCommandError, CloudCommandErrorCode, CloudCommandResult};
use path::CloudPath;
use provider::CloudProvider;
use providers::rclone::RcloneCloudProvider;
use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::sync::{Condvar, Mutex, OnceLock};
use std::time::{Duration, Instant};
use tracing::{debug, info, warn};
use types::{
    CloudConflictInfo, CloudEntry, CloudEntryKind, CloudProviderKind, CloudRemote,
    CloudRootSelection,
};

const CLOUD_REMOTE_DISCOVERY_CACHE_TTL: Duration = Duration::from_secs(45);
const CLOUD_DIR_LISTING_CACHE_TTL: Duration = Duration::from_secs(20);
const CLOUD_DIR_LISTING_STALE_MAX_AGE: Duration = Duration::from_secs(60);
const CLOUD_DIR_LISTING_RETRY_BACKOFFS_MS: &[u64] = &[150, 400];
const CLOUD_REMOTE_MAX_CONCURRENT_OPS: usize = 2;
const CLOUD_REMOTE_RATE_LIMIT_COOLDOWN: Duration = Duration::from_secs(3);
const CLOUD_DIR_REFRESHED_EVENT: &str = "cloud-dir-refreshed";

#[derive(Debug, Clone)]
struct CachedCloudRemoteDiscovery {
    fetched_at: Instant,
    remotes: Vec<CloudRemote>,
}

#[derive(Debug, Clone)]
struct CachedCloudDirListing {
    fetched_at: Instant,
    entries: Vec<CloudEntry>,
}

#[derive(Debug, Default)]
struct CloudRemoteOpLimiter {
    state: Mutex<CloudRemoteOpState>,
    cv: Condvar,
}

#[derive(Debug, Default)]
struct CloudRemoteOpState {
    counts: HashMap<String, usize>,
    cooldown_until: HashMap<String, Instant>,
}

#[derive(Debug)]
struct CloudRemotePermitGuard {
    remotes: Vec<String>,
}

fn cloud_remote_discovery_cache() -> &'static Mutex<Option<CachedCloudRemoteDiscovery>> {
    static CACHE: OnceLock<Mutex<Option<CachedCloudRemoteDiscovery>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(None))
}

fn cloud_dir_listing_cache() -> &'static Mutex<HashMap<String, CachedCloudDirListing>> {
    static CACHE: OnceLock<Mutex<HashMap<String, CachedCloudDirListing>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn cloud_dir_listing_refresh_inflight() -> &'static Mutex<HashSet<String>> {
    static INFLIGHT: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    INFLIGHT.get_or_init(|| Mutex::new(HashSet::new()))
}

fn cloud_remote_op_limiter() -> &'static CloudRemoteOpLimiter {
    static LIMITER: OnceLock<CloudRemoteOpLimiter> = OnceLock::new();
    LIMITER.get_or_init(CloudRemoteOpLimiter::default)
}

#[derive(Debug, Clone)]
enum CloudDirListingCacheLookup {
    Fresh(CachedCloudDirListing),
    Stale(CachedCloudDirListing),
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct CloudDirRefreshedEvent {
    path: String,
    entry_count: usize,
}

fn with_cloud_remote_permits<T>(
    mut remotes: Vec<String>,
    f: impl FnOnce() -> CloudCommandResult<T>,
) -> CloudCommandResult<T> {
    remotes.sort();
    remotes.dedup();
    let _guard = acquire_cloud_remote_permits(remotes);
    let result = f();
    if let Err(error) = &result {
        if error.code() == CloudCommandErrorCode::RateLimited {
            note_remote_rate_limit_cooldown(&_guard.remotes);
        }
    }
    result
}

fn acquire_cloud_remote_permits(remotes: Vec<String>) -> CloudRemotePermitGuard {
    if remotes.is_empty() {
        return CloudRemotePermitGuard { remotes };
    }
    let limiter = cloud_remote_op_limiter();
    let mut state = match limiter.state.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    loop {
        let now = Instant::now();
        state.cooldown_until.retain(|_, deadline| *deadline > now);
        let has_capacity = remotes.iter().all(|remote| {
            state.counts.get(remote).copied().unwrap_or(0) < CLOUD_REMOTE_MAX_CONCURRENT_OPS
        });
        let next_cooldown_deadline = remotes
            .iter()
            .filter_map(|remote| state.cooldown_until.get(remote).copied())
            .min();
        if has_capacity && next_cooldown_deadline.is_none() {
            for remote in &remotes {
                *state.counts.entry(remote.clone()).or_insert(0) += 1;
            }
            return CloudRemotePermitGuard { remotes };
        }
        if let Some(deadline) = next_cooldown_deadline {
            let wait = deadline.saturating_duration_since(now);
            state = match limiter.cv.wait_timeout(state, wait) {
                Ok((guard, _)) => guard,
                Err(poisoned) => poisoned.into_inner().0,
            };
        } else {
            state = match limiter.cv.wait(state) {
                Ok(guard) => guard,
                Err(poisoned) => poisoned.into_inner(),
            };
        }
    }
}

fn note_remote_rate_limit_cooldown(remotes: &[String]) {
    if remotes.is_empty() {
        return;
    }
    let limiter = cloud_remote_op_limiter();
    let now = Instant::now();
    let cooldown_deadline = now + CLOUD_REMOTE_RATE_LIMIT_COOLDOWN;
    let mut state = match limiter.state.lock() {
        Ok(guard) => guard,
        Err(poisoned) => poisoned.into_inner(),
    };
    for remote in remotes {
        let entry = state
            .cooldown_until
            .entry(remote.clone())
            .or_insert(cooldown_deadline);
        if *entry < cooldown_deadline {
            *entry = cooldown_deadline;
        }
    }
    drop(state);
    limiter.cv.notify_all();
    debug!(
        remotes = ?remotes,
        cooldown_ms = CLOUD_REMOTE_RATE_LIMIT_COOLDOWN.as_millis() as u64,
        "applied cloud remote rate-limit cooldown"
    );
}

impl Drop for CloudRemotePermitGuard {
    fn drop(&mut self) {
        if self.remotes.is_empty() {
            return;
        }
        let limiter = cloud_remote_op_limiter();
        let mut state = match limiter.state.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        for remote in &self.remotes {
            match state.counts.get_mut(remote) {
                Some(count) if *count > 1 => *count -= 1,
                Some(_) => {
                    state.counts.remove(remote);
                }
                None => {}
            }
        }
        limiter.cv.notify_all();
    }
}

fn list_cloud_remotes_cached(force_refresh: bool) -> CloudCommandResult<Vec<CloudRemote>> {
    let now = Instant::now();
    if !force_refresh {
        if let Ok(guard) = cloud_remote_discovery_cache().lock() {
            if let Some(cached) = guard.as_ref() {
                if now.duration_since(cached.fetched_at) <= CLOUD_REMOTE_DISCOVERY_CACHE_TTL {
                    return Ok(cached.remotes.clone());
                }
            }
        }
    }

    let provider = RcloneCloudProvider::default();
    let remotes = provider.list_remotes()?;
    if let Ok(mut guard) = cloud_remote_discovery_cache().lock() {
        *guard = Some(CachedCloudRemoteDiscovery {
            fetched_at: now,
            remotes: remotes.clone(),
        });
    }
    Ok(remotes)
}

fn list_cloud_dir_cached(path: &CloudPath) -> CloudCommandResult<Vec<CloudEntry>> {
    list_cloud_dir_cached_with_refresh_event(path, None)
}

fn list_cloud_dir_cached_with_refresh_event(
    path: &CloudPath,
    refresh_event_app: Option<tauri::AppHandle>,
) -> CloudCommandResult<Vec<CloudEntry>> {
    let now = Instant::now();
    let key = path.to_string();
    if let Ok(mut guard) = cloud_dir_listing_cache().lock() {
        prune_cloud_dir_listing_cache_locked(&mut guard, now);
        if let Some(cached) = lookup_cloud_dir_listing_cache_locked(&guard, &key, now) {
            match cached {
                CloudDirListingCacheLookup::Fresh(cached) => {
                    info!(
                        op = "cloud_list_entries",
                        phase = "cache_hit",
                        path = %path,
                        age_ms = now.duration_since(cached.fetched_at).as_millis() as u64,
                        entry_count = cached.entries.len(),
                        "cloud command phase timing"
                    );
                    return Ok(cached.entries);
                }
                CloudDirListingCacheLookup::Stale(cached) => {
                    info!(
                        op = "cloud_list_entries",
                        phase = "stale_cache_hit",
                        path = %path,
                        age_ms = now.duration_since(cached.fetched_at).as_millis() as u64,
                        entry_count = cached.entries.len(),
                        "cloud command phase timing"
                    );
                    schedule_cloud_dir_listing_refresh(
                        path.clone(),
                        key.clone(),
                        refresh_event_app.clone(),
                    );
                    return Ok(cached.entries);
                }
            }
        }
    }

    let entries = list_cloud_dir_with_retry(path)?;
    store_cloud_dir_listing_cache_entry(key, now, entries.clone());
    Ok(entries)
}

fn prune_cloud_dir_listing_cache_locked(
    cache: &mut HashMap<String, CachedCloudDirListing>,
    now: Instant,
) {
    cache.retain(|_, cached| {
        now.duration_since(cached.fetched_at) <= CLOUD_DIR_LISTING_STALE_MAX_AGE
    });
}

fn lookup_cloud_dir_listing_cache_locked(
    cache: &HashMap<String, CachedCloudDirListing>,
    key: &str,
    now: Instant,
) -> Option<CloudDirListingCacheLookup> {
    let cached = cache.get(key)?.clone();
    let age = now.duration_since(cached.fetched_at);
    if age <= CLOUD_DIR_LISTING_CACHE_TTL {
        Some(CloudDirListingCacheLookup::Fresh(cached))
    } else if age <= CLOUD_DIR_LISTING_STALE_MAX_AGE {
        Some(CloudDirListingCacheLookup::Stale(cached))
    } else {
        None
    }
}

fn store_cloud_dir_listing_cache_entry(key: String, fetched_at: Instant, entries: Vec<CloudEntry>) {
    if let Ok(mut guard) = cloud_dir_listing_cache().lock() {
        guard.insert(
            key,
            CachedCloudDirListing {
                fetched_at,
                entries,
            },
        );
    }
}

fn schedule_cloud_dir_listing_refresh(
    path: CloudPath,
    key: String,
    refresh_event_app: Option<tauri::AppHandle>,
) {
    {
        let mut inflight = match cloud_dir_listing_refresh_inflight().lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        if !inflight.insert(key.clone()) {
            return;
        }
    }

    std::thread::spawn(move || {
        let _background_guard = if let Some(app) = refresh_event_app.as_ref() {
            if let Some(handle) = runtime_lifecycle::handle_from_app(app) {
                match handle.try_enter_background_job() {
                    Some(guard) => Some(guard),
                    None => {
                        let mut inflight = match cloud_dir_listing_refresh_inflight().lock() {
                            Ok(guard) => guard,
                            Err(poisoned) => poisoned.into_inner(),
                        };
                        inflight.remove(&key);
                        return;
                    }
                }
            } else {
                None
            }
        } else {
            None
        };
        let started = Instant::now();
        let refresh_result = run_cloud_dir_listing_refresh(&path);
        match refresh_result {
            Ok(entries) => {
                store_cloud_dir_listing_cache_entry(key.clone(), Instant::now(), entries.clone());
                if let Some(app) = refresh_event_app.as_ref() {
                    let _ = runtime_lifecycle::emit_if_running(
                        app,
                        CLOUD_DIR_REFRESHED_EVENT,
                        CloudDirRefreshedEvent {
                            path: path.to_string(),
                            entry_count: entries.len(),
                        },
                    );
                }
                info!(
                    op = "cloud_list_entries",
                    phase = "background_refresh",
                    path = %path,
                    elapsed_ms = started.elapsed().as_millis() as u64,
                    entry_count = entries.len(),
                    "cloud command phase timing"
                );
            }
            Err(error) => {
                debug!(
                    op = "cloud_list_entries",
                    phase = "background_refresh",
                    path = %path,
                    elapsed_ms = started.elapsed().as_millis() as u64,
                    error = %error,
                    "cloud background refresh failed"
                );
            }
        }

        let mut inflight = match cloud_dir_listing_refresh_inflight().lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        inflight.remove(&key);
    });
}

fn run_cloud_dir_listing_refresh(path: &CloudPath) -> CloudCommandResult<Vec<CloudEntry>> {
    #[cfg(test)]
    {
        if let Some(entries) = run_cloud_dir_listing_refresh_test_hook(path)? {
            return Ok(entries);
        }
    }
    list_cloud_dir_with_retry(path)
}

#[cfg(test)]
type CloudDirRefreshHook =
    std::sync::Arc<dyn Fn(&CloudPath) -> CloudCommandResult<Vec<CloudEntry>> + Send + Sync>;

#[cfg(test)]
fn cloud_dir_listing_refresh_test_hook() -> &'static Mutex<Option<CloudDirRefreshHook>> {
    static HOOK: OnceLock<Mutex<Option<CloudDirRefreshHook>>> = OnceLock::new();
    HOOK.get_or_init(|| Mutex::new(None))
}

#[cfg(test)]
fn set_cloud_dir_listing_refresh_test_hook(hook: Option<CloudDirRefreshHook>) {
    let mut guard = cloud_dir_listing_refresh_test_hook()
        .lock()
        .expect("refresh hook lock");
    *guard = hook;
}

#[cfg(test)]
fn run_cloud_dir_listing_refresh_test_hook(
    path: &CloudPath,
) -> CloudCommandResult<Option<Vec<CloudEntry>>> {
    let hook = cloud_dir_listing_refresh_test_hook()
        .lock()
        .expect("refresh hook lock")
        .clone();
    match hook {
        Some(hook) => hook(path).map(Some),
        None => Ok(None),
    }
}

fn list_cloud_dir_with_retry(path: &CloudPath) -> CloudCommandResult<Vec<CloudEntry>> {
    let permit_started = Instant::now();
    let guard = acquire_cloud_remote_permits(vec![path.remote().to_string()]);
    let permit_wait_ms = permit_started.elapsed().as_millis() as u64;
    let provider = RcloneCloudProvider::default();
    let fetch_started = Instant::now();
    let mut attempt = 0usize;
    loop {
        match provider.list_dir(path) {
            Ok(entries) => {
                info!(
                    op = "cloud_list_entries",
                    phase = "backend_fetch",
                    path = %path,
                    permit_wait_ms,
                    fetch_ms = fetch_started.elapsed().as_millis() as u64,
                    attempts = attempt + 1,
                    entry_count = entries.len(),
                    "cloud command phase timing"
                );
                return Ok(entries);
            }
            Err(error) if should_retry_cloud_dir_error(&error) => {
                let Some(backoff_ms) = CLOUD_DIR_LISTING_RETRY_BACKOFFS_MS.get(attempt).copied()
                else {
                    if error.code() == CloudCommandErrorCode::RateLimited {
                        note_remote_rate_limit_cooldown(&guard.remotes);
                    }
                    debug!(
                        op = "cloud_list_entries",
                        phase = "backend_fetch",
                        path = %path,
                        permit_wait_ms,
                        fetch_ms = fetch_started.elapsed().as_millis() as u64,
                        attempts = attempt + 1,
                        error = %error,
                        "cloud command failed after retries"
                    );
                    return Err(error);
                };
                attempt += 1;
                debug!(
                    attempt,
                    backoff_ms,
                    path = %path,
                    error = %error,
                    "retrying cloud directory listing after transient error"
                );
                std::thread::sleep(Duration::from_millis(backoff_ms));
            }
            Err(error) => {
                if error.code() == CloudCommandErrorCode::RateLimited {
                    note_remote_rate_limit_cooldown(&guard.remotes);
                }
                debug!(
                    op = "cloud_list_entries",
                    phase = "backend_fetch",
                    path = %path,
                    permit_wait_ms,
                    fetch_ms = fetch_started.elapsed().as_millis() as u64,
                    attempts = attempt + 1,
                    error = %error,
                    "cloud command failed"
                );
                return Err(error);
            }
        }
    }
}

fn should_retry_cloud_dir_error(error: &CloudCommandError) -> bool {
    matches!(
        error.code(),
        CloudCommandErrorCode::Timeout
            | CloudCommandErrorCode::NetworkError
            | CloudCommandErrorCode::RateLimited
    )
}

fn invalidate_cloud_dir_listing_cache_path_locked(
    cache: &mut HashMap<String, CachedCloudDirListing>,
    path: &CloudPath,
) {
    let key = path.to_string();
    let subtree_prefix = format!("{key}/");
    cache.retain(|cached_path, _| cached_path != &key && !cached_path.starts_with(&subtree_prefix));
}

fn invalidate_cloud_dir_listing_cache_for_write_paths(paths: &[CloudPath]) {
    if let Ok(mut guard) = cloud_dir_listing_cache().lock() {
        for path in paths {
            invalidate_cloud_dir_listing_cache_path_locked(&mut guard, path);
            if let Some(parent) = path.parent_dir_path() {
                invalidate_cloud_dir_listing_cache_path_locked(&mut guard, &parent);
            }
        }
    }
}

pub(crate) fn cloud_provider_kind_for_remote(remote_id: &str) -> Option<CloudProviderKind> {
    list_cloud_remotes_cached(false)
        .ok()
        .and_then(|remotes| remotes.into_iter().find(|remote| remote.id == remote_id))
        .map(|remote| remote.provider)
}

pub(crate) fn cloud_conflict_name_key(provider: Option<CloudProviderKind>, name: &str) -> String {
    match provider {
        // OneDrive is effectively case-insensitive for path conflicts.
        Some(CloudProviderKind::Onedrive) => name.to_ascii_lowercase(),
        _ => name.to_string(),
    }
}

pub(crate) fn list_cloud_remotes_sync_best_effort(force_refresh: bool) -> Vec<CloudRemote> {
    match list_cloud_remotes_cached(force_refresh) {
        Ok(remotes) => remotes,
        Err(error) => {
            warn!(error = %error, "cloud remote discovery failed; omitting cloud remotes from Network view");
            Vec::new()
        }
    }
}

#[tauri::command]
pub async fn list_cloud_remotes() -> ApiResult<Vec<CloudRemote>> {
    map_api_result(list_cloud_remotes_impl().await)
}

#[tauri::command]
pub fn cloud_rc_health() -> ApiResult<rclone_rc::RcloneRcHealth> {
    map_api_result(Ok(rclone_rc::health_snapshot()))
}

async fn list_cloud_remotes_impl() -> CloudCommandResult<Vec<CloudRemote>> {
    let task = tauri::async_runtime::spawn_blocking(|| list_cloud_remotes_cached(false));
    match task.await {
        Ok(result) => result,
        Err(error) => Err(CloudCommandError::new(
            CloudCommandErrorCode::TaskFailed,
            format!("cloud remote list task failed: {error}"),
        )),
    }
}

#[tauri::command]
pub async fn validate_cloud_root(path: String) -> ApiResult<CloudRootSelection> {
    map_api_result(validate_cloud_root_impl(path).await)
}

async fn validate_cloud_root_impl(path: String) -> CloudCommandResult<CloudRootSelection> {
    let path = parse_cloud_path_arg(path)?;
    let task = tauri::async_runtime::spawn_blocking(move || {
        let provider = RcloneCloudProvider::default();
        let remotes = provider.list_remotes()?;
        let remote = remotes
            .into_iter()
            .find(|remote| remote.id == path.remote())
            .ok_or_else(|| {
                CloudCommandError::new(
                    CloudCommandErrorCode::InvalidConfig,
                    format!(
                        "Cloud remote is not configured or unsupported: {}",
                        path.remote()
                    ),
                )
            })?;

        if !path.is_root() {
            let stat = provider.stat_path(&path)?.ok_or_else(|| {
                CloudCommandError::new(
                    CloudCommandErrorCode::NotFound,
                    format!("Cloud root path does not exist: {path}"),
                )
            })?;
            if !matches!(stat.kind, CloudEntryKind::Dir) {
                return Err(CloudCommandError::new(
                    CloudCommandErrorCode::InvalidPath,
                    format!("Cloud root path must be a directory: {path}"),
                ));
            }
        }

        Ok(CloudRootSelection {
            remote,
            root_path: path.to_string(),
            is_remote_root: path.is_root(),
        })
    });
    map_spawn_result(task.await, "cloud root validation task failed")
}

#[tauri::command]
pub async fn list_cloud_entries(path: String, app: tauri::AppHandle) -> ApiResult<Vec<CloudEntry>> {
    map_api_result(list_cloud_entries_impl(path, app).await)
}

async fn list_cloud_entries_impl(
    path: String,
    app: tauri::AppHandle,
) -> CloudCommandResult<Vec<CloudEntry>> {
    let started = Instant::now();
    let path = parse_cloud_path_arg(path)?;
    let path_for_log = path.clone();
    let task = tauri::async_runtime::spawn_blocking(move || {
        list_cloud_dir_cached_with_refresh_event(&path, Some(app))
    });
    let result = match task.await {
        Ok(result) => result,
        Err(error) => Err(CloudCommandError::new(
            CloudCommandErrorCode::TaskFailed,
            format!("cloud list task failed: {error}"),
        )),
    };
    let elapsed_ms = started.elapsed().as_millis() as u64;
    match &result {
        Ok(entries) => info!(
            op = "cloud_list_entries",
            path = %path_for_log,
            entry_count = entries.len(),
            elapsed_ms,
            "cloud command timing"
        ),
        Err(error) => debug!(
            op = "cloud_list_entries",
            path = %path_for_log,
            elapsed_ms,
            error = %error,
            "cloud command failed"
        ),
    }
    result
}

#[tauri::command]
pub async fn stat_cloud_entry(path: String) -> ApiResult<Option<CloudEntry>> {
    map_api_result(stat_cloud_entry_impl(path).await)
}

async fn stat_cloud_entry_impl(path: String) -> CloudCommandResult<Option<CloudEntry>> {
    let path = parse_cloud_path_arg(path)?;
    let remote = path.remote().to_string();
    let task = tauri::async_runtime::spawn_blocking(move || {
        with_cloud_remote_permits(vec![remote], || {
            let provider = RcloneCloudProvider::default();
            provider.stat_path(&path)
        })
    });
    match task.await {
        Ok(result) => result,
        Err(error) => Err(CloudCommandError::new(
            CloudCommandErrorCode::TaskFailed,
            format!("cloud stat task failed: {error}"),
        )),
    }
}

#[tauri::command]
pub async fn create_cloud_folder(
    path: String,
    cancel: tauri::State<'_, CancelState>,
    progress_event: Option<String>,
) -> ApiResult<()> {
    map_api_result(create_cloud_folder_impl(path, cancel.inner().clone(), progress_event).await)
}

async fn create_cloud_folder_impl(
    path: String,
    cancel_state: CancelState,
    progress_event: Option<String>,
) -> CloudCommandResult<()> {
    let started = Instant::now();
    let path = parse_cloud_path_arg(path)?;
    let path_for_invalidate = path.clone();
    let path_for_log = path.clone();
    let remote = path.remote().to_string();
    let _cancel_guard = register_cloud_cancel(&cancel_state, &progress_event)?;
    let cancel_token = _cancel_guard.as_ref().map(|guard| guard.token());
    let task = tauri::async_runtime::spawn_blocking(move || {
        with_cloud_remote_permits(vec![remote], || {
            let provider = RcloneCloudProvider::default();
            provider.mkdir(&path, cancel_token.as_deref())
        })
    });
    let result = match task.await {
        Ok(result) => {
            result?;
            invalidate_cloud_dir_listing_cache_for_write_paths(&[path_for_invalidate]);
            Ok(())
        }
        Err(error) => Err(CloudCommandError::new(
            CloudCommandErrorCode::TaskFailed,
            format!("cloud mkdir task failed: {error}"),
        )),
    };
    let elapsed_ms = started.elapsed().as_millis() as u64;
    match &result {
        Ok(()) => info!(
            op = "cloud_write_mkdir",
            path = %path_for_log,
            elapsed_ms,
            "cloud command timing"
        ),
        Err(error) => debug!(
            op = "cloud_write_mkdir",
            path = %path_for_log,
            elapsed_ms,
            error = %error,
            "cloud command failed"
        ),
    }
    result
}

#[tauri::command]
pub async fn delete_cloud_file(
    path: String,
    cancel: tauri::State<'_, CancelState>,
    progress_event: Option<String>,
) -> ApiResult<()> {
    map_api_result(delete_cloud_file_impl(path, cancel.inner().clone(), progress_event).await)
}

async fn delete_cloud_file_impl(
    path: String,
    cancel_state: CancelState,
    progress_event: Option<String>,
) -> CloudCommandResult<()> {
    let started = Instant::now();
    let path = parse_cloud_path_arg(path)?;
    let path_for_invalidate = path.clone();
    let path_for_log = path.clone();
    let remote = path.remote().to_string();
    let _cancel_guard = register_cloud_cancel(&cancel_state, &progress_event)?;
    let cancel_token = _cancel_guard.as_ref().map(|guard| guard.token());
    let task = tauri::async_runtime::spawn_blocking(move || {
        with_cloud_remote_permits(vec![remote], || {
            let provider = RcloneCloudProvider::default();
            provider.delete_file(&path, cancel_token.as_deref())
        })
    });
    let result = map_spawn_result(task.await, "cloud delete file task failed").map(|_| {
        invalidate_cloud_dir_listing_cache_for_write_paths(&[path_for_invalidate]);
    });
    let elapsed_ms = started.elapsed().as_millis() as u64;
    match &result {
        Ok(()) => info!(
            op = "cloud_write_delete_file",
            path = %path_for_log,
            elapsed_ms,
            "cloud command timing"
        ),
        Err(error) => debug!(
            op = "cloud_write_delete_file",
            path = %path_for_log,
            elapsed_ms,
            error = %error,
            "cloud command failed"
        ),
    }
    result
}

#[tauri::command]
pub async fn delete_cloud_dir_recursive(
    path: String,
    cancel: tauri::State<'_, CancelState>,
    progress_event: Option<String>,
) -> ApiResult<()> {
    map_api_result(
        delete_cloud_dir_recursive_impl(path, cancel.inner().clone(), progress_event).await,
    )
}

async fn delete_cloud_dir_recursive_impl(
    path: String,
    cancel_state: CancelState,
    progress_event: Option<String>,
) -> CloudCommandResult<()> {
    let started = Instant::now();
    let path = parse_cloud_path_arg(path)?;
    let path_for_invalidate = path.clone();
    let path_for_log = path.clone();
    let remote = path.remote().to_string();
    let _cancel_guard = register_cloud_cancel(&cancel_state, &progress_event)?;
    let cancel_token = _cancel_guard.as_ref().map(|guard| guard.token());
    let task = tauri::async_runtime::spawn_blocking(move || {
        with_cloud_remote_permits(vec![remote], || {
            let provider = RcloneCloudProvider::default();
            provider.delete_dir_recursive(&path, cancel_token.as_deref())
        })
    });
    let result = map_spawn_result(task.await, "cloud delete dir task failed").map(|_| {
        invalidate_cloud_dir_listing_cache_for_write_paths(&[path_for_invalidate]);
    });
    let elapsed_ms = started.elapsed().as_millis() as u64;
    match &result {
        Ok(()) => info!(
            op = "cloud_write_delete_dir_recursive",
            path = %path_for_log,
            elapsed_ms,
            "cloud command timing"
        ),
        Err(error) => debug!(
            op = "cloud_write_delete_dir_recursive",
            path = %path_for_log,
            elapsed_ms,
            error = %error,
            "cloud command failed"
        ),
    }
    result
}

#[tauri::command]
pub async fn delete_cloud_dir_empty(
    path: String,
    cancel: tauri::State<'_, CancelState>,
    progress_event: Option<String>,
) -> ApiResult<()> {
    map_api_result(delete_cloud_dir_empty_impl(path, cancel.inner().clone(), progress_event).await)
}

async fn delete_cloud_dir_empty_impl(
    path: String,
    cancel_state: CancelState,
    progress_event: Option<String>,
) -> CloudCommandResult<()> {
    let started = Instant::now();
    let path = parse_cloud_path_arg(path)?;
    let path_for_invalidate = path.clone();
    let path_for_log = path.clone();
    let remote = path.remote().to_string();
    let _cancel_guard = register_cloud_cancel(&cancel_state, &progress_event)?;
    let cancel_token = _cancel_guard.as_ref().map(|guard| guard.token());
    let task = tauri::async_runtime::spawn_blocking(move || {
        with_cloud_remote_permits(vec![remote], || {
            let provider = RcloneCloudProvider::default();
            provider.delete_dir_empty(&path, cancel_token.as_deref())
        })
    });
    let result = map_spawn_result(task.await, "cloud rmdir task failed").map(|_| {
        invalidate_cloud_dir_listing_cache_for_write_paths(&[path_for_invalidate]);
    });
    let elapsed_ms = started.elapsed().as_millis() as u64;
    match &result {
        Ok(()) => info!(
            op = "cloud_write_delete_dir_empty",
            path = %path_for_log,
            elapsed_ms,
            "cloud command timing"
        ),
        Err(error) => debug!(
            op = "cloud_write_delete_dir_empty",
            path = %path_for_log,
            elapsed_ms,
            error = %error,
            "cloud command failed"
        ),
    }
    result
}

#[tauri::command]
pub async fn move_cloud_entry(
    src: String,
    dst: String,
    overwrite: Option<bool>,
    prechecked: Option<bool>,
    cancel: tauri::State<'_, CancelState>,
    progress_event: Option<String>,
) -> ApiResult<()> {
    map_api_result(
        move_cloud_entry_impl(
            src,
            dst,
            overwrite.unwrap_or(false),
            prechecked.unwrap_or(false),
            cancel.inner().clone(),
            progress_event,
        )
        .await,
    )
}

async fn move_cloud_entry_impl(
    src: String,
    dst: String,
    overwrite: bool,
    prechecked: bool,
    cancel_state: CancelState,
    progress_event: Option<String>,
) -> CloudCommandResult<()> {
    let started = Instant::now();
    let src = parse_cloud_path_arg(src)?;
    let dst = parse_cloud_path_arg(dst)?;
    let src_for_log = src.clone();
    let dst_for_log = dst.clone();
    let invalidate_paths = vec![src.clone(), dst.clone()];
    let remotes = vec![src.remote().to_string(), dst.remote().to_string()];
    let _cancel_guard = register_cloud_cancel(&cancel_state, &progress_event)?;
    let cancel_token = _cancel_guard.as_ref().map(|guard| guard.token());
    let task = tauri::async_runtime::spawn_blocking(move || {
        with_cloud_remote_permits(remotes, || {
            let provider = RcloneCloudProvider::default();
            provider.move_entry(&src, &dst, overwrite, prechecked, cancel_token.as_deref())
        })
    });
    let result = map_spawn_result(task.await, "cloud move task failed").map(|_| {
        invalidate_cloud_dir_listing_cache_for_write_paths(&invalidate_paths);
    });
    let elapsed_ms = started.elapsed().as_millis() as u64;
    match &result {
        Ok(()) => info!(
            op = "cloud_write_move",
            src = %src_for_log,
            dst = %dst_for_log,
            overwrite,
            prechecked,
            elapsed_ms,
            "cloud command timing"
        ),
        Err(error) => debug!(
            op = "cloud_write_move",
            src = %src_for_log,
            dst = %dst_for_log,
            overwrite,
            prechecked,
            elapsed_ms,
            error = %error,
            "cloud command failed"
        ),
    }
    result
}

#[tauri::command]
pub async fn rename_cloud_entry(
    src: String,
    dst: String,
    overwrite: Option<bool>,
    prechecked: Option<bool>,
    cancel: tauri::State<'_, CancelState>,
    progress_event: Option<String>,
) -> ApiResult<()> {
    map_api_result(
        move_cloud_entry_impl(
            src,
            dst,
            overwrite.unwrap_or(false),
            prechecked.unwrap_or(false),
            cancel.inner().clone(),
            progress_event,
        )
        .await,
    )
}

#[tauri::command]
pub async fn copy_cloud_entry(
    src: String,
    dst: String,
    overwrite: Option<bool>,
    prechecked: Option<bool>,
    cancel: tauri::State<'_, CancelState>,
    progress_event: Option<String>,
) -> ApiResult<()> {
    map_api_result(
        copy_cloud_entry_impl(
            src,
            dst,
            overwrite.unwrap_or(false),
            prechecked.unwrap_or(false),
            cancel.inner().clone(),
            progress_event,
        )
        .await,
    )
}

async fn copy_cloud_entry_impl(
    src: String,
    dst: String,
    overwrite: bool,
    prechecked: bool,
    cancel_state: CancelState,
    progress_event: Option<String>,
) -> CloudCommandResult<()> {
    let started = Instant::now();
    let src = parse_cloud_path_arg(src)?;
    let dst = parse_cloud_path_arg(dst)?;
    let src_for_log = src.clone();
    let dst_for_log = dst.clone();
    let invalidate_paths = vec![dst.clone()];
    let remotes = vec![src.remote().to_string(), dst.remote().to_string()];
    let _cancel_guard = register_cloud_cancel(&cancel_state, &progress_event)?;
    let cancel_token = _cancel_guard.as_ref().map(|guard| guard.token());
    let task = tauri::async_runtime::spawn_blocking(move || {
        with_cloud_remote_permits(remotes, || {
            let provider = RcloneCloudProvider::default();
            provider.copy_entry(&src, &dst, overwrite, prechecked, cancel_token.as_deref())
        })
    });
    let result = map_spawn_result(task.await, "cloud copy task failed").map(|_| {
        invalidate_cloud_dir_listing_cache_for_write_paths(&invalidate_paths);
    });
    let elapsed_ms = started.elapsed().as_millis() as u64;
    match &result {
        Ok(()) => info!(
            op = "cloud_write_copy",
            src = %src_for_log,
            dst = %dst_for_log,
            overwrite,
            prechecked,
            elapsed_ms,
            "cloud command timing"
        ),
        Err(error) => debug!(
            op = "cloud_write_copy",
            src = %src_for_log,
            dst = %dst_for_log,
            overwrite,
            prechecked,
            elapsed_ms,
            error = %error,
            "cloud command failed"
        ),
    }
    result
}

#[tauri::command]
pub async fn preview_cloud_conflicts(
    sources: Vec<String>,
    dest_dir: String,
) -> ApiResult<Vec<CloudConflictInfo>> {
    map_api_result(preview_cloud_conflicts_impl(sources, dest_dir).await)
}

async fn preview_cloud_conflicts_impl(
    sources: Vec<String>,
    dest_dir: String,
) -> CloudCommandResult<Vec<CloudConflictInfo>> {
    let started = Instant::now();
    let dest_dir = parse_cloud_path_arg(dest_dir)?;
    let sources = sources
        .into_iter()
        .map(parse_cloud_path_arg)
        .collect::<CloudCommandResult<Vec<_>>>()?;
    let source_count = sources.len();
    let dest_dir_for_log = dest_dir.clone();
    let task = tauri::async_runtime::spawn_blocking(move || {
        let provider = cloud_provider_kind_for_remote(dest_dir.remote());
        let dest_entries = list_cloud_dir_cached(&dest_dir)?;
        build_conflicts_from_dest_listing(&sources, &dest_dir, &dest_entries, provider)
    });
    let result = map_spawn_result(task.await, "cloud conflict preview task failed");
    let elapsed_ms = started.elapsed().as_millis() as u64;
    match &result {
        Ok(conflicts) => info!(
            op = "cloud_conflict_preview",
            dest_dir = %dest_dir_for_log,
            source_count,
            conflict_count = conflicts.len(),
            elapsed_ms,
            "cloud command timing"
        ),
        Err(error) => debug!(
            op = "cloud_conflict_preview",
            dest_dir = %dest_dir_for_log,
            source_count,
            elapsed_ms,
            error = %error,
            "cloud command failed"
        ),
    }
    result
}

fn build_conflicts_from_dest_listing(
    sources: &[CloudPath],
    dest_dir: &CloudPath,
    dest_entries: &[CloudEntry],
    provider: Option<CloudProviderKind>,
) -> CloudCommandResult<Vec<CloudConflictInfo>> {
    let mut name_to_is_dir: HashMap<String, bool> = HashMap::with_capacity(dest_entries.len());
    for entry in dest_entries {
        name_to_is_dir
            .entry(cloud_conflict_name_key(provider, &entry.name))
            .or_insert(matches!(entry.kind, CloudEntryKind::Dir));
    }

    let mut conflicts = Vec::new();
    for src in sources {
        let name = src.leaf_name().map_err(|error| {
            CloudCommandError::new(
                CloudCommandErrorCode::InvalidPath,
                format!("Invalid source cloud path for conflict preview: {error}"),
            )
        })?;
        let key = cloud_conflict_name_key(provider, name);
        let Some(is_dir) = name_to_is_dir.get(&key) else {
            continue;
        };
        let target = dest_dir.child_path(name).map_err(|error| {
            CloudCommandError::new(
                CloudCommandErrorCode::InvalidPath,
                format!("Invalid target cloud path for conflict preview: {error}"),
            )
        })?;
        conflicts.push(CloudConflictInfo {
            src: src.to_string(),
            target: target.to_string(),
            exists: true,
            is_dir: *is_dir,
        });
    }
    Ok(conflicts)
}

#[tauri::command]
pub fn normalize_cloud_path(path: String) -> ApiResult<String> {
    map_api_result(normalize_cloud_path_impl(path))
}

fn normalize_cloud_path_impl(path: String) -> CloudCommandResult<String> {
    let path = parse_cloud_path_arg(path)?;
    Ok(path.to_string())
}

fn parse_cloud_path_arg(path: String) -> CloudCommandResult<CloudPath> {
    CloudPath::parse(&path).map_err(|error| {
        CloudCommandError::new(
            CloudCommandErrorCode::InvalidPath,
            format!("Invalid cloud path: {error}"),
        )
    })
}

fn map_spawn_result<T>(
    result: Result<CloudCommandResult<T>, tauri::Error>,
    context: &str,
) -> CloudCommandResult<T> {
    match result {
        Ok(inner) => inner,
        Err(error) => Err(CloudCommandError::new(
            CloudCommandErrorCode::TaskFailed,
            format!("{context}: {error}"),
        )),
    }
}

fn register_cloud_cancel(
    cancel_state: &CancelState,
    progress_event: &Option<String>,
) -> CloudCommandResult<Option<CancelGuard>> {
    progress_event
        .as_ref()
        .map(|event| cancel_state.register(event.clone()))
        .transpose()
        .map_err(|error| {
            CloudCommandError::new(
                CloudCommandErrorCode::TaskFailed,
                format!("Failed to register cloud cancel token: {error}"),
            )
        })
}

#[cfg(test)]
mod tests {
    use super::{
        acquire_cloud_remote_permits, build_conflicts_from_dest_listing, cloud_dir_listing_cache,
        cloud_dir_listing_refresh_inflight, cloud_remote_op_limiter,
        invalidate_cloud_dir_listing_cache_for_write_paths, list_cloud_dir_cached,
        normalize_cloud_path, normalize_cloud_path_impl, prune_cloud_dir_listing_cache_locked,
        set_cloud_dir_listing_refresh_test_hook, with_cloud_remote_permits, CachedCloudDirListing,
    };
    use crate::commands::cloud::{
        error::{CloudCommandError, CloudCommandErrorCode},
        path::CloudPath,
        types::{CloudCapabilities, CloudEntry, CloudEntryKind, CloudProviderKind},
    };
    use std::{
        collections::HashMap,
        sync::{
            atomic::{AtomicUsize, Ordering},
            Arc, Barrier, Mutex, OnceLock,
        },
        thread,
        time::{Duration, Instant},
    };

    fn unique_test_remote(prefix: &str) -> String {
        static NEXT_ID: AtomicUsize = AtomicUsize::new(1);
        format!("{prefix}-{}", NEXT_ID.fetch_add(1, Ordering::Relaxed))
    }

    fn cloud_listing_test_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn sample_cloud_file(path: &str, name: &str) -> CloudEntry {
        CloudEntry {
            name: name.to_string(),
            path: path.to_string(),
            kind: CloudEntryKind::File,
            size: Some(1),
            modified: None,
            capabilities: CloudCapabilities::v1_core_rw(),
        }
    }

    fn clear_cloud_listing_test_state() {
        let mut cache = cloud_dir_listing_cache().lock().expect("cache lock");
        cache.clear();
        drop(cache);

        let mut inflight = cloud_dir_listing_refresh_inflight()
            .lock()
            .expect("inflight lock");
        inflight.clear();
        drop(inflight);

        set_cloud_dir_listing_refresh_test_hook(None);
    }

    #[test]
    fn normalize_cloud_path_command_returns_normalized_path() {
        let out = normalize_cloud_path("rclone://work/docs/file.txt".to_string())
            .expect("normalize_cloud_path should succeed");
        assert_eq!(out, "rclone://work/docs/file.txt");
    }

    #[test]
    fn normalize_cloud_path_command_maps_invalid_path_to_api_error() {
        let err = normalize_cloud_path("rclone://work/../docs".to_string())
            .expect_err("normalize_cloud_path should fail for relative segments");
        assert_eq!(err.code, "invalid_path");
        assert!(
            err.message.contains("Invalid cloud path"),
            "unexpected message: {}",
            err.message
        );
    }

    #[test]
    fn normalize_cloud_path_impl_rejects_non_rclone_paths() {
        let err =
            normalize_cloud_path_impl("/tmp/file.txt".to_string()).expect_err("should reject");
        assert_eq!(
            err.to_string(),
            "Invalid cloud path: Path must start with rclone://"
        );
    }

    #[test]
    fn conflict_preview_uses_dest_listing_names() {
        let src_file = CloudPath::parse("rclone://work/src/report.txt").expect("src file");
        let src_dir = CloudPath::parse("rclone://work/src/Folder").expect("src dir");
        let src_missing = CloudPath::parse("rclone://work/src/notes.txt").expect("src missing");
        let dest_dir = CloudPath::parse("rclone://work/dest").expect("dest");
        let dest_entries = vec![
            CloudEntry {
                name: "report.txt".to_string(),
                path: "rclone://work/dest/report.txt".to_string(),
                kind: CloudEntryKind::File,
                size: Some(1),
                modified: None,
                capabilities: CloudCapabilities::v1_core_rw(),
            },
            CloudEntry {
                name: "Folder".to_string(),
                path: "rclone://work/dest/Folder".to_string(),
                kind: CloudEntryKind::Dir,
                size: None,
                modified: None,
                capabilities: CloudCapabilities::v1_core_rw(),
            },
        ];

        let conflicts = build_conflicts_from_dest_listing(
            &[src_file.clone(), src_dir.clone(), src_missing],
            &dest_dir,
            &dest_entries,
            None,
        )
        .expect("conflicts");

        assert_eq!(conflicts.len(), 2);
        assert_eq!(conflicts[0].src, src_file.to_string());
        assert_eq!(conflicts[0].target, "rclone://work/dest/report.txt");
        assert!(!conflicts[0].is_dir);
        assert_eq!(conflicts[1].src, src_dir.to_string());
        assert_eq!(conflicts[1].target, "rclone://work/dest/Folder");
        assert!(conflicts[1].is_dir);
    }

    #[test]
    fn conflict_preview_is_case_insensitive_for_onedrive_names() {
        let src_file = CloudPath::parse("rclone://work/src/report.txt").expect("src file");
        let dest_dir = CloudPath::parse("rclone://work/dest").expect("dest");
        let dest_entries = vec![CloudEntry {
            name: "Report.txt".to_string(),
            path: "rclone://work/dest/Report.txt".to_string(),
            kind: CloudEntryKind::File,
            size: Some(1),
            modified: None,
            capabilities: CloudCapabilities::v1_core_rw(),
        }];

        let conflicts = build_conflicts_from_dest_listing(
            std::slice::from_ref(&src_file),
            &dest_dir,
            &dest_entries,
            Some(CloudProviderKind::Onedrive),
        )
        .expect("conflicts");

        assert_eq!(conflicts.len(), 1);
        assert_eq!(conflicts[0].src, src_file.to_string());
        // Preview target remains the requested path casing from source leaf.
        assert_eq!(conflicts[0].target, "rclone://work/dest/report.txt");
        assert!(!conflicts[0].is_dir);
    }

    #[test]
    fn cloud_listing_cache_prunes_stale_entries_and_invalidates_parent_subtree() {
        let now = Instant::now();
        let stale = now - (super::CLOUD_DIR_LISTING_STALE_MAX_AGE + Duration::from_millis(1));
        let fresh = now - Duration::from_millis(10);
        let mut cache = HashMap::new();
        cache.insert(
            "rclone://work/stale".to_string(),
            CachedCloudDirListing {
                fetched_at: stale,
                entries: Vec::new(),
            },
        );
        cache.insert(
            "rclone://work/docs".to_string(),
            CachedCloudDirListing {
                fetched_at: fresh,
                entries: Vec::new(),
            },
        );
        cache.insert(
            "rclone://work/docs/subdir".to_string(),
            CachedCloudDirListing {
                fetched_at: fresh,
                entries: Vec::new(),
            },
        );
        cache.insert(
            "rclone://work/docs/subdir/deeper".to_string(),
            CachedCloudDirListing {
                fetched_at: fresh,
                entries: Vec::new(),
            },
        );
        cache.insert(
            "rclone://work/other".to_string(),
            CachedCloudDirListing {
                fetched_at: fresh,
                entries: Vec::new(),
            },
        );

        prune_cloud_dir_listing_cache_locked(&mut cache, now);
        assert!(!cache.contains_key("rclone://work/stale"));
        assert!(cache.contains_key("rclone://work/docs"));

        {
            let mut global = cloud_dir_listing_cache().lock().expect("cache lock");
            global.clear();
            global.extend(cache);
        }

        let file_path = CloudPath::parse("rclone://work/docs/file.txt").expect("file path");
        invalidate_cloud_dir_listing_cache_for_write_paths(&[file_path]);

        let global = cloud_dir_listing_cache().lock().expect("cache lock");
        assert!(!global.contains_key("rclone://work/docs"));
        assert!(!global.contains_key("rclone://work/docs/subdir"));
        assert!(!global.contains_key("rclone://work/docs/subdir/deeper"));
        assert!(global.contains_key("rclone://work/other"));
        drop(global);

        let mut global = cloud_dir_listing_cache().lock().expect("cache lock");
        global.clear();
    }

    #[test]
    fn stale_cloud_listing_returns_cached_entries_and_refreshes_in_background() {
        let _guard = cloud_listing_test_lock().lock().expect("test lock");
        clear_cloud_listing_test_state();

        let path = CloudPath::parse("rclone://work/docs").expect("cloud path");
        let path_key = path.to_string();
        let stale_entries = vec![sample_cloud_file(
            "rclone://work/docs/stale.txt",
            "stale.txt",
        )];
        let refreshed_entries = vec![sample_cloud_file(
            "rclone://work/docs/fresh.txt",
            "fresh.txt",
        )];
        let refresh_calls = Arc::new(AtomicUsize::new(0));
        let refresh_calls_for_hook = Arc::clone(&refresh_calls);
        let refreshed_entries_for_hook = refreshed_entries.clone();

        {
            let mut cache = cloud_dir_listing_cache().lock().expect("cache lock");
            cache.insert(
                path_key.clone(),
                CachedCloudDirListing {
                    fetched_at: Instant::now()
                        - (super::CLOUD_DIR_LISTING_CACHE_TTL + Duration::from_millis(5)),
                    entries: stale_entries.clone(),
                },
            );
        }

        set_cloud_dir_listing_refresh_test_hook(Some(Arc::new(move |_| {
            refresh_calls_for_hook.fetch_add(1, Ordering::SeqCst);
            Ok(refreshed_entries_for_hook.clone())
        })));

        let result = list_cloud_dir_cached(&path).expect("stale cache hit should succeed");
        assert_eq!(result[0].name, "stale.txt");

        let deadline = Instant::now() + Duration::from_secs(1);
        loop {
            if refresh_calls.load(Ordering::SeqCst) == 1 {
                let cache = cloud_dir_listing_cache().lock().expect("cache lock");
                if let Some(updated) = cache.get(&path_key) {
                    if updated
                        .entries
                        .iter()
                        .any(|entry| entry.name == "fresh.txt")
                    {
                        break;
                    }
                }
            }
            assert!(
                Instant::now() < deadline,
                "background refresh did not complete in time"
            );
            thread::sleep(Duration::from_millis(10));
        }

        let refreshed = list_cloud_dir_cached(&path).expect("refreshed cache hit should succeed");
        assert_eq!(refreshed[0].name, "fresh.txt");

        clear_cloud_listing_test_state();
    }

    #[test]
    fn stale_cloud_listing_deduplicates_background_refresh_per_path() {
        let _guard = cloud_listing_test_lock().lock().expect("test lock");
        clear_cloud_listing_test_state();

        let path = CloudPath::parse("rclone://work/dedupe").expect("cloud path");
        let path_key = path.to_string();
        let refresh_calls = Arc::new(AtomicUsize::new(0));
        let refresh_calls_for_hook = Arc::clone(&refresh_calls);

        {
            let mut cache = cloud_dir_listing_cache().lock().expect("cache lock");
            cache.insert(
                path_key.clone(),
                CachedCloudDirListing {
                    fetched_at: Instant::now()
                        - (super::CLOUD_DIR_LISTING_CACHE_TTL + Duration::from_millis(5)),
                    entries: vec![sample_cloud_file("rclone://work/dedupe/old.txt", "old.txt")],
                },
            );
        }

        set_cloud_dir_listing_refresh_test_hook(Some(Arc::new(move |_| {
            refresh_calls_for_hook.fetch_add(1, Ordering::SeqCst);
            thread::sleep(Duration::from_millis(60));
            Ok(vec![sample_cloud_file(
                "rclone://work/dedupe/new.txt",
                "new.txt",
            )])
        })));

        let first = list_cloud_dir_cached(&path).expect("first stale cache hit");
        let second = list_cloud_dir_cached(&path).expect("second stale cache hit");
        assert_eq!(first[0].name, "old.txt");
        assert_eq!(second[0].name, "old.txt");

        let deadline = Instant::now() + Duration::from_secs(1);
        loop {
            if refresh_calls.load(Ordering::SeqCst) == 1
                && !cloud_dir_listing_refresh_inflight()
                    .lock()
                    .expect("inflight lock")
                    .contains(&path_key)
            {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "deduplicated refresh did not settle in time"
            );
            thread::sleep(Duration::from_millis(10));
        }

        assert_eq!(refresh_calls.load(Ordering::SeqCst), 1);

        clear_cloud_listing_test_state();
    }

    #[test]
    fn remote_permit_limiter_bounds_same_remote_concurrency() {
        let barrier = Arc::new(Barrier::new(4));
        let active = Arc::new(AtomicUsize::new(0));
        let max_seen = Arc::new(AtomicUsize::new(0));
        let mut handles = Vec::new();

        for _ in 0..3 {
            let barrier = Arc::clone(&barrier);
            let active = Arc::clone(&active);
            let max_seen = Arc::clone(&max_seen);
            handles.push(thread::spawn(move || {
                barrier.wait();
                let _permit = acquire_cloud_remote_permits(vec!["test-remote".to_string()]);
                let current = active.fetch_add(1, Ordering::SeqCst) + 1;
                loop {
                    let prev = max_seen.load(Ordering::SeqCst);
                    if current <= prev {
                        break;
                    }
                    if max_seen
                        .compare_exchange(prev, current, Ordering::SeqCst, Ordering::SeqCst)
                        .is_ok()
                    {
                        break;
                    }
                }
                thread::sleep(Duration::from_millis(40));
                active.fetch_sub(1, Ordering::SeqCst);
            }));
        }

        barrier.wait();
        for handle in handles {
            handle.join().expect("thread should finish");
        }

        assert!(max_seen.load(Ordering::SeqCst) <= super::CLOUD_REMOTE_MAX_CONCURRENT_OPS);
    }

    #[test]
    fn rate_limited_result_applies_remote_cooldown() {
        let remote = unique_test_remote("cooldown-remote");

        let result: Result<(), CloudCommandError> =
            with_cloud_remote_permits(vec![remote.clone()], || {
                Err(CloudCommandError::new(
                    CloudCommandErrorCode::RateLimited,
                    "provider rate limit",
                ))
            });
        let err = result.expect_err("expected rate-limited error");
        assert_eq!(err.code(), CloudCommandErrorCode::RateLimited);

        let limiter = cloud_remote_op_limiter();
        let state = limiter.state.lock().expect("limiter lock");
        let deadline = state
            .cooldown_until
            .get(&remote)
            .copied()
            .expect("cooldown deadline should be set");
        assert!(deadline > Instant::now());
    }

    #[test]
    fn acquire_remote_permit_waits_for_active_cooldown_window() {
        let remote = unique_test_remote("cooldown-wait-remote");
        let limiter = cloud_remote_op_limiter();
        {
            let mut state = limiter.state.lock().expect("limiter lock");
            state
                .cooldown_until
                .insert(remote.clone(), Instant::now() + Duration::from_millis(120));
        }
        let started = Instant::now();
        let _permit = acquire_cloud_remote_permits(vec![remote]);
        let waited = started.elapsed();
        assert!(
            waited >= Duration::from_millis(100),
            "expected to wait for cooldown, waited only {:?}",
            waited
        );
    }
}
