//! Successful network addresses only. Credentials belong to GVFS/the keyring.

use rusqlite::{params, Connection};
use serde::Serialize;
use url::Url;

use super::{
    error::{map_api_result, NetworkError, NetworkErrorCode, NetworkResult},
    uri::{canonicalize_uri, classify_uri, NetworkUriKind},
};
use crate::errors::api_error::ApiResult;

#[derive(Clone, Debug, Serialize, PartialEq, Eq)]
pub struct SavedNetworkConnection {
    pub uri: String,
    pub label: String,
}

pub(super) fn connection_uri(raw: &str, username: Option<&str>) -> NetworkResult<String> {
    let classified = classify_uri(raw);
    if classified.kind != NetworkUriKind::Mountable || classified.scheme.as_deref() == Some("mtp") {
        return Err(NetworkError::new(
            NetworkErrorCode::InvalidUri,
            "Not a server address",
        ));
    }
    let (_, normalized) = canonicalize_uri(raw).ok_or_else(invalid_address)?;
    let mut url = Url::parse(&normalized).map_err(|_| invalid_address())?;
    if url.host_str().is_none() || normalized.len() > 4096 {
        return Err(invalid_address());
    }
    let host = url.host_str().unwrap_or_default().to_ascii_lowercase();
    url.set_host(Some(&host)).map_err(|_| invalid_address())?;
    // Do not pass secrets in addresses to GIO, logs, history, or SQLite.
    if url.password().is_some() || url.query().is_some() || url.fragment().is_some() {
        return Err(NetworkError::new(NetworkErrorCode::InvalidUri,
            "Use a server address without a password, query or fragment. Enter credentials in the connection dialog."));
    }
    if let Some(username) = username.filter(|value| !value.is_empty()) {
        url.set_username(username).map_err(|_| invalid_address())?;
    }
    if url.path().is_empty() {
        url.set_path("/");
    }
    Ok(url.to_string())
}

fn invalid_address() -> NetworkError {
    NetworkError::new(NetworkErrorCode::InvalidUri, "Invalid server address")
}

fn database_error(error: impl std::fmt::Display) -> NetworkError {
    NetworkError::new(
        NetworkErrorCode::HistoryFailed,
        format!("Could not update saved network connections: {error}"),
    )
}

fn ensure_table(conn: &Connection) -> NetworkResult<()> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS network_connections (
        uri TEXT PRIMARY KEY, label TEXT NOT NULL
    );",
    )
    .map_err(database_error)
}

#[cfg(any(target_os = "linux", test))]
pub(super) fn remember_in(conn: &Connection, raw: &str) -> NetworkResult<()> {
    let uri = connection_uri(raw, None)?;
    let url = Url::parse(&uri).map_err(|_| invalid_address())?;
    let host = url.host_str().ok_or_else(invalid_address)?;
    let user = if url.username().is_empty() {
        String::new()
    } else {
        format!("{}@", url.username())
    };
    let port = url
        .port()
        .map(|port| format!(":{port}"))
        .unwrap_or_default();
    let path = url.path().trim_end_matches('/');
    let label = format!(
        "{} ({user}{host}{port}{path})",
        url.scheme().to_ascii_uppercase()
    );
    ensure_table(conn)?;
    conn.execute(
        "INSERT INTO network_connections (uri, label) VALUES (?1, ?2)
        ON CONFLICT(uri) DO UPDATE SET label = excluded.label",
        params![uri, label],
    )
    .map_err(database_error)?;
    Ok(())
}

pub(super) fn list_in(conn: &Connection) -> NetworkResult<Vec<SavedNetworkConnection>> {
    ensure_table(conn)?;
    let mut statement = conn
        .prepare("SELECT uri, label FROM network_connections ORDER BY label COLLATE NOCASE, uri")
        .map_err(database_error)?;
    let rows = statement
        .query_map([], |row| {
            Ok(SavedNetworkConnection {
                uri: row.get(0)?,
                label: row.get(1)?,
            })
        })
        .map_err(database_error)?;
    rows.collect::<Result<Vec<_>, _>>().map_err(database_error)
}

