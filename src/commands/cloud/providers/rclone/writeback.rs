//! Versioned, ID-scoped writes for editor working copies. Never fall back to
//! unconditional rclone copyto: If-Match protects changes made by other clients.
use super::*;
use aes::cipher::{Array, BlockCipherEncrypt, KeyInit};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use reqwest::{
    blocking::{Body, Client, RequestBuilder, Response},
    Method, StatusCode,
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{
    io::{self, Read},
    sync::{Arc, OnceLock},
    time::Duration,
};
use zeroize::{Zeroize, Zeroizing};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CloudWriteVersion {
    pub source_path: String,
    pub provider: CloudProviderKind,
    pub object_id: String,
    pub etag: String,
    pub mime_type: String,
}

static HTTP: OnceLock<Result<Client, String>> = OnceLock::new();
#[cfg(test)]
thread_local! { pub(super) static API_BASE: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) }; }

fn fail(code: CloudCommandErrorCode, text: &str) -> CloudCommandError {
    CloudCommandError::new(code, text)
}
fn unsupported() -> CloudCommandError {
    fail(CloudCommandErrorCode::Unsupported, "Automatic saving is unavailable for this file or remote configuration. Your local copy is kept; use Save as new file.")
}
fn conflict() -> CloudCommandError {
    fail(CloudCommandErrorCode::Conflict, "The cloud file changed, moved or was deleted. Automatic saving stopped; your local edits are kept. Review both versions or save as a new file.")
}
fn cancelled(cancel: Option<&AtomicBool>) -> CloudCommandResult<()> {
    if cancel.is_some_and(|c| c.load(std::sync::atomic::Ordering::Relaxed)) {
        Err(fail(
            CloudCommandErrorCode::Cancelled,
            "Cloud saving stopped; local edits are kept",
        ))
    } else {
        Ok(())
    }
}
fn http() -> CloudCommandResult<&'static Client> {
    HTTP.get_or_init(|| {
        Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(|_| "Cannot initialize cloud saving client".into())
    })
    .as_ref()
    .map_err(|s| fail(CloudCommandErrorCode::TaskFailed, s))
}
fn response(request: RequestBuilder, cancel: Option<&AtomicBool>) -> CloudCommandResult<Response> {
    cancelled(cancel)?;
    let response = request.send().map_err(|e| fail(if cancel.is_some_and(|c| c.load(std::sync::atomic::Ordering::Relaxed)) { CloudCommandErrorCode::Cancelled } else if e.is_timeout() { CloudCommandErrorCode::Timeout } else { CloudCommandErrorCode::NetworkError },
        "Cloud saving could not be confirmed. Your local edits are kept; check the connection and retry."))?;
    let status = response.status();
    if status.is_success() {
        return Ok(response);
    }
    Err(match status {
        StatusCode::PRECONDITION_FAILED
        | StatusCode::CONFLICT
        | StatusCode::NOT_FOUND
        | StatusCode::GONE => conflict(),
        StatusCode::UNAUTHORIZED => fail(
            CloudCommandErrorCode::AuthRequired,
            "Reconnect this rclone remote; your local edits are kept",
        ),
        StatusCode::FORBIDDEN => fail(
            CloudCommandErrorCode::PermissionDenied,
            "Cloud saving was denied; your local edits are kept",
        ),
        StatusCode::TOO_MANY_REQUESTS => fail(
            CloudCommandErrorCode::RateLimited,
            "Cloud saving is rate limited; local edits will be retried",
        ),
        _ => fail(
            CloudCommandErrorCode::TaskFailed,
            "Cloud saving was refused or could not be confirmed; your local edits are kept",
        ),
    })
}
fn json(response: Response) -> CloudCommandResult<Value> {
    let mut bytes = Vec::new();
    response
        .take(1024 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| {
            fail(
                CloudCommandErrorCode::NetworkError,
                "Cloud response was interrupted; local edits are kept",
            )
        })?;
    if bytes.len() > 1024 * 1024 {
        return Err(unsupported());
    }
    serde_json::from_slice(&bytes).map_err(|_| {
        fail(
            CloudCommandErrorCode::TaskFailed,
            "Cloud version response could not be verified",
        )
    })
}
fn string(value: &Value, field: &str) -> CloudCommandResult<String> {
    value
        .get(field)
        .and_then(Value::as_str)
        .filter(|v| !v.is_empty())
        .map(str::to_owned)
        .ok_or_else(unsupported)
}
fn etag(value: String) -> CloudCommandResult<String> {
    // Weak/wildcard validators are not safe for a conditional replacement.
    if value.starts_with('"')
        && value.ends_with('"')
        && value.len() > 2
        && !value[1..value.len() - 1]
            .bytes()
            .any(|b| b == b'\"' || b < 0x21 || b == 0x7f)
    {
        Ok(value)
    } else {
        Err(unsupported())
    }
}
fn wipe(value: &mut Value) {
    match value {
        Value::String(s) => s.zeroize(),
        Value::Array(a) => a.iter_mut().for_each(wipe),
        Value::Object(o) => o.values_mut().for_each(wipe),
        _ => {}
    }
}
struct Config(Value);
impl Drop for Config {
    fn drop(&mut self) {
        wipe(&mut self.0);
    }
}

