use std::collections::HashSet;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use rusqlite::{params, Connection, Row};

const MAX_RECENT: i64 = 50;

fn db_path() -> Result<PathBuf, String> {
    let base = dirs_next::data_dir()
        .ok_or_else(|| "Could not resolve data directory".to_string())?
        .join("filey");
    std::fs::create_dir_all(&base).map_err(|e| format!("Failed to create data dir: {e}"))?;
    Ok(base.join("filey.db"))
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
        );",
    )
    .map_err(|e| format!("Failed to init schema: {e}"))?;
    Ok(())
}

pub fn open() -> Result<Connection, String> {
    let path = db_path()?;
    let conn = Connection::open(path).map_err(|e| format!("Failed to open db: {e}"))?;
    ensure_schema(&conn)?;
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
            set.insert(p);
        }
    }
    Ok(set)
}

pub fn toggle_star(conn: &Connection, path: &str) -> Result<bool, String> {
    let mut stmt = conn
        .prepare("SELECT 1 FROM starred WHERE path = ?1")
        .map_err(|e| format!("Failed to prepare starred exists: {e}"))?;
    let exists = stmt.exists(params![path]).map_err(|e: rusqlite::Error| e.to_string())?;
    if exists {
        conn.execute("DELETE FROM starred WHERE path = ?1", params![path])
            .map_err(|e| format!("Failed to delete star: {e}"))?;
        return Ok(false);
    }
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    conn.execute(
        "INSERT OR REPLACE INTO starred (path, starred_at) VALUES (?1, ?2)",
        params![path, now],
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
        .query_map([], |row: &Row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)))
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
