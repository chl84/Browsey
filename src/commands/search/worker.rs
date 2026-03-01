use super::{
    error::{SearchError, SearchErrorCode},
    SearchProgress,
};
use crate::{
    commands::fs::expand_path,
    commands::listing::{ListingFacetBuilder, ListingFacets},
    commands::search::query::{matches_query, parse_query, simple_name_contains_needle_lc},
    db,
    entry::{normalize_key_for_db, FsEntry},
    runtime_lifecycle,
    tasks::CancelState,
};
use std::collections::HashSet;
use std::sync::atomic::Ordering;
use tracing::{debug, warn};

const SEARCH_BATCH_SIZE: usize = 256;

fn map_db_error(error: db::DbError) -> SearchError {
    let code = match error.code() {
        db::DbErrorCode::OpenFailed
        | db::DbErrorCode::DataDirUnavailable
        | db::DbErrorCode::PermissionDenied
        | db::DbErrorCode::ReadOnlyFilesystem => SearchErrorCode::DatabaseOpenFailed,
        _ => SearchErrorCode::DatabaseReadFailed,
    };
    SearchError::new(code, error.to_string())
}

fn error_progress(error: SearchError) -> SearchProgress {
    SearchProgress {
        entries: Vec::new(),
        done: true,
        error_code: Some(error.code_str_value().to_string()),
        error: Some(error.to_string()),
        facets: Some(ListingFacets::default()),
    }
}

fn invalid_query_progress(error: impl ToString) -> SearchProgress {
    error_progress(SearchError::new(
        SearchErrorCode::InvalidQuery,
        format!("Invalid search query: {}", error.to_string()),
    ))
}

pub(super) fn run_search_stream(
    app: tauri::AppHandle,
    cancel_state: CancelState,
    path: Option<String>,
    query: String,
    progress_event: String,
) {
    let send = |entries: Vec<FsEntry>,
                done: bool,
                error_code: Option<String>,
                error: Option<String>,
                facets: Option<ListingFacets>| {
        let payload = SearchProgress {
            entries,
            done,
            error_code,
            error,
            facets,
        };
        let _ = runtime_lifecycle::emit_if_running(&app, &progress_event, payload);
    };

    let send_error = |error: SearchError| {
        let payload = error_progress(error);
        send(
            payload.entries,
            payload.done,
            payload.error_code,
            payload.error,
            payload.facets,
        );
    };

    let cancel_guard = match cancel_state.register(progress_event.clone()) {
        Ok(g) => g,
        Err(e) => {
            send_error(SearchError::new(SearchErrorCode::TaskFailed, e.to_string()));
            return;
        }
    };
    let cancel_token = cancel_guard.token();

    let needle = query.trim();
    if needle.is_empty() {
        send(Vec::new(), true, None, None, Some(ListingFacets::default()));
        return;
    }
    let parsed_query = match parse_query(needle) {
        Ok(q) => q,
        Err(e) => {
            let payload = invalid_query_progress(e);
            send(
                payload.entries,
                payload.done,
                payload.error_code,
                payload.error,
                payload.facets,
            );
            return;
        }
    };
    let simple_name_contains_needle_lc = simple_name_contains_needle_lc(&parsed_query);

    let target = match expand_path(path) {
        Ok(p) if p.exists() => p,
        Ok(_) => match dirs_next::home_dir() {
            Some(h) => h,
            None => {
                send_error(SearchError::new(
                    SearchErrorCode::NotFound,
                    "Start directory not found",
                ));
                return;
            }
        },
        Err(e) => {
            send_error(SearchError::from_external_message(e.to_string()));
            return;
        }
    };

    let star_set = match db::open().and_then(|conn| db::starred_set(&conn)) {
        Ok(set) => set,
        Err(error) => {
            send_error(map_db_error(error));
            return;
        }
    };

    let mut stack = vec![target];
    let mut seen: HashSet<String> = HashSet::new();
    let mut batch: Vec<FsEntry> = Vec::with_capacity(SEARCH_BATCH_SIZE);
    let mut facets = ListingFacetBuilder::default();

    while let Some(dir) = stack.pop() {
        if cancel_token.load(Ordering::Relaxed) || runtime_lifecycle::is_shutting_down(&app) {
            return;
        }

        let iter = match std::fs::read_dir(&dir) {
            Ok(i) => i,
            Err(err) => {
                if err.kind() == std::io::ErrorKind::PermissionDenied {
                    debug!(
                        "search read_dir permission denied: dir={} err={}",
                        dir.display(),
                        err
                    );
                } else {
                    warn!("search read_dir failed: dir={} err={}", dir.display(), err);
                }
                continue;
            }
        };

        for entry in iter.flatten() {
            if cancel_token.load(Ordering::Relaxed) || runtime_lifecycle::is_shutting_down(&app) {
                return;
            }

            let path = entry.path();
            let file_type = match entry.file_type() {
                Ok(ft) => ft,
                Err(_) => continue,
            };
            let is_link = file_type.is_symlink();
            let is_dir = file_type.is_dir();
            if let Some(needle_lc) = simple_name_contains_needle_lc.as_deref() {
                let name_lc = entry.file_name().to_string_lossy().to_lowercase();
                if !name_lc.contains(needle_lc) {
                    if is_dir && !is_link {
                        stack.push(path);
                    }
                    continue;
                }
            }
            {
                let meta = match std::fs::symlink_metadata(&path) {
                    Ok(m) => m,
                    Err(_) => continue,
                };
                let key = path.to_string_lossy().to_string();
                if seen.insert(key) {
                    let mut item = crate::entry::build_entry(&path, &meta, is_link, false);
                    if star_set.contains(&normalize_key_for_db(&path)) {
                        item.starred = true;
                    }
                    if matches_query(&item, &parsed_query) {
                        facets.add(&item);
                        batch.push(item);
                        if batch.len() >= SEARCH_BATCH_SIZE {
                            send(std::mem::take(&mut batch), false, None, None, None);
                        }
                    }
                }
            }

            if is_dir && !is_link {
                stack.push(path);
            }
        }
    }

    if !batch.is_empty() {
        send(batch, false, None, None, None);
    }

    if cancel_token.load(Ordering::Relaxed) || runtime_lifecycle::is_shutting_down(&app) {
        return;
    }
    send(Vec::new(), true, None, None, Some(facets.finish()));
}

#[cfg(test)]
mod tests {
    use super::{invalid_query_progress, SearchError, SearchErrorCode};

    #[test]
    fn invalid_query_progress_matches_search_error_payload_shape() {
        let payload = invalid_query_progress("Unclosed group at position 0");
        assert!(payload.done);
        assert!(payload.entries.is_empty());
        assert!(payload.facets.is_some());
        assert_eq!(payload.error_code.as_deref(), Some("invalid_query"));
        assert!(payload
            .error
            .as_deref()
            .unwrap_or_default()
            .contains("Invalid search query"));
    }

    #[test]
    fn error_progress_exposes_search_error_code() {
        let payload = super::error_progress(SearchError::new(
            SearchErrorCode::DatabaseOpenFailed,
            "db unavailable",
        ));
        assert_eq!(payload.error_code.as_deref(), Some("database_open_failed"));
        assert_eq!(payload.error.as_deref(), Some("db unavailable"));
    }
}
