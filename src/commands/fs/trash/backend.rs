use super::super::error::{FsError, FsErrorCode, FsResult};
use ::trash::{delete as trash_delete, os_limited::list as trash_list, TrashItem};
use std::path::Path;

pub(super) trait TrashBackend {
    fn list_items(&self) -> FsResult<Vec<TrashItem>>;
    fn delete_path(&self, path: &Path) -> FsResult<()>;
    fn restore_item(&self, item: &TrashItem, original_path: &Path) -> FsResult<()>;
    fn rewrite_original_path(&self, item: &TrashItem, original_path: &Path) -> FsResult<()>;
}

pub(super) struct SystemTrashBackend;

impl TrashBackend for SystemTrashBackend {
    fn restore_item(&self, item: &TrashItem, original_path: &Path) -> FsResult<()> {
        restore_trashed_item(item, original_path)
    }
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

pub(super) fn restore_trashed_item(item: &TrashItem, original_path: &Path) -> FsResult<()> {
    let payload = super::listing::trash_item_path(item);
    #[cfg(not(target_os = "windows"))]
    {
        let info = std::path::PathBuf::from(&item.id);
        crate::fs_utils::check_no_symlink_components(&info).map_err(FsError::from)?;
        let identity = crate::fs_utils::FileIdentity::capture(&info).ok_or_else(|| {
            FsError::new(
                FsErrorCode::TrashFailed,
                "Cannot verify trash metadata ownership",
            )
        })?;
        crate::undo::move_with_fallback(&payload, original_path).map_err(FsError::from)?;
        if !identity.matches(&info) {
            return Err(FsError::new(
                FsErrorCode::TrashFailed,
                "Source restored, but trash metadata changed; refresh Wastebasket",
            ));
        }
        std::fs::remove_file(&info).map_err(|error| {
            FsError::from_io_error(
                FsErrorCode::TrashFailed,
                "Source restored, but trash metadata cleanup failed",
                error,
            )
        })?;
        Ok(())
    }
    #[cfg(target_os = "windows")]
    {
        let _ = payload;
        crate::fs_utils::check_no_symlink_components(
            original_path
                .parent()
                .ok_or_else(|| FsError::new(FsErrorCode::InvalidPath, "Invalid restore path"))?,
        )
        .map_err(FsError::from)?;
        let mut target = item.clone();
        target.name = original_path
            .file_name()
            .ok_or_else(|| FsError::new(FsErrorCode::InvalidPath, "Invalid restore filename"))?
            .to_os_string();
        target.original_parent = original_path.parent().unwrap().to_path_buf();
        ::trash::os_limited::restore_all([target]).map_err(|error| {
            FsError::new(
                FsErrorCode::TrashFailed,
                format!("Failed to restore trash item: {error}"),
            )
        })
    }
}
