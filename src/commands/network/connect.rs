//! High-level network URI connect flow used by the frontend.

use crate::errors::api_error::ApiResult;
use serde::Serialize;

#[cfg(target_os = "windows")]
use super::error::NetworkErrorCode;
#[cfg(not(target_os = "windows"))]
use super::mounts;
use super::{
    discovery,
    error::{map_api_result, NetworkResult},
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
) -> ApiResult<ConnectNetworkUriResult> {
    map_api_result(connect_network_uri_impl(uri, app).await)
}

#[cfg(not(target_os = "windows"))]
async fn connect_network_uri_impl(
    uri: String,
    app: tauri::AppHandle,
) -> NetworkResult<ConnectNetworkUriResult> {
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
            discovery::open_network_uri_impl(target)?;
            Ok(ConnectNetworkUriResult {
                kind,
                normalized_uri: classified.normalized_uri,
                mounted_path: None,
            })
        }
        NetworkUriKind::Mountable => {
            let target = normalized_uri.unwrap_or_default();
            mounts::mount_partition_impl(target.clone(), app).await?;
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
pub async fn connect_network_uri(uri: String) -> ApiResult<ConnectNetworkUriResult> {
    map_api_result(connect_network_uri_impl(uri).await)
}

#[cfg(target_os = "windows")]
async fn connect_network_uri_impl(uri: String) -> NetworkResult<ConnectNetworkUriResult> {
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
            discovery::open_network_uri_impl(target)?;
            Ok(ConnectNetworkUriResult {
                kind,
                normalized_uri: classified.normalized_uri,
                mounted_path: None,
            })
        }
        NetworkUriKind::Mountable => Err(NetworkError::new(
            NetworkErrorCode::UnsupportedUri,
            "Network mounts are not supported on Windows yet",
        )),
    }
}
