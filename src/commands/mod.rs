//! Aggregates Tauri command modules and re-exports them for the builder.

pub mod bookmarks;
pub mod fs;
pub mod library;
pub mod meta;
pub mod search;
pub mod settings;

pub use crate::clipboard::{paste_clipboard_cmd, set_clipboard_cmd};
pub use bookmarks::{add_bookmark, get_bookmarks, remove_bookmark};
pub use fs::{
    delete_entry, list_dir, list_mounts, list_trash, move_to_trash, open_entry, rename_entry,
    watch_dir,
};
pub use library::{list_recent, list_starred, toggle_star};
pub use meta::entry_times_cmd;
pub use search::search;
pub use settings::{load_saved_column_widths, store_column_widths};
