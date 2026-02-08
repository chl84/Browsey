//! Duplicate detection commands.
//!
//! Current strategy:
//! 1) coarse filter by byte length
//! 2) byte-for-byte comparison for same-length candidates (early exit on mismatch)

mod scan;

use crate::{
    commands::tasks::CancelState,
    commands::fs::expand_path,
    fs_utils::{check_no_symlink_components, sanitize_path_follow},
};
use serde::Serialize;
use std::{
    path::PathBuf,
    sync::atomic::Ordering,
};
use tauri::Emitter;

struct DuplicateScanInput {
    target: PathBuf,
    start: PathBuf,
    target_len: u64,
}

#[derive(Serialize, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum DuplicateScanPhase {
    Collecting,
    Comparing,
    Done,
}

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateScanProgress {
    pub phase: DuplicateScanPhase,
    pub percent: u8,
    pub scanned_files: u64,
    pub candidate_files: u64,
    pub compared_files: u64,
    pub matched_files: u64,
    pub done: bool,
    pub error: Option<String>,
    pub duplicates: Option<Vec<String>>,
}

#[tauri::command]
pub async fn check_duplicates(target_path: String, start_path: String) -> Result<Vec<String>, String> {
    tauri::async_runtime::spawn_blocking(move || check_duplicates_sync(target_path, start_path))
        .await
        .unwrap_or_else(|e| Err(format!("duplicate scan task panicked: {e}")))
}

#[tauri::command]
pub fn check_duplicates_stream(
    app: tauri::AppHandle,
    cancel: tauri::State<'_, CancelState>,
    target_path: String,
    start_path: String,
    progress_event: Option<String>,
) -> Result<(), String> {
    let progress_event = progress_event.ok_or_else(|| "progress_event is required".to_string())?;
    let cancel_state = cancel.inner().clone();

    tauri::async_runtime::spawn_blocking(move || {
        let send = |payload: DuplicateScanProgress| {
            let _ = app.emit(&progress_event, payload);
        };
        let cancel_guard = match cancel_state.register(progress_event.clone()) {
            Ok(guard) => guard,
            Err(err) => {
                send(error_payload(err));
                return;
            }
        };
        let cancel_token = cancel_guard.token();
        let progress_cancel = cancel_token.clone();

        let input = match validate_scan_input(target_path, start_path) {
            Ok(input) => input,
            Err(err) => {
                send(error_payload(err));
                return;
            }
        };

        let outcome = scan::find_identical_files_with_progress(
            &input.target,
            &input.start,
            input.target_len,
            Some(cancel_token.as_ref()),
            |progress| {
                if progress_cancel.load(Ordering::Relaxed) {
                    return;
                }
                send(progress_payload(progress, false, None, None));
            },
        );

        if cancel_token.load(Ordering::Relaxed) {
            return;
        }

        if let scan::ScanResult::Completed { matches, progress } = outcome {
            send(progress_payload(
                progress,
                true,
                None,
                Some(to_string_paths(matches)),
            ));
        }
    });

    Ok(())
}

fn check_duplicates_sync(target_path: String, start_path: String) -> Result<Vec<String>, String> {
    let input = validate_scan_input(target_path, start_path)?;
    let matches = scan::find_identical_files(&input.target, &input.start, input.target_len);
    Ok(to_string_paths(matches))
}

fn validate_scan_input(target_path: String, start_path: String) -> Result<DuplicateScanInput, String> {
    let target = sanitize_path_follow(&target_path, false)?;
    check_no_symlink_components(&target)?;

    let target_meta =
        std::fs::symlink_metadata(&target).map_err(|e| format!("Failed to read target metadata: {e}"))?;
    if target_meta.file_type().is_symlink() {
        return Err("Target must be a regular file (symlinks are ignored)".into());
    }
    if !target_meta.is_file() {
        return Err("Target must be a file".into());
    }

    let start_expanded = expand_path(Some(start_path))?;
    let start = sanitize_path_follow(&start_expanded.to_string_lossy(), false)?;
    check_no_symlink_components(&start)?;

    let start_meta = std::fs::symlink_metadata(&start)
        .map_err(|e| format!("Failed to read start folder metadata: {e}"))?;
    if start_meta.file_type().is_symlink() {
        return Err("Start path must be a directory (symlinks are ignored)".into());
    }
    if !start_meta.is_dir() {
        return Err("Start path must be a directory".into());
    }

    Ok(DuplicateScanInput {
        target,
        start,
        target_len: target_meta.len(),
    })
}

fn to_string_paths(paths: Vec<PathBuf>) -> Vec<String> {
    paths
        .into_iter()
        .map(|path| path.to_string_lossy().into_owned())
        .collect()
}

fn map_phase(phase: scan::ScanPhase) -> DuplicateScanPhase {
    match phase {
        scan::ScanPhase::Collecting => DuplicateScanPhase::Collecting,
        scan::ScanPhase::Comparing => DuplicateScanPhase::Comparing,
        scan::ScanPhase::Done => DuplicateScanPhase::Done,
    }
}

fn progress_payload(
    progress: scan::ScanProgress,
    done: bool,
    error: Option<String>,
    duplicates: Option<Vec<String>>,
) -> DuplicateScanProgress {
    DuplicateScanProgress {
        phase: map_phase(progress.phase),
        percent: progress.percent,
        scanned_files: progress.scanned_files,
        candidate_files: progress.candidate_files,
        compared_files: progress.compared_files,
        matched_files: progress.matched_files,
        done,
        error,
        duplicates,
    }
}

fn error_payload(error: String) -> DuplicateScanProgress {
    DuplicateScanProgress {
        phase: DuplicateScanPhase::Done,
        percent: 100,
        scanned_files: 0,
        candidate_files: 0,
        compared_files: 0,
        matched_files: 0,
        done: true,
        error: Some(error),
        duplicates: None,
    }
}
