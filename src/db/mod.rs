use crate::entry::normalize_key_for_db;
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

mod error;

pub use error::{DbError, DbErrorCode, DbResult};

const MAX_RECENT: i64 = 50;

fn map_db_io(
    fallback: DbErrorCode,
    context: impl FnOnce() -> String,
) -> impl FnOnce(std::io::Error) -> DbError {
    move |error| DbError::from_io_error(fallback, context(), error)
}

fn map_db_sqlite(
    fallback: DbErrorCode,
    context: impl FnOnce() -> String,
) -> impl FnOnce(rusqlite::Error) -> DbError {
    move |error| DbError::from_sqlite_error(fallback, context(), error)
}

fn db_path() -> DbResult<PathBuf> {
    let base = dirs_next::data_dir()
        .ok_or_else(|| {
            DbError::new(
                DbErrorCode::DataDirUnavailable,
                "Could not resolve data directory",
            )
        })?
        .join("browsey");
    std::fs::create_dir_all(&base).map_err(map_db_io(DbErrorCode::DataDirUnavailable, || {
        "Failed to create data dir".to_string()
    }))?;
    Ok(base.join("browsey.db"))
}

fn ensure_schema(conn: &Connection) -> DbResult<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS starred (
            path TEXT PRIMARY KEY,
            starred_at INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS recent (
            path TEXT PRIMARY KEY,
            opened_at INTEGER NOT NULL
        );
        CREATE TABLE IF NOT EXISTS bookmarks (
            path TEXT PRIMARY KEY,
            label TEXT NOT NULL
        );
        CREATE TABLE IF NOT EXISTS settings (
            key TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );",
    )
    .map_err(map_db_sqlite(DbErrorCode::SchemaInitFailed, || {
        "Failed to init schema".to_string()
    }))?;
    Ok(())
}

fn normalize_starred_paths(conn: &mut Connection) -> DbResult<()> {
    let mut fixes = Vec::new();
    {
        let mut stmt = conn
            .prepare("SELECT path, starred_at FROM starred")
            .map_err(map_db_sqlite(DbErrorCode::ReadFailed, || {
                "Failed to prepare starred scan".to_string()
            }))?;
        let rows = stmt
            .query_map([], |row: &Row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })
            .map_err(map_db_sqlite(DbErrorCode::ReadFailed, || {
                "Failed to read starred".to_string()
            }))?;

        for (raw, ts) in rows.flatten() {
            let norm = normalize_key_for_db(Path::new(&raw));
            if norm != raw {
                fixes.push((raw, norm, ts));
            }
        }
    }
    if fixes.is_empty() {
        return Ok(());
    }

    let tx = conn
        .transaction()
        .map_err(map_db_sqlite(DbErrorCode::TransactionFailed, || {
            "Failed to start transaction".to_string()
        }))?;
    for (raw, norm, ts) in fixes {
        tx.execute("DELETE FROM starred WHERE path = ?1", params![raw.as_str()])
            .map_err(map_db_sqlite(DbErrorCode::WriteFailed, || {
                "Failed to delete stale star".to_string()
            }))?;
        tx.execute(
            "INSERT OR IGNORE INTO starred (path, starred_at) VALUES (?1, ?2)",
            params![norm.as_str(), ts],
        )
        .map_err(map_db_sqlite(DbErrorCode::WriteFailed, || {
            "Failed to store normalized star".to_string()
        }))?;
    }
    tx.commit()
        .map_err(map_db_sqlite(DbErrorCode::TransactionFailed, || {
            "Failed to commit normalized stars".to_string()
        }))
}

pub fn open() -> DbResult<Connection> {
    let path = db_path()?;
    let mut conn = Connection::open(path)
        .map_err(map_db_sqlite(DbErrorCode::OpenFailed, || {
            "Failed to open db".to_string()
        }))?;
    ensure_schema(&conn)?;
    normalize_starred_paths(&mut conn)?;
    Ok(conn)
}

pub fn starred_set(conn: &Connection) -> DbResult<HashSet<String>> {
    let mut stmt = conn
        .prepare("SELECT path FROM starred")
        .map_err(map_db_sqlite(DbErrorCode::ReadFailed, || {
            "Failed to prepare starred query".to_string()
        }))?;
    let rows = stmt
        .query_map([], |row: &Row| row.get::<_, String>(0))
        .map_err(map_db_sqlite(DbErrorCode::ReadFailed, || {
            "Failed to read starred".to_string()
        }))?;
    let mut set = HashSet::new();
    for p in rows.flatten() {
        set.insert(normalize_key_for_db(Path::new(&p)));
    }
    Ok(set)
}

