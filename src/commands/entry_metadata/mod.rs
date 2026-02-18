//! Metadata helpers used by the properties modal.

use crate::commands::network::extra_metadata::{
    build_network_uri_extra_metadata, looks_like_uri_path,
};
use crate::entry::{entry_times, EntryTimes};
use crate::errors::api_error::ApiResult;
use crate::fs_utils::sanitize_path_follow;
use crate::metadata::{collect_extra_metadata, types::ExtraMetadataResult};
use error::{map_api_result, EntryMetadataError, EntryMetadataErrorCode, EntryMetadataResult};
use std::fs;
use std::path::PathBuf;

mod error;

#[tauri::command]
pub fn entry_times_cmd(path: String) -> ApiResult<EntryTimes> {
    map_api_result(entry_times_cmd_impl(path))
}

fn entry_times_cmd_impl(path: String) -> EntryMetadataResult<EntryTimes> {
    let pb = PathBuf::from(path);
    entry_times(&pb).map_err(EntryMetadataError::from_external_message)
}

#[tauri::command]
pub fn entry_kind_cmd(path: String) -> ApiResult<String> {
    map_api_result(entry_kind_cmd_impl(path))
}

fn entry_kind_cmd_impl(path: String) -> EntryMetadataResult<String> {
    let pb =
        sanitize_path_follow(&path, false).map_err(EntryMetadataError::from_external_message)?;
    let meta = fs::metadata(&pb).map_err(|error| {
        EntryMetadataError::new(
            EntryMetadataErrorCode::MetadataReadFailed,
            format!("Failed to read metadata: {error}"),
        )
    })?;
    if meta.is_dir() {
        Ok("dir".into())
    } else {
        Ok("file".into())
    }
}

#[tauri::command]
pub fn entry_extra_metadata_cmd(path: String) -> ApiResult<ExtraMetadataResult> {
    map_api_result(entry_extra_metadata_cmd_impl(path))
}

fn entry_extra_metadata_cmd_impl(path: String) -> EntryMetadataResult<ExtraMetadataResult> {
    if looks_like_uri_path(&path) {
        return Ok(build_network_uri_extra_metadata(&path));
    }
    let pb =
        sanitize_path_follow(&path, false).map_err(EntryMetadataError::from_external_message)?;
    collect_extra_metadata(&pb).map_err(EntryMetadataError::from_external_message)
}
