use super::{CloudEntry, CloudPath, CloudRemote};
use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

pub(super) const CLOUD_REMOTE_DISCOVERY_CACHE_TTL: Duration = Duration::from_secs(45);
pub(super) const CLOUD_DIR_LISTING_CACHE_TTL: Duration = Duration::from_secs(20);
pub(super) const CLOUD_DIR_LISTING_STALE_MAX_AGE: Duration = Duration::from_secs(60);

#[derive(Debug, Clone)]
pub(super) struct CachedCloudRemoteDiscovery {
    pub(super) fetched_at: Instant,
    pub(super) remotes: Vec<CloudRemote>,
}

#[derive(Debug, Clone)]
pub(super) struct CachedCloudDirListing {
    pub(super) fetched_at: Instant,
    pub(super) entries: Vec<CloudEntry>,
}

#[derive(Debug, Clone)]
pub(super) enum CloudDirListingCacheLookup {
    Fresh(CachedCloudDirListing),
    Stale(CachedCloudDirListing),
}

pub(super) fn cloud_remote_discovery_cache() -> &'static Mutex<Option<CachedCloudRemoteDiscovery>> {
    static CACHE: OnceLock<Mutex<Option<CachedCloudRemoteDiscovery>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(None))
}

pub(super) fn cloud_dir_listing_cache() -> &'static Mutex<HashMap<String, CachedCloudDirListing>> {
    static CACHE: OnceLock<Mutex<HashMap<String, CachedCloudDirListing>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

pub(super) fn cloud_dir_listing_refresh_inflight() -> &'static Mutex<HashSet<String>> {
    static INFLIGHT: OnceLock<Mutex<HashSet<String>>> = OnceLock::new();
    INFLIGHT.get_or_init(|| Mutex::new(HashSet::new()))
}

pub(super) fn prune_cloud_dir_listing_cache_locked(
    cache: &mut HashMap<String, CachedCloudDirListing>,
    now: Instant,
) {
    cache.retain(|_, cached| {
        now.duration_since(cached.fetched_at) <= CLOUD_DIR_LISTING_STALE_MAX_AGE
    });
}

pub(super) fn lookup_cloud_dir_listing_cache_locked(
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

pub(super) fn store_cloud_dir_listing_cache_entry(
    key: String,
    fetched_at: Instant,
    entries: Vec<CloudEntry>,
) {
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

pub(super) fn invalidate_cloud_dir_listing_cache_path_locked(
    cache: &mut HashMap<String, CachedCloudDirListing>,
    path: &CloudPath,
) {
    let key = path.to_string();
    let subtree_prefix = format!("{key}/");
    cache.retain(|cached_path, _| cached_path != &key && !cached_path.starts_with(&subtree_prefix));
}
