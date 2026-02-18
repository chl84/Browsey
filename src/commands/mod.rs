//! Aggregates Tauri command modules and re-exports them for the builder.

pub mod about;
pub mod bookmarks;
pub mod compress;
pub mod console;
pub mod decompress;
pub mod duplicates;
pub mod file_types;
pub mod fs;
pub mod keymap;
pub mod library;
pub mod listing;
pub mod meta;
pub mod network;
pub mod open_with;
pub(crate) mod path_guard;
pub mod permissions;
pub mod rename;
pub mod search;
pub mod settings;
pub mod system_clipboard;
pub mod tasks;
pub mod thumbnails;

pub use crate::clipboard::{
    paste_clipboard_cmd, paste_clipboard_preview, resolve_drop_clipboard_mode, set_clipboard_cmd,
};
pub use about::about_info;
pub use bookmarks::{add_bookmark, clear_bookmarks, get_bookmarks, remove_bookmark};
pub use compress::compress_entries;
pub use console::open_console;
pub use decompress::{extract_archive, extract_archives};
pub use duplicates::{check_duplicates, check_duplicates_stream};
pub use file_types::detect_new_file_type;
pub use fs::{
    create_file, create_folder, delete_entries, delete_entry, list_trash, move_to_trash,
    move_to_trash_many, open_entry, purge_trash_items, restore_trash_items, set_hidden,
};
pub use keymap::{
    load_shortcuts, reset_all_shortcuts, reset_shortcut_binding, set_shortcut_binding,
};
pub use library::{
    clear_recents, clear_stars, list_recent, list_starred, remove_recent, toggle_star,
};
pub use listing::{list_dir, list_facets, watch_dir};
pub use meta::{entry_extra_metadata_cmd, entry_kind_cmd, entry_times_cmd};
pub use network::connect::connect_network_uri;
pub use network::discovery::{list_network_devices, open_network_uri};
pub use network::entries::list_network_entries;
pub use network::mounts::{eject_drive, list_mounts, mount_partition};
pub use network::uri::{classify_network_uri, resolve_mounted_path_for_uri};
pub use open_with::{list_open_with_apps, open_with};
pub use permissions::{
    get_permissions, get_permissions_batch, list_ownership_principals,
    maybe_run_ownership_helper_from_args, set_ownership, set_permissions,
};
pub use rename::{preview_rename_entries, rename_entries, rename_entry};
pub use search::search_stream;
pub use settings::{
    load_archive_level, load_archive_name, load_confirm_delete, load_default_view, load_density,
    load_double_click_ms, load_ffmpeg_path, load_folders_first, load_hardware_acceleration,
    load_hidden_files_last, load_mounts_poll_ms, load_open_dest_after_extract,
    load_saved_column_widths, load_show_hidden, load_sort_direction, load_sort_field,
    load_start_dir, load_thumb_cache_mb, load_video_thumbs, store_archive_level,
    store_archive_name, store_column_widths, store_confirm_delete, store_default_view,
    store_density, store_double_click_ms, store_ffmpeg_path, store_folders_first,
    store_hardware_acceleration, store_hidden_files_last, store_mounts_poll_ms,
    store_open_dest_after_extract, store_show_hidden, store_sort_direction, store_sort_field,
    store_start_dir, store_thumb_cache_mb, store_video_thumbs,
};
pub use system_clipboard::clear_system_clipboard;
pub use system_clipboard::copy_paths_to_system_clipboard;
pub use system_clipboard::system_clipboard_paths;
pub use tasks::{cancel_task, CancelState};
pub use thumbnails::{clear_thumbnail_cache, get_thumbnail};
