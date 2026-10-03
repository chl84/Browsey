//! Protected local staging for archives and copy-only external exports.
//! Failure/cancellation retains local data and never deletes cloud originals.
use super::{
    error::{map_api_result, CloudCommandError, CloudCommandErrorCode, CloudCommandResult},
    path::CloudPath,
    provider::CloudProvider,
    workspace,
};
use crate::{
    errors::api_error::{ApiError, ApiResult},
    tasks::CancelState,
};
use std::{
    fs,
    path::{Path, PathBuf},
    sync::{atomic::AtomicBool, Arc},
};
use tauri::Emitter;
use zeroize::Zeroizing;

fn phase(app: &tauri::AppHandle, event: Option<&str>, label: &str) {
    if let Some(event) = event {
        let _ = app.emit(
            event,
            serde_json::json!({
                "bytes": 0, "total": 0, "finished": false, "phase": label,
            }),
        );
    }
}

pub(crate) fn is_cloud_archive_candidate(raw: &str) -> bool {
    let Ok(path) = CloudPath::parse(raw) else {
        return false;
    };
    let Ok(name) = path.leaf_name() else {
        return false;
    };
    let lower = name.to_lowercase();
    [
        ".zip", ".rar", ".7z", ".tar", ".gz", ".tgz", ".bz2", ".tbz2", ".xz", ".txz", ".zst",
        ".tzst",
    ]
    .iter()
    .any(|suffix| lower.ends_with(suffix))
}

fn paths_and_parent(paths: Vec<String>) -> CloudCommandResult<(Vec<CloudPath>, CloudPath)> {
    let paths = paths
        .into_iter()
        .map(super::parse_cloud_path_arg)
        .collect::<CloudCommandResult<Vec<_>>>()?;
    let parent = paths
        .first()
        .and_then(CloudPath::parent_dir_path)
        .ok_or_else(|| {
            CloudCommandError::new(
                CloudCommandErrorCode::InvalidPath,
                "Select cloud entries, not a remote root",
            )
        })?;
    let mut names = std::collections::HashSet::new();
    for path in &paths {
        if path.parent_dir_path().as_ref() != Some(&parent)
            || !names.insert(
                path.leaf_name()
                    .map_err(super::map_cloud_path_error)?
                    .to_owned(),
            )
        {
            return Err(CloudCommandError::new(
                CloudCommandErrorCode::InvalidPath,
                "Cloud staging requires distinct entries from the same folder",
            ));
        }
    }
    Ok((paths, parent))
}

fn retained(mut error: ApiError, stage: &Path) -> ApiError {
    error.message = format!(
        "{} Local working data retained at {}. Cloud originals were not removed.",
        error.message,
        stage.display()
    );
    error
}

fn download(
    paths: &[CloudPath],
    stage: &Path,
    app: &tauri::AppHandle,
    cancel: Option<Arc<AtomicBool>>,
    event: Option<String>,
) -> ApiResult<Vec<String>> {
    phase(app, event.as_deref(), "Downloading cloud entries…");
    let inputs = stage.join("inputs");
    map_api_result(workspace::private_dir(&inputs))?;
    let mut local = Vec::new();
    for path in paths {
        let name = map_api_result(path.leaf_name().map_err(super::map_cloud_path_error))?;
        let target = inputs.join(name).to_string_lossy().into_owned();
        crate::commands::transfer::copy_staged_entry(
            path.to_string(),
            target.clone(),
            app.clone(),
            cancel.clone(),
            event.clone(),
        )?;
        local.push(target);
    }
    Ok(local)
}

#[tauri::command]
pub async fn prepare_cloud_external_copy(
    paths: Vec<String>,
    app: tauri::AppHandle,
    cancel: tauri::State<'_, CancelState>,
    progress_event: Option<String>,
) -> ApiResult<Vec<String>> {
    map_api_result(super::ensure_cloud_enabled())?;
    let (paths, _) = map_api_result(paths_and_parent(paths))?;
    let guard = map_api_result(super::register_cloud_cancel(
        cancel.inner(),
        &progress_event,
    ))?;
    let token = guard.as_ref().map(|guard| guard.token());
    let result = tauri::async_runtime::spawn_blocking(move || {
        let stage = map_api_result(workspace::operation_dir("export"))?;
        download(&paths, &stage, &app, token, progress_event)
            .map_err(|error| retained(error, &stage))
    })
    .await;
    result.map_err(|error| {
        ApiError::new("task_failed", format!("Cloud export task failed: {error}"))
    })?
}

