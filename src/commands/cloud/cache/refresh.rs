use super::{
    cloud_dir_listing_refresh_inflight, store_cloud_dir_listing_cache_entry, CloudCommandError,
    CloudCommandErrorCode, CloudCommandResult, CloudEntry, CloudPath,
};
use crate::commands::cloud::events::emit_cloud_dir_refreshed;
use crate::commands::cloud::limits::{
    acquire_cloud_remote_permits, note_remote_rate_limit_cooldown,
};
use crate::commands::cloud::providers::rclone::RcloneReadOptions;
use crate::errors::domain::ErrorCode;
use crate::runtime_lifecycle;
use std::sync::atomic::AtomicBool;
#[cfg(test)]
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use tracing::{debug, warn};

pub(super) const CLOUD_DIR_LISTING_RETRY_BACKOFFS_MS: &[u64] = &[150, 400];
const CLOUD_INTERACTIVE_DIR_LISTING_RETRY_BACKOFFS_MS: &[u64] = &[150];
const CLOUD_INTERACTIVE_RC_READ_TIMEOUT: Duration = Duration::from_secs(10);
const CLOUD_INTERACTIVE_CLI_READ_TIMEOUT: Duration = Duration::from_secs(20);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum CloudDirReadMode {
    Interactive,
    BackgroundRefresh,
}

#[derive(Clone, Copy, Debug, Default)]
pub(super) struct CloudDirReadRequest<'a> {
    pub mode: Option<CloudDirReadMode>,
    pub cancel: Option<&'a AtomicBool>,
}

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
    list_cloud_dir_with_request(
        path,
        CloudDirReadRequest {
            mode: Some(CloudDirReadMode::BackgroundRefresh),
            cancel: None,
        },
    )
}

pub(super) fn list_cloud_dir_with_retry(path: &CloudPath) -> CloudCommandResult<Vec<CloudEntry>> {
    list_cloud_dir_with_request(
        path,
        CloudDirReadRequest {
            mode: Some(CloudDirReadMode::BackgroundRefresh),
            cancel: None,
        },
    )
}

pub(super) fn list_cloud_dir_interactive(
    path: &CloudPath,
    cancel: Option<&AtomicBool>,
) -> CloudCommandResult<Vec<CloudEntry>> {
    list_cloud_dir_with_request(
        path,
        CloudDirReadRequest {
            mode: Some(CloudDirReadMode::Interactive),
            cancel,
        },
    )
}

fn list_cloud_dir_with_request(
    path: &CloudPath,
    request: CloudDirReadRequest<'_>,
) -> CloudCommandResult<Vec<CloudEntry>> {
    let permit_started = Instant::now();
    let guard = acquire_cloud_remote_permits(vec![path.remote().to_string()]);
    let permit_wait_ms = permit_started.elapsed().as_millis() as u64;
    let provider =
        crate::commands::cloud::configured_rclone_provider().map_err(CloudCommandError::from)?;
    let fetch_started = Instant::now();
    let mode = request.mode.unwrap_or(CloudDirReadMode::BackgroundRefresh);
    let (retry_backoffs, read_options) = read_policy_for_request(request);
    let mut attempt = 0usize;
    loop {
        if is_cancelled(request.cancel) {
            return Err(cloud_listing_cancelled_error());
        }
        match provider.list_dir_with_read_options(path, read_options) {
            Ok(entries) => {
                debug!(
                    op = "cloud_list_entries",
                    phase = "backend_fetch",
                    path = %path,
                    mode = cloud_dir_read_mode_label(mode),
                    permit_wait_ms,
                    fetch_ms = fetch_started.elapsed().as_millis() as u64,
                    attempts = attempt + 1,
                    entry_count = entries.len(),
                    "cloud command phase timing"
                );
                return Ok(entries);
            }
            Err(error) if should_retry_cloud_dir_error(&error) => {
                let Some(backoff_ms) = retry_backoffs.get(attempt).copied() else {
                    if error.code() == CloudCommandErrorCode::RateLimited {
                        note_remote_rate_limit_cooldown(&guard.remotes);
                    }
                    warn!(
                        op = "cloud_list_entries",
                        phase = "backend_fetch",
                        path = %path,
                        mode = cloud_dir_read_mode_label(mode),
                        permit_wait_ms,
                        fetch_ms = fetch_started.elapsed().as_millis() as u64,
                        attempts = attempt + 1,
                        code = error.code().as_code_str(),
                        error = %error,
                        "cloud command failed after retries"
                    );
                    return Err(error);
                };
                attempt += 1;
                if is_cancelled(request.cancel) {
                    return Err(cloud_listing_cancelled_error());
                }
                debug!(
                    attempt,
                    backoff_ms,
                    path = %path,
                    mode = cloud_dir_read_mode_label(mode),
                    error = %error,
                    "retrying cloud directory listing after transient error"
                );
                std::thread::sleep(Duration::from_millis(backoff_ms));
            }
            Err(error) => {
                if error.code() == CloudCommandErrorCode::RateLimited {
                    note_remote_rate_limit_cooldown(&guard.remotes);
                }
                warn!(
                    op = "cloud_list_entries",
                    phase = "backend_fetch",
                    path = %path,
                    mode = cloud_dir_read_mode_label(mode),
                    permit_wait_ms,
                    fetch_ms = fetch_started.elapsed().as_millis() as u64,
                    attempts = attempt + 1,
                    code = error.code().as_code_str(),
                    error = %error,
                    "cloud command failed"
                );
                return Err(error);
            }
        }
    }
}

fn read_policy_for_request(
    request: CloudDirReadRequest<'_>,
) -> (&'static [u64], RcloneReadOptions<'_>) {
    match request.mode.unwrap_or(CloudDirReadMode::BackgroundRefresh) {
        CloudDirReadMode::Interactive => (
            CLOUD_INTERACTIVE_DIR_LISTING_RETRY_BACKOFFS_MS,
            RcloneReadOptions {
                cancel: request.cancel,
                rc_timeout: Some(CLOUD_INTERACTIVE_RC_READ_TIMEOUT),
                cli_timeout: Some(CLOUD_INTERACTIVE_CLI_READ_TIMEOUT),
                ..RcloneReadOptions::default()
            },
        ),
        CloudDirReadMode::BackgroundRefresh => (
            CLOUD_DIR_LISTING_RETRY_BACKOFFS_MS,
            RcloneReadOptions {
                cancel: request.cancel,
                rc_timeout: None,
                cli_timeout: None,
                ..RcloneReadOptions::default()
            },
        ),
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

fn is_cancelled(cancel: Option<&AtomicBool>) -> bool {
    cancel
        .map(|token| token.load(std::sync::atomic::Ordering::SeqCst))
        .unwrap_or(false)
}

fn cloud_listing_cancelled_error() -> CloudCommandError {
    CloudCommandError::new(
        CloudCommandErrorCode::Cancelled,
        "Cloud folder loading cancelled",
    )
}

fn cloud_dir_read_mode_label(mode: CloudDirReadMode) -> &'static str {
    match mode {
        CloudDirReadMode::Interactive => "interactive",
        CloudDirReadMode::BackgroundRefresh => "background_refresh",
    }
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
