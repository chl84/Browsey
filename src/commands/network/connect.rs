//! High-level network URI connect flow used by the frontend.

use serde::Serialize;

#[cfg(not(target_os = "windows"))]
use super::mounts;
use super::{
    discovery,
    uri::{self, NetworkUriKind},
};

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ConnectNetworkUriResult {
    pub kind: NetworkUriKind,
    pub normalized_uri: Option<String>,
    pub mounted_path: Option<String>,
}

#[cfg(not(target_os = "windows"))]
#[tauri::command]
pub async fn connect_network_uri(
    uri: String,
    app: tauri::AppHandle,
) -> Result<ConnectNetworkUriResult, String> {
    let classified = uri::classify_uri(&uri);
    let kind = classified.kind;
    let normalized_uri = classified
        .normalized_uri
        .clone()
        .or_else(|| Some(uri.trim().to_string()));

    match kind {
        NetworkUriKind::NotUri | NetworkUriKind::Unsupported => Ok(ConnectNetworkUriResult {
            kind,
            normalized_uri: classified.normalized_uri,
            mounted_path: None,
        }),
        NetworkUriKind::External => {
            let target = normalized_uri.unwrap_or_default();
            discovery::open_network_uri(target)?;
            Ok(ConnectNetworkUriResult {
                kind,
                normalized_uri: classified.normalized_uri,
                mounted_path: None,
            })
        }
        NetworkUriKind::Mountable => {
            let target = normalized_uri.unwrap_or_default();
            mounts::mount_partition(target.clone(), app).await?;
            let mounted_path =
                uri::resolve_mounted_path_for_uri_in_mounts(&target, &mounts::list_mounts_sync());
            Ok(ConnectNetworkUriResult {
                kind,
                normalized_uri: classified.normalized_uri,
                mounted_path,
            })
        }
    }
}

#[cfg(target_os = "windows")]
#[tauri::command]
pub async fn connect_network_uri(uri: String) -> Result<ConnectNetworkUriResult, String> {
    let classified = uri::classify_uri(&uri);
    let kind = classified.kind;
    let normalized_uri = classified
        .normalized_uri
        .clone()
        .or_else(|| Some(uri.trim().to_string()));

    match kind {
        NetworkUriKind::NotUri | NetworkUriKind::Unsupported => Ok(ConnectNetworkUriResult {
            kind,
            normalized_uri: classified.normalized_uri,
            mounted_path: None,
        }),
        NetworkUriKind::External => {
            let target = normalized_uri.unwrap_or_default();
            discovery::open_network_uri(target)?;
            Ok(ConnectNetworkUriResult {
                kind,
                normalized_uri: classified.normalized_uri,
                mounted_path: None,
            })
        }
        NetworkUriKind::Mountable => Err("Network mounts are not supported on Windows yet".into()),
    }
}
