use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CloudProviderKind {
    Onedrive,
    Gdrive,
    Nextcloud,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CloudEntryKind {
    File,
    Dir,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudCapabilities {
    pub can_list: bool,
    pub can_mkdir: bool,
    pub can_delete: bool,
    pub can_rename: bool,
    pub can_move: bool,
    pub can_copy: bool,
    pub can_trash: bool,
    pub can_undo: bool,
    pub can_permissions: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudRemote {
    pub id: String,
    pub label: String,
    pub provider: CloudProviderKind,
    pub root_path: String,
    pub capabilities: CloudCapabilities,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudEntry {
    pub name: String,
    pub path: String,
    pub kind: CloudEntryKind,
    pub size: Option<u64>,
    pub modified: Option<String>,
    pub capabilities: CloudCapabilities,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CloudConflictInfo {
    pub src: String,
    pub target: String,
    pub exists: bool,
    pub is_dir: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudRootSelection {
    pub remote: CloudRemote,
    pub root_path: String,
    pub is_remote_root: bool,
}

impl CloudCapabilities {
    pub fn v1_for_provider(provider: CloudProviderKind) -> Self {
        match provider {
            CloudProviderKind::Onedrive => Self::v1_core_rw(),
            CloudProviderKind::Gdrive => Self::v1_core_rw(),
            CloudProviderKind::Nextcloud => Self::v1_core_rw(),
        }
    }

    pub fn v1_core_rw() -> Self {
        Self {
            can_list: true,
            can_mkdir: true,
            can_delete: true,
            can_rename: true,
            can_move: true,
            can_copy: true,
            can_trash: false,
            can_undo: false,
            can_permissions: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{CloudCapabilities, CloudProviderKind};

    #[test]
    fn provider_capability_matrix_is_defined_for_v1_providers() {
        let onedrive = CloudCapabilities::v1_for_provider(CloudProviderKind::Onedrive);
        let gdrive = CloudCapabilities::v1_for_provider(CloudProviderKind::Gdrive);
        let nextcloud = CloudCapabilities::v1_for_provider(CloudProviderKind::Nextcloud);

        assert!(onedrive.can_list && onedrive.can_copy && onedrive.can_move);
        assert!(gdrive.can_list && gdrive.can_copy && gdrive.can_move);
        assert!(nextcloud.can_list && nextcloud.can_copy && nextcloud.can_move);
        assert!(!onedrive.can_trash && !gdrive.can_trash && !nextcloud.can_trash);
    }
}
