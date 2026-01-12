//! Bookmark CRUD commands backed by SQLite.

use crate::db::{delete_bookmark, list_bookmarks, upsert_bookmark};
use serde::Serialize;

#[derive(Serialize, Clone)]
pub struct Bookmark {
    pub label: String,
    pub path: String,
}

#[tauri::command]
pub fn get_bookmarks() -> Result<Vec<Bookmark>, String> {
    let conn = crate::db::open()?;
    let rows = list_bookmarks(&conn)?;
    Ok(rows
        .into_iter()
        .map(|(label, path)| Bookmark { label, path })
        .collect())
}

#[tauri::command]
pub fn add_bookmark(label: String, path: String) -> Result<(), String> {
    let conn = crate::db::open()?;
    upsert_bookmark(&conn, &label, &path)
}

#[tauri::command]
pub fn remove_bookmark(path: String) -> Result<(), String> {
    let conn = crate::db::open()?;
    delete_bookmark(&conn, &path)
}
