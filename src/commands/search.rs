//! Recursive search command that decorates entries with starred state.

use crate::{
    commands::fs::expand_path,
    db,
    entry::{normalize_key_for_db, FsEntry},
    search::search_recursive,
    sorting::{sort_entries, SortSpec},
};
use serde::Serialize;
use std::collections::HashSet;
use std::path::Path;
use tauri::Emitter;
use tracing::warn;

#[tauri::command]
pub async fn search(
    path: Option<String>,
    query: String,
    sort: Option<SortSpec>,
) -> Result<Vec<FsEntry>, String> {
    let path_arg = path.clone();
    let query_arg = query.clone();
    tauri::async_runtime::spawn_blocking(move || {
        let base_path = expand_path(path_arg)?;
        let target = if base_path.exists() {
            base_path
        } else if let Some(home) = dirs_next::home_dir() {
            home
        } else {
            return Err("Start directory not found".to_string());
        };
        let star_conn = db::open()?;
        let star_set: HashSet<String> = db::starred_set(&star_conn)?;
        let mut res = search_recursive(target, query_arg)?;
        for item in &mut res {
            if star_set.contains(&normalize_key_for_db(Path::new(&item.path))) {
                item.starred = true;
            }
        }
        sort_entries(&mut res, sort);
        Ok(res)
    })
    .await
    .map_err(|e| format!("search task failed: {e}"))?
}

#[derive(Serialize, Clone)]
pub struct SearchProgress {
    pub entries: Vec<FsEntry>,
    pub done: bool,
    pub error: Option<String>,
}

#[tauri::command]
pub fn search_stream(
    app: tauri::AppHandle,
    path: Option<String>,
    query: String,
    sort: Option<SortSpec>,
    progress_event: Option<String>,
) -> Result<(), String> {
    let progress_event = progress_event.ok_or_else(|| "progress_event is required".to_string())?;
    tauri::async_runtime::spawn_blocking(move || {
        let send = |entries: Vec<FsEntry>, done: bool, error: Option<String>| {
            let payload = SearchProgress {
                entries,
                done,
                error,
            };
            let _ = app.emit(&progress_event, payload);
        };

        let needle = query.trim();
        if needle.is_empty() {
            send(Vec::new(), true, None);
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
                    );
                    return;
                }
            },
            Err(e) => {
                send(Vec::new(), true, Some(e));
                return;
            }
        };

        let star_set = match db::open().and_then(|conn| db::starred_set(&conn)) {
            Ok(set) => set,
            Err(e) => {
                send(Vec::new(), true, Some(e));
                return;
            }
        };

        let mut stack = vec![target];
        let mut all = Vec::new();
        let mut seen: HashSet<String> = HashSet::new();
        let mut last_sent: usize = 0;
        let needle_lc = needle.to_lowercase();

        while let Some(dir) = stack.pop() {
            let iter = match std::fs::read_dir(&dir) {
                Ok(i) => i,
                Err(err) => {
                    warn!(
                        "search read_dir failed: dir={} err={:?}",
                        dir.display(),
                        err
                    );
                    continue;
                }
            };

            for entry in iter.flatten() {
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
                        all.push(item);
                        if all.len() - last_sent >= 64 {
                            send(all[last_sent..].to_vec(), false, None);
                            last_sent = all.len();
                        }
                    }
                }

                if is_dir && !is_link {
                    stack.push(path);
                }
            }
        }

        if last_sent < all.len() {
            send(all[last_sent..].to_vec(), false, None);
        }

        sort_entries(&mut all, sort);
        send(all, true, None);
    });

    Ok(())
}