pub fn toggle_star(conn: &Connection, path: &str) -> DbResult<bool> {
    let norm = normalize_key_for_db(Path::new(path));
    let mut stmt = conn
        .prepare("SELECT 1 FROM starred WHERE path = ?1")
        .map_err(map_db_sqlite(DbErrorCode::ReadFailed, || {
            "Failed to prepare starred exists".to_string()
        }))?;
    let exists = stmt
        .exists(params![norm.as_str()])
        .map_err(map_db_sqlite(DbErrorCode::ReadFailed, || {
            "Failed to query starred existence".to_string()
        }))?;
    if exists {
        conn.execute(
            "DELETE FROM starred WHERE path = ?1",
            params![norm.as_str()],
        )
        .map_err(map_db_sqlite(DbErrorCode::WriteFailed, || {
            "Failed to delete star".to_string()
        }))?;
        return Ok(false);
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    conn.execute(
        "INSERT OR REPLACE INTO starred (path, starred_at) VALUES (?1, ?2)",
        params![norm.as_str(), now],
    )
    .map_err(map_db_sqlite(DbErrorCode::WriteFailed, || {
        "Failed to insert star".to_string()
    }))?;
    Ok(true)
}

pub fn recent_paths(conn: &Connection) -> DbResult<Vec<String>> {
    let mut stmt = conn
        .prepare("SELECT path FROM recent ORDER BY opened_at DESC LIMIT ?1")
        .map_err(map_db_sqlite(DbErrorCode::ReadFailed, || {
            "Failed to query recent".to_string()
        }))?;
    let rows = stmt
        .query_map(params![MAX_RECENT], |row: &Row| row.get::<_, String>(0))
        .map_err(map_db_sqlite(DbErrorCode::ReadFailed, || {
            "Failed to read recent".to_string()
        }))?;
    let mut res = Vec::new();
    for p in rows.flatten() {
        res.push(p);
    }
    Ok(res)
}

pub fn starred_entries(conn: &Connection) -> DbResult<Vec<(String, i64)>> {
    let mut stmt = conn
        .prepare("SELECT path, starred_at FROM starred ORDER BY starred_at DESC")
        .map_err(map_db_sqlite(DbErrorCode::ReadFailed, || {
            "Failed to query starred".to_string()
        }))?;
    let rows = stmt
        .query_map([], |row: &Row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })
        .map_err(map_db_sqlite(DbErrorCode::ReadFailed, || {
            "Failed to read starred".to_string()
        }))?;
    let mut res = Vec::new();
    for p in rows.flatten() {
        res.push(p);
    }
    Ok(res)
}

pub fn touch_recent(conn: &Connection, path: &str) -> DbResult<()> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    conn.execute(
        "INSERT OR REPLACE INTO recent (path, opened_at) VALUES (?1, ?2)",
        params![path, now],
    )
    .map_err(map_db_sqlite(DbErrorCode::WriteFailed, || {
        "Failed to upsert recent".to_string()
    }))?;

    conn.execute(
        "DELETE FROM recent WHERE path NOT IN (SELECT path FROM recent ORDER BY opened_at DESC LIMIT ?1)",
        params![MAX_RECENT],
    )
    .map_err(map_db_sqlite(DbErrorCode::WriteFailed, || {
        "Failed to trim recent".to_string()
    }))?;
    Ok(())
}

pub fn delete_recent_paths(conn: &mut Connection, paths: &[String]) -> DbResult<usize> {
    let tx = conn
        .transaction()
        .map_err(map_db_sqlite(DbErrorCode::TransactionFailed, || {
            "Failed to start transaction".to_string()
        }))?;
    let mut deleted = 0;
    for path in paths {
        let changes = tx
            .execute("DELETE FROM recent WHERE path = ?1", params![path])
            .map_err(map_db_sqlite(DbErrorCode::WriteFailed, || {
                "Failed to delete recent entry".to_string()
            }))?;
        deleted += changes;
    }
    tx.commit()
        .map_err(map_db_sqlite(DbErrorCode::TransactionFailed, || {
            "Failed to commit recent deletion".to_string()
        }))?;
    Ok(deleted)
}

