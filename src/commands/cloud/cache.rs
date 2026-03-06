use super::{
    configured_rclone_provider,
    error::{CloudCommandError, CloudCommandErrorCode, CloudCommandResult},
    path::CloudPath,
    provider::CloudProvider,
    types::{CloudEntry, CloudRemote},
};
use std::time::Instant;
use tracing::debug;

mod refresh;
mod store;

#[cfg(test)]
use refresh::set_cloud_dir_listing_refresh_test_hook;
use refresh::{list_cloud_dir_with_retry, schedule_cloud_dir_listing_refresh};
use store::{
    cloud_dir_listing_cache, cloud_dir_listing_refresh_inflight, cloud_remote_discovery_cache,
    invalidate_cloud_dir_listing_cache_path_locked, lookup_cloud_dir_listing_cache_locked,
    prune_cloud_dir_listing_cache_locked, store_cloud_dir_listing_cache_entry,
    CachedCloudRemoteDiscovery, CloudDirListingCacheLookup, CLOUD_REMOTE_DISCOVERY_CACHE_TTL,
};
#[cfg(test)]
use store::{CachedCloudDirListing, CLOUD_DIR_LISTING_CACHE_TTL, CLOUD_DIR_LISTING_STALE_MAX_AGE};

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

    let provider = configured_rclone_provider().map_err(CloudCommandError::from)?;
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

pub(crate) fn invalidate_all_cloud_caches() {
    if let Ok(mut guard) = cloud_remote_discovery_cache().lock() {
        *guard = None;
    }
    if let Ok(mut guard) = cloud_dir_listing_cache().lock() {
        guard.clear();
    }
    if let Ok(mut inflight) = cloud_dir_listing_refresh_inflight().lock() {
        inflight.clear();
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

#[cfg(test)]
pub(crate) fn store_cloud_remote_discovery_cache_entry_for_tests(remotes: Vec<CloudRemote>) {
    let now = Instant::now();
    match cloud_remote_discovery_cache().lock() {
        Ok(mut guard) => {
            *guard = Some(CachedCloudRemoteDiscovery {
                fetched_at: now,
                remotes,
            });
        }
        Err(poisoned) => {
            let mut guard = poisoned.into_inner();
            *guard = Some(CachedCloudRemoteDiscovery {
                fetched_at: now,
                remotes,
            });
        }
    }
}

#[cfg(test)]
pub(crate) fn cloud_remote_discovery_cache_is_populated_for_tests() -> bool {
    match cloud_remote_discovery_cache().lock() {
        Ok(guard) => guard.is_some(),
        Err(poisoned) => poisoned.into_inner().is_some(),
    }
}

#[cfg(test)]
pub(crate) fn cloud_remote_discovery_cache_contains_remote_for_tests(remote_id: &str) -> bool {
    match cloud_remote_discovery_cache().lock() {
        Ok(guard) => guard
            .as_ref()
            .map(|cached| cached.remotes.iter().any(|remote| remote.id == remote_id))
            .unwrap_or(false),
        Err(poisoned) => poisoned
            .into_inner()
            .as_ref()
            .map(|cached| cached.remotes.iter().any(|remote| remote.id == remote_id))
            .unwrap_or(false),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        cloud_dir_listing_cache, cloud_dir_listing_refresh_inflight,
        cloud_remote_discovery_cache_is_populated_for_tests, invalidate_all_cloud_caches,
        invalidate_cloud_dir_listing_cache_for_write_paths, list_cloud_dir_cached,
        list_cloud_remotes_cached, prune_cloud_dir_listing_cache_locked,
        set_cloud_dir_listing_refresh_test_hook,
        store_cloud_remote_discovery_cache_entry_for_tests, CachedCloudDirListing,
        CLOUD_DIR_LISTING_CACHE_TTL, CLOUD_DIR_LISTING_STALE_MAX_AGE,
    };
    use crate::commands::cloud::{
        path::CloudPath,
        set_rclone_path_override_for_tests,
        types::{CloudCapabilities, CloudEntry, CloudEntryKind, CloudProviderKind, CloudRemote},
    };
    use crate::errors::domain::DomainError;
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
        invalidate_all_cloud_caches();
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
    fn invalidate_all_cloud_caches_clears_remote_and_listing_caches() {
        let _guard = lock_cloud_listing_test_state();
        clear_cloud_listing_test_state();

        let path = CloudPath::parse("rclone://work/docs").expect("cloud path");
        store_cloud_remote_discovery_cache_entry_for_tests(vec![CloudRemote {
            id: "work".to_string(),
            label: "Work".to_string(),
            provider: CloudProviderKind::Onedrive,
            root_path: "rclone://work".to_string(),
            capabilities: CloudCapabilities::v1_core_rw(),
        }]);
        super::store_cloud_dir_listing_cache_entry_for_tests(
            &path,
            vec![sample_cloud_file("rclone://work/docs/a.txt", "a.txt")],
        );

        assert!(cloud_remote_discovery_cache_is_populated_for_tests());
        assert!(super::cloud_dir_listing_cache_contains_for_tests(&path));

        invalidate_all_cloud_caches();

        assert!(!cloud_remote_discovery_cache_is_populated_for_tests());
        assert!(!super::cloud_dir_listing_cache_contains_for_tests(&path));
    }

    #[test]
    fn list_cloud_remotes_errors_for_invalid_explicit_rclone_path() {
        let _guard = lock_cloud_listing_test_state();
        clear_cloud_listing_test_state();
        set_rclone_path_override_for_tests(Some("/usr/bin/rclone-does-not-exist"));

        let error = list_cloud_remotes_cached(false).expect_err("invalid path should fail");
        assert_eq!(error.code_str(), "invalid_config");
        assert!(
            error
                .to_string()
                .contains("Configured Rclone path is invalid or not executable"),
            "unexpected error: {error}"
        );
        set_rclone_path_override_for_tests(None);
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