#[tauri::command]
pub async fn compress_cloud_entries(
    paths: Vec<String>,
    name: String,
    level: Option<u32>,
    password: Option<String>,
    app: tauri::AppHandle,
    cancel: tauri::State<'_, CancelState>,
    progress_event: Option<String>,
) -> ApiResult<String> {
    map_api_result(super::ensure_cloud_enabled())?;
    let (paths, parent) = map_api_result(paths_and_parent(paths))?;
    let target = map_api_result(
        parent
            .child_path(&name)
            .map_err(super::map_cloud_path_error),
    )?;
    let guard = map_api_result(super::register_cloud_cancel(
        cancel.inner(),
        &progress_event,
    ))?;
    let token = guard.as_ref().map(|guard| guard.token());
    let password = password.map(Zeroizing::new);
    let result = tauri::async_runtime::spawn_blocking(move || {
        let stage = map_api_result(workspace::operation_dir("compress"))?;
        let result = (|| {
            let provider = map_api_result(
                super::configured_rclone_provider().map_err(CloudCommandError::from),
            )?;
            if map_api_result(provider.stat_path(&target))?.is_some() {
                return Err(ApiError::new(
                    "destination_exists",
                    "The cloud archive already exists",
                ));
            }
            let local = download(&paths, &stage, &app, token.clone(), progress_event.clone())?;
            phase(&app, progress_event.as_deref(), "Creating local archive…");
            let archive = crate::commands::compress::compress_staged(
                Some(app.clone()),
                local,
                name,
                level,
                progress_event.clone(),
                password.as_deref().map(String::as_str),
                token.clone(),
            )?;
            phase(&app, progress_event.as_deref(), "Uploading archive…");
            let _permit = super::limits_for_staging(vec![target.remote().to_owned()]);
            let result = map_api_result(provider.upload_new_file(
                Path::new(&archive),
                &target,
                token.as_deref(),
            ));
            super::invalidate_cloud_write_paths(std::slice::from_ref(&target));
            result?;
            Ok(target.to_string())
        })();
        result.map_err(|error| retained(error, &stage))
    })
    .await;
    result.map_err(|error| {
        ApiError::new(
            "task_failed",
            format!("Cloud compression task failed: {error}"),
        )
    })?
}

#[tauri::command]
pub async fn extract_cloud_archive(
    path: String,
    password: Option<String>,
    app: tauri::AppHandle,
    cancel: tauri::State<'_, CancelState>,
    progress_event: Option<String>,
) -> ApiResult<crate::commands::decompress::ExtractResult> {
    map_api_result(super::ensure_cloud_enabled())?;
    let (paths, parent) = map_api_result(paths_and_parent(vec![path]))?;
    let guard = map_api_result(super::register_cloud_cancel(
        cancel.inner(),
        &progress_event,
    ))?;
    let token = guard.as_ref().map(|guard| guard.token());
    let password = password.map(Zeroizing::new);
    let result = tauri::async_runtime::spawn_blocking(move || {
        let stage = map_api_result(workspace::operation_dir("extract"))?;
        let result = (|| {
            let local = download(&paths, &stage, &app, token.clone(), progress_event.clone())?;
            phase(&app, progress_event.as_deref(), "Extracting local archive…");
            let mut extracted = crate::commands::decompress::extract_staged(
                Some(&app),
                local[0].clone(),
                progress_event.clone(),
                password.as_deref().map(String::as_str),
                token.clone(),
            )?;
            let output = PathBuf::from(&extracted.destination);
            reject_upload_symlinks(&output)?;
            let suffix = stage
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or_else(|| ApiError::new("invalid_path", "Invalid staging directory"))?;
            let name = output
                .file_name()
                .and_then(|name| name.to_str())
                .ok_or_else(|| ApiError::new("invalid_path", "Invalid extraction output"))?;
            let target = map_api_result(
                parent
                    .child_path(&format!("{name}-{suffix}"))
                    .map_err(super::map_cloud_path_error),
            )?;
            phase(
                &app,
                progress_event.as_deref(),
                "Uploading extracted folder…",
            );
            let result = crate::commands::transfer::copy_staged_entry(
                output.to_string_lossy().into_owned(),
                target.to_string(),
                app,
                token,
                progress_event,
            );
            super::invalidate_cloud_write_paths(std::slice::from_ref(&target));
            result?;
            extracted.destination = target.to_string();
            Ok(extracted)
        })();
        result.map_err(|error| retained(error, &stage))
    })
    .await;
    result.map_err(|error| {
        ApiError::new(
            "task_failed",
            format!("Cloud extraction task failed: {error}"),
        )
    })?
}

