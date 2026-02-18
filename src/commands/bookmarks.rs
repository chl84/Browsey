//! Bookmark CRUD commands backed by SQLite.

use crate::db::{delete_all_bookmarks, delete_bookmark, list_bookmarks, upsert_bookmark};
use crate::errors::api_error::ApiResult;
use error::{map_api_result, BookmarkError, BookmarkErrorCode, BookmarkResult};
use serde::Serialize;

mod error;

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
    let conn = crate::db::open().map_err(|error| {
        BookmarkError::new(
            BookmarkErrorCode::DatabaseOpenFailed,
            format!("Failed to open bookmarks database: {error}"),
        )
    })?;
    let rows = list_bookmarks(&conn).map_err(|error| {
        BookmarkError::new(
            BookmarkErrorCode::BookmarksReadFailed,
            format!("Failed to read bookmarks: {error}"),
        )
    })?;
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
    let conn = crate::db::open().map_err(|error| {
        BookmarkError::new(
            BookmarkErrorCode::DatabaseOpenFailed,
            format!("Failed to open bookmarks database: {error}"),
        )
    })?;
    upsert_bookmark(&conn, &label, &path).map_err(|error| {
        BookmarkError::new(
            BookmarkErrorCode::BookmarksWriteFailed,
            format!("Failed to store bookmark: {error}"),
        )
    })
}

#[tauri::command]
pub fn remove_bookmark(path: String) -> ApiResult<()> {
    map_api_result(remove_bookmark_impl(path))
}

fn remove_bookmark_impl(path: String) -> BookmarkResult<()> {
    let conn = crate::db::open().map_err(|error| {
        BookmarkError::new(
            BookmarkErrorCode::DatabaseOpenFailed,
            format!("Failed to open bookmarks database: {error}"),
        )
    })?;
    delete_bookmark(&conn, &path).map_err(|error| {
        BookmarkError::new(
            BookmarkErrorCode::BookmarksWriteFailed,
            format!("Failed to remove bookmark: {error}"),
        )
    })
}

#[tauri::command]
pub fn clear_bookmarks() -> ApiResult<u64> {
    map_api_result(clear_bookmarks_impl())
}

fn clear_bookmarks_impl() -> BookmarkResult<u64> {
    let conn = crate::db::open().map_err(|error| {
        BookmarkError::new(
            BookmarkErrorCode::DatabaseOpenFailed,
            format!("Failed to open bookmarks database: {error}"),
        )
    })?;
    let removed = delete_all_bookmarks(&conn).map_err(|error| {
        BookmarkError::new(
            BookmarkErrorCode::BookmarksWriteFailed,
            format!("Failed to clear bookmarks: {error}"),
        )
    })?;
    Ok(removed as u64)
}
