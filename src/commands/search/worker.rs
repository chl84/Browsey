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

#[derive(Debug, PartialEq, Eq)]
enum SearchRoot {
    Local(std::path::PathBuf),
    Cloud(crate::commands::cloud::path::CloudPath),
}

fn resolve_search_root(path: Option<String>) -> super::error::SearchResult<SearchRoot> {
    let raw = path
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| {
            SearchError::new(
                SearchErrorCode::InvalidInput,
                "A search start directory is required",
            )
        })?;
    if raw.starts_with("rclone://") {
        return crate::commands::cloud::path::CloudPath::parse(&raw)
            .map(SearchRoot::Cloud)
            .map_err(|error| SearchError::new(SearchErrorCode::InvalidPath, error.to_string()));
    }
    let target = expand_path(Some(raw)).map_err(SearchError::from)?;
    if !target.is_absolute() {
        return Err(SearchError::new(
            SearchErrorCode::InvalidPath,
            "Search start directory must be absolute",
        ));
    }
    let metadata = std::fs::metadata(&target).map_err(|error| {
        SearchError::new(
            if error.kind() == std::io::ErrorKind::NotFound {
                SearchErrorCode::NotFound
            } else {
                SearchErrorCode::InvalidPath
            },
            format!("Cannot access search start directory: {error}"),
        )
    })?;
    if !metadata.is_dir() {
        return Err(SearchError::new(
            SearchErrorCode::InvalidPath,
            "Search start path is not a directory",
        ));
    }
    Ok(SearchRoot::Local(target))
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

    let target = match resolve_search_root(path) {
        Ok(target) => target,
        Err(error) => {
            send_error(error);
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

    let cancelled =
        || cancel_token.load(Ordering::Relaxed) || runtime_lifecycle::is_shutting_down(&app);
    match target {
        SearchRoot::Local(target) => scan_search(
            target,
            &parsed_query,
            simple_name_contains_needle_lc.as_deref(),
            &star_set,
            cancelled,
            send,
        ),
        SearchRoot::Cloud(target) => {
            if let Err(error) = super::cloud::run_cloud_search(
                target,
                &parsed_query,
                &star_set,
                &cancel_token,
                cancelled,
                |payload| {
                    send(
                        payload.entries,
                        payload.done,
                        payload.error_code,
                        payload.error,
                        payload.facets,
                    );
                },
            ) {
                if !cancelled() {
                    send_error(error);
                }
            }
        }
    }
}

fn scan_search(
    target: std::path::PathBuf,
    parsed_query: &super::query::Expr,
    simple_name_contains_needle_lc: Option<&str>,
    star_set: &HashSet<String>,
    mut cancelled: impl FnMut() -> bool,
    mut send: impl FnMut(Vec<FsEntry>, bool, Option<String>, Option<String>, Option<ListingFacets>),
) {
    let mut stack = vec![target];
    let mut seen: HashSet<String> = HashSet::new();
    let mut batch: Vec<FsEntry> = Vec::with_capacity(SEARCH_BATCH_SIZE);
    let mut facets = ListingFacetBuilder::default();

    while let Some(dir) = stack.pop() {
        if cancelled() {
            return;
        }

        let iter = match std::fs::read_dir(&dir) {
            Ok(i) => i,
            Err(err) => {
                if err.kind() == std::io::ErrorKind::PermissionDenied {
                    debug!(
                        path = %dir.display(),
                        error = %err,
                        "search read_dir permission denied"
                    );
                } else {
                    warn!(path = %dir.display(), error = %err, "search read_dir failed");
                }
                continue;
            }
        };

        for entry in iter.flatten() {
            if cancelled() {
                return;
            }

            let path = entry.path();
            let file_type = match entry.file_type() {
                Ok(ft) => ft,
                Err(_) => continue,
            };
            let is_link = file_type.is_symlink();
            let is_dir = file_type.is_dir();
            if let Some(needle_lc) = simple_name_contains_needle_lc {
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
                    if matches_query(&item, parsed_query) {
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

    if cancelled() {
        return;
    }
    send(Vec::new(), true, None, None, Some(facets.finish()));
}

#[cfg(all(test, target_os = "linux"))]
mod measurements;

#[cfg(test)]
mod tests {
    use super::{
        invalid_query_progress, resolve_search_root, SearchError, SearchErrorCode, SearchRoot,
    };

    #[test]
    fn search_root_rejects_implicit_relative_and_malformed_cloud_paths() {
        for path in [
            None,
            Some(""),
            Some("   "),
            Some("relative"),
            Some("rclone://Generated/owned/../outside"),
        ] {
            assert!(resolve_search_root(path.map(str::to_string)).is_err());
        }
        assert!(matches!(
            resolve_search_root(Some("rclone://Generated/owned".to_string())).unwrap(),
            SearchRoot::Cloud(_)
        ));
    }

    #[test]
    fn search_root_never_falls_back_when_directory_is_missing_or_a_file() {
        let temp = std::env::temp_dir().join(format!(
            "browsey-search-root-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&temp).unwrap();
        assert_eq!(
            resolve_search_root(Some(temp.to_string_lossy().into_owned())).unwrap(),
            SearchRoot::Local(temp.clone())
        );
        let missing = temp.join("missing");
        assert_eq!(
            resolve_search_root(Some(missing.to_string_lossy().into_owned()))
                .unwrap_err()
                .code_str_value(),
            "not_found"
        );
        let file = temp.join("generated.txt");
        std::fs::write(&file, b"generated search fixture").unwrap();
        assert_eq!(
            resolve_search_root(Some(file.to_string_lossy().into_owned()))
                .unwrap_err()
                .code_str_value(),
            "invalid_path"
        );
        std::fs::remove_file(file).unwrap();
        std::fs::remove_dir(&temp).unwrap();
        assert_eq!(
            resolve_search_root(Some(temp.to_string_lossy().into_owned()))
                .unwrap_err()
                .code_str_value(),
            "not_found"
        );
    }

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
