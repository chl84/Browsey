use super::{
    error::CloudCommandResult,
    path::CloudPath,
    types::{CloudEntry, CloudRemote},
};

pub(super) trait CloudProvider: Send + Sync {
    fn list_remotes(&self) -> CloudCommandResult<Vec<CloudRemote>>;

    fn stat_path(&self, path: &CloudPath) -> CloudCommandResult<Option<CloudEntry>>;

    fn list_dir(&self, path: &CloudPath) -> CloudCommandResult<Vec<CloudEntry>>;

    fn mkdir(&self, path: &CloudPath) -> CloudCommandResult<()>;

    fn delete_file(&self, path: &CloudPath) -> CloudCommandResult<()>;

    fn delete_dir_recursive(&self, path: &CloudPath) -> CloudCommandResult<()>;

    fn delete_dir_empty(&self, path: &CloudPath) -> CloudCommandResult<()>;

    fn move_entry(
        &self,
        src: &CloudPath,
        dst: &CloudPath,
        overwrite: bool,
        prechecked: bool,
    ) -> CloudCommandResult<()>;

    fn copy_entry(
        &self,
        src: &CloudPath,
        dst: &CloudPath,
        overwrite: bool,
        prechecked: bool,
    ) -> CloudCommandResult<()>;
}
