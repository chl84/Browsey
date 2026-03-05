use super::{
    build_listing_facets_with_hidden, DirListing, ListingError, ListingErrorCode, ListingFacets,
    ListingResult,
};
use crate::{
    commands::cloud::types::{CloudEntry as BrowseyCloudEntry, CloudEntryKind},
    entry::{EntryCapabilities, FsEntry},
    icons::icon_id_for_virtual_entry,
    sorting::{sort_entries, SortSpec},
};

pub(super) fn is_cloud_path(path: &str) -> bool {
    path.starts_with("rclone://")
}

pub(super) fn fs_entry_from_cloud_entry(entry: BrowseyCloudEntry) -> FsEntry {
    let is_dir = matches!(entry.kind, CloudEntryKind::Dir);
    let ext = if is_dir {
        None
    } else {
        entry.name.rsplit_once('.').map(|(_, ext)| ext.to_string())
    };
    FsEntry {
        name: entry.name.clone(),
        path: entry.path,
        kind: if is_dir { "dir" } else { "file" }.to_string(),
        ext,
        size: if is_dir { None } else { entry.size },
        items: None,
        modified: entry.modified,
        original_path: None,
        trash_id: None,
        icon_id: icon_id_for_virtual_entry(&entry.name, is_dir),
        starred: false,
        hidden: entry.name.starts_with('.'),
        network: true,
        read_only: false,
        read_denied: false,
        capabilities: Some(EntryCapabilities {
            can_list: entry.capabilities.can_list,
            can_mkdir: entry.capabilities.can_mkdir,
            can_delete: entry.capabilities.can_delete,
            can_rename: entry.capabilities.can_rename,
            can_move: entry.capabilities.can_move,
            can_copy: entry.capabilities.can_copy,
            can_trash: entry.capabilities.can_trash,
            can_undo: entry.capabilities.can_undo,
            can_permissions: entry.capabilities.can_permissions,
        }),
    }
}

pub(super) fn listing_error_from_api(error: crate::errors::api_error::ApiError) -> ListingError {
    let code = match error.code.as_str() {
        "invalid_path" => ListingErrorCode::InvalidPath,
        "not_found" => ListingErrorCode::NotFound,
        "permission_denied" => ListingErrorCode::PermissionDenied,
        "task_failed" => ListingErrorCode::TaskFailed,
        _ => ListingErrorCode::UnknownError,
    };
    ListingError::new(code, error.message)
}

pub(super) async fn list_cloud_dir(
    raw_path: &str,
    sort: Option<SortSpec>,
    app: tauri::AppHandle,
) -> ListingResult<DirListing> {
    let entries = crate::commands::cloud::list_cloud_entries(raw_path.to_string(), app.clone())
        .await
        .map_err(listing_error_from_api)?;
    let mut mapped: Vec<FsEntry> = entries.into_iter().map(fs_entry_from_cloud_entry).collect();
    sort_entries(&mut mapped, sort);
    Ok(DirListing {
        current: raw_path.to_string(),
        entries: mapped,
    })
}

pub(super) async fn list_cloud_facets(
    raw_path: &str,
    include_hidden: bool,
    app: tauri::AppHandle,
) -> ListingResult<ListingFacets> {
    let cloud_entries = crate::commands::cloud::list_cloud_entries(raw_path.to_string(), app)
        .await
        .map_err(listing_error_from_api)?;
    let entries: Vec<FsEntry> = cloud_entries
        .into_iter()
        .map(fs_entry_from_cloud_entry)
        .collect();
    Ok(build_listing_facets_with_hidden(&entries, include_hidden))
}
