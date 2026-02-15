//! Streaming recursive search command that decorates entries with starred state.

use super::tasks::CancelState;
use crate::{
    commands::fs::expand_path,
    commands::listing::{ListingFacetBuilder, ListingFacets},
    db,
    entry::{normalize_key_for_db, FsEntry},
    runtime_lifecycle,
    sorting::SortSpec,
};
use serde::Serialize;
use std::collections::HashSet;
use std::sync::atomic::Ordering;
use tracing::{debug, warn};

#[derive(Serialize, Clone)]
pub struct SearchProgress {
    pub entries: Vec<FsEntry>,
    pub done: bool,
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub facets: Option<ListingFacets>,
}

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
                send(Vec::new(), true, Some(e), Some(ListingFacets::default()));
                return;
            }
        };
        let cancel_token = cancel_guard.token();

        let needle = query.trim();
        if needle.is_empty() {
            send(Vec::new(), true, None, Some(ListingFacets::default()));
            return;
        }

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
                send(Vec::new(), true, Some(e), Some(ListingFacets::default()));
                return;
            }
        };

        let star_set = match db::open().and_then(|conn| db::starred_set(&conn)) {
            Ok(set) => set,
            Err(e) => {
                send(Vec::new(), true, Some(e), Some(ListingFacets::default()));
                return;
            }
        };

        let mut stack = vec![target];
        let mut seen: HashSet<String> = HashSet::new();
        let needle_lc = needle.to_lowercase();
        const BATCH: usize = 256;
        let mut batch: Vec<FsEntry> = Vec::with_capacity(BATCH);
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
                if cancel_token.load(Ordering::Relaxed) || runtime_lifecycle::is_shutting_down(&app)
                {
                    return;
                }
                let path = entry.path();
                let file_type = match entry.file_type() {
                    Ok(ft) => ft,
                    Err(_) => continue,
                };
                let is_link = file_type.is_symlink();
                let name_lc = entry.file_name().to_string_lossy().to_lowercase();
                let is_dir = file_type.is_dir();

                if name_lc.contains(&needle_lc) {
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
                        facets.add(&item);
                        batch.push(item);
                        if batch.len() >= BATCH {
                            send(std::mem::take(&mut batch), false, None, None);
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
    });

    Ok(())
}