enum Credentials {
    Bearer(Zeroizing<String>),
    Basic(String, Zeroizing<String>),
}
impl Credentials {
    fn apply(&self, request: RequestBuilder) -> RequestBuilder {
        match self {
            Self::Bearer(token) => request.bearer_auth(token.as_str()),
            Self::Basic(user, password) => request.basic_auth(user, Some(password.as_str())),
        }
    }
}
fn reveal(value: &str) -> CloudCommandResult<Zeroizing<String>> {
    // rclone's public obscure format: 16-byte IV followed by AES-256-CTR,
    // raw URL-safe base64. This is decoding, not a new credential store.
    let key = [
        0x9c, 0x93, 0x5b, 0x48, 0x73, 0x0a, 0x55, 0x4d, 0x6b, 0xfd, 0x7c, 0x63, 0xc8, 0x86, 0xa9,
        0x2b, 0xd3, 0x90, 0x19, 0x8e, 0xb8, 0x12, 0x8a, 0xfb, 0xf4, 0xde, 0x16, 0x2b, 0x8b, 0x95,
        0xf6, 0x38,
    ];
    let mut bytes = Zeroizing::new(URL_SAFE_NO_PAD.decode(value).map_err(|_| unsupported())?);
    if bytes.len() < 16 {
        return Err(unsupported());
    }
    let mut counter: [u8; 16] = bytes[..16].try_into().map_err(|_| unsupported())?;
    let cipher = aes::Aes256::new(&Array::from(key));
    for chunk in bytes[16..].chunks_mut(16) {
        let mut block = Array::from(counter);
        cipher.encrypt_block(&mut block);
        for (byte, pad) in chunk.iter_mut().zip(block.iter()) {
            *byte ^= pad;
        }
        block.as_mut_slice().zeroize();
        for byte in counter.iter_mut().rev() {
            let (next, carry) = byte.overflowing_add(1);
            *byte = next;
            if !carry {
                break;
            }
        }
    }
    counter.zeroize();
    std::str::from_utf8(&bytes[16..])
        .map(|s| Zeroizing::new(s.to_owned()))
        .map_err(|_| unsupported())
}
fn endpoint(base: &str, segments: &[&str]) -> CloudCommandResult<String> {
    let mut url = url::Url::parse(base).map_err(|_| unsupported())?;
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        #[cfg(test)]
        if url.scheme() == "http" && url.host_str() == Some("127.0.0.1") {
            url.path_segments_mut()
                .map_err(|_| unsupported())?
                .pop_if_empty()
                .extend(segments);
            return Ok(url.into());
        }
        return Err(unsupported());
    }
    url.path_segments_mut()
        .map_err(|_| unsupported())?
        .pop_if_empty()
        .extend(segments);
    Ok(url.into())
}
fn api_base(default: &str) -> String {
    #[cfg(test)]
    if let Some(base) = API_BASE.with(|b| b.borrow().clone()) {
        return base;
    }
    default.into()
}

