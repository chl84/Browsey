use super::super::{
    error::{CloudCommandError, CloudCommandErrorCode, CloudCommandResult},
    path::CloudPath,
    provider::CloudProvider,
    rclone_cli::RcloneCli,
    types::{CloudEntry, CloudRemote},
};

#[allow(dead_code)]
#[derive(Debug, Clone, Default)]
pub(in crate::commands::cloud) struct RcloneCloudProvider {
    cli: RcloneCli,
}

#[allow(dead_code)]
impl RcloneCloudProvider {
    pub fn new(cli: RcloneCli) -> Self {
        Self { cli }
    }

    pub fn cli(&self) -> &RcloneCli {
        &self.cli
    }
}

impl CloudProvider for RcloneCloudProvider {
    fn list_remotes(&self) -> CloudCommandResult<Vec<CloudRemote>> {
        Err(CloudCommandError::new(
            CloudCommandErrorCode::Unsupported,
            "Cloud remotes are not implemented yet",
        ))
    }

    fn list_dir(&self, _path: &CloudPath) -> CloudCommandResult<Vec<CloudEntry>> {
        Err(CloudCommandError::new(
            CloudCommandErrorCode::Unsupported,
            "Cloud directory listing is not implemented yet",
        ))
    }
}
