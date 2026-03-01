//! Bookmark CRUD commands backed by SQLite.

use crate::db::{delete_all_bookmarks, delete_bookmark, list_bookmarks, upsert_bookmark};
use crate::errors::api_error::ApiResult;
use error::{map_api_result, BookmarkError, BookmarkErrorCode, BookmarkResult};
use serde::Serialize;

mod error;

fn map_db_open_error(error: crate::db::DbError) -> BookmarkError {
    BookmarkError::new(BookmarkErrorCode::DatabaseOpenFailed, error.to_string())
}

fn map_db_read_error(error: crate::db::DbError) -> BookmarkError {
    BookmarkError::new(BookmarkErrorCode::BookmarksReadFailed, error.to_string())
}

fn map_db_write_error(error: crate::db::DbError) -> BookmarkError {
    BookmarkError::new(BookmarkErrorCode::BookmarksWriteFailed, error.to_string())
}

#[derive(Serialize, Clone)]
pub struct Bookmark {
    pub label: String,
    pub path: String,
}

#[tauri::command]
pub fn get_bookmarks() -> ApiResult<Vec<Bookmark>> {
    map_api_result(get_bookmarks_impl())
}

fn get_bookmarks_impl() -> BookmarkResult<Vec<Bookmark>> {
    let conn = crate::db::open().map_err(map_db_open_error)?;
    let rows = list_bookmarks(&conn).map_err(map_db_read_error)?;
    Ok(rows
        .into_iter()
        .map(|(label, path)| Bookmark { label, path })
        .collect())
}

#[tauri::command]
pub fn add_bookmark(label: String, path: String) -> ApiResult<()> {
    map_api_result(add_bookmark_impl(label, path))
}

fn add_bookmark_impl(label: String, path: String) -> BookmarkResult<()> {
    let conn = crate::db::open().map_err(map_db_open_error)?;
    upsert_bookmark(&conn, &label, &path).map_err(map_db_write_error)
}

#[tauri::command]
pub fn remove_bookmark(path: String) -> ApiResult<()> {
    map_api_result(remove_bookmark_impl(path))
}

fn remove_bookmark_impl(path: String) -> BookmarkResult<()> {
    let conn = crate::db::open().map_err(map_db_open_error)?;
    delete_bookmark(&conn, &path).map_err(map_db_write_error)
}

#[tauri::command]
pub fn clear_bookmarks() -> ApiResult<u64> {
    map_api_result(clear_bookmarks_impl())
}

fn clear_bookmarks_impl() -> BookmarkResult<u64> {
    let conn = crate::db::open().map_err(map_db_open_error)?;
    let removed = delete_all_bookmarks(&conn).map_err(map_db_write_error)?;
    Ok(removed as u64)
}
