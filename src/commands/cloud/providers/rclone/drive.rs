//! Google Drive object addressing. Listings keep rclone's IDs and exports; only
//! ID mutations/metadata use the Drive API, sharing a lazy pooled HTTP client.
//! Authentication remains owned by rclone, including refresh/encrypted configs.
use super::*;
use reqwest::{blocking::Client, Method};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    ffi::OsString,
    sync::{Arc, Mutex, OnceLock},
    time::Duration,
};
use zeroize::Zeroizing;

struct Auth {
    token: Zeroizing<String>,
    expires: i64,
    root_id: String,
    default_encoding: bool,
}
type AuthCache = Mutex<HashMap<(OsString, String), Arc<Auth>>>;
static AUTH: OnceLock<AuthCache> = OnceLock::new();
static HTTP: OnceLock<Result<Client, String>> = OnceLock::new();
#[cfg(test)]
thread_local! { static API_BASE: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) }; }
#[cfg(all(test, unix))]
mod tests;

const FIELDS: &str = "id,name,mimeType,size,modifiedTime,parents,trashed,shortcutDetails";

fn error(code: CloudCommandErrorCode, message: &str) -> CloudCommandError {
    CloudCommandError::new(code, message)
}
fn cancelled(cancel: Option<&AtomicBool>) -> CloudCommandResult<()> {
    if cancel.is_some_and(|c| c.load(std::sync::atomic::Ordering::Relaxed)) {
        Err(error(
            CloudCommandErrorCode::Cancelled,
            "Google Drive operation cancelled",
        ))
    } else {
        Ok(())
    }
}
pub(crate) fn reset_auth() {
    if let Some(cache) = AUTH.get() {
        if let Ok(mut cache) = cache.lock() {
            cache.clear();
        }
    }
}
fn wipe_json(value: &mut Value) {
    match value {
        Value::String(s) => {
            use zeroize::Zeroize;
            s.zeroize();
        }
        Value::Array(a) => a.iter_mut().for_each(wipe_json),
        Value::Object(o) => o.values_mut().for_each(wipe_json),
        _ => {}
    }
}
fn parse_auth(config: &Value, remote: &str) -> CloudCommandResult<Auth> {
    let token = config.get(remote).and_then(|c| c.get("token")).and_then(Value::as_str)
        .ok_or_else(|| error(CloudCommandErrorCode::Unsupported,
            "Google Drive ID changes require an OAuth rclone remote; this authentication type cannot safely address the selected object"))?;
    let mut token: Value = serde_json::from_str(token).map_err(|_| {
        error(
            CloudCommandErrorCode::AuthRequired,
            "Invalid Google Drive authentication; reconnect this rclone remote",
        )
    })?;
    let result = (|| {
        let access = token
            .get("access_token")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| {
                error(
                    CloudCommandErrorCode::AuthRequired,
                    "Google Drive authentication is missing; reconnect this rclone remote",
                )
            })?;
        let expires = token
            .get("expiry")
            .and_then(Value::as_str)
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .map(|d| d.timestamp())
            .unwrap_or(0);
        Ok(Auth {
            token: Zeroizing::new(access.to_owned()),
            expires,
            root_id: config[remote]
                .get("root_folder_id")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
                .or_else(|| {
                    config[remote]
                        .get("team_drive")
                        .and_then(Value::as_str)
                        .filter(|s| !s.is_empty())
                })
                .unwrap_or("root")
                .to_owned(),
            default_encoding: config[remote]
                .get("encoding")
                .and_then(Value::as_str)
                .is_none_or(|s| s.is_empty() || s == "InvalidUtf8"),
        })
    })();
    wipe_json(&mut token);
    result
}
impl RcloneCloudProvider {
    pub(super) fn is_drive_remote(&self, path: &CloudPath) -> CloudCommandResult<bool> {
        let kind = self.resolve_provider_kind_for_write_policy(path.remote())?;
        if path.is_drive_address() && kind != CloudProviderKind::Gdrive {
            return Err(error(
                CloudCommandErrorCode::InvalidPath,
                "Google Drive object address used with another provider",
            ));
        }
        Ok(kind == CloudProviderKind::Gdrive)
    }

