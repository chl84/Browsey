use super::{
    error::CloudCommandResult,
    path::CloudPath,
    types::{CloudEntry, CloudRemote},
};
use std::path::Path;
use std::sync::atomic::AtomicBool;

pub(crate) trait CloudProvider: Send + Sync {
    fn list_remotes(&self) -> CloudCommandResult<Vec<CloudRemote>>;

    fn stat_path(&self, path: &CloudPath) -> CloudCommandResult<Option<CloudEntry>>;

    fn list_dir(&self, path: &CloudPath) -> CloudCommandResult<Vec<CloudEntry>>;

    fn mkdir(&self, path: &CloudPath, cancel: Option<&AtomicBool>) -> CloudCommandResult<()>;

    fn delete_file(&self, path: &CloudPath, cancel: Option<&AtomicBool>) -> CloudCommandResult<()>;

    fn delete_dir_recursive(
        &self,
        path: &CloudPath,
        cancel: Option<&AtomicBool>,
    ) -> CloudCommandResult<()>;

    fn delete_dir_empty(
        &self,
        path: &CloudPath,
        cancel: Option<&AtomicBool>,
    ) -> CloudCommandResult<()>;

    fn move_entry(
        &self,
        src: &CloudPath,
        dst: &CloudPath,
        overwrite: bool,
        prechecked: bool,
        cancel: Option<&AtomicBool>,
    ) -> CloudCommandResult<()>;

    fn copy_entry(
        &self,
        src: &CloudPath,
        dst: &CloudPath,
        overwrite: bool,
        prechecked: bool,
        cancel: Option<&AtomicBool>,
    ) -> CloudCommandResult<()>;

    fn download_file(
        &self,
        src: &CloudPath,
        local_dest: &Path,
        cancel: Option<&AtomicBool>,
    ) -> CloudCommandResult<()>;
}
