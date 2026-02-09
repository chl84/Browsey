#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod clipboard;
mod commands;
mod context_menu;
mod db;
mod entry;
mod fs_utils;
mod icons;
mod metadata;
mod sorting;
mod statusbar;
mod undo;
mod watcher;

use std::io::Write;

use commands::*;
use context_menu::context_menu_actions;
use fs_utils::debug_log;
use once_cell::sync::OnceCell;
use statusbar::dir_sizes;
use undo::{redo_action, undo_action, UndoState};
use watcher::WatchState;

const MAX_LOG_BYTES: u64 = 10 * 1024 * 1024; // 10 MiB

struct SizeLimitedWriter {
    file: std::fs::File,
    path: std::path::PathBuf,
    max_bytes: u64,
}

impl SizeLimitedWriter {
    fn new(path: std::path::PathBuf, max_bytes: u64) -> std::io::Result<Self> {
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)?;
        Ok(Self {
            file,
            path,
            max_bytes,
        })
    }

    fn rotate_if_needed(&mut self) {
        if let Ok(meta) = self.file.metadata() {
            if meta.len() < self.max_bytes {
                return;
            }
        }
        let _ = self.file.flush();
        let rotated = self.path.with_extension("log.1");
        let _ = std::fs::remove_file(&rotated);
        let _ = std::fs::rename(&self.path, &rotated);
        if let Ok(new_file) = std::fs::OpenOptions::new()
            .create(true)
            .truncate(true)
            .open(&self.path)
        {
            self.file = new_file;
        }
    }
}

impl std::io::Write for SizeLimitedWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        self.rotate_if_needed();
        self.file.write(buf)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.file.flush()
    }
}

fn init_logging() {
    static GUARD: OnceCell<tracing_appender::non_blocking::WorkerGuard> = OnceCell::new();
    let base = dirs_next::data_dir().unwrap_or_else(|| std::env::temp_dir());
    let log_dir = base.join("browsey").join("logs");
    if let Err(e) = std::fs::create_dir_all(&log_dir) {
        eprintln!("Failed to create log dir {:?}: {}", log_dir, e);
        return;
    }
    let writer = match SizeLimitedWriter::new(log_dir.join("browsey.log"), MAX_LOG_BYTES) {
        Ok(w) => w,
        Err(e) => {
            eprintln!("Failed to open log file: {e}");
            return;
        }
    };
    let (non_blocking, guard) =
        tracing_appender::non_blocking::NonBlockingBuilder::default().finish(writer);
    let _ = GUARD.set(guard);
    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive("info".parse().unwrap()),
        )
        .with_ansi(false)
        .with_writer(non_blocking);
    if let Err(e) = subscriber.try_init() {
        eprintln!("Failed to init tracing subscriber: {e}");
    }

    debug_log(&format!(
        "logging initialized: log_dir={:?} temp_dir={:?} cwd={:?}",
        log_dir,
        std::env::temp_dir(),
        std::env::current_dir().ok()
    ));
}

#[cfg(target_os = "linux")]
fn apply_webview_rendering_policy_from_settings() {
    let hardware_acceleration = db::open()
        .ok()
        .and_then(|conn| {
            db::get_setting_bool(&conn, "hardwareAcceleration")
                .ok()
                .flatten()
        })
        .unwrap_or(true);

    if !hardware_acceleration {
        // Keep compositing enabled, but disable the DMA-BUF renderer to reduce artifacts.
        std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
    }
}

#[cfg(not(target_os = "linux"))]
fn apply_webview_rendering_policy_from_settings() {}

fn main() {
    init_logging();
    apply_webview_rendering_policy_from_settings();
    undo::cleanup_stale_backups(None);
    tauri::Builder::default()
        .plugin(tauri_plugin_drag::init())
        .manage(WatchState::default())
        .manage(CancelState::default())
        .manage(UndoState::default())
        .setup(|app| {
            for window in &app.config().app.windows {
                if window.create {
                    continue;
                }
                tauri::WebviewWindowBuilder::from_config(app, window)?
                    .enable_clipboard_access()
                    .build()?;
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_dir,
            list_mounts,
            get_bookmarks,
            add_bookmark,
            remove_bookmark,
            watch_dir,
            open_entry,
            list_open_with_apps,
            open_with,
            toggle_star,
            list_starred,
            remove_recent,
            list_recent,
            list_trash,
            store_column_widths,
            load_saved_column_widths,
            store_show_hidden,
            load_show_hidden,
            store_hidden_files_last,
            load_hidden_files_last,
            store_default_view,
            load_default_view,
            store_start_dir,
            load_start_dir,
            store_folders_first,
            load_folders_first,
            store_confirm_delete,
            load_confirm_delete,
            store_sort_field,
            load_sort_field,
            store_sort_direction,
            load_sort_direction,
            store_archive_name,
            load_archive_name,
            store_archive_level,
            load_archive_level,
            store_open_dest_after_extract,
            load_open_dest_after_extract,
            store_ffmpeg_path,
            load_ffmpeg_path,
            store_thumb_cache_mb,
            load_thumb_cache_mb,
            store_mounts_poll_ms,
            load_mounts_poll_ms,
            store_video_thumbs,
            load_video_thumbs,
            store_hardware_acceleration,
            load_hardware_acceleration,
            store_density,
            load_density,
            dir_sizes,
            eject_drive,
            mount_partition,
            context_menu_actions,
            rename_entry,
            rename_entries,
            move_to_trash,
            move_to_trash_many,
            create_file,
            delete_entry,
            delete_entries,
            entry_times_cmd,
            entry_kind_cmd,
            entry_extra_metadata_cmd,
            set_hidden,
            extract_archive,
            extract_archives,
            open_console,
            set_clipboard_cmd,
            copy_paths_to_system_clipboard,
            system_clipboard_paths,
            clear_system_clipboard,
            paste_clipboard_cmd,
            paste_clipboard_preview,
            search_stream,
            restore_trash_items,
            purge_trash_items,
            create_folder,
            compress_entries,
            check_duplicates,
            check_duplicates_stream,
            cancel_task,
            get_permissions,
            set_permissions,
            undo_action,
            redo_action,
            get_thumbnail
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