pub fn list_bookmarks(conn: &Connection) -> DbResult<Vec<(String, String)>> {
    let mut stmt = conn
        .prepare("SELECT label, path FROM bookmarks ORDER BY label COLLATE NOCASE ASC")
        .map_err(map_db_sqlite(DbErrorCode::ReadFailed, || {
            "Failed to prepare bookmarks query".to_string()
        }))?;
    let rows = stmt
        .query_map([], |row: &Row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(map_db_sqlite(DbErrorCode::ReadFailed, || {
            "Failed to read bookmarks".to_string()
        }))?;
    let mut res = Vec::new();
    for b in rows.flatten() {
        res.push(b);
    }
    Ok(res)
}

pub fn upsert_bookmark(conn: &Connection, label: &str, path: &str) -> DbResult<()> {
    conn.execute(
        "INSERT OR REPLACE INTO bookmarks (path, label) VALUES (?1, ?2)",
        params![path, label],
    )
    .map_err(map_db_sqlite(DbErrorCode::WriteFailed, || {
        "Failed to upsert bookmark".to_string()
    }))?;
    Ok(())
}

pub fn delete_bookmark(conn: &Connection, path: &str) -> DbResult<()> {
    conn.execute("DELETE FROM bookmarks WHERE path = ?1", params![path])
        .map_err(map_db_sqlite(DbErrorCode::WriteFailed, || {
            "Failed to delete bookmark".to_string()
        }))?;
    Ok(())
}

pub fn delete_all_starred(conn: &Connection) -> DbResult<usize> {
    conn.execute("DELETE FROM starred", [])
        .map_err(map_db_sqlite(DbErrorCode::WriteFailed, || {
            "Failed to clear stars".to_string()
        }))
}

pub fn delete_all_recent(conn: &Connection) -> DbResult<usize> {
    conn.execute("DELETE FROM recent", [])
        .map_err(map_db_sqlite(DbErrorCode::WriteFailed, || {
            "Failed to clear recents".to_string()
        }))
}

pub fn delete_all_bookmarks(conn: &Connection) -> DbResult<usize> {
    conn.execute("DELETE FROM bookmarks", [])
        .map_err(map_db_sqlite(DbErrorCode::WriteFailed, || {
            "Failed to clear bookmarks".to_string()
        }))
}

#[derive(Serialize, Deserialize)]
pub struct ColumnWidths {
    pub widths: Vec<f64>,
}

pub fn save_column_widths(conn: &Connection, widths: &[f64]) -> DbResult<()> {
    let payload = serde_json::to_string(&ColumnWidths {
        widths: widths.to_vec(),
    })
    .map_err(|e| DbError::from_external_message(format!("Failed to serialize widths: {e}")))?;

    conn.execute(
        "INSERT OR REPLACE INTO settings (key, value) VALUES ('column_widths', ?1)",
        params![payload],
    )
    .map_err(map_db_sqlite(DbErrorCode::WriteFailed, || {
        "Failed to store widths".to_string()
    }))?;
    Ok(())
}

pub fn load_column_widths(conn: &Connection) -> DbResult<Option<Vec<f64>>> {
    let val: Option<String> = conn
        .query_row(
            "SELECT value FROM settings WHERE key = 'column_widths'",
            [],
            |row| row.get(0),
        )
        .optional()
        .map_err(map_db_sqlite(DbErrorCode::ReadFailed, || {
            "Failed to read settings".to_string()
        }))?;

    if let Some(json) = val {
        let parsed: ColumnWidths = serde_json::from_str(&json)
            .map_err(|e| DbError::from_external_message(format!("Failed to parse widths: {e}")))?;
        Ok(Some(parsed.widths))
    } else {
        Ok(None)
    }
}

pub fn set_setting_bool(conn: &Connection, key: &str, value: bool) -> DbResult<()> {
    conn.execute(
        "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
        params![key, if value { "true" } else { "false" }],
    )
    .map_err(map_db_sqlite(DbErrorCode::WriteFailed, || {
        format!("Failed to store setting {key}")
    }))?;
    Ok(())
}

pub fn get_setting_bool(conn: &Connection, key: &str) -> DbResult<Option<bool>> {
    let val: Option<String> = conn
        .query_row(
            "SELECT value FROM settings WHERE key = ?1",
            params![key],
            |row| row.get(0),
        )
        .optional()
        .map_err(map_db_sqlite(DbErrorCode::ReadFailed, || {
            format!("Failed to read setting {key}")
        }))?;

    if let Some(s) = val {
        match s.as_str() {
            "true" => Ok(Some(true)),
            "false" => Ok(Some(false)),
            _ => Ok(None),
        }
    } else {
        Ok(None)
    }
}

pub fn set_setting_string(conn: &Connection, key: &str, value: &str) -> DbResult<()> {
    conn.execute(
        "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
        params![key, value],
    )
    .map_err(map_db_sqlite(DbErrorCode::WriteFailed, || {
        format!("Failed to store setting {key}")
    }))?;
    Ok(())
}

pub fn get_setting_string(conn: &Connection, key: &str) -> DbResult<Option<String>> {
    let val: Option<String> = conn
        .query_row(
            "SELECT value FROM settings WHERE key = ?1",
            params![key],
            |row| row.get(0),
        )
        .optional()
        .map_err(map_db_sqlite(DbErrorCode::ReadFailed, || {
            format!("Failed to read setting {key}")
        }))?;
    Ok(val)
}
