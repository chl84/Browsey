use super::{
    load_archive_name, load_cloud_enabled, load_cloud_thumbs, load_default_view, load_density,
    load_double_click_ms, load_ffmpeg_path, load_hardware_acceleration, load_log_level,
    load_mounts_poll_ms, load_rclone_path, load_scrollbar_width, load_sort_direction,
    load_sort_field, store_cloud_enabled, store_cloud_thumbs, store_double_click_ms,
    store_hardware_acceleration, store_log_level, store_mounts_poll_ms, store_rclone_path,
    store_scrollbar_width,
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

#[test]
fn bounded_linux_ui_settings_roundtrip() {
    let _lock = TEST_ENV_LOCK.lock().expect("settings test env lock");
    let _data_home = temp_data_home_guard();

    assert_eq!(
        load_mounts_poll_ms().expect("load default mountsPollMs"),
        None
    );
    assert_eq!(
        load_double_click_ms().expect("load default doubleClickMs"),
        None
    );
    assert_eq!(
        load_scrollbar_width().expect("load default scrollbarWidth"),
        None
    );

    store_mounts_poll_ms(500).expect("store min mountsPollMs");
    store_double_click_ms(600).expect("store max doubleClickMs");
    store_scrollbar_width(16).expect("store max scrollbarWidth");

    assert_eq!(
        load_mounts_poll_ms().expect("load stored mountsPollMs"),
        Some(500)
    );
    assert_eq!(
        load_double_click_ms().expect("load stored doubleClickMs"),
        Some(600)
    );
    assert_eq!(
        load_scrollbar_width().expect("load stored scrollbarWidth"),
        Some(16)
    );
}

#[test]
fn bounded_linux_ui_settings_reject_invalid_store_values() {
    let _lock = TEST_ENV_LOCK.lock().expect("settings test env lock");
    let _data_home = temp_data_home_guard();

    let err = store_mounts_poll_ms(499).expect_err("mountsPollMs below range should fail");
    assert!(
        err.message.contains("mounts poll must be 500-10000 ms"),
        "unexpected mounts poll error: {:?}",
        err
    );
    assert_eq!(err.code, "invalid_input");

    let err = store_double_click_ms(149).expect_err("doubleClickMs below range should fail");
    assert!(
        err.message
            .contains("double click speed must be 150-600 ms"),
        "unexpected double click error: {:?}",
        err
    );
    assert_eq!(err.code, "invalid_input");

    let err = store_scrollbar_width(17).expect_err("scrollbarWidth above range should fail");
    assert!(
        err.message.contains("scrollbar width must be 6-16 px"),
        "unexpected scrollbar width error: {:?}",
        err
    );
    assert_eq!(err.code, "invalid_input");
}

#[test]
fn bounded_linux_ui_settings_ignore_legacy_or_malformed_db_values() {
    let _lock = TEST_ENV_LOCK.lock().expect("settings test env lock");
    let _data_home = temp_data_home_guard();
    let conn = crate::db::open().expect("open settings db");

    crate::db::set_setting_string(&conn, "mountsPollMs", "12000").expect("seed mountsPollMs");
    crate::db::set_setting_string(&conn, "doubleClickMs", "fast").expect("seed doubleClickMs");
    crate::db::set_setting_string(&conn, "scrollbarWidth", "5").expect("seed scrollbarWidth");

    assert_eq!(
        load_mounts_poll_ms().expect("load invalid mountsPollMs"),
        None
    );
    assert_eq!(
        load_double_click_ms().expect("load invalid doubleClickMs"),
        None
    );
    assert_eq!(
        load_scrollbar_width().expect("load invalid scrollbarWidth"),
        None
    );
}

#[test]
fn store_rclone_path_trims_and_roundtrips() {
    let _lock = TEST_ENV_LOCK.lock().expect("settings test env lock");
    let _data_home = temp_data_home_guard();

    assert_eq!(load_rclone_path().expect("load default rclonePath"), None);

    store_rclone_path("  /usr/local/bin/rclone  ".to_string()).expect("store trimmed rclonePath");

    assert_eq!(
        load_rclone_path().expect("load trimmed rclonePath"),
        Some("/usr/local/bin/rclone".to_string())
    );
}

#[test]
fn hardware_acceleration_roundtrips() {
    let _lock = TEST_ENV_LOCK.lock().expect("settings test env lock");
    let _data_home = temp_data_home_guard();

    assert_eq!(
        load_hardware_acceleration().expect("load default hardwareAcceleration"),
        None
    );

    store_hardware_acceleration(true).expect("store hardwareAcceleration true");
    assert_eq!(
        load_hardware_acceleration().expect("load hardwareAcceleration true"),
        Some(true)
    );

    store_hardware_acceleration(false).expect("store hardwareAcceleration false");
    assert_eq!(
        load_hardware_acceleration().expect("load hardwareAcceleration false"),
        Some(false)
    );
}

#[test]
fn load_log_level_normalizes_persisted_values() {
    let _lock = TEST_ENV_LOCK.lock().expect("settings test env lock");
    let _data_home = temp_data_home_guard();
    let conn = crate::db::open().expect("open settings db");

    assert_eq!(load_log_level().expect("load default logLevel"), None);

    crate::db::set_setting_string(&conn, "logLevel", "DEBUG").expect("seed uppercase logLevel");
    assert_eq!(
        load_log_level().expect("load normalized logLevel"),
        Some("debug".to_string())
    );

    crate::db::set_setting_string(&conn, "logLevel", " warn ").expect("seed trimmed logLevel");
    assert_eq!(
        load_log_level().expect("load trimmed logLevel"),
        Some("warn".to_string())
    );
}

#[test]
fn log_level_rejects_invalid_values_and_ignores_legacy_db_value() {
    let _lock = TEST_ENV_LOCK.lock().expect("settings test env lock");
    let _data_home = temp_data_home_guard();

    let err = store_log_level("verbose".to_string()).expect_err("invalid logLevel should fail");
    assert_eq!(err.code, "invalid_input");
    assert!(
        err.message.contains("invalid log level"),
        "unexpected log level error: {:?}",
        err
    );

    let conn = crate::db::open().expect("open settings db");
    crate::db::set_setting_string(&conn, "logLevel", "trace").expect("seed invalid logLevel");
    assert_eq!(
        load_log_level().expect("load legacy invalid logLevel"),
        None
    );
}

#[test]
fn enum_settings_ignore_legacy_invalid_values() {
    let _lock = TEST_ENV_LOCK.lock().expect("settings test env lock");
    let _data_home = temp_data_home_guard();
    let conn = crate::db::open().expect("open settings db");

    crate::db::set_setting_string(&conn, "defaultView", "columns").expect("seed invalid defaultView");
    crate::db::set_setting_string(&conn, "density", "roomy").expect("seed invalid density");
    crate::db::set_setting_string(&conn, "sortField", "ctime").expect("seed invalid sortField");
    crate::db::set_setting_string(&conn, "sortDirection", "up").expect("seed invalid sortDirection");

    assert_eq!(load_default_view().expect("load invalid defaultView"), None);
    assert_eq!(load_density().expect("load invalid density"), None);
    assert_eq!(load_sort_field().expect("load invalid sortField"), None);
    assert_eq!(load_sort_direction().expect("load invalid sortDirection"), None);
}

#[test]
fn trimmed_string_settings_normalize_when_loaded() {
    let _lock = TEST_ENV_LOCK.lock().expect("settings test env lock");
    let _data_home = temp_data_home_guard();
    let conn = crate::db::open().expect("open settings db");

    crate::db::set_setting_string(&conn, "archiveName", "Backup.zip").expect("seed archiveName");
    crate::db::set_setting_string(&conn, "ffmpegPath", "/usr/bin/ffmpeg").expect("seed ffmpegPath");

    assert_eq!(
        load_archive_name().expect("load normalized archiveName"),
        Some("Backup".to_string())
    );
    assert_eq!(
        load_ffmpeg_path().expect("load ffmpegPath"),
        Some("/usr/bin/ffmpeg".to_string())
    );
}
