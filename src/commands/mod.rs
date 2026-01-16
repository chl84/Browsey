//! Aggregates Tauri command modules and re-exports them for the builder.

pub mod bookmarks;
pub mod compress;
pub mod fs;
pub mod library;
pub mod meta;
pub mod search;
pub mod settings;

pub use crate::clipboard::{paste_clipboard_cmd, paste_clipboard_preview, set_clipboard_cmd};
pub use bookmarks::{add_bookmark, get_bookmarks, remove_bookmark};
pub use compress::compress_entries;
pub use fs::{
    create_folder, delete_entry, list_dir, list_mounts, list_trash, move_to_trash, open_entry,
    purge_trash_items, rename_entry, restore_trash_items, watch_dir,
};
pub use library::{list_recent, list_starred, toggle_star};
pub use meta::entry_times_cmd;
pub use search::{search, search_stream};
pub use settings::{load_saved_column_widths, store_column_widths};