pub(super) fn forget_in(conn: &Connection, raw: &str) -> NetworkResult<()> {
    let uri = connection_uri(raw, None)?;
    ensure_table(conn)?;
    conn.execute(
        "DELETE FROM network_connections WHERE uri = ?1",
        params![uri],
    )
    .map_err(database_error)?;
    Ok(())
}

#[cfg(target_os = "linux")]
pub(super) fn remember(uri: &str) -> NetworkResult<()> {
    remember_in(&crate::db::open().map_err(database_error)?, uri)
}

pub(super) fn list() -> NetworkResult<Vec<SavedNetworkConnection>> {
    list_in(&crate::db::open().map_err(database_error)?)
}

#[tauri::command]
pub fn list_saved_network_connections() -> ApiResult<Vec<SavedNetworkConnection>> {
    map_api_result(list())
}

#[tauri::command]
pub fn forget_network_connection(uri: String) -> ApiResult<()> {
    map_api_result((|| {
        forget_in(&crate::db::open().map_err(database_error)?, &uri)
    })())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_addresses_preserve_account_port_and_case_sensitive_path() {
        assert_eq!(
            connection_uri("ssh://alice@SERVER:2222/Photos", None).unwrap(),
            "sftp://alice@server:2222/Photos"
        );
        assert_eq!(
            connection_uri("sftp://[2001:db8::1]:2222", Some("other user")).unwrap(),
            "sftp://other%20user@[2001:db8::1]:2222/"
        );
        assert_eq!(
            connection_uri("ssh://server", None).unwrap(),
            "sftp://server/"
        );
    }

    #[test]
    fn rejects_secrets_and_non_server_locations_without_echoing_input() {
        for uri in [
            "ssh://alice:very-secret@server",
            "sftp://server/?password=very-secret",
            "sftp://server/#very-secret",
            "mtp://phone/",
            "https://server/",
            "/tmp",
            "sftp:///",
            "sftp://",
        ] {
            let error = connection_uri(uri, None).unwrap_err();
            assert!(!error.message().contains("very-secret"));
        }
    }

    #[test]
    fn successful_connections_deduplicate_aliases_but_keep_accounts_and_path_case() {
        let conn = Connection::open_in_memory().unwrap();
        remember_in(&conn, "ssh://alice@SERVER:2222/Photos").unwrap();
        remember_in(&conn, "sftp://alice@server:2222/Photos").unwrap();
        remember_in(&conn, "sftp://bob@server:2222/Photos").unwrap();
        remember_in(&conn, "sftp://alice@server:2222/photos").unwrap();
        let entries = list_in(&conn).unwrap();
        assert_eq!(entries.len(), 3);
        assert!(entries
            .iter()
            .any(|entry| entry.label == "SFTP (alice@server:2222/Photos)"));
        forget_in(&conn, "ssh://alice@server:2222/Photos").unwrap();
        assert_eq!(list_in(&conn).unwrap().len(), 2);
        forget_in(&conn, "ssh://alice@server:2222/Photos").unwrap();
        assert_eq!(list_in(&conn).unwrap().len(), 2);
    }

    #[test]
    fn rejected_addresses_never_enter_the_database() {
        let conn = Connection::open_in_memory().unwrap();
        assert!(remember_in(&conn, "sftp://user:secret@server/").is_err());
        assert!(list_in(&conn).unwrap().is_empty());
    }

    #[test]
    fn remembered_addresses_survive_database_reopening() {
        let path = std::env::temp_dir().join(format!(
            "browsey-network-history-{}-{}.db",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let conn = Connection::open(&path).unwrap();
        remember_in(&conn, "ssh://alice@server/").unwrap();
        drop(conn);
        let reopened = Connection::open(&path).unwrap();
        assert_eq!(list_in(&reopened).unwrap()[0].uri, "sftp://alice@server/");
        forget_in(&reopened, "sftp://alice@server/").unwrap();
        drop(reopened);
        let reopened = Connection::open(&path).unwrap();
        assert!(list_in(&reopened).unwrap().is_empty());
        drop(reopened);
        std::fs::remove_file(path).unwrap();
    }
}
