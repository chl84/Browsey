//! Network entries from saved connections, mounts, discovery and cloud remotes.

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
    mounts, saved, uri,
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
            can_delete: false,
            can_rename: false,
            can_move: false,
            can_copy: false,
            can_trash: false,
            can_undo: remote.capabilities.can_undo,
            can_permissions: remote.capabilities.can_permissions,
            can_create_file: remote.capabilities.can_create_file,
            can_open_with: false,
            can_archive: false,
            can_advanced_rename: false,
            can_external_copy: false,
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

pub(super) fn list_network_entries_sync(force_refresh: bool) -> NetworkResult<Vec<FsEntry>> {
    let mut mounts_list = mounts::list_mounts_sync()?;
    // Optional LAN discovery must not hide saved servers when discovery fails.
    match discovery::list_network_devices_sync(force_refresh) {
        Ok(discovered) => mounts_list.extend(discovered),
        Err(_) => {
            tracing::warn!("Network discovery unavailable; retaining mounted and saved connections")
        }
    }
    let mut entries = to_network_entries(&mounts_list);
    merge_saved_connections(&mut entries, &saved::list()?, |uri| {
        #[cfg(not(target_os = "windows"))]
        {
            use gio::prelude::*;
            // path() only maps an existing mount; it does not mount/contact servers.
            gio::File::for_uri(uri)
                .path()
                .map(|path| path.to_string_lossy().into_owned())
        }
        #[cfg(target_os = "windows")]
        {
            let _ = uri;
            None
        }
    });
    entries.extend(
        cloud::list_cloud_remotes_sync_best_effort(force_refresh)
            .into_iter()
            .map(|remote| to_cloud_network_entry(&remote)),
    );
    Ok(entries)
}

fn merge_saved_connections(
    entries: &mut Vec<FsEntry>,
    saved: &[saved::SavedNetworkConnection],
    mut local_path: impl FnMut(&str) -> Option<String>,
) {
    // Saved targets win over discovery, which in turn wins over local mount
    // labels. Preserve the selected saved folder instead of rewriting its path.
    let saved = saved::unique_connections(saved.to_vec());
    let mut candidates: Vec<_> = saved
        .into_iter()
        .map(|connection| {
            to_network_entry(&MountInfo {
                label: connection.label.clone(),
                path: connection.uri.clone(),
                fs: uri::classify_uri(&connection.uri)
                    .scheme
                    .unwrap_or_default(),
                removable: false,
                size_bytes: None,
            })
        })
        .collect();
    entries.sort_by_key(|entry| {
        (
            saved::connection_identity(&entry.path).is_err(),
            entry.path.len(),
            entry.path.clone(),
        )
    });
    candidates.append(entries);
    let mut mapped_paths = HashMap::new();
    let mut groups: Vec<NetworkConnectionEntry> = Vec::new();
    for entry in candidates {
        let identity = saved::connection_identity(&entry.path).ok();
        let mounted_root = if identity.is_some() {
            mapped_paths
                .entry(entry.path.clone())
                .or_insert_with(|| local_path(&entry.path))
                .as_deref()
                .map(mount_path_identity)
        } else if std::path::Path::new(&entry.path).is_absolute() {
            Some(mount_path_identity(&entry.path))
        } else {
            None
        };
        let candidate = NetworkConnectionEntry {
            entry,
            identity,
            mounted_root,
        };
        if let Some(group) = groups
            .iter_mut()
            .find(|group| group.same_connection(&candidate))
        {
            if group.mounted_root.is_none() {
                group.mounted_root.clone_from(&candidate.mounted_root);
            }
            // Bind an unspecified user to the proven live account so a second,
            // different explicit account can never be swallowed by that alias.
            if let Some(identity) = &candidate.identity {
                if let Some(current) = &mut group.identity {
                    current.bind_account(identity);
                } else {
                    group.identity = Some(identity.clone());
                }
            }
        } else {
            groups.push(candidate);
        }
    }
    entries.extend(groups.into_iter().map(|group| group.entry));
}

struct NetworkConnectionEntry {
    entry: FsEntry,
    identity: Option<saved::ConnectionIdentity>,
    mounted_root: Option<String>,
}

impl NetworkConnectionEntry {
    fn same_connection(&self, other: &Self) -> bool {
        let same_mount = self
            .mounted_root
            .as_ref()
            .is_some_and(|root| Some(root) == other.mounted_root.as_ref());
        match (&self.identity, &other.identity) {
            (Some(a), Some(b)) => a == b || (same_mount && a.compatible_mount(b)),
            _ => same_mount || self.entry.path == other.entry.path,
        }
    }
}

