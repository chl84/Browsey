use super::*;
use crate::commands::cloud::set_cloud_provider_kind_override_for_tests;
use std::{
    collections::{HashMap, VecDeque},
    fs,
    io::Write,
    net::TcpListener,
    path::PathBuf,
    sync::{atomic::Ordering, Mutex},
    thread,
};

#[derive(Default)]
pub(crate) struct ServerState {
    pub race: bool,
    pub lose_response: bool,
    pub native: bool,
    pub puts: Vec<(String, String)>,
    download_responses: VecDeque<(u16, Option<String>)>,
    download_requests: Vec<(String, bool)>,
    revisions: HashMap<String, u64>,
}
pub(crate) struct Fixture {
    pub base: PathBuf,
    pub provider: RcloneCloudProvider,
    pub source: CloudPath,
    pub state: Arc<Mutex<ServerState>>,
    stop: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}
impl Fixture {
    pub fn new(kind: CloudProviderKind) -> Self {
        use std::os::unix::fs::PermissionsExt;
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let base = std::env::temp_dir().join(format!(
            "browsey-writeback-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&base).unwrap();
        fs::write(base.join("fileA.bytes"), b"original").unwrap();
        fs::write(base.join("fileB.bytes"), b"neighbor").unwrap();
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let token=serde_json::json!({"access_token":"owned-fixture-token","expiry":"2099-01-01T00:00:00Z"}).to_string();
        fs::write(base.join("config.json"),serde_json::json!({"Generated":{"type":match kind { CloudProviderKind::Gdrive=>"drive",CloudProviderKind::Onedrive=>"onedrive",CloudProviderKind::Nextcloud=>"webdav" },"token":token,"drive_id":"drive","vendor":"nextcloud","url":endpoint,"bearer_token":"fixture-dav-token"}}).to_string()).unwrap();
        let binary = base.join("rclone");
        fs::write(&binary,r#"#!/usr/bin/env python3
import sys,pathlib,json,shutil
args=sys.argv[1:];base=pathlib.Path(__file__).parent
if 'version' in args:print('rclone v1.75.1');sys.exit()
if 'listremotes' in args:print('Generated:');sys.exit()
if 'config' in args and 'dump' in args:print((base/'config.json').read_text());sys.exit()
if 'lsjson' in args:print(json.dumps({'ID':'drive#fileA','Name':'same.txt','Path':'same.txt','Size':8,'IsDir':False}));sys.exit()
if 'backend' in args:
 i=args.index('backend')
 if args[i+1]=='copyid':shutil.copyfile(base/(args[i+3]+'.bytes'),args[i+4]);sys.exit()
if 'copyto' in args:
 i=args.index('copyto');shutil.copyfile(base/'fileA.bytes',args[i+2]);sys.exit()
sys.exit(3)
"#).unwrap();
        fs::set_permissions(&binary, fs::Permissions::from_mode(0o700)).unwrap();
        let provider = RcloneCloudProvider::new(RcloneCli::new(binary));
        set_cloud_provider_kind_override_for_tests("Generated", kind);
        API_BASE.with(|v| *v.borrow_mut() = Some(endpoint));
        let source = CloudPath::parse(if kind == CloudProviderKind::Gdrive {
            "rclone://Generated//gdrive/parent~folder/fileA~same.txt"
        } else {
            "rclone://Generated/same.txt"
        })
        .unwrap();
        let state = Arc::new(Mutex::new(ServerState::default()));
        let stop = Arc::new(AtomicBool::new(false));
        let (server_state, server_stop, server_base) = (state.clone(), stop.clone(), base.clone());
        let thread = thread::spawn(move || {
            while !server_stop.load(Ordering::Relaxed) {
                let Ok((mut socket, _)) = listener.accept() else {
                    thread::sleep(Duration::from_millis(5));
                    continue;
                };
                socket
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut bytes = Vec::new();
                let mut buffer = [0u8; 4096];
                let header_end = loop {
                    let n = socket.read(&mut buffer).unwrap();
                    if n == 0 {
                        break None;
                    }
                    bytes.extend_from_slice(&buffer[..n]);
                    if let Some(end) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
                        break Some(end + 4);
                    }
                };
                let Some(end) = header_end else {
                    continue;
                };
                let headers = String::from_utf8(bytes[..end].to_vec()).unwrap();
                let mut first = headers.lines().next().unwrap().split_whitespace();
                let method = first.next().unwrap();
                let path = first.next().unwrap();
                if kind == CloudProviderKind::Gdrive {
                    let expected = if method == "PUT" {
                        "uploadType=media&supportsAllDrives=true&fields=id%2Cetag%2CmimeType"
                    } else {
                        "supportsAllDrives=true&fields=id%2Cetag%2CmimeType%2Clabels%2Ccapabilities"
                    };
                    assert_eq!(path.split_once('?').map(|(_, query)| query), Some(expected));
                }
                let length = headers
                    .lines()
                    .filter_map(|line| line.split_once(':'))
                    .find(|(k, _)| k.eq_ignore_ascii_case("content-length"))
                    .and_then(|(_, v)| v.trim().parse::<usize>().ok())
                    .unwrap_or(0);
                while bytes.len() < end + length {
                    let n = socket.read(&mut buffer).unwrap();
                    if n == 0 {
                        break;
                    }
                    bytes.extend_from_slice(&buffer[..n]);
                }
                let id = if path.contains("fileB") {
                    "fileB"
                } else {
                    "fileA"
                };
                let content = server_base.join(format!("{id}.bytes"));
                let mut state = server_state.lock().unwrap();
                let downloading =
                    method == "GET" && (path.contains("/content") || path.starts_with("/download"));
                if downloading {
                    let authenticated = headers.lines().any(|line| {
                        line.split_once(':')
                            .is_some_and(|(name, _)| name.eq_ignore_ascii_case("authorization"))
                    });
                    state.download_requests.push((path.into(), authenticated));
                    if let Some((status, location)) = state.download_responses.pop_front() {
                        let location = location
                            .map(|value| format!("Location: {value}\r\n"))
                            .unwrap_or_default();
                        let response = format!(
                            "HTTP/1.1 {status} Fixture\r\n{location}Content-Length: 0\r\nConnection: close\r\n\r\n"
                        );
                        let _ = socket.write_all(response.as_bytes());
                        continue;
                    }
                }
                let mut revision = *state.revisions.entry(id.into()).or_insert(1);
                let mut status = "200 OK";
                let mut body = Vec::new();
                let mut tag = format!("\"{id}:{revision}\"");
                if method == "PUT" {
                    let validator = headers
                        .lines()
                        .filter_map(|line| line.split_once(':'))
                        .find(|(k, _)| k.eq_ignore_ascii_case("if-match"))
                        .map(|(_, v)| v.trim())
                        .unwrap_or("");
                    state.puts.push((path.into(), validator.into()));
                    if state.race {
                        state.race = false;
                        revision += 1;
                        tag = format!("\"{id}:{revision}\"");
                        fs::write(&content, b"other writer").unwrap();
                    }
                    if validator != tag {
                        status = "412 Precondition Failed";
                    } else {
                        fs::write(&content, &bytes[end..end + length]).unwrap();
                        revision += 1;
                        tag = format!("\"{id}:{revision}\"");
                    }
                    state.revisions.insert(id.into(), revision);
                    if status == "200 OK" && state.lose_response {
                        state.lose_response = false;
                        continue;
                    }
                }
                if downloading {
                    body = fs::read(&content).unwrap();
                } else if method != "HEAD" && status == "200 OK" {
                    body=serde_json::json!({"id":id,"etag":tag,"eTag":tag,"mimeType":if state.native {"application/vnd.google-apps.document"}else{"text/plain"},"labels":{"trashed":false},"capabilities":{"canModifyContent":true},"file":{"mimeType":"text/plain"}}).to_string().into_bytes();
                }
                let response=format!("HTTP/1.1 {status}\r\nContent-Length: {}\r\nContent-Type: application/json\r\nETag: {tag}\r\nOC-FileId: {id}\r\nConnection: close\r\n\r\n",body.len());
                let _ = socket.write_all(response.as_bytes());
                let _ = socket.write_all(&body);
            }
        });
        Self {
            base,
            provider,
            source,
            state,
            stop,
            thread: Some(thread),
        }
    }
    pub fn version(&self) -> CloudWriteVersion {
        self.provider
            .cloud_write_version(&self.source, None, None)
            .unwrap()
    }
    pub fn edit(&self) -> PathBuf {
        let file = self.base.join("edit");
        fs::write(&file, b"local edited").unwrap();
        file
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(thread) = self.thread.take() {
            thread.join().unwrap();
        }
        API_BASE.with(|v| *v.borrow_mut() = None);
        fs::remove_dir_all(&self.base).unwrap();
    }
}
fn save(f: &Fixture, version: &CloudWriteVersion) -> CloudCommandResult<CloudWriteVersion> {
    f.provider.replace_cloud_file(
        &f.edit(),
        version,
        Arc::new(AtomicBool::new(false)),
        Arc::new(|_, _| {}),
    )
}
#[test]
fn identical_names_update_only_the_selected_drive_id() {
    let f = Fixture::new(CloudProviderKind::Gdrive);
    let version = f.version();
    let saved = save(&f, &version).unwrap();
    assert_eq!(saved.object_id, "fileA");
    assert_ne!(saved.etag, version.etag);
    assert_eq!(
        fs::read(f.base.join("fileA.bytes")).unwrap(),
        b"local edited"
    );
    assert_eq!(fs::read(f.base.join("fileB.bytes")).unwrap(), b"neighbor");
    assert!(f.state.lock().unwrap().puts[0].0.starts_with("/fileA?"));
}
#[test]
fn conditional_put_closes_the_metadata_to_upload_race_on_all_providers() {
    for kind in [
        CloudProviderKind::Gdrive,
        CloudProviderKind::Onedrive,
        CloudProviderKind::Nextcloud,
    ] {
        let f = Fixture::new(kind);
        let version = f.version();
        f.state.lock().unwrap().race = true;
        assert_eq!(
            save(&f, &version).unwrap_err().code(),
            CloudCommandErrorCode::Conflict
        );
        assert_eq!(
            fs::read(f.base.join("fileA.bytes")).unwrap(),
            b"other writer"
        );
        assert_eq!(f.state.lock().unwrap().puts[0].1, version.etag);
    }
}
#[test]
fn stale_baseline_never_sends_a_write() {
    let f = Fixture::new(CloudProviderKind::Gdrive);
    let version = f.version();
    f.state.lock().unwrap().revisions.insert("fileA".into(), 2);
    assert_eq!(
        save(&f, &version).unwrap_err().code(),
        CloudCommandErrorCode::Conflict
    );
    assert!(f.state.lock().unwrap().puts.is_empty());
}
#[test]
fn one_drive_baseline_download_is_scoped_to_item_id() {
    let f = Fixture::new(CloudProviderKind::Onedrive);
    let version = f.version();
    let destination = f.base.join("download");
    let mut progress = Vec::new();
    f.provider
        .download_cloud_write_version_with_progress(&version, &destination, None, |bytes, total| {
            progress.push((bytes, total))
        })
        .unwrap();
    assert_eq!(progress.last(), Some(&(8, 8)));
    assert_eq!(fs::read(destination).unwrap(), b"original");
    let saved = save(&f, &version).unwrap();
    assert_eq!(saved.object_id, "drive#fileA");
    assert!(f.state.lock().unwrap().puts[0]
        .0
        .ends_with("/items/fileA/content"));
}

fn first_download_response() -> Response {
    let base = API_BASE.with(|value| value.borrow().clone().unwrap());
    send_download_request(
        Credentials::Bearer(Zeroizing::new("owned-fixture-token".into())).apply(
            http()
                .unwrap()
                .get(format!("{base}/drives/drive/items/fileA/content")),
        ),
        None,
    )
    .unwrap()
}

fn fetch_fixture_download(request: RequestBuilder) -> CloudCommandResult<Response> {
    // Logical HTTPS destinations are served by the owned HTTP fixture. The
    // production callback uses the validated URL directly and adds no token.
    let base = API_BASE.with(|value| value.borrow().clone().unwrap());
    let mut request = request.build().unwrap();
    assert!(!request
        .headers()
        .contains_key(reqwest::header::AUTHORIZATION));
    let mut local = url::Url::parse(&base).unwrap();
    local.set_path(request.url().path());
    local.set_query(request.url().query());
    *request.url_mut() = local;
    send_download_request(RequestBuilder::from_parts(http()?.clone(), request), None)
}

#[test]
fn onedrive_download_follows_redirect_chains_without_forwarding_credentials() {
    for statuses in [vec![302, 302], vec![301, 302, 303, 307, 308]] {
        let f = Fixture::new(CloudProviderKind::Onedrive);
        f.state.lock().unwrap().download_responses = statuses
            .iter()
            .enumerate()
            .map(|(index, status)| {
                (
                    *status,
                    Some(format!(
                        "https://cdn{index}.example.test/download?secret=owned-signed-link"
                    )),
                )
            })
            .collect();
        let mut download =
            follow_download_redirects(first_download_response(), None, fetch_fixture_download)
                .unwrap();
        let mut bytes = Vec::new();
        download.read_to_end(&mut bytes).unwrap();
        assert_eq!(bytes, b"original");
        let state = f.state.lock().unwrap();
        assert_eq!(state.download_requests.len(), statuses.len() + 1);
        assert!(state.download_requests[0].1);
        assert!(state.download_requests[1..]
            .iter()
            .all(|(_, authenticated)| !authenticated));
        assert!(state.puts.is_empty());
    }
}

#[test]
fn onedrive_download_stops_redirect_loops_without_exposing_signed_urls() {
    let f = Fixture::new(CloudProviderKind::Onedrive);
    f.state.lock().unwrap().download_responses = (0..=MAX_DOWNLOAD_REDIRECTS)
        .map(|_| {
            (
                302,
                Some("https://cdn.example.test/download?secret=owned-signed-link".into()),
            )
        })
        .collect();
    let error = follow_download_redirects(first_download_response(), None, fetch_fixture_download)
        .unwrap_err();
    assert_eq!(error.code(), CloudCommandErrorCode::Unsupported);
    assert!(error.message().contains("too many redirects"));
    assert!(!error.message().contains("owned-signed-link"));
    assert_eq!(
        f.state.lock().unwrap().download_requests.len(),
        MAX_DOWNLOAD_REDIRECTS + 1
    );
}

#[test]
fn onedrive_download_rejects_unsafe_or_missing_redirects_at_every_hop() {
    for location in [
        None,
        Some("http://cdn.example.test/download?secret=owned-signed-link"),
    ] {
        let f = Fixture::new(CloudProviderKind::Onedrive);
        f.state.lock().unwrap().download_responses = [
            (302, Some("https://cdn.example.test/download".into())),
            (302, location.map(str::to_owned)),
        ]
        .into();
        let error =
            follow_download_redirects(first_download_response(), None, fetch_fixture_download)
                .unwrap_err();
        assert_eq!(error.code(), CloudCommandErrorCode::Unsupported);
        assert!(!error.message().contains("owned-signed-link"));
        assert_eq!(f.state.lock().unwrap().download_requests.len(), 2);
    }
    let base = url::Url::parse("https://cdn.example.test/path/download").unwrap();
    assert_eq!(
        download_redirect_url(&base, "../file?key=fixture")
            .unwrap()
            .as_str(),
        "https://cdn.example.test/file?key=fixture"
    );
    for location in [
        "",
        " ",
        "https://[",
        "http://cdn.example.test",
        "https://name:secret@cdn.example.test",
        "https://name@cdn.example.test",
        "https://cdn.example.test/#fragment",
        "file:///tmp/document",
    ] {
        assert!(
            download_redirect_url(&base, location).is_none(),
            "{location}"
        );
    }
}

#[test]
fn onedrive_download_checks_cancellation_between_redirects() {
    let f = Fixture::new(CloudProviderKind::Onedrive);
    f.state.lock().unwrap().download_responses = [
        (302, Some("https://cdn.example.test/download".into())),
        (302, Some("https://cdn.example.test/download-again".into())),
    ]
    .into();
    let cancel = AtomicBool::new(false);
    let error = follow_download_redirects(first_download_response(), Some(&cancel), |url| {
        let result = fetch_fixture_download(url);
        cancel.store(true, Ordering::Relaxed);
        result
    })
    .unwrap_err();
    assert_eq!(error.code(), CloudCommandErrorCode::Cancelled);
    assert_eq!(f.state.lock().unwrap().download_requests.len(), 2);
}

#[test]
fn onedrive_download_preserves_http_failure_codes() {
    for (status, expected) in [
        (401, CloudCommandErrorCode::AuthRequired),
        (403, CloudCommandErrorCode::PermissionDenied),
        (404, CloudCommandErrorCode::Conflict),
        (429, CloudCommandErrorCode::RateLimited),
        (500, CloudCommandErrorCode::TaskFailed),
    ] {
        let f = Fixture::new(CloudProviderKind::Onedrive);
        f.state
            .lock()
            .unwrap()
            .download_responses
            .push_back((status, None));
        let error = follow_download_redirects(first_download_response(), None, |_| {
            panic!("failed responses must not be retried")
        })
        .unwrap_err();
        assert_eq!(error.code(), expected);
        assert_eq!(f.state.lock().unwrap().download_requests.len(), 1);
    }
}

#[test]
fn download_request_errors_do_not_expose_urls_or_credentials() {
    let error = send_download_request(
        http()
            .unwrap()
            .get("https://secret:token@[invalid?key=owned-signed-link"),
        None,
    )
    .unwrap_err();
    assert_eq!(error.code(), CloudCommandErrorCode::NetworkError);
    for secret in ["secret", "token", "owned-signed-link"] {
        assert!(!error.message().contains(secret));
    }
    let cancelled = AtomicBool::new(true);
    assert_eq!(
        send_download_request(
            http().unwrap().get("https://example.test"),
            Some(&cancelled)
        )
        .unwrap_err()
        .code(),
        CloudCommandErrorCode::Cancelled
    );
}
#[test]
fn native_google_documents_and_weak_validators_are_not_overwritten() {
    let f = Fixture::new(CloudProviderKind::Gdrive);
    f.state.lock().unwrap().native = true;
    assert_eq!(
        f.provider
            .cloud_write_version(&f.source, None, None)
            .unwrap_err()
            .code(),
        CloudCommandErrorCode::Unsupported
    );
    for value in ["*", "W/\"1\"", "\"1\",\"2\"", "\"line\nfeed\""] {
        assert!(etag(value.into()).is_err());
    }
    assert!(etag("\"id,revision\"".into()).is_ok());
    assert!(f.state.lock().unwrap().puts.is_empty());
}
#[test]
fn redirects_and_invalid_passwords_fail_without_exposing_credentials() {
    assert!(endpoint("https://name:secret@example.test", &[]).is_err());
    assert!(endpoint("http://example.test", &[]).is_err());
    assert!(reveal("not a password")
        .unwrap_err()
        .message()
        .contains("Your local copy is kept"));
    // Known rclone obscure vector generated from a disposable fixture password.
    assert_eq!(
        reveal("EScHacBHKzYHxPx3Qm-3sI-MAQUqUCzD-vWrcN0-Sg8Plmcxx2E")
            .unwrap()
            .as_str(),
        "owned-fixture-password"
    );
}