    fn drive_auth(
        &self,
        path: &CloudPath,
        cancel: Option<&AtomicBool>,
    ) -> CloudCommandResult<Arc<Auth>> {
        cancelled(cancel)?;
        let key = (self.cli.binary().to_os_string(), path.remote().to_owned());
        let cache = AUTH.get_or_init(Default::default);
        let now = chrono::Utc::now().timestamp();
        if let Some(auth) = cache
            .lock()
            .ok()
            .and_then(|c| c.get(&key).cloned())
            .filter(|a| a.expires > now + 30)
        {
            return Ok(auth);
        }
        let read = || -> CloudCommandResult<Auth> {
            let bytes = Zeroizing::new(
                self.cli
                    .run_capture_text_with_cancel(
                        RcloneCommandSpec::new(RcloneSubcommand::ConfigDump),
                        cancel,
                    )
                    .map_err(|e| super::error::map_rclone_error_for_remote(path.remote(), e))?
                    .stdout,
            );
            let mut config: Value = serde_json::from_str(&bytes).map_err(|_| {
                error(
                    CloudCommandErrorCode::InvalidConfig,
                    "Cannot read rclone authentication",
                )
            })?;
            let auth = parse_auth(&config, path.remote());
            wipe_json(&mut config);
            auth
        };
        let mut auth = read()?;
        if auth.expires <= now + 30 {
            // A read-only scoped command lets rclone refresh using its own client
            // credentials. No custom OAuth flow and no write replay on auth failure.
            let id = path.drive_target_id().ok_or_else(|| {
                error(
                    CloudCommandErrorCode::InvalidPath,
                    "Missing Google Drive object ID",
                )
            })?;
            self.cli
                .run_capture_text_with_cancel(
                    RcloneCommandSpec::new(RcloneSubcommand::Backend)
                        .arg("query")
                        .arg(format!("{}:", path.remote()))
                        .arg(format!("'{id}' in parents and trashed = false")),
                    cancel,
                )
                .map_err(|e| super::error::map_rclone_error_for_remote(path.remote(), e))?;
            auth = read()?;
        }
        if auth.expires <= now + 5 {
            return Err(error(
                CloudCommandErrorCode::AuthRequired,
                "Google Drive authentication could not be refreshed; reconnect this rclone remote",
            ));
        }
        let auth = Arc::new(auth);
        if let Ok(mut cache) = cache.lock() {
            cache.insert(key, auth.clone());
        }
        Ok(auth)
    }

