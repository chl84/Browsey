use super::{cloud, local, ListingError, ListingErrorCode, ListingResult};
use crate::entry::FsEntry;

pub(super) fn list_scope_entries(
    scope: &str,
    path: Option<String>,
    app: tauri::AppHandle,
) -> ListingResult<Vec<FsEntry>> {
    match scope {
        "dir" => Ok(local::list_dir_sync(path, None, app)?.entries),
        "recent" => Ok(crate::commands::library::list_recent(None)
            .map_err(cloud::listing_error_from_api)?
            .entries),
        "starred" => Ok(crate::commands::library::list_starred(None)
            .map_err(cloud::listing_error_from_api)?
            .entries),
        "trash" => Ok(crate::commands::fs::list_trash(None)
            .map_err(cloud::listing_error_from_api)?
            .entries),
        _ => Err(ListingError::new(
            ListingErrorCode::UnsupportedScope,
            format!("Unsupported facet scope: {scope}"),
        )),
    }
}
