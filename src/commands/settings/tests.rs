use super::{
    load_cloud_enabled, load_cloud_thumbs, store_cloud_enabled, store_cloud_thumbs,
    store_rclone_path,
};
use crate::commands::cloud::{
    cloud_dir_listing_cache_contains_for_tests,
    cloud_remote_discovery_cache_contains_remote_for_tests,
    path::CloudPath,
    store_cloud_dir_listing_cache_entry_for_tests,
    store_cloud_remote_discovery_cache_entry_for_tests,
    types::{CloudCapabilities, CloudProviderKind, CloudRemote},
};
use once_cell::sync::Lazy;
use std::{
    ffi::OsString,
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
    sync::Mutex,
};

static TEST_ENV_LOCK: Lazy<Mutex<()>> = Lazy::new(|| Mutex::new(()));

struct TempDataHomeGuard {
    previous: Option<OsString>,
    dir: PathBuf,
}

impl Drop for TempDataHomeGuard {
    fn drop(&mut self) {
        match &self.previous {
            Some(value) => std::env::set_var("XDG_DATA_HOME", value),
            None => std::env::remove_var("XDG_DATA_HOME"),
        }
        let _ = fs::remove_dir_all(&self.dir);
    }
}

fn temp_data_home_guard() -> TempDataHomeGuard {
    static NEXT_ID: AtomicU64 = AtomicU64::new(1);
    let dir = std::env::temp_dir().join(format!(
        "browsey-settings-test-data-{}-{}",
        std::process::id(),
        NEXT_ID.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&dir).expect("create temp data dir");
    let previous = std::env::var_os("XDG_DATA_HOME");
    std::env::set_var("XDG_DATA_HOME", &dir);
    TempDataHomeGuard { previous, dir }
}

#[test]
fn store_rclone_path_invalidates_cloud_caches() {
    let _lock = TEST_ENV_LOCK.lock().expect("settings test env lock");
    let _data_home = temp_data_home_guard();
    static NEXT_REMOTE_ID: AtomicU64 = AtomicU64::new(1);
    let remote_id = format!(
        "work-{}-{}",
        std::process::id(),
        NEXT_REMOTE_ID.fetch_add(1, Ordering::Relaxed)
    );
    let path = CloudPath::parse(&format!("rclone://{remote_id}/docs")).expect("cloud path");
    store_cloud_remote_discovery_cache_entry_for_tests(vec![CloudRemote {
        id: remote_id.clone(),
        label: "Work".to_string(),
        provider: CloudProviderKind::Onedrive,
        root_path: format!("rclone://{remote_id}"),
        capabilities: CloudCapabilities::v1_core_rw(),
    }]);
    store_cloud_dir_listing_cache_entry_for_tests(&path, Vec::new());

    assert!(cloud_remote_discovery_cache_contains_remote_for_tests(
        &remote_id
    ));
    assert!(cloud_dir_listing_cache_contains_for_tests(&path));

    store_rclone_path("/usr/bin/rclone-does-not-exist".to_string()).expect("store rclone path");

    assert!(!cloud_remote_discovery_cache_contains_remote_for_tests(
        &remote_id
    ));
    assert!(!cloud_dir_listing_cache_contains_for_tests(&path));
}

#[test]
fn store_cloud_thumbs_roundtrip() {
    let _lock = TEST_ENV_LOCK.lock().expect("settings test env lock");
    let _data_home = temp_data_home_guard();

    assert_eq!(
        load_cloud_thumbs().expect("load default cloud thumbs"),
        None
    );

    store_cloud_thumbs(true).expect("store cloud thumbs true");
    assert_eq!(
        load_cloud_thumbs().expect("load cloud thumbs true"),
        Some(true)
    );

    store_cloud_thumbs(false).expect("store cloud thumbs false");
    assert_eq!(
        load_cloud_thumbs().expect("load cloud thumbs false"),
        Some(false)
    );
}

#[test]
fn store_cloud_enabled_roundtrip_and_invalidates_cloud_caches() {
    let _lock = TEST_ENV_LOCK.lock().expect("settings test env lock");
    let _data_home = temp_data_home_guard();
    static NEXT_REMOTE_ID: AtomicU64 = AtomicU64::new(1000);
    let remote_id = format!(
        "work-cloud-enabled-{}-{}",
        std::process::id(),
        NEXT_REMOTE_ID.fetch_add(1, Ordering::Relaxed)
    );
    let path = CloudPath::parse(&format!("rclone://{remote_id}/docs")).expect("cloud path");
    store_cloud_remote_discovery_cache_entry_for_tests(vec![CloudRemote {
        id: remote_id.clone(),
        label: "Work".to_string(),
        provider: CloudProviderKind::Onedrive,
        root_path: format!("rclone://{remote_id}"),
        capabilities: CloudCapabilities::v1_core_rw(),
    }]);
    store_cloud_dir_listing_cache_entry_for_tests(&path, Vec::new());

    assert_eq!(
        load_cloud_enabled().expect("load default cloudEnabled"),
        None
    );
    assert!(cloud_remote_discovery_cache_contains_remote_for_tests(
        &remote_id
    ));
    assert!(cloud_dir_listing_cache_contains_for_tests(&path));

    store_cloud_enabled(false).expect("store cloudEnabled false");

    assert_eq!(
        load_cloud_enabled().expect("load cloudEnabled false"),
        Some(false)
    );
    assert!(!cloud_remote_discovery_cache_contains_remote_for_tests(
        &remote_id
    ));
    assert!(!cloud_dir_listing_cache_contains_for_tests(&path));

    store_cloud_enabled(true).expect("store cloudEnabled true");
    assert_eq!(
        load_cloud_enabled().expect("load cloudEnabled true"),
        Some(true)
    );
}
