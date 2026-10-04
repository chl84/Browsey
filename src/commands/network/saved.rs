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

/// Connection identity, not a bookmark: SFTP/FTP folders share one session.
/// Share/export scopes remain distinct for protocols mounting separate volumes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ConnectionIdentity {
    scheme: String,
    host: String,
    port: Option<u16>,
    user: String,
    scope: String,
}

impl ConnectionIdentity {
    /// An unspecified account is an alias only with proof of the same live mount.
    pub(super) fn compatible_mount(&self, other: &Self) -> bool {
        self.scheme == other.scheme
            && self.host == other.host
            && self.port == other.port
            && self.scope == other.scope
            && (self.user == other.user || self.user.is_empty() || other.user.is_empty())
    }

    pub(super) fn bind_account(&mut self, other: &Self) {
        if self.user.is_empty() {
            self.user.clone_from(&other.user);
        }
    }
}

pub(super) fn connection_identity(raw: &str) -> NetworkResult<ConnectionIdentity> {
    let url = Url::parse(&connection_uri(raw, None)?).map_err(|_| invalid_address())?;
    let scheme = url.scheme().to_string();
    let port = url.port().or(match scheme.as_str() {
        "sftp" => Some(22),
        "ftp" => Some(21),
        "ftps" => Some(990),
        "smb" => Some(445),
        "nfs" => Some(2049),
        "dav" => Some(80),
        "davs" => Some(443),
        "afp" => Some(548),
        _ => None,
    });
    let scope = match scheme.as_str() {
        "sftp" | "ftp" | "ftps" => String::new(),
        "smb" | "afp" => url
            .path()
            .split('/')
            .find(|part| !part.is_empty())
            .unwrap_or_default()
            .to_string(),
        _ => url.path().trim_end_matches('/').to_string(),
    };
    Ok(ConnectionIdentity {
        scheme,
        host: url.host_str().unwrap_or_default().to_string(),
        port,
        user: url.username().to_string(),
        scope,
    })
}