    fn drive_request(
        &self,
        path: &CloudPath,
        id: &str,
        method: Method,
        body: Option<Value>,
        query: &[(&str, String)],
        cancel: Option<&AtomicBool>,
    ) -> CloudCommandResult<Option<Value>> {
        cancelled(cancel)?;
        self.is_drive_remote(path)?;
        let auth = self.drive_auth(path, cancel)?;
        let http = HTTP
            .get_or_init(|| {
                Client::builder()
                    .redirect(reqwest::redirect::Policy::none())
                    .connect_timeout(Duration::from_secs(10))
                    .timeout(Duration::from_secs(30))
                    .build()
                    .map_err(|_| "Cannot initialize Google Drive HTTP client".to_owned())
            })
            .as_ref()
            .map_err(|s| error(CloudCommandErrorCode::TaskFailed, s))?;
        let mut request = http
            .request(method.clone(), {
                #[cfg(test)]
                let base = API_BASE
                    .with(|v| v.borrow().clone())
                    .unwrap_or_else(|| "https://www.googleapis.com/drive/v3/files".into());
                #[cfg(not(test))]
                let base = "https://www.googleapis.com/drive/v3/files";
                format!("{base}/{id}")
            })
            .bearer_auth(auth.token.as_str())
            .query(&[("supportsAllDrives", "true"), ("fields", FIELDS)])
            .query(query);
        if let Some(body) = body {
            request = request.json(&body);
        }
        let response = request.send().map_err(|e| {
            error(
                if e.is_timeout() {
                    CloudCommandErrorCode::Timeout
                } else {
                    CloudCommandErrorCode::NetworkError
                },
                if method == Method::GET {
                    "Google Drive metadata request failed"
                } else {
                    "Google Drive change could not be confirmed; refresh before retrying"
                },
            )
        })?;
        let status = response.status();
        if status == reqwest::StatusCode::NOT_FOUND && method == Method::GET {
            return Ok(None);
        }
        if !status.is_success() {
            let code = match status.as_u16() {
                401 => {
                    reset_auth();
                    CloudCommandErrorCode::AuthRequired
                }
                403 => CloudCommandErrorCode::PermissionDenied,
                404 => CloudCommandErrorCode::NotFound,
                429 => CloudCommandErrorCode::RateLimited,
                _ => CloudCommandErrorCode::TaskFailed,
            };
            return Err(error(code, "Google Drive refused the selected-object operation; refresh or check account permissions before retrying"));
        }
        if status == reqwest::StatusCode::NO_CONTENT {
            return Ok(Some(Value::Null));
        }
        response.json().map(Some).map_err(|_| {
            error(
                CloudCommandErrorCode::TaskFailed,
                "Google Drive response could not be verified; refresh before retrying",
            )
        })
    }

    pub(super) fn drive_metadata(
        &self,
        path: &CloudPath,
        cancel: Option<&AtomicBool>,
    ) -> CloudCommandResult<Option<Value>> {
        let id = path.drive_object_id().ok_or_else(|| {
            error(
                CloudCommandErrorCode::InvalidPath,
                "Missing Google Drive object identity",
            )
        })?;
        self.drive_request(path, id, Method::GET, None, &[], cancel)
            .map(|v| v.filter(|m| m.get("trashed").and_then(Value::as_bool) != Some(true)))
    }

    pub(super) fn drive_stat(
        &self,
        path: &CloudPath,
        cancel: Option<&AtomicBool>,
    ) -> CloudCommandResult<Option<CloudEntry>> {
        let metadata = match self.drive_metadata(path, cancel) {
            Err(e) if e.code() == CloudCommandErrorCode::Unsupported => {
                // Service-account/ADC remotes keep ID-aware reading through
                // rclone. Direct ID mutations need supported OAuth credentials.
                let parent = path.parent_dir_path().expect("object parent");
                return self
                    .list_dir_impl(
                        &parent,
                        RcloneReadOptions {
                            cancel,
                            ..Default::default()
                        },
                    )
                    .map(|entries| {
                        entries.into_iter().find(|entry| {
                            CloudPath::parse(&entry.path)
                                .is_ok_and(|p| p.drive_id() == path.drive_id())
                        })
                    });
            }
            result => result?,
        };
        let Some(mut metadata) = metadata else {
            return Ok(None);
        };
        if path.drive_id().is_some_and(|s| s.contains(':')) {
            let target = path.drive_target_id().expect("ID");
            let Some(target_metadata) =
                self.drive_request(path, target, Method::GET, None, &[], cancel)?
            else {
                return Ok(None);
            };
            metadata["mimeType"] = target_metadata["mimeType"].clone();
            metadata["size"] = target_metadata["size"].clone();
        }
        let dir = metadata["mimeType"] == "application/vnd.google-apps.folder";
        Ok(Some(CloudEntry {
            name: path
                .leaf_name()
                .map_err(crate::commands::cloud::map_cloud_path_error)?
                .to_owned(),
            path: path.to_string(),
            kind: if dir {
                CloudEntryKind::Dir
            } else {
                CloudEntryKind::File
            },
            size: if dir {
                None
            } else {
                metadata
                    .get("size")
                    .and_then(Value::as_str)
                    .and_then(|s| s.parse().ok())
            },
            modified: metadata
                .get("modifiedTime")
                .and_then(Value::as_str)
                .map(super::read::normalize_cloud_modified_time_value),
            capabilities: CloudCapabilities::v1_for_provider(CloudProviderKind::Gdrive),
        }))
    }

