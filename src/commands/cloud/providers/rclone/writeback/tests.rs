use super::*;
use crate::commands::cloud::set_cloud_provider_kind_override_for_tests;
use std::{
    collections::HashMap,
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
                if method == "GET" && path.contains("/content") {
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
