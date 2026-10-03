use super::{
    error::{map_api_result, CloudCommandError, CloudCommandErrorCode, CloudCommandResult},
    path::CloudPath,
    provider::CloudProvider,
};
use crate::{commands::rename::RenameEntryRequest, errors::api_error::ApiResult};
use serde::Serialize;
use std::collections::HashSet;

#[derive(Debug, Serialize)]
pub struct CloudRenameResult {
    pub renamed: Vec<String>,
    pub error: Option<String>,
}

pub(super) fn rename_batch(
    provider: &impl CloudProvider,
    entries: Vec<RenameEntryRequest>,
) -> CloudCommandResult<CloudRenameResult> {
    let mut sources = HashSet::new();
    let mut targets = HashSet::new();
    let mut pairs = Vec::new();
    let remotes = provider.list_remotes()?;
    // Preflight the entire plan before moving anything. Cycles/swaps are refused,
    // not simulated through destructive temporary remote names.
    for entry in entries {
        let source = super::parse_cloud_path_arg(entry.path)?;
        let parent = source.parent_dir_path().ok_or_else(|| {
            CloudCommandError::new(
                CloudCommandErrorCode::InvalidPath,
                "Cannot rename a cloud root",
            )
        })?;
        let target = parent
            .child_path(entry.new_name.trim())
            .map_err(super::map_cloud_path_error)?;
        let provider_kind = remotes
            .iter()
            .find(|remote| remote.id == source.remote())
            .map(|remote| remote.provider)
            .ok_or_else(|| {
                CloudCommandError::new(
                    CloudCommandErrorCode::InvalidConfig,
                    "Rename provider policy could not be verified",
                )
            })?;
        let key = |path: &CloudPath| {
            (
                path.remote().to_owned(),
                super::cloud_conflict_name_key(Some(provider_kind), path.rel_path()),
            )
        };
        if !sources.insert(key(&source)) || !targets.insert(key(&target)) {
            return Err(CloudCommandError::new(
                CloudCommandErrorCode::DestinationExists,
                "Duplicate source or destination in rename plan",
            ));
        }
        if provider.stat_path(&source)?.is_none() {
            return Err(CloudCommandError::new(
                CloudCommandErrorCode::NotFound,
                format!("Rename source disappeared: {source}"),
            ));
        }
        if source != target
            && key(&source) != key(&target)
            && provider.stat_path(&target)?.is_some()
        {
            return Err(CloudCommandError::new(
                CloudCommandErrorCode::DestinationExists,
                format!("Rename destination already exists: {target}"),
            ));
        }
        pairs.push((source, target));
    }
    let mut result = CloudRenameResult {
        renamed: Vec::new(),
        error: None,
    };
    for (source, target) in pairs {
        if source == target {
            continue;
        }
        let operation = provider.move_entry(&source, &target, false, false, None);
        super::invalidate_cloud_write_paths(&[source.clone(), target.clone()]);
        match operation {
            Ok(()) => result.renamed.push(target.to_string()),
            Err(error) => {
                result.error = Some(format!("Renamed {} items before stopping at {source}: {error}. Refresh before retrying; cloud rename has no undo.", result.renamed.len()));
                break;
            }
        }
    }
    Ok(result)
}

#[tauri::command]
pub async fn rename_cloud_entries(
    entries: Vec<RenameEntryRequest>,
) -> ApiResult<CloudRenameResult> {
    map_api_result(super::ensure_cloud_enabled())?;
    let remotes = map_api_result(
        entries
            .iter()
            .map(|entry| {
                super::parse_cloud_path_arg(entry.path.clone()).map(|path| path.remote().to_owned())
            })
            .collect::<CloudCommandResult<Vec<_>>>(),
    )?;
    let result = tauri::async_runtime::spawn_blocking(move || {
        super::limits::with_cloud_remote_permits(remotes, || {
            let provider = super::configured_rclone_provider().map_err(CloudCommandError::from)?;
            rename_batch(&provider, entries)
        })
    })
    .await;
    map_api_result(super::map_spawn_result(
        result,
        "Cloud batch rename task failed",
    ))
}
