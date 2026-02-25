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

impl CloudCapabilities {
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
