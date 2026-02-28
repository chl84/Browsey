mod error;
mod logging;
mod parse;
mod read;
mod remotes;
mod runtime;
#[cfg(test)]
mod tests;
mod write;

use super::super::{
    error::{CloudCommandError, CloudCommandErrorCode, CloudCommandResult},
    path::CloudPath,
    provider::CloudProvider,
    rclone_cli::{RcloneCli, RcloneCliError, RcloneCommandSpec, RcloneSubcommand},
    rclone_rc::RcloneRcClient,
    types::{CloudCapabilities, CloudEntry, CloudEntryKind, CloudProviderKind, CloudRemote},
};
use std::path::Path;
use std::sync::atomic::AtomicBool;

#[derive(Debug, Clone, Default)]
pub(crate) struct RcloneCloudProvider {
    cli: RcloneCli,
    rc: RcloneRcClient,
}

impl RcloneCloudProvider {
    #[cfg(test)]
    pub fn new(cli: RcloneCli) -> Self {
        let rc = RcloneRcClient::new(cli.binary().to_os_string());
        Self { cli, rc }
    }

    pub fn cli(&self) -> &RcloneCli {
        &self.cli
    }

    pub(crate) fn download_file_with_progress<F>(
        &self,
        src: &CloudPath,
        local_dest: &Path,
        progress_group: &str,
        cancel: Option<&AtomicBool>,
        on_progress: F,
    ) -> CloudCommandResult<()>
    where
        F: FnMut(u64, u64),
    {
        self.download_file_with_progress_impl(src, local_dest, progress_group, cancel, on_progress)
    }

    pub(crate) fn upload_file_with_progress<F>(
        &self,
        local_src: &Path,
        dst: &CloudPath,
        progress_group: &str,
        cancel: Option<&AtomicBool>,
        on_progress: F,
    ) -> CloudCommandResult<()>
    where
        F: FnMut(u64, u64),
    {
        self.upload_file_with_progress_impl(local_src, dst, progress_group, cancel, on_progress)
    }
}

impl CloudProvider for RcloneCloudProvider {
    fn list_remotes(&self) -> CloudCommandResult<Vec<CloudRemote>> {
        self.list_remotes_impl()
    }

    fn stat_path(&self, path: &CloudPath) -> CloudCommandResult<Option<CloudEntry>> {
        self.stat_path_impl(path)
    }

    fn list_dir(&self, path: &CloudPath) -> CloudCommandResult<Vec<CloudEntry>> {
        self.list_dir_impl(path)
    }

    fn mkdir(&self, path: &CloudPath, cancel: Option<&AtomicBool>) -> CloudCommandResult<()> {
        self.mkdir_impl(path, cancel)
    }

    fn delete_file(&self, path: &CloudPath, cancel: Option<&AtomicBool>) -> CloudCommandResult<()> {
        self.delete_file_impl(path, cancel)
    }

    fn delete_dir_recursive(
        &self,
        path: &CloudPath,
        cancel: Option<&AtomicBool>,
    ) -> CloudCommandResult<()> {
        self.delete_dir_recursive_impl(path, cancel)
    }

    fn delete_dir_empty(
        &self,
        path: &CloudPath,
        cancel: Option<&AtomicBool>,
    ) -> CloudCommandResult<()> {
        self.delete_dir_empty_impl(path, cancel)
    }

    fn move_entry(
        &self,
        src: &CloudPath,
        dst: &CloudPath,
        overwrite: bool,
        prechecked: bool,
        cancel: Option<&AtomicBool>,
    ) -> CloudCommandResult<()> {
        self.move_entry_impl(src, dst, overwrite, prechecked, cancel)
    }

    fn copy_entry(
        &self,
        src: &CloudPath,
        dst: &CloudPath,
        overwrite: bool,
        prechecked: bool,
        cancel: Option<&AtomicBool>,
    ) -> CloudCommandResult<()> {
        self.copy_entry_impl(src, dst, overwrite, prechecked, cancel)
    }

    fn download_file(
        &self,
        src: &CloudPath,
        local_dest: &Path,
        cancel: Option<&AtomicBool>,
    ) -> CloudCommandResult<()> {
        self.download_file_impl(src, local_dest, cancel)
    }
}
