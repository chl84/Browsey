#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod clipboard;
mod commands;
mod context_menu;
mod db;
mod entry;
mod fs_utils;
mod icons;
mod search;
mod sorting;
mod statusbar;
mod watcher;
mod undo;

use commands::*;
use context_menu::context_menu_actions;
use fs_utils::debug_log;
use once_cell::sync::OnceCell;
use statusbar::dir_sizes;
use undo::{redo_action, undo_action, UndoState};
use watcher::WatchState;

fn init_logging() {
    static GUARD: OnceCell<tracing_appender::non_blocking::WorkerGuard> = OnceCell::new();
    let base = dirs_next::data_dir().unwrap_or_else(|| std::env::temp_dir());
    let log_dir = base.join("browsey").join("logs");
    if let Err(e) = std::fs::create_dir_all(&log_dir) {
        eprintln!("Failed to create log dir {:?}: {}", log_dir, e);
        return;
    }
    let file_appender = tracing_appender::rolling::never(&log_dir, "browsey.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
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

fn main() {
    init_logging();
    tauri::Builder::default()
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
            search,
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
            list_recent,
            list_trash,
            store_column_widths,
            load_saved_column_widths,
            dir_sizes,
            context_menu_actions,
            rename_entry,
            move_to_trash,
            delete_entry,
            delete_entries,
            entry_times_cmd,
            extract_archive,
            open_console,
            set_clipboard_cmd,
            paste_clipboard_cmd,
            paste_clipboard_preview,
            search_stream,
            restore_trash_items,
            purge_trash_items,
            create_folder,
            compress_entries,
            cancel_task,
            get_permissions,
            set_permissions,
            undo_action,
            redo_action
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
