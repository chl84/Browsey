use super::super::error::{map_api_result, FsError, FsErrorCode, FsResult};
use super::super::DirListing;
use crate::{
    entry::{build_entry, FsEntry},
    errors::api_error::ApiResult,
    fs_utils::debug_log,
    icons::icon_id_for,
    sorting::{sort_entries, SortSpec},
};
use ::trash::{
    os_limited::{list as trash_list, metadata as trash_metadata},
    TrashItem, TrashItemSize,
};
use std::path::{Path, PathBuf};

#[cfg(not(target_os = "windows"))]
fn restorable_file_in_trash_from_info_file(info_file: &Path) -> PathBuf {
    let trash_folder = info_file.parent().and_then(|p| p.parent());
    let name_in_trash = info_file.file_stem();
    match (trash_folder, name_in_trash) {
        (Some(folder), Some(name)) => folder.join("files").join(name),
        _ => PathBuf::from(info_file),
    }
}

pub(super) fn trash_item_path(item: &TrashItem) -> PathBuf {
    #[cfg(target_os = "windows")]
    {
        PathBuf::from(&item.id)
    }
    #[cfg(not(target_os = "windows"))]
    {
        restorable_file_in_trash_from_info_file(Path::new(&item.id))
    }
}

pub(super) fn apply_original_trash_fields(
    entry: &mut FsEntry,
    original_path: &Path,
    item: &TrashItem,
    meta: &std::fs::Metadata,
    is_link: bool,
) {
    entry.name = original_path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| original_path.to_string_lossy().into_owned());
    entry.ext = original_path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_string());
    entry.original_path = Some(original_path.to_string_lossy().into_owned());
    entry.trash_id = Some(item.id.to_string_lossy().into_owned());
    entry.icon_id = icon_id_for(original_path, meta, is_link);
}

#[tauri::command]
pub fn list_trash(sort: Option<SortSpec>) -> ApiResult<DirListing> {
    map_api_result(list_trash_impl(sort))
}

fn list_trash_impl(sort: Option<SortSpec>) -> FsResult<DirListing> {
    let items = trash_list().map_err(|error| {
        FsError::new(
            FsErrorCode::TrashFailed,
            format!("Failed to list trash: {error}"),
        )
    })?;
    let mut entries = Vec::new();
    for item in items {
        let path = trash_item_path(&item);
        let meta = match std::fs::symlink_metadata(&path) {
            Ok(m) => m,
            Err(e) => {
                debug_log(&format!(
                    "trash list: missing item path={}, skipping: {e:?}",
                    path.display()
                ));
                continue;
            }
        };
        let is_link = meta.file_type().is_symlink();
        let mut entry = build_entry(&path, &meta, is_link, false);
        let original_path = item.original_path();
        apply_original_trash_fields(&mut entry, &original_path, &item, &meta, is_link);
        if let Ok(info) = trash_metadata(&item) {
            match info.size {
                TrashItemSize::Bytes(b) => entry.size = Some(b),
                TrashItemSize::Entries(n) => entry.items = Some(n as u64),
            }
        }
        entries.push(entry);
    }
    sort_entries(&mut entries, sort);
    Ok(DirListing {
        current: "Trash".to_string(),
        entries,
    })
}
