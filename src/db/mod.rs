use crate::entry::normalize_key_for_db;
use rusqlite::{params, Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_RECENT: i64 = 50;

fn db_path() -> Result<PathBuf, String> {
    let base = dirs_next::data_dir()
        .ok_or_else(|| "Could not resolve data directory".to_string())?
        .join("browsey");
    std::fs::create_dir_all(&base).map_err(|e| format!("Failed to create data dir: {e}"))?;
    Ok(base.join("browsey.db"))
}

fn ensure_schema(conn: &Connection) -> Result<(), String> {
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
    .map_err(|e| format!("Failed to init schema: {e}"))?;
    Ok(())
}

fn normalize_starred_paths(conn: &mut Connection) -> Result<(), String> {
    let mut fixes = Vec::new();
    {
        let mut stmt = conn
            .prepare("SELECT path, starred_at FROM starred")
            .map_err(|e| format!("Failed to prepare starred scan: {e}"))?;
        let rows = stmt
            .query_map([], |row: &Row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
            })
            .map_err(|e| format!("Failed to read starred: {e}"))?;

        for r in rows {
            if let Ok((raw, ts)) = r {
                let norm = normalize_key_for_db(Path::new(&raw));
                if norm != raw {
                    fixes.push((raw, norm, ts));
                }
            }
        }
    }
    if fixes.is_empty() {
        return Ok(());
    }

    let tx = conn
        .transaction()
        .map_err(|e| format!("Failed to start transaction: {e}"))?;
    for (raw, norm, ts) in fixes {
        tx.execute("DELETE FROM starred WHERE path = ?1", params![raw.as_str()])
            .map_err(|e| format!("Failed to delete stale star: {e}"))?;
        tx.execute(
            "INSERT OR IGNORE INTO starred (path, starred_at) VALUES (?1, ?2)",
            params![norm.as_str(), ts],
        )
        .map_err(|e| format!("Failed to store normalized star: {e}"))?;
    }
    tx.commit()
        .map_err(|e| format!("Failed to commit normalized stars: {e}"))
}

pub fn open() -> Result<Connection, String> {
    let path = db_path()?;
    let mut conn = Connection::open(path).map_err(|e| format!("Failed to open db: {e}"))?;
    ensure_schema(&conn)?;
    normalize_starred_paths(&mut conn)?;
    Ok(conn)
}

pub fn starred_set(conn: &Connection) -> Result<HashSet<String>, String> {
    let mut stmt = conn
        .prepare("SELECT path FROM starred")
        .map_err(|e| format!("Failed to prepare starred query: {e}"))?;
    let rows = stmt
        .query_map([], |row: &Row| row.get::<_, String>(0))
        .map_err(|e| format!("Failed to read starred: {e}"))?;
    let mut set = HashSet::new();
    for r in rows {
        if let Ok(p) = r {
            set.insert(normalize_key_for_db(Path::new(&p)));
        }
    }
    Ok(set)
}

pub fn toggle_star(conn: &Connection, path: &str) -> Result<bool, String> {
    let norm = normalize_key_for_db(Path::new(path));
    let mut stmt = conn
        .prepare("SELECT 1 FROM starred WHERE path = ?1")
        .map_err(|e| format!("Failed to prepare starred exists: {e}"))?;
    let exists = stmt
        .exists(params![norm.as_str()])
        .map_err(|e: rusqlite::Error| e.to_string())?;
    if exists {
        conn.execute(
            "DELETE FROM starred WHERE path = ?1",
            params![norm.as_str()],
        )
        .map_err(|e| format!("Failed to delete star: {e}"))?;
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
    .map_err(|e| format!("Failed to insert star: {e}"))?;
    Ok(true)
}

pub fn recent_paths(conn: &Connection) -> Result<Vec<String>, String> {
    let mut stmt = conn
        .prepare("SELECT path FROM recent ORDER BY opened_at DESC LIMIT ?1")
        .map_err(|e| format!("Failed to query recent: {e}"))?;
    let rows = stmt
        .query_map(params![MAX_RECENT], |row: &Row| row.get::<_, String>(0))
        .map_err(|e| format!("Failed to read recent: {e}"))?;
    let mut res = Vec::new();
    for r in rows {
        if let Ok(p) = r {
            res.push(p);
        }
    }
    Ok(res)
}

pub fn starred_entries(conn: &Connection) -> Result<Vec<(String, i64)>, String> {
    let mut stmt = conn
        .prepare("SELECT path, starred_at FROM starred ORDER BY starred_at DESC")
        .map_err(|e| format!("Failed to query starred: {e}"))?;
    let rows = stmt
        .query_map([], |row: &Row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })
        .map_err(|e| format!("Failed to read starred: {e}"))?;
    let mut res = Vec::new();
    for r in rows {
        if let Ok(p) = r {
            res.push(p);
        }
    }
    Ok(res)
}

pub fn touch_recent(conn: &Connection, path: &str) -> Result<(), String> {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    conn.execute(
        "INSERT OR REPLACE INTO recent (path, opened_at) VALUES (?1, ?2)",
        params![path, now],
    )
    .map_err(|e| format!("Failed to upsert recent: {e}"))?;

    conn.execute(
        "DELETE FROM recent WHERE path NOT IN (SELECT path FROM recent ORDER BY opened_at DESC LIMIT ?1)",
        params![MAX_RECENT],
    )
    .map_err(|e| format!("Failed to trim recent: {e}"))?;
    Ok(())
}

