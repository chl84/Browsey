//! Aggregates Tauri command modules and re-exports them for the builder.

pub mod bookmarks;
pub mod compress;
pub mod console;
pub mod decompress;
pub mod fs;
pub mod library;
pub mod meta;
pub mod open_with;
pub mod permissions;
pub mod search;
pub mod settings;
pub mod system_clipboard;
pub mod tasks;
pub mod thumbnails;

pub use crate::clipboard::{paste_clipboard_cmd, paste_clipboard_preview, set_clipboard_cmd};
pub use bookmarks::{add_bookmark, get_bookmarks, remove_bookmark};
pub use compress::compress_entries;
pub use console::open_console;
pub use decompress::{extract_archive, extract_archives};
pub use fs::{
    create_file, create_folder, delete_entries, delete_entry, eject_drive, list_dir, list_mounts,
    list_trash, move_to_trash, move_to_trash_many, open_entry, purge_trash_items, rename_entry,
    set_hidden, restore_trash_items, watch_dir,
};
pub use library::{list_recent, list_starred, remove_recent, toggle_star};
pub use meta::entry_times_cmd;
pub use open_with::{list_open_with_apps, open_with};
pub use permissions::{get_permissions, set_permissions};
pub use search::{search, search_stream};
pub use settings::{
    load_default_view, load_folders_first, load_hidden_files_last, load_saved_column_widths,
    load_show_hidden, load_start_dir, store_column_widths, store_default_view, store_folders_first,
    store_hidden_files_last, store_show_hidden, store_start_dir,
};
pub use system_clipboard::clear_system_clipboard;
pub use system_clipboard::copy_paths_to_system_clipboard;
pub use system_clipboard::system_clipboard_paths;
pub use tasks::{cancel_task, CancelState};
pub use thumbnails::get_thumbnail;