fn mount_path_identity(path: &str) -> String {
    let mut root = std::path::PathBuf::new();
    let mut after_gvfs = false;
    for component in std::path::Path::new(path).components() {
        root.push(component.as_os_str());
        let name = component.as_os_str().to_string_lossy();
        if after_gvfs
            && name
                .split_once(':')
                .is_some_and(|(scheme, _)| uri::canonical_scheme(scheme).is_some())
        {
            // A URI child maps below its actual GVFS mount; the mount root and
            // its default/home folder are two representations of that session.
            return normalize_path(&root.to_string_lossy());
        }
        after_gvfs = name == "gvfs";
    }
    // Non-GVFS paths require an exact match; never guess another filesystem root.
    normalize_path(path)
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
        Ok(result) => result,
        Err(error) => Err(NetworkError::new(
            NetworkErrorCode::TaskFailed,
            format!("network listing failed: {error}"),
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saved_server_root_home_and_gvfs_mount_are_one_connection() {
        let root = "/run/user/1000/gvfs/sftp:host=server,user=alice";
        let mut entries = to_network_entries(&[
            mount("alice on server", root, "sftp"),
            mount("Home from GIO", "sftp://alice@server/home/alice", "sftp"),
        ]);
        let saved = vec![
            saved::SavedNetworkConnection {
                uri: "sftp://alice@server/".into(),
                label: "Saved root".into(),
            },
            saved::SavedNetworkConnection {
                uri: "sftp://alice@server/home/alice".into(),
                label: "Saved home".into(),
            },
        ];
        merge_saved_connections(&mut entries, &saved, |uri| {
            Some(if uri.ends_with("/home/alice") {
                format!("{root}/home/alice")
            } else {
                root.to_string()
            })
        });
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].path, "sftp://alice@server/");
        assert_eq!(entries[0].name, "Saved root");
    }

    #[test]
    fn saved_home_hides_mount_root_without_changing_the_requested_folder() {
        let root = "/run/user/1000/gvfs/sftp:host=server";
        let mut entries = to_network_entries(&[mount("Mounted root", root, "sftp")]);
        let saved = vec![saved::SavedNetworkConnection {
            uri: "sftp://server/root".into(),
            label: "Saved home".into(),
        }];
        merge_saved_connections(&mut entries, &saved, |_| Some(format!("{root}/root")));
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].path, "sftp://server/root");
    }

    #[test]
    fn duplicate_connection_addresses_are_collapsed_offline_but_accounts_and_ports_are_not() {
        let saved = [
            "ssh://alice@server/home/alice",
            "sftp://alice@server/",
            "sftp://alice@server:22/",
            "sftp://bob@server/",
            "sftp://alice@server:2222/",
            "sftp://alice@other/",
        ]
        .map(|uri| saved::SavedNetworkConnection {
            uri: uri.into(),
            label: uri.into(),
        });
        let mut entries = Vec::new();
        merge_saved_connections(&mut entries, &saved, |_| None);
        assert_eq!(entries.len(), 4);
        assert!(entries
            .iter()
            .any(|entry| entry.path == "sftp://alice@server/"));
        assert!(entries
            .iter()
            .any(|entry| entry.path == "sftp://bob@server/"));
        assert!(entries
            .iter()
            .any(|entry| entry.path == "sftp://alice@server:2222/"));
        assert!(entries
            .iter()
            .any(|entry| entry.path == "sftp://alice@other/"));
    }

    #[test]
    fn unsaved_mount_and_discovered_home_are_one_entry_without_connecting() {
        let root = "/run/user/1000/gvfs/sftp:host=server,user=alice";
        let mut entries = to_network_entries(&[
            mount("Mounted", root, "sftp"),
            mount("Discovered", "sftp://alice@server/home/alice", "sftp"),
        ]);
        merge_saved_connections(&mut entries, &[], |_| Some(format!("{root}/home/alice")));
        assert_eq!(entries.len(), 1);
    }

    #[test]
    fn keeps_different_accounts_even_if_a_mapping_provider_returns_the_same_path() {
        let saved = [
            "sftp://alice@server/",
            "sftp://bob@server/",
            "sftp://alice@server:2222/",
        ]
        .map(|uri| saved::SavedNetworkConnection {
            uri: uri.into(),
            label: uri.into(),
        });
        let mut entries = Vec::new();
        merge_saved_connections(&mut entries, &saved, |_| {
            Some("/run/user/1000/gvfs/sftp:host=server".into())
        });
        assert_eq!(entries.len(), 3);
    }

    #[test]
    fn missing_account_requires_live_mount_proof_to_merge_and_does_not_hide_other_users() {
        let mut entries = to_network_entries(&[
            mount("Default account", "sftp://server/", "sftp"),
            mount("Alice", "sftp://alice@server/", "sftp"),
            mount("Bob", "sftp://bob@server/", "sftp"),
        ]);
        merge_saved_connections(&mut entries, &[], |_| None);
        assert_eq!(entries.len(), 3);
        merge_saved_connections(&mut entries, &[], |_| {
            Some("/run/user/1000/gvfs/sftp:host=server".into())
        });
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn discovered_mount_supplies_missing_mapping_without_reintroducing_a_local_duplicate() {
        let root = "/run/user/1000/gvfs/sftp:host=server,user=alice";
        let mut entries = to_network_entries(&[
            mount("Mounted", root, "sftp"),
            mount("Home", "sftp://alice@server/home/alice", "sftp"),
        ]);
        let saved = vec![saved::SavedNetworkConnection {
            uri: "sftp://alice@server/".into(),
            label: "Saved".into(),
        }];
        merge_saved_connections(&mut entries, &saved, |uri| {
            uri.ends_with("/home/alice")
                .then(|| format!("{root}/home/alice"))
        });
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].name, "Saved");
    }

    #[test]
    fn connection_merge_does_not_change_phone_cloud_or_ordinary_mount_paths() {
        let mut entries = vec![
            to_network_entry(&mount("Phone", "mtp://phone/", "mtp")),
            to_network_entry(&mount("Cloud", "rclone://remote/", "rclone")),
            to_network_entry(&mount("Mounted NFS", "/mnt/export", "nfs")),
        ];
        let before: Vec<_> = entries.iter().map(|entry| entry.path.clone()).collect();
        let mut mapping_calls = 0;
        merge_saved_connections(&mut entries, &[], |_| {
            mapping_calls += 1;
            None
        });
        assert_eq!(mapping_calls, 0);
        assert_eq!(entries.len(), 3);
        for path in before {
            assert!(entries.iter().any(|entry| entry.path == path));
        }
    }

    #[test]
    fn saved_servers_remain_available_offline_and_keep_account_and_child_paths() {
        let saved = vec![
            saved::SavedNetworkConnection {
                uri: "ssh://alice@server/Photos".into(),
                label: "Photos server".into(),
            },
            saved::SavedNetworkConnection {
                uri: "sftp://bob@server/".into(),
                label: "Other account".into(),
            },
        ];
        let mut entries = Vec::new();
        merge_saved_connections(&mut entries, &saved, |_| None);
        assert_eq!(entries.len(), 2);
        let photos = entries
            .iter()
            .find(|entry| entry.path == "sftp://alice@server/Photos")
            .unwrap();
        assert_eq!(photos.name, "Photos server");
        assert!(entries
            .iter()
            .any(|entry| entry.path == "sftp://bob@server/"));
        assert!(entries
            .iter()
            .all(|entry| entry.network && entry.kind == "dir"));
    }

    #[test]
    fn saved_connection_replaces_only_exact_discovery_and_mount_duplicates() {
        let mut entries = to_network_entries(&[
            mount("Discovered", "ssh://alice@server/", "sftp"),
            mount(
                "Mounted",
                "/run/user/1000/gvfs/sftp:host=server,user=alice",
                "sftp",
            ),
            mount(
                "Another server",
                "/run/user/1000/gvfs/sftp:host=other,user=alice",
                "sftp",
            ),
        ]);
        let saved = vec![saved::SavedNetworkConnection {
            uri: "sftp://alice@server/".into(),
            label: "Saved".into(),
        }];
        merge_saved_connections(&mut entries, &saved, |_| {
            Some("/run/user/1000/gvfs/sftp:host=server,user=alice".into())
        });
        assert_eq!(entries.len(), 2);
        assert!(entries
            .iter()
            .any(|entry| entry.name == "Saved" && entry.path == "sftp://alice@server/"));
        assert!(entries.iter().any(|entry| entry.name == "Another server"));
    }

    fn mount(label: &str, path: &str, fs: &str) -> MountInfo {
        MountInfo {
            label: label.to_string(),
            path: path.to_string(),
            fs: fs.to_string(),
            removable: false,
            size_bytes: None,
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

    #[test]
    fn network_and_cloud_root_icons_remain_dedicated() {
        let net_entry = to_network_entry(&mount("NAS", "smb://nas.local/share", "smb"));
        assert_eq!(net_entry.icon_id, CLOUD_ICON_ID);

        let remote = CloudRemote {
            id: "work".to_string(),
            label: "Work".to_string(),
            provider: cloud::types::CloudProviderKind::Onedrive,
            root_path: "rclone://work".to_string(),
            capabilities: cloud::types::CloudCapabilities::v1_core_rw(),
        };
        let remote_entry = to_cloud_network_entry(&remote);
        assert_eq!(remote_entry.icon_id, NETWORK_ICON_ID);
        let caps = remote_entry.capabilities.unwrap();
        assert!(caps.can_list && caps.can_mkdir && caps.can_create_file);
        assert!(
            !caps.can_delete
                && !caps.can_rename
                && !caps.can_move
                && !caps.can_copy
                && !caps.can_trash
        );
        assert!(!caps.can_archive && !caps.can_external_copy && !caps.can_advanced_rename);
    }
}
