//! Streaming recursive search command that decorates entries with starred state.

mod types;
mod worker;

pub use types::SearchProgress;

use crate::{sorting::SortSpec, tasks::CancelState};

#[tauri::command]
pub fn search_stream(
    app: tauri::AppHandle,
    cancel: tauri::State<'_, CancelState>,
    path: Option<String>,
    query: String,
    _sort: Option<SortSpec>,
    progress_event: Option<String>,
) -> Result<(), String> {
    let progress_event = progress_event.ok_or_else(|| "progress_event is required".to_string())?;
    let cancel_state = cancel.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        worker::run_search_stream(app, cancel_state, path, query, progress_event);
    });

    Ok(())
}
