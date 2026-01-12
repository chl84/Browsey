//! Recursive search command that decorates entries with starred state.

use crate::{
    commands::fs::expand_path,
    db,
    entry::FsEntry,
    search::search_recursive,
    sorting::{sort_entries, SortSpec},
};
use std::collections::HashSet;

#[tauri::command]
pub fn search(
    path: Option<String>,
    query: String,
    sort: Option<SortSpec>,
) -> Result<Vec<FsEntry>, String> {
    let base_path = expand_path(path)?;
    let target = if base_path.exists() {
        base_path
    } else if let Some(home) = dirs_next::home_dir() {
        home
    } else {
        return Err("Fant ikke startkatalog".to_string());
    };
    let star_conn = db::open()?;
    let star_set: HashSet<String> = db::starred_set(&star_conn)?;
    let mut res = search_recursive(target, query)?;
    for item in &mut res {
        if star_set.contains(&item.path) {
            item.starred = true;
        }
    }
    sort_entries(&mut res, sort);
    Ok(res)
}
