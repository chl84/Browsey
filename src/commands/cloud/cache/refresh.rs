use super::{
    cloud_dir_listing_refresh_inflight, store_cloud_dir_listing_cache_entry, CloudCommandError,
    CloudCommandErrorCode, CloudCommandResult, CloudEntry, CloudPath,
};
use crate::commands::cloud::events::emit_cloud_dir_refreshed;
use crate::commands::cloud::limits::{
    acquire_cloud_remote_permits, note_remote_rate_limit_cooldown,
};
use crate::commands::cloud::provider::CloudProvider;
use crate::runtime_lifecycle;
#[cfg(test)]
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use tracing::debug;

pub(super) const CLOUD_DIR_LISTING_RETRY_BACKOFFS_MS: &[u64] = &[150, 400];

pub(super) fn schedule_cloud_dir_listing_refresh(
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
                    emit_cloud_dir_refreshed(app, &path, entries.len());
                }
                debug!(
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

pub(super) fn list_cloud_dir_with_retry(path: &CloudPath) -> CloudCommandResult<Vec<CloudEntry>> {
    let permit_started = Instant::now();
    let guard = acquire_cloud_remote_permits(vec![path.remote().to_string()]);
    let permit_wait_ms = permit_started.elapsed().as_millis() as u64;
    let provider =
        crate::commands::cloud::configured_rclone_provider().map_err(CloudCommandError::from)?;
    let fetch_started = Instant::now();
    let mut attempt = 0usize;
    loop {
        match provider.list_dir(path) {
            Ok(entries) => {
                debug!(
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

#[cfg(test)]
type CloudDirRefreshHook =
    std::sync::Arc<dyn Fn(&CloudPath) -> CloudCommandResult<Vec<CloudEntry>> + Send + Sync>;

#[cfg(test)]
fn cloud_dir_listing_refresh_test_hook() -> &'static Mutex<Option<CloudDirRefreshHook>> {
    static HOOK: OnceLock<Mutex<Option<CloudDirRefreshHook>>> = OnceLock::new();
    HOOK.get_or_init(|| Mutex::new(None))
}

#[cfg(test)]
pub(super) fn set_cloud_dir_listing_refresh_test_hook(hook: Option<CloudDirRefreshHook>) {
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