    pub(super) fn drive_delete(
        &self,
        path: &CloudPath,
        trash: bool,
        empty_only: bool,
        cancel: Option<&AtomicBool>,
    ) -> CloudCommandResult<()> {
        if self.drive_metadata(path, cancel)?.is_none() {
            return Err(error(
                CloudCommandErrorCode::NotFound,
                "Selected Google Drive object no longer exists",
            ));
        }
        if empty_only
            && !self
                .list_dir_impl(
                    path,
                    RcloneReadOptions {
                        cancel,
                        ..Default::default()
                    },
                )?
                .is_empty()
        {
            return Err(error(
                CloudCommandErrorCode::DestinationExists,
                "Google Drive folder is not empty",
            ));
        }
        let id = path.drive_object_id().expect("checked ID");
        self.drive_request(
            path,
            id,
            if trash { Method::PATCH } else { Method::DELETE },
            trash.then(|| json!({"trashed": true})),
            &[],
            cancel,
        )?;
        Ok(())
    }

    /// Resolve a legacy/name-only address once per directory, refusing ambiguous
    /// names. Already selected IDs incur no traversal or name lookup.
    pub(super) fn resolve_drive_path(
        &self,
        path: &CloudPath,
        missing_leaf: bool,
        cancel: Option<&AtomicBool>,
    ) -> CloudCommandResult<Option<CloudPath>> {
        if path.is_root() || path.drive_id().is_some() {
            return Ok(Some(path.clone()));
        }
        let parent = path.parent_dir_path().expect("non-root");
        let Some(parent) = self.resolve_drive_path(&parent, false, cancel)? else {
            return Ok(None);
        };
        let name = path
            .leaf_name()
            .map_err(crate::commands::cloud::map_cloud_path_error)?;
        let entries = self.list_dir_impl(
            &parent,
            RcloneReadOptions {
                cancel,
                ..Default::default()
            },
        )?;
        let matches: Vec<_> = entries.iter().filter(|e| e.name == name).collect();
        if matches.len() > 1 {
            return Err(error(CloudCommandErrorCode::InvalidPath, "Multiple Google Drive objects have this name; refresh the folder and select a specific item"));
        }
        if let Some(entry) = matches.first() {
            return CloudPath::parse(&entry.path)
                .map(Some)
                .map_err(crate::commands::cloud::map_cloud_path_error);
        }
        if missing_leaf {
            parent
                .child_path(name)
                .map(Some)
                .map_err(crate::commands::cloud::map_cloud_path_error)
        } else {
            Ok(None)
        }
    }

    pub(super) fn drive_download(
        &self,
        path: &CloudPath,
        dest: &Path,
        cancel: Option<&AtomicBool>,
    ) -> CloudCommandResult<()> {
        let id = path.drive_target_id().ok_or_else(|| {
            error(
                CloudCommandErrorCode::InvalidPath,
                "Missing Google Drive source ID",
            )
        })?;
        self.cli
            .run_capture_text_with_cancel(
                RcloneCommandSpec::new(RcloneSubcommand::BackendTransfer)
                    .arg("copyid")
                    .arg(format!("{}:", path.remote()))
                    .arg(id)
                    .arg(dest.as_os_str())
                    .arg("--ignore-times")
                    .local_destination_options(dest),
                cancel,
            )
            .map_err(|e| super::error::map_rclone_error_for_remote(path.remote(), e))?;
        Ok(())
    }