impl RcloneCloudProvider {
    fn write_config(&self, cancel: Option<&AtomicBool>) -> CloudCommandResult<Config> {
        let text = Zeroizing::new(
            self.cli
                .run_capture_text_with_cancel(
                    RcloneCommandSpec::new(RcloneSubcommand::ConfigDump),
                    cancel,
                )
                .map_err(|e| super::error::map_rclone_error_for_remote("cloud editing", e))?
                .stdout,
        );
        serde_json::from_str(&text)
            .map(Config)
            .map_err(|_| unsupported())
    }
    fn write_credentials(
        &self,
        path: &CloudPath,
        kind: CloudProviderKind,
        cancel: Option<&AtomicBool>,
    ) -> CloudCommandResult<(Credentials, Config)> {
        if kind == CloudProviderKind::Gdrive {
            return Ok((
                Credentials::Bearer(self.drive_write_token(path, cancel)?),
                Config(Value::Null),
            ));
        }
        let mut config = self.write_config(cancel)?;
        if kind == CloudProviderKind::Nextcloud {
            let cfg = &config.0[path.remote()];
            if cfg
                .get("headers")
                .and_then(Value::as_str)
                .is_some_and(|s| !s.is_empty())
            {
                return Err(unsupported());
            }
            let credentials = if let Some(token) = cfg
                .get("bearer_token")
                .and_then(Value::as_str)
                .filter(|s| !s.is_empty())
            {
                Credentials::Bearer(Zeroizing::new(token.to_owned()))
            } else {
                Credentials::Basic(string(cfg, "user")?, reveal(&string(cfg, "pass")?)?)
            };
            return Ok((credentials, config));
        }
        let token = |config: &Config| -> CloudCommandResult<(Zeroizing<String>, i64)> {
            let text = config.0[path.remote()]
                .get("token")
                .and_then(Value::as_str)
                .ok_or_else(unsupported)?;
            let mut token: Value = serde_json::from_str(text).map_err(|_| unsupported())?;
            let result = (|| {
                Ok((
                    Zeroizing::new(string(&token, "access_token")?),
                    token
                        .get("expiry")
                        .and_then(Value::as_str)
                        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                        .map(|d| d.timestamp())
                        .unwrap_or(0),
                ))
            })();
            wipe(&mut token);
            result
        };
        let (mut access, expires) = token(&config)?;
        if expires <= chrono::Utc::now().timestamp() + 30 {
            // CLI owns OAuth refresh and encrypted config handling. This is read-only.
            self.cli
                .run_capture_text_with_cancel(
                    RcloneCommandSpec::new(RcloneSubcommand::LsJson)
                        .arg(path.to_rclone_remote_spec())
                        .arg("--stat"),
                    cancel,
                )
                .map_err(|e| super::error::map_rclone_error_for_remote(path.remote(), e))?;
            config = self.write_config(cancel)?;
            let fresh = token(&config)?;
            if fresh.1 <= chrono::Utc::now().timestamp() + 5 {
                return Err(fail(
                    CloudCommandErrorCode::AuthRequired,
                    "Cloud authentication expired; reconnect this rclone remote",
                ));
            }
            access = fresh.0;
        }
        Ok((Credentials::Bearer(access), config))
    }
    fn version_request(
        &self,
        path: &CloudPath,
        previous: Option<&CloudWriteVersion>,
        cancel: Option<&AtomicBool>,
    ) -> CloudCommandResult<(CloudWriteVersion, String, Credentials)> {
        cancelled(cancel)?;
        let kind = self.resolve_provider_kind_for_write_policy(path.remote())?;
        let path = if kind == CloudProviderKind::Gdrive {
            self.resolve_drive_path(path, false, cancel)?
                .ok_or_else(conflict)?
        } else {
            path.clone()
        };
        let (credentials, config) = self.write_credentials(&path, kind, cancel)?;
        let (id, url) = match kind {
            CloudProviderKind::Gdrive => {
                let id = path.drive_object_id().ok_or_else(unsupported)?.to_owned();
                let url = endpoint(
                    &api_base("https://www.googleapis.com/drive/v2/files"),
                    &[&id],
                )?;
                (id, url)
            }
            CloudProviderKind::Onedrive => {
                let cfg = &config.0[path.remote()];
                if cfg
                    .get("region")
                    .and_then(Value::as_str)
                    .is_some_and(|s| !s.is_empty() && s != "global")
                {
                    return Err(unsupported());
                }
                let id = if let Some(v) = previous {
                    v.object_id.clone()
                } else {
                    let text = self
                        .cli
                        .run_capture_text_with_cancel(
                            RcloneCommandSpec::new(RcloneSubcommand::LsJson)
                                .arg(path.to_rclone_remote_spec())
                                .arg("--stat"),
                            cancel,
                        )
                        .map_err(|e| super::error::map_rclone_error_for_remote(path.remote(), e))?
                        .stdout;
                    string(
                        &serde_json::from_str(&text).map_err(|_| unsupported())?,
                        "ID",
                    )?
                };
                let (drive, item) = if let Some((drive, item)) = id.split_once('#') {
                    (drive.to_owned(), item.to_owned())
                } else {
                    (string(cfg, "drive_id")?, id.clone())
                };
                let url = endpoint(
                    &api_base("https://graph.microsoft.com/v1.0"),
                    &["drives", &drive, "items", &item],
                )?;
                (format!("{drive}#{item}"), url)
            }
            CloudProviderKind::Nextcloud => {
                let cfg = &config.0[path.remote()];
                // Rclone's default Nextcloud encoding maps ordinary Unicode paths
                // directly. Custom encodings cannot be guessed by a DAV writer.
                if cfg
                    .get("encoding")
                    .and_then(Value::as_str)
                    .is_some_and(|s| !s.is_empty() && s != "Slash,InvalidUtf8,Dot")
                {
                    return Err(unsupported());
                }
                let base = string(cfg, "url")?;
                let url = endpoint(&base, &path.rel_path().split('/').collect::<Vec<_>>())?;
                (path.to_string(), url)
            }
        };
        let mut request = credentials.apply(http()?.request(
            if kind == CloudProviderKind::Nextcloud {
                Method::HEAD
            } else {
                Method::GET
            },
            &url,
        ));
        if kind == CloudProviderKind::Gdrive {
            request = request.query(&[
                ("supportsAllDrives", "true"),
                ("fields", "id,etag,mimeType,labels,capabilities"),
            ]);
        }
        let result = response(request, cancel)?;
        let (tag, mime, identity) = if kind == CloudProviderKind::Nextcloud {
            let tag = result
                .headers()
                .get(reqwest::header::ETAG)
                .and_then(|v| v.to_str().ok())
                .ok_or_else(unsupported)?
                .to_owned();
            let identity = result
                .headers()
                .get("oc-fileid")
                .and_then(|v| v.to_str().ok())
                .map(str::to_owned)
                .unwrap_or(id);
            (tag, "application/octet-stream".into(), identity)
        } else {
            let data = json(result)?;
            if kind == CloudProviderKind::Gdrive {
                let mime = string(&data, "mimeType")?;
                if mime.starts_with("application/vnd.google-apps.") {
                    return Err(unsupported());
                }
                if data["labels"]["trashed"] == true {
                    return Err(conflict());
                }
                if data["capabilities"]["canModifyContent"] == false {
                    return Err(unsupported());
                }
                let actual = string(&data, "id")?;
                if actual != id {
                    return Err(conflict());
                }
                (string(&data, "etag")?, mime, actual)
            } else {
                if data.get("file").is_none() || data.get("deleted").is_some() {
                    return Err(unsupported());
                }
                let actual = string(&data, "id")?;
                if !id.ends_with(&format!("#{actual}")) {
                    return Err(conflict());
                }
                (
                    string(&data, "eTag")?,
                    data["file"]["mimeType"]
                        .as_str()
                        .unwrap_or("application/octet-stream")
                        .to_owned(),
                    id,
                )
            }
        };
        let version = CloudWriteVersion {
            source_path: path.to_string(),
            provider: kind,
            object_id: identity,
            etag: etag(tag)?,
            mime_type: mime,
        };
        if let Some(previous) = previous {
            if version.source_path != previous.source_path
                || version.provider != previous.provider
                || version.object_id != previous.object_id
            {
                return Err(conflict());
            }
        }
        Ok((version, url, credentials))
    }
    pub(crate) fn cloud_write_version(
        &self,
        path: &CloudPath,
        previous: Option<&CloudWriteVersion>,
        cancel: Option<&AtomicBool>,
    ) -> CloudCommandResult<CloudWriteVersion> {
        self.version_request(path, previous, cancel)
            .map(|(version, _, _)| version)
    }
    /// OneDrive reads used to establish/recover a write baseline must use the
    /// same item ID as the conditional write, even if its name moved meanwhile.
    pub(crate) fn download_cloud_write_version(
        &self,
        version: &CloudWriteVersion,
        destination: &Path,
        cancel: Option<&AtomicBool>,
    ) -> CloudCommandResult<()> {
        self.download_cloud_write_version_with_progress(version, destination, cancel, |_, _| {})
    }

