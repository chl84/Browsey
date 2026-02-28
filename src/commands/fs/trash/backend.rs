use super::super::error::{FsError, FsErrorCode, FsResult};
use ::trash::{delete as trash_delete, os_limited::list as trash_list, TrashItem};
use std::path::Path;

pub(super) trait TrashBackend {
    fn list_items(&self) -> FsResult<Vec<TrashItem>>;
    fn delete_path(&self, path: &Path) -> FsResult<()>;
    fn rewrite_original_path(&self, item: &TrashItem, original_path: &Path) -> FsResult<()>;
}

pub(super) struct SystemTrashBackend;

impl TrashBackend for SystemTrashBackend {
    fn list_items(&self) -> FsResult<Vec<TrashItem>> {
        trash_list().map_err(|e| {
            FsError::new(
                FsErrorCode::TrashFailed,
                format!("Failed to list trash: {e}"),
            )
        })
    }

    fn delete_path(&self, path: &Path) -> FsResult<()> {
        trash_delete(path).map_err(|e| {
            FsError::new(
                FsErrorCode::TrashFailed,
                format!("Failed to move to trash: {e}"),
            )
        })
    }

    fn rewrite_original_path(&self, item: &TrashItem, original_path: &Path) -> FsResult<()> {
        #[cfg(not(target_os = "windows"))]
        {
            super::staging::rewrite_trash_info_original_path(item, original_path)
        }
        #[cfg(target_os = "windows")]
        {
            let _ = (item, original_path);
            Ok(())
        }
    }
}