    pub(super) fn drive_transfer(
        &self,
        src: &CloudPath,
        dst: &CloudPath,
        moving: bool,
        overwrite: bool,
        cancel: Option<&AtomicBool>,
    ) -> CloudCommandResult<()> {
        cancelled(cancel)?;
        let src = if self.is_drive_remote(src)? {
            self.resolve_drive_path(src, false, cancel)?
                .ok_or_else(|| {
                    error(
                        CloudCommandErrorCode::NotFound,
                        "Google Drive source no longer exists",
                    )
                })?
        } else {
            src.clone()
        };
        let dst = if self.is_drive_remote(dst)? {
            self.resolve_drive_path(dst, true, cancel)?.ok_or_else(|| {
                error(
                    CloudCommandErrorCode::NotFound,
                    "Google Drive destination parent no longer exists",
                )
            })?
        } else {
            dst.clone()
        };
        if src == dst
            || (src.remote() == dst.remote()
                && src.drive_object_id().is_some()
                && src.drive_object_id() == dst.drive_object_id())
        {
            return Err(error(
                CloudCommandErrorCode::InvalidPath,
                "Source and destination are the same",
            ));
        }
        if dst.contains_drive_ancestor(src.drive_target_id()) {
            return Err(error(
                CloudCommandErrorCode::InvalidPath,
                "Cannot transfer a Google Drive folder into itself or its descendant",
            ));
        }
        let source = self.stat_path(&src)?.ok_or_else(|| {
            error(
                CloudCommandErrorCode::NotFound,
                "Cloud source no longer exists",
            )
        })?;
        let target = self.stat_path(&dst)?;
        if dst.drive_id().is_some() && target.is_none() {
            return Err(error(
                CloudCommandErrorCode::NotFound,
                "Selected Google Drive destination no longer exists; refresh before retrying",
            ));
        }
        if !overwrite && target.is_some() {
            return Err(error(
                CloudCommandErrorCode::DestinationExists,
                "A file or folder with the same name already exists",
            ));
        }
        if target.as_ref().is_some_and(|e| e.kind != source.kind) {
            return Err(error(
                CloudCommandErrorCode::Unsupported,
                "Cannot overwrite a file with a folder or a folder with a file",
            ));
        }
        if target
            .as_ref()
            .is_some_and(|e| e.kind == CloudEntryKind::Dir)
            && dst.is_drive_address()
        {
            self.ensure_drive_tree_unambiguous(&dst, cancel)?;
        }
        if target
            .as_ref()
            .is_some_and(|e| e.kind == CloudEntryKind::File)
        {
            self.ensure_drive_file_destination_unambiguous(&dst, cancel)?;
            if src.is_drive_address() {
                // copyid passes a nil destination to rclone. Drive would create
                // another object instead of replacing the selected destination.
                // The standard engine is safe only with uniquely verified names.
                self.ensure_drive_file_destination_unambiguous(&src, cancel)?;
                self.cli
                    .run_capture_text_with_cancel(
                        RcloneCommandSpec::new(if moving {
                            RcloneSubcommand::MoveTo
                        } else {
                            RcloneSubcommand::CopyTo
                        })
                        .arg(src.drive_destination_spec())
                        .arg(if dst.is_drive_address() {
                            dst.drive_destination_spec()
                        } else {
                            dst.to_rclone_remote_spec()
                        })
                        .arg("--drive-server-side-across-configs")
                        .arg("--ignore-times"),
                        cancel,
                    )
                    .map_err(|e| super::error::map_rclone_error_for_paths(&[&src, &dst], e))?;
                return Ok(());
            }
        }
        if moving
            && source.kind == CloudEntryKind::Dir
            && src.is_drive_address()
            && src.remote() == dst.remote()
            && target.is_none()
        {
            return self.drive_move(&src, &dst, overwrite, cancel);
        }
        if src.is_drive_address()
            && (source.kind == CloudEntryKind::File
                || src.drive_id().is_some_and(|id| id.contains(':')))
        {
            let destination = if dst.is_drive_address() {
                dst.drive_destination_spec()
            } else {
                dst.to_rclone_remote_spec()
            };
            let mut spec = RcloneCommandSpec::new(RcloneSubcommand::BackendTransfer)
                .arg(if moving { "moveid" } else { "copyid" })
                .arg(format!("{}:", src.remote()))
                .arg(src.drive_object_id().expect("file identity"))
                .arg(destination)
                .arg("--drive-server-side-across-configs")
                .arg("--ignore-times");
            if !overwrite {
                spec = spec.arg("--immutable").arg("--ignore-existing");
                if !moving {
                    spec = spec.arg("--error-on-no-transfer");
                }
            }
            if moving {
                // Backend Move is accounted as a check, even after a successful
                // server-side rename. Override this flag, including its env default.
                spec = spec.arg("--error-on-no-transfer=false");
            }
            self.cli
                .run_capture_text_with_cancel(spec, cancel)
                .map_err(|e| super::error::map_rclone_error_for_paths(&[&src, &dst], e))?;
            return Ok(());
        }
        // Directory contents use the existing bulk rclone transfer. A single
        // recursive metadata pass detects duplicate descendants before writes;
        // rclone's name-based bulk engine cannot safely represent those trees.
        let src_spec = if src.is_drive_address() {
            self.ensure_drive_tree_unambiguous(&src, cancel)?;
            let (fs, rel) = src.drive_directory_spec();
            format!("{fs}{rel}")
        } else {
            src.to_rclone_remote_spec()
        };
        let dst_spec = if dst.is_drive_address()
            && dst.drive_id().is_some()
            && source.kind == CloudEntryKind::Dir
        {
            let (fs, rel) = dst.drive_directory_spec();
            format!("{fs}{rel}")
        } else if dst.is_drive_address() {
            dst.drive_destination_spec()
        } else {
            dst.to_rclone_remote_spec()
        };
        let mut spec = RcloneCommandSpec::new(if moving && source.kind == CloudEntryKind::Dir {
            RcloneSubcommand::Move
        } else if moving {
            RcloneSubcommand::MoveTo
        } else if source.kind == CloudEntryKind::Dir {
            RcloneSubcommand::Copy
        } else {
            RcloneSubcommand::CopyTo
        })
        .arg(src_spec)
        .arg(&dst_spec)
        .arg("--drive-server-side-across-configs");
        if source.kind == CloudEntryKind::Dir {
            spec = spec.arg("--create-empty-src-dirs");
        }
        if moving {
            spec = spec.arg("--delete-empty-src-dirs");
        }
        if overwrite {
            spec = spec.arg("--ignore-times");
        } else {
            spec = spec.arg("--immutable").arg("--checksum");
        }
        self.cli
            .run_capture_text_with_cancel(spec, cancel)
            .map_err(|e| super::error::map_rclone_error_for_paths(&[&src, &dst], e))?;
        if source.kind == CloudEntryKind::Dir {
            self.cli
                .run_capture_text_with_cancel(
                    RcloneCommandSpec::new(RcloneSubcommand::Mkdir).arg(dst_spec),
                    cancel,
                )
                .map_err(|e| super::error::map_rclone_error_for_paths(&[&src, &dst], e))?;
            if moving && src.is_drive_address() {
                self.drive_delete(&src, false, true, cancel)?;
            }
        }
        Ok(())
    }