    pub(crate) fn download_cloud_write_version_with_progress(
        &self,
        version: &CloudWriteVersion,
        destination: &Path,
        cancel: Option<&AtomicBool>,
        mut progress: impl FnMut(u64, u64),
    ) -> CloudCommandResult<()> {
        let path = CloudPath::parse(&version.source_path)
            .map_err(crate::commands::cloud::map_cloud_path_error)?;
        if version.provider != CloudProviderKind::Onedrive {
            return self.download_file(&path, destination, cancel);
        }
        let (_, url, credentials) = self.version_request(&path, Some(version), cancel)?;
        cancelled(cancel)?;
        let first = credentials
            .apply(http()?.get(format!("{url}/content")))
            .send()
            .map_err(|_| {
                fail(
                    CloudCommandErrorCode::NetworkError,
                    "Cloud download failed; your local edits are kept",
                )
            })?;
        let mut download = if first.status() == StatusCode::FOUND
            || first.status() == StatusCode::TEMPORARY_REDIRECT
        {
            let location = first
                .headers()
                .get(reqwest::header::LOCATION)
                .and_then(|v| v.to_str().ok())
                .ok_or_else(unsupported)?;
            let url = url::Url::parse(location).map_err(|_| unsupported())?;
            if url.scheme() != "https"
                || !url.username().is_empty()
                || url.password().is_some()
                || url.fragment().is_some()
            {
                return Err(unsupported());
            }
            // Download URLs are preauthenticated. Never forward the OAuth token.
            response(http()?.get(url), cancel)?
        } else if first.status().is_success() {
            first
        } else {
            return Err(conflict());
        };
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(destination)
            .map_err(crate::commands::cloud::workspace::io_error)?;
        crate::commands::cloud::workspace::private_permissions(destination, false)?;
        let mut buffer = [0u8; 64 * 1024];
        let total = download.content_length().unwrap_or(0);
        let mut received = 0;
        let mut tick = std::time::Instant::now();
        loop {
            cancelled(cancel)?;
            let count = download.read(&mut buffer).map_err(|_| {
                fail(
                    CloudCommandErrorCode::NetworkError,
                    "Cloud download was interrupted; your local edits are kept",
                )
            })?;
            if count == 0 {
                break;
            }
            received += count as u64;
            if tick.elapsed() >= Duration::from_millis(200) || received == total {
                progress(received, total);
                tick = std::time::Instant::now();
            }
            std::io::Write::write_all(&mut file, &buffer[..count])
                .map_err(crate::commands::cloud::workspace::io_error)?;
        }
        file.sync_all()
            .map_err(crate::commands::cloud::workspace::io_error)
    }

