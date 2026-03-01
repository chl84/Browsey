use super::SearchProgress;
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

fn invalid_query_progress(error: impl ToString) -> SearchProgress {
    SearchProgress {
        entries: Vec::new(),
        done: true,
        error: Some(format!("Invalid search query: {}", error.to_string())),
        facets: Some(ListingFacets::default()),
    }
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
                error: Option<String>,
                facets: Option<ListingFacets>| {
        let payload = SearchProgress {
            entries,
            done,
            error,
            facets,
        };
        let _ = runtime_lifecycle::emit_if_running(&app, &progress_event, payload);
    };

    let cancel_guard = match cancel_state.register(progress_event.clone()) {
        Ok(g) => g,
        Err(e) => {
            send(
                Vec::new(),
                true,
                Some(e.to_string()),
                Some(ListingFacets::default()),
            );
            return;
        }
    };
    let cancel_token = cancel_guard.token();

    let needle = query.trim();
    if needle.is_empty() {
        send(Vec::new(), true, None, Some(ListingFacets::default()));
        return;
    }
    let parsed_query = match parse_query(needle) {
        Ok(q) => q,
        Err(e) => {
            let payload = invalid_query_progress(e);
            send(payload.entries, payload.done, payload.error, payload.facets);
            return;
        }
    };
    let simple_name_contains_needle_lc = simple_name_contains_needle_lc(&parsed_query);

    let target = match expand_path(path) {
        Ok(p) if p.exists() => p,
        Ok(_) => match dirs_next::home_dir() {
            Some(h) => h,
            None => {
                send(
                    Vec::new(),
                    true,
                    Some("Start directory not found".to_string()),
                    Some(ListingFacets::default()),
                );
                return;
            }
        },
        Err(e) => {
            send(
                Vec::new(),
                true,
                Some(e.to_string()),
                Some(ListingFacets::default()),
            );
            return;
        }
    };

    let star_set = match db::open().and_then(|conn| db::starred_set(&conn)) {
        Ok(set) => set,
        Err(e) => {
            send(
                Vec::new(),
                true,
                Some(e.to_string()),
                Some(ListingFacets::default()),
            );
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
                            send(std::mem::take(&mut batch), false, None, None);
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
        send(batch, false, None, None);
    }

    if cancel_token.load(Ordering::Relaxed) || runtime_lifecycle::is_shutting_down(&app) {
        return;
    }
    send(Vec::new(), true, None, Some(facets.finish()));
}

#[cfg(test)]
mod tests {
    use super::invalid_query_progress;

    #[test]
    fn invalid_query_progress_matches_search_error_payload_shape() {
        let payload = invalid_query_progress("Unclosed group at position 0");
        assert!(payload.done);
        assert!(payload.entries.is_empty());
        assert!(payload.facets.is_some());
        assert!(payload
            .error
            .as_deref()
            .unwrap_or_default()
            .contains("Invalid search query"));
    }
}