    /// rclone copyid/upload replaces by destination name. An existing duplicate
    /// must never be chosen implicitly, even when the caller supplied its ID.
    pub(super) fn ensure_drive_file_destination_unambiguous(
        &self,
        path: &CloudPath,
        cancel: Option<&AtomicBool>,
    ) -> CloudCommandResult<()> {
        if !path.is_drive_address() {
            return Ok(());
        }
        self.is_drive_remote(path)?;
        let parent = path.parent_dir_path().ok_or_else(|| {
            error(
                CloudCommandErrorCode::InvalidPath,
                "Missing Google Drive destination parent",
            )
        })?;
        let name = path
            .leaf_name()
            .map_err(crate::commands::cloud::map_cloud_path_error)?;
        let entries = self.list_dir_impl(
            &parent,
            RcloneReadOptions {
                cancel,
                ..Default::default()
            },
        )?;
        if entries.iter().filter(|e| e.name == name).count() > 1 {
            return Err(error(CloudCommandErrorCode::Unsupported,
                "Cannot overwrite one of several Google Drive objects with the same name; choose a unique destination name"));
        }
        if let Some(id) = path.drive_id() {
            if !entries
                .iter()
                .filter(|e| e.name == name)
                .any(|entry| CloudPath::parse(&entry.path).is_ok_and(|p| p.drive_id() == Some(id)))
            {
                return Err(error(CloudCommandErrorCode::NotFound,
                    "Selected Google Drive destination moved, changed name or disappeared; refresh before retrying"));
            }
        }
        Ok(())
    }