    pub(crate) fn replace_cloud_file(
        &self,
        local: &Path,
        expected: &CloudWriteVersion,
        stop: Arc<AtomicBool>,
        progress: Arc<dyn Fn(u64, u64) + Send + Sync>,
    ) -> CloudCommandResult<CloudWriteVersion> {
        let path = CloudPath::parse(&expected.source_path)
            .map_err(crate::commands::cloud::map_cloud_path_error)?;
        let (current, url, credentials) =
            self.version_request(&path, Some(expected), Some(&stop))?;
        if current != *expected {
            return Err(conflict());
        }
        let file = crate::fs_utils::open_regular_file_nofollow(local)
            .map_err(crate::commands::cloud::workspace::io_error)?;
        let size = file
            .metadata()
            .map_err(crate::commands::cloud::workspace::io_error)?
            .len();
        if current.provider == CloudProviderKind::Onedrive && size > 250_000_000 {
            return Err(unsupported());
        }
        let url = match current.provider {
            CloudProviderKind::Gdrive => endpoint(
                &api_base("https://www.googleapis.com/upload/drive/v2/files"),
                &[&current.object_id],
            )?,
            CloudProviderKind::Onedrive => format!("{url}/content"),
            CloudProviderKind::Nextcloud => url,
        };
        let body = Body::sized(
            UploadReader {
                file,
                stop: stop.clone(),
                sent: 0,
                total: size,
                progress,
            },
            size,
        );
        let mut request = credentials
            .apply(http()?.put(url))
            .timeout(Duration::from_secs(300))
            .header(reqwest::header::IF_MATCH, &expected.etag)
            .header(reqwest::header::CONTENT_TYPE, &expected.mime_type)
            .body(body);
        if current.provider == CloudProviderKind::Gdrive {
            request = request.query(&[
                ("uploadType", "media"),
                ("supportsAllDrives", "true"),
                ("fields", "id,etag,mimeType"),
            ]);
        }
        let result = response(request, Some(&stop))?;
        let next = if current.provider == CloudProviderKind::Nextcloud {
            let next = result
                .headers()
                .get(reqwest::header::ETAG)
                .and_then(|v| v.to_str().ok())
                .map(str::to_owned);
            let mut version = current;
            // Do not infer a successful revision from a separate GET after PUT:
            // another writer may have already changed it. The workspace journal
            // verifies bytes when a provider omits its committed validator.
            version.etag = next.map(etag).transpose()?.unwrap_or_default();
            version
        } else {
            let value = json(result)?;
            let expected_id = if current.provider == CloudProviderKind::Onedrive {
                current.object_id.split('#').next_back().unwrap_or("")
            } else {
                &current.object_id
            };
            if string(&value, "id")? != expected_id {
                return Err(conflict());
            }
            let mut version = current;
            version.etag = etag(string(
                &value,
                if version.provider == CloudProviderKind::Gdrive {
                    "etag"
                } else {
                    "eTag"
                },
            )?)?;
            version
        };
        Ok(next)
    }
}
struct UploadReader {
    file: std::fs::File,
    stop: Arc<AtomicBool>,
    sent: u64,
    total: u64,
    progress: Arc<dyn Fn(u64, u64) + Send + Sync>,
}
impl Read for UploadReader {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        if self.stop.load(std::sync::atomic::Ordering::Relaxed) {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "Cloud saving cancelled",
            ));
        }
        let n = self.file.read(buf)?;
        self.sent += n as u64;
        (self.progress)(self.sent, self.total);
        Ok(n)
    }
}

#[cfg(all(test, unix))]
pub(super) mod tests;
