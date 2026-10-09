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

#[cfg(test)]
pub(super) fn rename_batch(
    provider: &impl CloudProvider,
    entries: Vec<RenameEntryRequest>,
) -> CloudCommandResult<CloudRenameResult> {
    rename_batch_with_progress(provider, entries, None, |_, _| {})
}

fn rename_batch_with_progress(
    provider: &impl CloudProvider,
    entries: Vec<RenameEntryRequest>,
    cancel: Option<&std::sync::atomic::AtomicBool>,
    report: impl Fn(u64, u64),
) -> CloudCommandResult<CloudRenameResult> {
    let check_cancel = || {
        if cancel.is_some_and(|token| token.load(std::sync::atomic::Ordering::Relaxed)) {
            Err(CloudCommandError::new(
                CloudCommandErrorCode::Cancelled,
                "Cloud rename cancelled",
            ))
        } else {
            Ok(())
        }
    };
    let mut sources = HashSet::new();
    let mut targets = HashSet::new();
    let mut pairs = Vec::new();
    let remotes = provider.list_remotes()?;
    // Preflight the entire plan before moving anything. Cycles/swaps are refused,
    // not simulated through destructive temporary remote names.
    for entry in entries {
        check_cancel()?;
        let source = super::parse_cloud_path_arg(entry.path)?;
        let parent = source.parent_dir_path().ok_or_else(|| {
            CloudCommandError::new(
                CloudCommandErrorCode::InvalidPath,
                "Cannot rename a cloud root",
            )
        })?;
        let target = parent
            .child_path(&entry.new_name)
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
        let source_key = source.drive_object_id().map_or_else(
            || key(&source),
            |id| (source.remote().to_owned(), format!("//gdrive/{id}")),
        );
        if !sources.insert(source_key) || !targets.insert(key(&target)) {
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
    let total = pairs.len() as u64;
    report(0, total);
    for (index, (source, target)) in pairs.into_iter().enumerate() {
        check_cancel()?;
        if source == target || source.rel_path() == target.rel_path() {
            report(index as u64 + 1, total);
            continue;
        }
        let operation = provider.move_entry(&source, &target, false, false, cancel);
        super::invalidate_cloud_write_paths(&[source.clone(), target.clone()]);
        match operation {
            Ok(()) => {
                result.renamed.push(target.to_string());
                report(index as u64 + 1, total);
            }
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
    app: tauri::AppHandle,
    cancel: tauri::State<'_, crate::tasks::CancelState>,
    progress_event: Option<String>,
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
    let guard = map_api_result(super::register_cloud_cancel(&cancel, &progress_event))?;
    let token = guard.as_ref().map(|guard| guard.token());
    let result = tauri::async_runtime::spawn_blocking(move || {
        super::limits::with_cloud_remote_permits(remotes, || {
            let provider = super::configured_rclone_provider().map_err(CloudCommandError::from)?;
            rename_batch_with_progress(&provider, entries, token.as_deref(), |done, total| {
                super::progress::items(&app, progress_event.as_deref(), done, total);
            })
        })
    })
    .await;
    map_api_result(super::map_spawn_result(
        result,
        "Cloud batch rename task failed",
    ))
}