pub(super) fn unique_connections(
    records: Vec<SavedNetworkConnection>,
) -> Vec<SavedNetworkConnection> {
    let mut unique: Vec<(ConnectionIdentity, SavedNetworkConnection)> = Vec::new();
    for mut record in records {
        let Ok(address) = connection_uri(&record.uri, None) else {
            continue;
        };
        record.uri = address;
        let Ok(identity) = connection_identity(&record.uri) else {
            continue;
        };
        if let Some((_, current)) = unique.iter_mut().find(|(key, _)| *key == identity) {
            // Prefer a saved server root over its home/child folders, without
            // rewriting the URI or changing the actual folder to be opened.
            if (record.uri.len(), &record.uri) < (current.uri.len(), &current.uri) {
                *current = record;
            }
        } else {
            unique.push((identity, record));
        }
    }
    let mut result: Vec<_> = unique.into_iter().map(|(_, record)| record).collect();
    result.sort_by(|a, b| {
        a.label
            .to_lowercase()
            .cmp(&b.label.to_lowercase())
            .then(a.uri.cmp(&b.uri))
    });
    result
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

fn list_records(conn: &Connection) -> NetworkResult<Vec<SavedNetworkConnection>> {
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

pub(super) fn list_in(conn: &Connection) -> NetworkResult<Vec<SavedNetworkConnection>> {
    Ok(unique_connections(list_records(conn)?))
}

pub(super) fn forget_in(conn: &Connection, raw: &str) -> NetworkResult<()> {
    let identity = connection_identity(raw)?;
    ensure_table(conn)?;
    let transaction = conn.unchecked_transaction().map_err(database_error)?;
    let records = list_records(&transaction)?;
    for record in records {
        if connection_identity(&record.uri).is_ok_and(|key| key == identity) {
            transaction
                .execute(
                    "DELETE FROM network_connections WHERE uri = ?1",
                    params![record.uri],
                )
                .map_err(database_error)?;
        }
    }
    transaction.commit().map_err(database_error)?;
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
    fn saved_root_and_home_list_once_and_forget_removes_the_whole_connection() {
        let conn = Connection::open_in_memory().unwrap();
        remember_in(&conn, "sftp://alice@server/home/alice").unwrap();
        remember_in(&conn, "ssh://alice@server/").unwrap();
        remember_in(&conn, "sftp://alice@server:22/").unwrap();
        remember_in(&conn, "sftp://bob@server/").unwrap();
        remember_in(&conn, "sftp://alice@server:2222/").unwrap();
        let entries = list_in(&conn).unwrap();
        assert_eq!(entries.len(), 3);
        assert!(entries
            .iter()
            .any(|entry| entry.uri == "sftp://alice@server/"));
        forget_in(&conn, "sftp://alice@server/home/alice").unwrap();
        let remaining = list_in(&conn).unwrap();
        assert_eq!(remaining.len(), 2);
        assert!(remaining
            .iter()
            .any(|entry| entry.uri == "sftp://bob@server/"));
        assert!(remaining
            .iter()
            .any(|entry| entry.uri == "sftp://alice@server:2222/"));
        let stored: i64 = conn
            .query_row("SELECT COUNT(*) FROM network_connections", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(stored, 2);
    }

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
        assert_eq!(entries.len(), 2);
        assert!(entries
            .iter()
            .any(|entry| entry.label == "SFTP (alice@server:2222/Photos)"));
        forget_in(&conn, "ssh://alice@server:2222/Photos").unwrap();
        assert_eq!(list_in(&conn).unwrap().len(), 1);
        forget_in(&conn, "ssh://alice@server:2222/Photos").unwrap();
        assert_eq!(list_in(&conn).unwrap().len(), 1);
    }

    #[test]
    fn rejected_addresses_never_enter_the_database() {
        let conn = Connection::open_in_memory().unwrap();
        assert!(remember_in(&conn, "sftp://user:secret@server/").is_err());
        assert!(list_in(&conn).unwrap().is_empty());
    }

    #[test]
    fn distinct_share_export_and_webdav_scopes_are_not_collapsed() {
        for (a, b) in [
            ("smb://alice@server/share-a", "smb://alice@server/share-b"),
            ("afp://server/volume-a", "afp://server/volume-b"),
            ("nfs://server/export-a", "nfs://server/export-b"),
            ("dav://server/files/alice", "dav://server/files/bob"),
            ("ftp://server/", "ftps://server/"),
        ] {
            assert_ne!(
                connection_identity(a).unwrap(),
                connection_identity(b).unwrap()
            );
        }
        assert_eq!(
            connection_identity("smb://alice@server/share/child").unwrap(),
            connection_identity("smb://alice@server/share/").unwrap()
        );
        assert_eq!(
            connection_identity("sftp://alice@server:22/home/alice").unwrap(),
            connection_identity("ssh://alice@server/").unwrap()
        );
        assert_eq!(
            connection_identity("sftp://alice@[2001:db8::1]:22/home/alice").unwrap(),
            connection_identity("ssh://alice@[2001:db8::1]/").unwrap()
        );
    }

    #[test]
    fn listing_preserves_stored_folders_and_forget_does_not_remove_another_share() {
        let conn = Connection::open_in_memory().unwrap();
        remember_in(&conn, "smb://alice@server/share-a/child").unwrap();
        remember_in(&conn, "smb://alice@server/share-a/").unwrap();
        remember_in(&conn, "smb://alice@server/share-b/").unwrap();
        assert_eq!(list_in(&conn).unwrap().len(), 2);
        let stored: i64 = conn
            .query_row("SELECT COUNT(*) FROM network_connections", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(stored, 3);
        forget_in(&conn, "smb://alice@server/share-a/").unwrap();
        assert_eq!(
            list_in(&conn).unwrap()[0].uri,
            "smb://alice@server/share-b/"
        );
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
