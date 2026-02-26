//! Build deduped network listing entries from mounts, discovered network targets, and cloud remotes.

use crate::{
    commands::{cloud, cloud::types::CloudRemote, fs::MountInfo},
    entry::{EntryCapabilities, FsEntry},
    errors::api_error::ApiResult,
    icons::icon_ids,
};
use std::collections::HashMap;

use super::{
    discovery,
    error::{map_api_result, NetworkError, NetworkErrorCode, NetworkResult},
    mounts, uri,
};

const NETWORK_ICON_ID: u16 = 10;
const CLOUD_ICON_ID: u16 = icon_ids::CLOUD;

const NETWORK_FS: &[&str] = &[
    "mtp",
    "sftp",
    "ssh",
    "cifs",
    "smb3",
    "smbfs",
    "smb",
    "nfs",
    "nfs4",
    "sshfs",
    "fuse.sshfs",
    "davfs2",
    "afpfs",
    "ftpfs",
    "ftp",
    "dav",
    "davs",
    "curlftpfs",
    "afp",
];

fn is_known_network_uri_scheme(scheme: &str) -> bool {
    matches!(
        scheme,
        "sftp" | "smb" | "nfs" | "ftp" | "dav" | "davs" | "afp" | "http" | "https"
    )
}

fn uri_scheme(value: &str) -> Option<String> {
    let trimmed = value.trim();
    let (raw_scheme, _) = trimmed.split_once("://")?;
    let raw = raw_scheme.trim();
    if raw.is_empty() {
        return None;
    }
    if let Some(canonical) = uri::canonical_scheme(raw) {
        Some(canonical.to_string())
    } else {
        Some(raw.to_ascii_lowercase())
    }
}

fn is_network_mount(mount: &MountInfo) -> bool {
    let path = mount.path.trim();
    if path.is_empty() {
        return false;
    }

    let fs_lc = mount.fs.to_ascii_lowercase();
    let scheme = uri_scheme(path);
    if scheme
        .as_deref()
        .map(is_known_network_uri_scheme)
        .unwrap_or(false)
    {
        return true;
    }

    let path_lc = path.to_ascii_lowercase();
    if path_lc.contains("/gvfs/") || path_lc.contains("\\gvfs\\") {
        return true;
    }
    NETWORK_FS.contains(&fs_lc.as_str())
}

fn normalize_path(p: &str) -> String {
    if p.is_empty() {
        return String::new();
    }
    let with_slashes = p.replace('\\', "/");
    let trimmed = with_slashes.trim_end_matches('/').to_string();
    if trimmed.is_empty() {
        if with_slashes.starts_with('/') {
            "/".to_string()
        } else {
            String::new()
        }
    } else if trimmed.chars().nth(1).map(|c| c == ':').unwrap_or(false)
        && trimmed.chars().count() == 2
    {
        format!("{trimmed}/")
    } else {
        trimmed
    }
}

fn to_network_entry(mount: &MountInfo) -> FsEntry {
    let label = mount.label.trim();
    FsEntry {
        name: if label.is_empty() {
            mount.path.clone()
        } else {
            label.to_string()
        },
        path: mount.path.clone(),
        kind: "dir".to_string(),
        ext: None,
        size: None,
        items: None,
        modified: None,
        original_path: None,
        trash_id: None,
        icon_id: CLOUD_ICON_ID,
        starred: false,
        hidden: false,
        network: true,
        read_only: false,
        read_denied: false,
        capabilities: None,
    }
}

fn to_cloud_network_entry(remote: &CloudRemote) -> FsEntry {
    FsEntry {
        name: remote.label.clone(),
        path: remote.root_path.clone(),
        kind: "dir".to_string(),
        ext: None,
        size: None,
        items: None,
        modified: None,
        original_path: None,
        trash_id: None,
        icon_id: NETWORK_ICON_ID,
        starred: false,
        hidden: false,
        network: true,
        read_only: false,
        read_denied: false,
        capabilities: Some(EntryCapabilities {
            can_list: remote.capabilities.can_list,
            can_mkdir: remote.capabilities.can_mkdir,
            can_delete: remote.capabilities.can_delete,
            can_rename: remote.capabilities.can_rename,
            can_move: remote.capabilities.can_move,
            can_copy: remote.capabilities.can_copy,
            can_trash: remote.capabilities.can_trash,
            can_undo: remote.capabilities.can_undo,
            can_permissions: remote.capabilities.can_permissions,
        }),
    }
}

pub(super) fn to_network_entries(mounts: &[MountInfo]) -> Vec<FsEntry> {
    let mut deduped: HashMap<String, MountInfo> = HashMap::new();
    for mount in mounts {
        if !is_network_mount(mount) {
            continue;
        }

        let raw_path = mount.path.trim();
        let scheme = uri_scheme(raw_path);
        if let Some(s) = scheme.as_deref() {
            if !is_known_network_uri_scheme(s) {
                continue;
            }
        }

        let key = {
            let normalized = normalize_path(raw_path);
            if normalized.is_empty() {
                raw_path.to_string()
            } else {
                normalized
            }
        };

        deduped.entry(key).or_insert_with(|| mount.clone());
    }

    deduped.values().map(to_network_entry).collect()
}

pub(super) fn list_network_entries_sync(force_refresh: bool) -> Vec<FsEntry> {
    let mut mounts_list = mounts::list_mounts_sync();
    mounts_list.extend(discovery::list_network_devices_sync(force_refresh));
    let mut entries = to_network_entries(&mounts_list);
    entries.extend(
        cloud::list_cloud_remotes_sync_best_effort(force_refresh)
            .into_iter()
            .map(|remote| to_cloud_network_entry(&remote)),
    );
    entries
}

#[tauri::command]
pub async fn list_network_entries(force_refresh: Option<bool>) -> ApiResult<Vec<FsEntry>> {
    map_api_result(list_network_entries_impl(force_refresh).await)
}

async fn list_network_entries_impl(force_refresh: Option<bool>) -> NetworkResult<Vec<FsEntry>> {
    let force_refresh = force_refresh.unwrap_or(false);
    let task =
        tauri::async_runtime::spawn_blocking(move || list_network_entries_sync(force_refresh));
    match task.await {
        Ok(result) => Ok(result),
        Err(error) => Err(NetworkError::new(
            NetworkErrorCode::DiscoveryFailed,
            format!("network listing failed: {error}"),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mount(label: &str, path: &str, fs: &str) -> MountInfo {
        MountInfo {
            label: label.to_string(),
            path: path.to_string(),
            fs: fs.to_string(),
            removable: false,
        }
    }

    #[test]
    fn to_network_entries_skips_unknown_uri_scheme() {
        let mounts = vec![mount("Custom", "foo+bar://host/path", "foo")];
        let entries = to_network_entries(&mounts);
        assert!(entries.is_empty());
    }

    #[test]
    fn to_network_entries_dedupes_by_normalized_path() {
        let mounts = vec![
            mount("A", "smb://nas.local/share/", "smb"),
            mount("B", "smb://nas.local/share", "smb"),
        ];
        let entries = to_network_entries(&mounts);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].path, "smb://nas.local/share/");
    }
}