    pub(crate) fn ensure_drive_tree_unambiguous(
        &self,
        path: &CloudPath,
        cancel: Option<&AtomicBool>,
    ) -> CloudCommandResult<()> {
        self.is_drive_remote(path)?;
        let (fs, rel) = path.drive_directory_spec();
        let output = self
            .cli
            .run_capture_text_with_cancel(
                RcloneCommandSpec::new(RcloneSubcommand::LsJson)
                    .arg(format!("{fs}{rel}"))
                    .arg("--recursive")
                    .arg("--fast-list"),
                cancel,
            )
            .map_err(|e| super::error::map_rclone_error_for_remote(path.remote(), e))?;
        let entries: Vec<Value> = serde_json::from_str(&output.stdout).map_err(|_| {
            error(
                CloudCommandErrorCode::TaskFailed,
                "Cannot verify Google Drive directory tree",
            )
        })?;
        let mut paths = std::collections::HashSet::with_capacity(entries.len());
        for entry in &entries {
            let Some(name) = entry.get("Path").and_then(Value::as_str) else {
                return Err(error(
                    CloudCommandErrorCode::TaskFailed,
                    "Invalid Google Drive tree metadata",
                ));
            };
            if !paths.insert(name) {
                return Err(error(CloudCommandErrorCode::Unsupported,
                "This Google Drive tree contains identical names; select the individual objects and copy them with unique destination names"));
            }
        }
        Ok(())
    }

