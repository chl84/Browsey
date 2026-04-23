use super::{
    load_archive_level, load_archive_name, load_cloud_enabled, load_cloud_thumbs,
    load_confirm_delete, load_default_view, load_density, load_double_click_ms, load_ffmpeg_path,
    load_folders_first, load_hardware_acceleration, load_hidden_files_last, load_high_contrast,
    load_log_level, load_mounts_poll_ms, load_open_dest_after_extract, load_rclone_path,
    load_scrollbar_width, load_show_hidden, load_sort_direction, load_sort_field, load_start_dir,
    load_thumb_cache_mb, load_video_thumbs, store_archive_level, store_archive_name,
    store_cloud_enabled, store_cloud_thumbs, store_confirm_delete, store_default_view,
    store_density, store_double_click_ms, store_ffmpeg_path, store_folders_first,
    store_hardware_acceleration, store_hidden_files_last, store_high_contrast, store_log_level,
    store_mounts_poll_ms, store_open_dest_after_extract, store_rclone_path, store_scrollbar_width,
    store_show_hidden, store_sort_direction, store_sort_field, store_start_dir,
    store_thumb_cache_mb, store_video_thumbs,
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

    crate::db::set_setting_string(&conn, "defaultView", "columns")
        .expect("seed invalid defaultView");
    crate::db::set_setting_string(&conn, "density", "roomy").expect("seed invalid density");
    crate::db::set_setting_string(&conn, "sortField", "ctime").expect("seed invalid sortField");
    crate::db::set_setting_string(&conn, "sortDirection", "up")
        .expect("seed invalid sortDirection");

    assert_eq!(load_default_view().expect("load invalid defaultView"), None);
    assert_eq!(load_density().expect("load invalid density"), None);
    assert_eq!(load_sort_field().expect("load invalid sortField"), None);
    assert_eq!(
        load_sort_direction().expect("load invalid sortDirection"),
        None
    );
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

#[test]
fn linux_settings_surface_roundtrips_through_backend_commands() {
    let _lock = TEST_ENV_LOCK.lock().expect("settings test env lock");
    let _data_home = temp_data_home_guard();

    store_show_hidden(true).expect("store showHidden");
    store_hidden_files_last(true).expect("store hiddenFilesLast");
    store_high_contrast(true).expect("store highContrast");
    store_folders_first(false).expect("store foldersFirst");
    store_default_view("grid".to_string()).expect("store defaultView");
    store_start_dir("/tmp/browsey".to_string()).expect("store startDir");
    store_confirm_delete(false).expect("store confirmDelete");
    store_sort_field("size".to_string()).expect("store sortField");
    store_sort_direction("desc".to_string()).expect("store sortDirection");
    store_archive_name("Release.zip".to_string()).expect("store archiveName");
    store_density("compact".to_string()).expect("store density");
    store_archive_level(9).expect("store archiveLevel");
    store_open_dest_after_extract(true).expect("store openDestAfterExtract");
    store_video_thumbs(false).expect("store videoThumbs");
    store_cloud_thumbs(true).expect("store cloudThumbs");
    store_cloud_enabled(true).expect("store cloudEnabled");
    store_hardware_acceleration(true).expect("store hardwareAcceleration");
    store_ffmpeg_path("  /usr/local/bin/ffmpeg  ".to_string()).expect("store ffmpegPath");
    store_thumb_cache_mb(512).expect("store thumbCacheMb");
    store_mounts_poll_ms(2400).expect("store mountsPollMs");
    store_double_click_ms(450).expect("store doubleClickMs");
    store_scrollbar_width(14).expect("store scrollbarWidth");
    store_rclone_path("  /usr/local/bin/rclone  ".to_string()).expect("store rclonePath");

    assert_eq!(load_show_hidden().expect("load showHidden"), Some(true));
    assert_eq!(
        load_hidden_files_last().expect("load hiddenFilesLast"),
        Some(true)
    );
    assert_eq!(load_high_contrast().expect("load highContrast"), Some(true));
    assert_eq!(
        load_folders_first().expect("load foldersFirst"),
        Some(false)
    );
    assert_eq!(
        load_default_view().expect("load defaultView"),
        Some("grid".to_string())
    );
    assert_eq!(
        load_start_dir().expect("load startDir"),
        Some("/tmp/browsey".to_string())
    );
    assert_eq!(
        load_confirm_delete().expect("load confirmDelete"),
        Some(false)
    );
    assert_eq!(
        load_sort_field().expect("load sortField"),
        Some("size".to_string())
    );
    assert_eq!(
        load_sort_direction().expect("load sortDirection"),
        Some("desc".to_string())
    );
    assert_eq!(
        load_archive_name().expect("load archiveName"),
        Some("Release".to_string())
    );
    assert_eq!(
        load_density().expect("load density"),
        Some("compact".to_string())
    );
    assert_eq!(load_archive_level().expect("load archiveLevel"), Some(9));
    assert_eq!(
        load_open_dest_after_extract().expect("load openDestAfterExtract"),
        Some(true)
    );
    assert_eq!(load_video_thumbs().expect("load videoThumbs"), Some(false));
    assert_eq!(load_cloud_thumbs().expect("load cloudThumbs"), Some(true));
    assert_eq!(load_cloud_enabled().expect("load cloudEnabled"), Some(true));
    assert_eq!(
        load_hardware_acceleration().expect("load hardwareAcceleration"),
        Some(true)
    );
    assert_eq!(
        load_ffmpeg_path().expect("load ffmpegPath"),
        Some("/usr/local/bin/ffmpeg".to_string())
    );
    assert_eq!(load_thumb_cache_mb().expect("load thumbCacheMb"), Some(512));
    assert_eq!(
        load_mounts_poll_ms().expect("load mountsPollMs"),
        Some(2400)
    );
    assert_eq!(
        load_double_click_ms().expect("load doubleClickMs"),
        Some(450)
    );
    assert_eq!(
        load_scrollbar_width().expect("load scrollbarWidth"),
        Some(14)
    );
    assert_eq!(
        load_rclone_path().expect("load rclonePath"),
        Some("/usr/local/bin/rclone".to_string())
    );
}

#[test]
fn settings_migration_normalizes_and_prunes_legacy_values() {
    let _lock = TEST_ENV_LOCK.lock().expect("settings test env lock");
    let _data_home = temp_data_home_guard();
    let conn = crate::db::open().expect("open settings db");

    crate::db::set_setting_string(&conn, "settingsSchemaVersion", "0")
        .expect("seed old settings schema version");
    crate::db::set_setting_string(&conn, "archiveName", "  Legacy.zip  ")
        .expect("seed archiveName");
    crate::db::set_setting_string(&conn, "ffmpegPath", "  /usr/bin/ffmpeg  ")
        .expect("seed ffmpegPath");
    crate::db::set_setting_string(&conn, "rclonePath", "  /usr/bin/rclone  ")
        .expect("seed rclonePath");
    crate::db::set_setting_string(&conn, "logLevel", " INFO ").expect("seed logLevel");
    crate::db::set_setting_string(&conn, "density", "roomy").expect("seed invalid density");
    crate::db::set_setting_string(&conn, "showHidden", "yes").expect("seed invalid bool");
    crate::db::set_setting_string(&conn, "thumbCacheMb", " 512 ").expect("seed thumbCacheMb");
    crate::db::set_setting_string(&conn, "mountsPollMs", "12000")
        .expect("seed invalid mountsPollMs");

    let migrated = super::persistence::open_connection().expect("run settings migrations");

    assert_eq!(
        crate::db::get_setting_string(&migrated, "settingsSchemaVersion")
            .expect("load schema version"),
        Some("1".to_string())
    );
    assert_eq!(
        crate::db::get_setting_string(&migrated, "archiveName").expect("load archiveName"),
        Some("Legacy".to_string())
    );
    assert_eq!(
        crate::db::get_setting_string(&migrated, "ffmpegPath").expect("load ffmpegPath"),
        Some("/usr/bin/ffmpeg".to_string())
    );
    assert_eq!(
        crate::db::get_setting_string(&migrated, "rclonePath").expect("load rclonePath"),
        Some("/usr/bin/rclone".to_string())
    );
    assert_eq!(
        crate::db::get_setting_string(&migrated, "logLevel").expect("load logLevel"),
        Some("info".to_string())
    );
    assert_eq!(
        crate::db::get_setting_string(&migrated, "thumbCacheMb").expect("load thumbCacheMb"),
        Some("512".to_string())
    );
    assert_eq!(
        crate::db::get_setting_string(&migrated, "density").expect("load density"),
        None
    );
    assert_eq!(
        crate::db::get_setting_string(&migrated, "showHidden").expect("load showHidden"),
        None
    );
    assert_eq!(
        crate::db::get_setting_string(&migrated, "mountsPollMs").expect("load mountsPollMs"),
        None
    );
}