fn reject_upload_symlinks(path: &Path) -> ApiResult<()> {
    for entry in walkdir::WalkDir::new(path).follow_links(false) {
        let entry = entry.map_err(|error| {
            ApiError::new(
                "task_failed",
                format!("Cannot inspect staged output: {error}"),
            )
        })?;
        let metadata =
            map_api_result(fs::symlink_metadata(entry.path()).map_err(workspace::io_error))?;
        if metadata.file_type().is_symlink() || (!metadata.is_file() && !metadata.is_dir()) {
            return Err(ApiError::new("unsupported", "Cloud archive upload does not support symlinks or special files; use the retained local output"));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> PathBuf {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "browsey-staging-test-{}-{stamp}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        path
    }

    #[test]
    fn retained_errors_preserve_password_and_cancellation_codes() {
        for code in [
            "archive_password_required",
            "archive_invalid_password",
            "cancelled",
        ] {
            let error = retained(
                ApiError::new(code, "original failure"),
                Path::new("/private/stage"),
            );
            assert_eq!(error.code, code);
            assert!(error.message.contains("/private/stage"));
            assert!(error.message.contains("Cloud originals were not removed"));
        }
    }

    #[cfg(unix)]
    #[test]
    fn staged_upload_accepts_empty_directories_but_rejects_symlinks() {
        let root = fixture();
        fs::create_dir(root.join("empty")).unwrap();
        fs::write(root.join("regular.txt"), "contents").unwrap();
        assert!(reject_upload_symlinks(&root).is_ok());
        std::os::unix::fs::symlink("regular.txt", root.join("link")).unwrap();
        assert_eq!(
            reject_upload_symlinks(&root).unwrap_err().code,
            "unsupported"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn staged_archive_engine_uses_shared_cancellation_without_removing_inputs() {
        let root = fixture();
        let input = root.join("original.txt");
        fs::write(&input, "keep original").unwrap();
        let token = Arc::new(AtomicBool::new(true));
        let result = crate::commands::compress::compress_staged(
            None,
            vec![input.to_string_lossy().into_owned()],
            "cancelled.zip".into(),
            None,
            Some("shared-stage-task".into()),
            Some("password"),
            Some(token),
        );
        assert_eq!(result.unwrap_err().code, "cancelled");
        assert_eq!(fs::read_to_string(&input).unwrap(), "keep original");
        fs::remove_dir_all(root).unwrap();
    }
    #[test]
    fn staging_rejects_roots_mixed_parents_and_duplicate_entries() {
        for paths in [
            vec![],
            vec!["rclone://r"],
            vec!["rclone://r/a", "rclone://other/b"],
            vec!["rclone://r/a", "rclone://r/a"],
        ] {
            assert!(paths_and_parent(paths.into_iter().map(str::to_owned).collect()).is_err());
        }
        assert!(paths_and_parent(vec![
            "rclone://r/folder/a".into(),
            "rclone://r/folder/b".into()
        ])
        .is_ok());
    }
}
