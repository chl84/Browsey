//! Streaming recursive search command that decorates entries with starred state.

mod error;
mod query;
mod types;
mod worker;

pub use types::SearchProgress;

use crate::errors::api_error::ApiResult;
use crate::{sorting::SortSpec, tasks::CancelState};
use error::{map_api_result, SearchError, SearchErrorCode};

#[tauri::command]
pub fn search_stream(
    app: tauri::AppHandle,
    cancel: tauri::State<'_, CancelState>,
    path: Option<String>,
    query: String,
    _sort: Option<SortSpec>,
    progress_event: Option<String>,
) -> ApiResult<()> {
    let progress_event = progress_event.ok_or_else(|| {
        SearchError::new(SearchErrorCode::InvalidInput, "progress_event is required")
    });
    let progress_event = match progress_event {
        Ok(value) => value,
        Err(error) => return map_api_result(Err(error)),
    };
    let cancel_state = cancel.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        worker::run_search_stream(app, cancel_state, path, query, progress_event);
    });

    map_api_result(Ok(()))
}
