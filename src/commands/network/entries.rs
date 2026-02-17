//! Build deduped network listing entries from mounts and discovered network targets.

use crate::{commands::fs::MountInfo, entry::FsEntry};
use std::collections::HashMap;

use super::{discovery, mounts, uri};

const NETWORK_ICON_ID: u16 = 10;

const NETWORK_FS: &[&str] = &[
    "mtp",
    "onedrive",
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
        "onedrive" | "sftp" | "smb" | "nfs" | "ftp" | "dav" | "davs" | "afp" | "http" | "https"
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

fn onedrive_account_key(raw_path: &str) -> Option<String> {
    let path = raw_path.trim();
    let scheme = uri_scheme(path)?;
    if scheme != "onedrive" {
        return None;
    }
    let rest = &path["onedrive://".len()..];
    let slash = rest.find('/').unwrap_or(rest.len());
    let account = rest[..slash].trim().to_ascii_lowercase();
    if account.is_empty() {
        None
    } else {
        Some(format!("onedrive://{account}"))
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
        icon_id: NETWORK_ICON_ID,
        starred: false,
        hidden: false,
        network: true,
        read_only: false,
        read_denied: false,
    }
}

pub(super) fn to_network_entries(mounts: &[MountInfo]) -> Vec<FsEntry> {
    let onedrive_mounted = mounts.iter().any(|mount| {
        let fs_lc = mount.fs.to_ascii_lowercase();
        let path_lc = mount.path.trim().to_ascii_lowercase();
        fs_lc == "onedrive" && !path_lc.starts_with("onedrive://")
    });

    let mut deduped: HashMap<String, MountInfo> = HashMap::new();
    for mount in mounts {
        if !is_network_mount(mount) {
            continue;
        }

        let raw_path = mount.path.trim();
        let raw_path_lc = raw_path.to_ascii_lowercase();
        let scheme = uri_scheme(raw_path);

        if onedrive_mounted && raw_path_lc.starts_with("onedrive://") {
            continue;
        }
        if let Some(s) = scheme.as_deref() {
            if !is_known_network_uri_scheme(s) {
                continue;
            }
        }

        let key = onedrive_account_key(raw_path)
            .or_else(|| {
                let normalized = normalize_path(raw_path);
                if normalized.is_empty() {
                    None
                } else {
                    Some(normalized)
                }
            })
            .unwrap_or_else(|| raw_path.to_string());

        deduped.entry(key).or_insert_with(|| mount.clone());
    }

    deduped.values().map(to_network_entry).collect()
}

pub(super) fn list_network_entries_sync() -> Vec<FsEntry> {
    let mut mounts_list = mounts::list_mounts_sync();
    mounts_list.extend(discovery::list_network_devices_sync());
    to_network_entries(&mounts_list)
}

#[tauri::command]
pub async fn list_network_entries() -> Result<Vec<FsEntry>, String> {
    tauri::async_runtime::spawn_blocking(list_network_entries_sync)
        .await
        .map_err(|e| format!("network listing failed: {e}"))
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
    fn to_network_entries_hides_onedrive_activation_root_when_mounted() {
        let mounts = vec![
            mount("OneDrive root", "onedrive://account-123/", "onedrive"),
            mount(
                "OneDrive mounted",
                "/run/user/1000/gvfs/onedrive:host=example",
                "onedrive",
            ),
        ];
        let entries = to_network_entries(&mounts);
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].path, "/run/user/1000/gvfs/onedrive:host=example");
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
