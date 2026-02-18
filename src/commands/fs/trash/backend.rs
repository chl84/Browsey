use ::trash::{delete as trash_delete, os_limited::list as trash_list, TrashItem};
use std::path::Path;

pub(super) trait TrashBackend {
    fn list_items(&self) -> Result<Vec<TrashItem>, String>;
    fn delete_path(&self, path: &Path) -> Result<(), String>;
    fn rewrite_original_path(&self, item: &TrashItem, original_path: &Path) -> Result<(), String>;
}

pub(super) struct SystemTrashBackend;

impl TrashBackend for SystemTrashBackend {
    fn list_items(&self) -> Result<Vec<TrashItem>, String> {
        trash_list().map_err(|e| format!("Failed to list trash: {e}"))
    }

    fn delete_path(&self, path: &Path) -> Result<(), String> {
        trash_delete(path).map_err(|e| format!("Failed to move to trash: {e}"))
    }

    fn rewrite_original_path(&self, item: &TrashItem, original_path: &Path) -> Result<(), String> {
        #[cfg(not(target_os = "windows"))]
        {
            return super::staging::rewrite_trash_info_original_path(item, original_path);
        }
        #[cfg(target_os = "windows")]
        {
            let _ = (item, original_path);
            Ok(())
        }
    }
}