    pub(super) fn drive_move(
        &self,
        src: &CloudPath,
        dst: &CloudPath,
        overwrite: bool,
        cancel: Option<&AtomicBool>,
    ) -> CloudCommandResult<()> {
        if src.remote() != dst.remote() {
            return Err(error(
                CloudCommandErrorCode::Unsupported,
                "Moving Google Drive objects across remotes requires copy then explicit removal",
            ));
        }
        let metadata = self.drive_metadata(src, cancel)?.ok_or_else(|| {
            error(
                CloudCommandErrorCode::NotFound,
                "Selected Google Drive object no longer exists",
            )
        })?;
        let parent = dst.parent_dir_path().ok_or_else(|| {
            error(
                CloudCommandErrorCode::InvalidPath,
                "Missing destination folder",
            )
        })?;
        let parent = self
            .resolve_drive_path(&parent, false, cancel)?
            .ok_or_else(|| {
                error(
                    CloudCommandErrorCode::NotFound,
                    "Destination folder no longer exists",
                )
            })?;
        let mut query = Vec::new();
        let parent_id = if parent.is_root() {
            let root = self.drive_auth(src, cancel)?.root_id.clone();
            let metadata = self
                .drive_request(src, &root, Method::GET, None, &[], cancel)?
                .ok_or_else(|| {
                    error(
                        CloudCommandErrorCode::NotFound,
                        "Google Drive root no longer exists",
                    )
                })?;
            metadata["id"]
                .as_str()
                .ok_or_else(|| {
                    error(
                        CloudCommandErrorCode::TaskFailed,
                        "Cannot identify Google Drive root",
                    )
                })?
                .to_owned()
        } else {
            parent
                .drive_target_id()
                .ok_or_else(|| {
                    error(
                        CloudCommandErrorCode::Unsupported,
                        "Select a Google Drive destination folder before moving this object",
                    )
                })?
                .to_owned()
        };
        if metadata["parents"].as_array().is_none_or(|parents| {
            parents.len() != 1 || parents[0].as_str() != Some(parent_id.as_str())
        }) {
            let parents = metadata["parents"].as_array().ok_or_else(|| {
                error(
                    CloudCommandErrorCode::TaskFailed,
                    "Cannot verify Google Drive source parent",
                )
            })?;
            if parents.len() != 1 {
                return Err(error(
                    CloudCommandErrorCode::Unsupported,
                    "Cannot safely move a Google Drive object with multiple parents",
                ));
            }
            query.push(("addParents", parent_id));
            query.push((
                "removeParents",
                parents[0].as_str().unwrap_or_default().to_owned(),
            ));
        }
        if self.stat_path(dst)?.is_some() && src != dst {
            return Err(error(
                if overwrite {
                    CloudCommandErrorCode::Unsupported
                } else {
                    CloudCommandErrorCode::DestinationExists
                },
                "Google Drive destination exists; use Auto-rename or Skip for an ID-addressed move",
            ));
        }
        let name = dst
            .leaf_name()
            .map_err(crate::commands::cloud::map_cloud_path_error)?;
        let name = if name
            == src
                .leaf_name()
                .map_err(crate::commands::cloud::map_cloud_path_error)?
        {
            metadata["name"]
                .as_str()
                .ok_or_else(|| {
                    error(
                        CloudCommandErrorCode::TaskFailed,
                        "Missing Google Drive folder name",
                    )
                })?
                .to_owned()
        } else {
            if !self.drive_auth(src, cancel)?.default_encoding {
                return Err(error(
                    CloudCommandErrorCode::Unsupported,
                    "Google Drive folder rename with custom rclone encoding is not supported",
                ));
            }
            default_drive_name(name)
        };
        self.drive_request(
            src,
            src.drive_object_id().expect("source ID"),
            Method::PATCH,
            Some(json!({"name": name})),
            &query,
            cancel,
        )?;
        Ok(())
    }
}

/// rclone's Standard/Display encoding, decoded to the default Drive UTF-8 name.
/// Files use backend moveid, which also applies native-document export suffixes.
fn decode_standard_name(name: &str) -> String {
    match name {
        "．" => return ".".into(),
        "．．" => return "..".into(),
        "‛．" => return "．".into(),
        "‛．‛．" => return "．．".into(),
        _ => {}
    }
    let mut output = String::with_capacity(name.len());
    let mut chars = name.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '‛'
            && chars
                .peek()
                .is_some_and(|c| matches!(c, '‛' | '／' | '␡' | '␀'..='␟'))
        {
            output.push(chars.next().expect("peeked"));
        } else {
            output.push(match c {
                '／' => '/',
                '␡' => '\u{7f}',
                '␀'..='␟' => char::from_u32(c as u32 - '␀' as u32).expect("control"),
                _ => c,
            });
        }
    }
    output
}

fn default_drive_name(name: &str) -> String {
    let decoded = decode_standard_name(name);
    let mut encoded = String::with_capacity(decoded.len());
    for c in decoded.chars() {
        match c {
            '\0' => encoded.push('␀'),
            '␀' | '‛' => {
                encoded.push('‛');
                encoded.push(c);
            }
            _ => encoded.push(c),
        }
    }
    encoded
}