pub fn delete_recent_paths(conn: &mut Connection, paths: &[String]) -> Result<usize, String> {
    let tx = conn
        .transaction()
        .map_err(|e| format!("Failed to start transaction: {e}"))?;
    let mut deleted = 0;
    for path in paths {
        let changes = tx
            .execute("DELETE FROM recent WHERE path = ?1", params![path])
            .map_err(|e| format!("Failed to delete recent entry: {e}"))?;
        deleted += changes;
    }
    tx.commit()
        .map_err(|e| format!("Failed to commit recent deletion: {e}"))?;
    Ok(deleted)
}

pub fn list_bookmarks(conn: &Connection) -> Result<Vec<(String, String)>, String> {
    let mut stmt = conn
        .prepare("SELECT label, path FROM bookmarks ORDER BY label COLLATE NOCASE ASC")
        .map_err(|e| format!("Failed to prepare bookmarks query: {e}"))?;
    let rows = stmt
        .query_map([], |row: &Row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
        })
        .map_err(|e| format!("Failed to read bookmarks: {e}"))?;
    let mut res = Vec::new();
    for r in rows {
        if let Ok(b) = r {
            res.push(b);
        }
    }
    Ok(res)
}

pub fn upsert_bookmark(conn: &Connection, label: &str, path: &str) -> Result<(), String> {
    conn.execute(
        "INSERT OR REPLACE INTO bookmarks (path, label) VALUES (?1, ?2)",
        params![path, label],
    )
    .map_err(|e| format!("Failed to upsert bookmark: {e}"))?;
    Ok(())
}

pub fn delete_bookmark(conn: &Connection, path: &str) -> Result<(), String> {
    conn.execute("DELETE FROM bookmarks WHERE path = ?1", params![path])
        .map_err(|e| format!("Failed to delete bookmark: {e}"))?;
    Ok(())
}

pub fn delete_all_starred(conn: &Connection) -> Result<usize, String> {
    conn.execute("DELETE FROM starred", [])
        .map_err(|e| format!("Failed to clear stars: {e}"))
}

pub fn delete_all_recent(conn: &Connection) -> Result<usize, String> {
    conn.execute("DELETE FROM recent", [])
        .map_err(|e| format!("Failed to clear recents: {e}"))
}

pub fn delete_all_bookmarks(conn: &Connection) -> Result<usize, String> {
    conn.execute("DELETE FROM bookmarks", [])
        .map_err(|e| format!("Failed to clear bookmarks: {e}"))
}

#[derive(Serialize, Deserialize)]
pub struct ColumnWidths {
    pub widths: Vec<f64>,
}

pub fn save_column_widths(conn: &Connection, widths: &[f64]) -> Result<(), String> {
    let payload = serde_json::to_string(&ColumnWidths {
        widths: widths.to_vec(),
    })
    .map_err(|e| format!("Failed to serialize widths: {e}"))?;

    conn.execute(
        "INSERT OR REPLACE INTO settings (key, value) VALUES ('column_widths', ?1)",
        params![payload],
    )
    .map_err(|e| format!("Failed to store widths: {e}"))?;
    Ok(())
}

pub fn load_column_widths(conn: &Connection) -> Result<Option<Vec<f64>>, String> {
    let val: Option<String> = conn
        .query_row(
            "SELECT value FROM settings WHERE key = 'column_widths'",
            [],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| format!("Failed to read settings: {e}"))?;

    if let Some(json) = val {
        let parsed: ColumnWidths =
            serde_json::from_str(&json).map_err(|e| format!("Failed to parse widths: {e}"))?;
        Ok(Some(parsed.widths))
    } else {
        Ok(None)
    }
}

pub fn set_setting_bool(conn: &Connection, key: &str, value: bool) -> Result<(), String> {
    conn.execute(
        "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
        params![key, if value { "true" } else { "false" }],
    )
    .map_err(|e| format!("Failed to store setting {key}: {e}"))?;
    Ok(())
}

pub fn get_setting_bool(conn: &Connection, key: &str) -> Result<Option<bool>, String> {
    let val: Option<String> = conn
        .query_row(
            "SELECT value FROM settings WHERE key = ?1",
            params![key],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| format!("Failed to read setting {key}: {e}"))?;

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

pub fn set_setting_string(conn: &Connection, key: &str, value: &str) -> Result<(), String> {
    conn.execute(
        "INSERT OR REPLACE INTO settings (key, value) VALUES (?1, ?2)",
        params![key, value],
    )
    .map_err(|e| format!("Failed to store setting {key}: {e}"))?;
    Ok(())
}

pub fn get_setting_string(conn: &Connection, key: &str) -> Result<Option<String>, String> {
    let val: Option<String> = conn
        .query_row(
            "SELECT value FROM settings WHERE key = ?1",
            params![key],
            |row| row.get(0),
        )
        .optional()
        .map_err(|e| format!("Failed to read setting {key}: {e}"))?;
    Ok(val)
}
