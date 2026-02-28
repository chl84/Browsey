use super::{
    error::{CloudCommandError, CloudCommandErrorCode, CloudCommandResult},
    events::emit_cloud_dir_refreshed,
    limits::{acquire_cloud_remote_permits, note_remote_rate_limit_cooldown},
    path::CloudPath,
    provider::CloudProvider,
    providers::rclone::RcloneCloudProvider,
    types::{CloudEntry, CloudRemote},
};
use crate::runtime_lifecycle;
use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};
use tracing::debug;

const CLOUD_REMOTE_DISCOVERY_CACHE_TTL: Duration = Duration::from_secs(45);
const CLOUD_DIR_LISTING_CACHE_TTL: Duration = Duration::from_secs(20);
const CLOUD_DIR_LISTING_STALE_MAX_AGE: Duration = Duration::from_secs(60);
const CLOUD_DIR_LISTING_RETRY_BACKOFFS_MS: &[u64] = &[150, 400];

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

#[derive(Debug, Clone)]
enum CloudDirListingCacheLookup {
    Fresh(CachedCloudDirListing),
    Stale(CachedCloudDirListing),
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

pub(crate) fn list_cloud_remotes_cached(
    force_refresh: bool,
) -> CloudCommandResult<Vec<CloudRemote>> {
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

pub(crate) fn list_cloud_dir_cached(path: &CloudPath) -> CloudCommandResult<Vec<CloudEntry>> {
    list_cloud_dir_cached_with_refresh_event(path, None)
}

pub(crate) fn list_cloud_dir_cached_with_refresh_event(
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
                    debug!(
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
                    debug!(
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

pub(crate) fn invalidate_cloud_dir_listing_cache_for_write_paths(paths: &[CloudPath]) {
    if let Ok(mut guard) = cloud_dir_listing_cache().lock() {
        for path in paths {
            invalidate_cloud_dir_listing_cache_path_locked(&mut guard, path);
            if let Some(parent) = path.parent_dir_path() {
                invalidate_cloud_dir_listing_cache_path_locked(&mut guard, &parent);
            }
        }
    }
}

#[cfg(test)]
pub(crate) fn store_cloud_dir_listing_cache_entry_for_tests(
    path: &CloudPath,
    entries: Vec<CloudEntry>,
) {
    store_cloud_dir_listing_cache_entry(path.to_string(), Instant::now(), entries);
}

#[cfg(test)]
pub(crate) fn cloud_dir_listing_cache_contains_for_tests(path: &CloudPath) -> bool {
    match cloud_dir_listing_cache().lock() {
        Ok(guard) => guard.contains_key(&path.to_string()),
        Err(poisoned) => poisoned.into_inner().contains_key(&path.to_string()),
    }
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

fn invalidate_cloud_dir_listing_cache_path_locked(
    cache: &mut HashMap<String, CachedCloudDirListing>,
    path: &CloudPath,
) {
    let key = path.to_string();
    let subtree_prefix = format!("{key}/");
    cache.retain(|cached_path, _| cached_path != &key && !cached_path.starts_with(&subtree_prefix));
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

#[cfg(test)]
mod tests {
    use super::{
        cloud_dir_listing_cache, cloud_dir_listing_refresh_inflight,
        invalidate_cloud_dir_listing_cache_for_write_paths, list_cloud_dir_cached,
        prune_cloud_dir_listing_cache_locked, set_cloud_dir_listing_refresh_test_hook,
        CachedCloudDirListing, CLOUD_DIR_LISTING_CACHE_TTL, CLOUD_DIR_LISTING_STALE_MAX_AGE,
    };
    use crate::commands::cloud::{
        path::CloudPath,
        types::{CloudCapabilities, CloudEntry, CloudEntryKind},
    };
    use std::{
        collections::HashMap,
        sync::{
            atomic::{AtomicUsize, Ordering},
            Arc, Mutex, OnceLock,
        },
        thread,
        time::{Duration, Instant},
    };

    fn cloud_listing_test_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    fn lock_cloud_listing_test_state() -> std::sync::MutexGuard<'static, ()> {
        match cloud_listing_test_lock().lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        }
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
    fn cloud_listing_cache_prunes_stale_entries_and_invalidates_parent_subtree() {
        let _guard = lock_cloud_listing_test_state();
        let now = Instant::now();
        let stale = now - (CLOUD_DIR_LISTING_STALE_MAX_AGE + Duration::from_millis(1));
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
        let _guard = lock_cloud_listing_test_state();
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
                        - (CLOUD_DIR_LISTING_CACHE_TTL + Duration::from_millis(5)),
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

        let deadline = Instant::now() + Duration::from_secs(2);
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
        let _guard = lock_cloud_listing_test_state();
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
                        - (CLOUD_DIR_LISTING_CACHE_TTL + Duration::from_millis(5)),
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

        let deadline = Instant::now() + Duration::from_secs(2);
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
}
