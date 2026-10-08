use super::*;
use crate::commands::cloud::{
    clear_cloud_provider_kind_overrides_for_tests, set_cloud_provider_kind_override_for_tests,
};
use std::{
    fs,
    io::{Read, Write},
    net::TcpListener,
    path::PathBuf,
    sync::atomic::{AtomicBool, Ordering},
    thread,
};

struct Fixture {
    base: PathBuf,
    provider: RcloneCloudProvider,
    rows: Arc<Mutex<HashMap<String, Value>>>,
    requests: Arc<Mutex<Vec<(String, String)>>>,
    request_urls: Arc<Mutex<Vec<String>>>,
    stop: Arc<AtomicBool>,
    thread: Option<thread::JoinHandle<()>>,
}
impl Fixture {
    fn new() -> Self {
        use std::os::unix::fs::PermissionsExt;
        static SEQUENCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let base = std::env::temp_dir().join(format!(
            "browsey-drive-ids-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&base).unwrap();
        let binary = base.join("rclone");
        fs::write(&binary, r#"#!/usr/bin/env python3
import sys,json,pathlib,urllib.request
args=sys.argv[1:]
if 'version' in args: print('rclone v1.75.1');sys.exit()
if 'listremotes' in args: print('Generated:');sys.exit()
if 'config' in args and 'dump' in args: print(json.dumps({'Generated':{'type':'drive'}}));sys.exit()
if 'backend' in args:
 i=args.index('backend');command=args[i+1];remote,ident,destination=args[i+2:i+5]
 if command=='copyid':
  values={'fileA':b'A'*24,'fileB':b'B'*47}
  if ident not in values: sys.stderr.write('File not found');sys.exit(3)
  pathlib.Path(destination).write_bytes(values[ident]);sys.exit()
 if command=='moveid':
  endpoint=pathlib.Path(__file__).with_name('endpoint').read_text()
  name=destination.rsplit(':',1)[-1].rsplit('/',1)[-1]
  request=urllib.request.Request(endpoint+'/'+ident,data=json.dumps({'name':name}).encode(),method='PATCH',headers={'Content-Type':'application/json'})
  with urllib.request.urlopen(request) as response: response.read()
  if '--error-on-no-transfer' in args:sys.exit(9)
  sys.exit()
if 'lsjson' in args:
 spec=args[args.index('lsjson')+1]
 if '--stat' in args:
  print(json.dumps({'Name':'destination.txt','Size':11,'IsDir':False}));sys.exit()
 rows=[{'ID':'fileA','Name':'same.txt','Path':'same.txt','Size':24,'IsDir':False},{'ID':'fileB','Name':'same.txt','Path':'same.txt','Size':47,'IsDir':False}]
 if 'root_folder_id=folderA' not in spec: rows=[]
 print(json.dumps(rows));sys.exit()
sys.stderr.write('Unsupported fixture command');sys.exit(3)
"#).unwrap();
        fs::set_permissions(&binary, fs::Permissions::from_mode(0o700)).unwrap();
        let provider = RcloneCloudProvider::new(RcloneCli::new(binary));
        set_cloud_provider_kind_override_for_tests("Generated", CloudProviderKind::Gdrive);
        let rows = Arc::new(Mutex::new(HashMap::from([
            (
                "fileA".to_owned(),
                json!({"id":"fileA","name":"same.txt","mimeType":"text/plain","size":"24","parents":["folderA"],"trashed":false}),
            ),
            (
                "fileB".to_owned(),
                json!({"id":"fileB","name":"same.txt","mimeType":"text/plain","size":"47","parents":["folderA"],"trashed":false}),
            ),
            (
                "folderA".to_owned(),
                json!({"id":"folderA","name":"dupes","mimeType":"application/vnd.google-apps.folder","parents":["rootID"],"trashed":false}),
            ),
            (
                "folderB".to_owned(),
                json!({"id":"folderB","name":"dupes","mimeType":"application/vnd.google-apps.folder","parents":["rootID"],"trashed":false}),
            ),
            (
                "root".to_owned(),
                json!({"id":"rootID","name":"My Drive","mimeType":"application/vnd.google-apps.folder","trashed":false}),
            ),
            (
                "linkID".to_owned(),
                json!({"id":"linkID","name":"shortcut.txt","mimeType":"application/vnd.google-apps.shortcut","shortcutDetails":{"targetId":"fileB"},"parents":["folderA"],"trashed":false}),
            ),
        ])));
        let requests = Arc::new(Mutex::new(Vec::new()));
        let request_urls = Arc::new(Mutex::new(Vec::new()));
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        listener.set_nonblocking(true).unwrap();
        let address = listener.local_addr().unwrap();
        fs::write(
            base.join("endpoint"),
            format!("http://{address}/drive/v3/files"),
        )
        .unwrap();
        let stop = Arc::new(AtomicBool::new(false));
        let (server_rows, server_requests, server_stop) =
            (rows.clone(), requests.clone(), stop.clone());
        let server_urls = request_urls.clone();
        let thread = thread::spawn(move || {
            while !server_stop.load(Ordering::Relaxed) {
                let (mut stream, _) = match listener.accept() {
                    Ok(v) => v,
                    Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(5));
                        continue;
                    }
                    Err(e) => panic!("{e}"),
                };
                stream
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut bytes = Vec::new();
                let mut buffer = [0; 4096];
                let split = loop {
                    let n = stream.read(&mut buffer).unwrap();
                    assert!(n > 0);
                    bytes.extend_from_slice(&buffer[..n]);
                    if let Some(i) = bytes.windows(4).position(|v| v == b"\r\n\r\n") {
                        break i + 4;
                    }
                };
                let headers = String::from_utf8_lossy(&bytes[..split]);
                let mut start = headers.lines().next().unwrap().split_whitespace();
                let method = start.next().unwrap().to_owned();
                let url = start.next().unwrap().to_owned();
                server_urls.lock().unwrap().push(url.clone());
                let length: usize = headers
                    .lines()
                    .find_map(|s| {
                        s.to_lowercase()
                            .strip_prefix("content-length:")
                            .map(|v| v.trim().parse().unwrap())
                    })
                    .unwrap_or(0);
                while bytes.len() < split + length {
                    let n = stream.read(&mut buffer).unwrap();
                    assert!(n > 0);
                    bytes.extend_from_slice(&buffer[..n]);
                }
                let id = url
                    .split('?')
                    .next()
                    .unwrap()
                    .rsplit('/')
                    .next()
                    .unwrap()
                    .to_owned();
                server_requests
                    .lock()
                    .unwrap()
                    .push((method.clone(), id.clone()));
                let mut rows = server_rows.lock().unwrap();
                let (status, body) = if method == "DELETE" {
                    rows.remove(&id);
                    (204, String::new())
                } else if let Some(row) = rows.get_mut(&id) {
                    if method == "PATCH" {
                        let patch: Value =
                            serde_json::from_slice(&bytes[split..split + length]).unwrap();
                        for (key, value) in patch.as_object().unwrap() {
                            row[key] = value.clone();
                        }
                    }
                    (200, row.to_string())
                } else {
                    (404, "{}".to_owned())
                };
                write!(stream, "HTTP/1.1 {status} Result\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len()).unwrap();
            }
        });
        API_BASE.with(|v| *v.borrow_mut() = Some(format!("http://{address}/drive/v3/files")));
        AUTH.get_or_init(Default::default).lock().unwrap().insert(
            (provider.cli.binary().to_os_string(), "Generated".to_owned()),
            Arc::new(Auth {
                token: Zeroizing::new("fixture-token".into()),
                expires: chrono::Utc::now().timestamp() + 3600,
                root_id: "root".into(),
                default_encoding: true,
            }),
        );
        Self {
            base,
            provider,
            rows,
            requests,
            request_urls,
            stop,
            thread: Some(thread),
        }
    }
    fn folder(&self, id: &str) -> CloudPath {
        CloudPath::parse("rclone://Generated/dupes")
            .unwrap()
            .with_drive_id(id)
            .unwrap()
    }
    fn file(&self, id: &str) -> CloudPath {
        self.folder("folderA")
            .child_path("same.txt")
            .unwrap()
            .with_drive_id(id)
            .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        self.thread.take().unwrap().join().unwrap();
        API_BASE.with(|v| *v.borrow_mut() = None);
        clear_cloud_provider_kind_overrides_for_tests();
        AUTH.get().unwrap().lock().unwrap().remove(&(
            self.provider.cli.binary().to_os_string(),
            "Generated".to_owned(),
        ));
        fs::remove_dir_all(&self.base).unwrap();
    }
}

#[test]
fn query_parameters_preserve_unicode_symbols_empty_values_and_repeated_keys() {
    let f = Fixture::new();
    f.provider
        .drive_request(
            &f.file("fileA"),
            "fileA",
            Method::GET,
            None,
            &[
                ("q", "navn blå +&=%/?#".into()),
                ("empty", String::new()),
                ("q", "andre".into()),
            ],
            None,
        )
        .unwrap();
    // Assert the wire request rather than decoding it with the same URL library.
    assert_eq!(
        f.request_urls.lock().unwrap().as_slice(),
        [concat!(
            "/drive/v3/files/fileA?supportsAllDrives=true&fields=",
            "id%2Cname%2CmimeType%2Csize%2CmodifiedTime%2Cparents%2Ctrashed%2CshortcutDetails",
            "&q=navn+bl%C3%A5+%2B%26%3D%25%2F%3F%23&empty=&q=andre",
        )]
    );
}

#[test]
fn duplicate_listing_and_download_preserve_each_selected_object() {
    let f = Fixture::new();
    let list = f
        .provider
        .list_dir_with_read_options(
            &f.folder("folderA"),
            RcloneReadOptions {
                backend: RcloneReadBackend::CliOnly,
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(list.len(), 2);
    assert_ne!(list[0].path, list[1].path);
    for (id, size, byte) in [("fileA", 24, b'A'), ("fileB", 47, b'B')] {
        let path = f.file(id);
        assert_eq!(
            f.provider.stat_path(&path).unwrap().unwrap().size,
            Some(size)
        );
        let out = f.base.join(id);
        f.provider.download_file(&path, &out, None).unwrap();
        assert_eq!(fs::read(out).unwrap(), vec![byte; size as usize]);
    }
    assert!(f.rows.lock().unwrap().contains_key("fileA"));
    assert!(f.rows.lock().unwrap().contains_key("fileB"));
}
#[test]
fn renaming_and_trashing_one_duplicate_never_changes_its_neighbor() {
    let f = Fixture::new();
    let source = f.file("fileA");
    let target = f.folder("folderA").child_path("renamed.txt").unwrap();
    f.provider
        .move_entry(&source, &target, false, false, None)
        .unwrap();
    assert_eq!(f.rows.lock().unwrap()["fileA"]["name"], "renamed.txt");
    assert_eq!(f.rows.lock().unwrap()["fileB"]["name"], "same.txt");
    f.provider.trash_entry(&source, None).unwrap();
    let rows = f.rows.lock().unwrap();
    assert_eq!(rows["fileA"]["trashed"], true);
    assert_eq!(rows["fileB"]["trashed"], false);
}
#[test]
fn duplicate_folder_deletion_and_shortcut_trash_address_only_selected_ids() {
    let f = Fixture::new();
    f.provider
        .delete_dir_recursive(&f.folder("folderA"), None)
        .unwrap();
    assert!(!f.rows.lock().unwrap().contains_key("folderA"));
    assert!(f.rows.lock().unwrap().contains_key("folderB"));
    let link = CloudPath::parse("rclone://Generated/shortcut.txt")
        .unwrap()
        .with_drive_id("fileB\tlinkID")
        .unwrap();
    f.provider.trash_entry(&link, None).unwrap();
    assert_eq!(f.rows.lock().unwrap()["linkID"]["trashed"], true);
    assert_eq!(f.rows.lock().unwrap()["fileB"]["trashed"], false);
    assert!(!f
        .requests
        .lock()
        .unwrap()
        .iter()
        .any(|(method, id)| method != "GET" && id == "fileB"));
}
#[test]
fn disappeared_id_and_cancelled_delete_never_fall_back_to_name() {
    let f = Fixture::new();
    f.rows.lock().unwrap().remove("fileA");
    assert!(f.provider.stat_path(&f.file("fileA")).unwrap().is_none());
    assert!(f.provider.delete_file(&f.file("fileA"), None).is_err());
    let cancel = AtomicBool::new(true);
    assert!(f
        .provider
        .delete_file(&f.file("fileB"), Some(&cancel))
        .is_err());
    assert!(f.rows.lock().unwrap().contains_key("fileB"));
    assert!(!f
        .requests
        .lock()
        .unwrap()
        .iter()
        .any(|(method, _)| method != "GET"));
}
#[test]
fn legacy_ambiguous_name_is_refused_and_tree_bulk_copy_is_refused_before_write() {
    let f = Fixture::new();
    let legacy = f.folder("folderA").child_path("same.txt").unwrap();
    assert!(f.provider.stat_path(&legacy).is_err());
    assert!(f
        .provider
        .ensure_drive_tree_unambiguous(&f.folder("folderA"), None)
        .is_err());
    assert!(!f
        .requests
        .lock()
        .unwrap()
        .iter()
        .any(|(method, _)| method != "GET"));
}

#[test]
fn ambiguous_existing_file_overwrite_is_refused_even_with_selected_id() {
    let f = Fixture::new();
    assert!(f
        .provider
        .ensure_drive_file_destination_unambiguous(&f.file("fileA"), None)
        .is_err());
    assert!(f
        .requests
        .lock()
        .unwrap()
        .iter()
        .all(|(method, _)| method == "GET"));
}

#[test]
fn identical_folder_names_navigate_and_rename_by_selected_id() {
    let f = Fixture::new();
    assert_eq!(f.provider.list_dir(&f.folder("folderA")).unwrap().len(), 2);
    assert!(f
        .provider
        .list_dir(&f.folder("folderB"))
        .unwrap()
        .is_empty());
    let destination = CloudPath::parse("rclone://Generated/renamed-folder").unwrap();
    f.provider
        .move_entry(&f.folder("folderB"), &destination, false, false, None)
        .unwrap();
    let rows = f.rows.lock().unwrap();
    assert_eq!(rows["folderB"]["name"], "renamed-folder");
    assert_eq!(rows["folderA"]["name"], "dupes");
    assert_eq!(rows["fileA"]["parents"], json!(["folderA"]));
}

#[test]
fn advanced_rename_accepts_two_same_name_sources_with_distinct_ids() {
    let f = Fixture::new();
    let result = crate::commands::cloud::batch_rename::rename_batch(
        &f.provider,
        vec![
            crate::commands::rename::RenameEntryRequest {
                path: f.file("fileA").to_string(),
                new_name: "A.txt".into(),
            },
            crate::commands::rename::RenameEntryRequest {
                path: f.file("fileB").to_string(),
                new_name: "B.txt".into(),
            },
        ],
    )
    .unwrap();
    assert!(result.error.is_none(), "{:?}", result.error);
    assert_eq!(result.renamed.len(), 2);
    let rows = f.rows.lock().unwrap();
    assert_eq!(rows["fileA"]["name"], "A.txt");
    assert_eq!(rows["fileB"]["name"], "B.txt");
}

#[test]
fn rclone_only_authentication_keeps_selected_id_reads_and_refuses_direct_mutations() {
    let f = Fixture::new();
    AUTH.get().unwrap().lock().unwrap().remove(&(
        f.provider.cli.binary().to_os_string(),
        "Generated".to_owned(),
    ));
    let selected = f.file("fileB");
    let entry = f.provider.stat_path(&selected).unwrap().unwrap();
    assert_eq!(entry.size, Some(47));
    assert_eq!(entry.path, selected.to_string());
    let out = f.base.join("service-account-download");
    f.provider.download_file(&selected, &out, None).unwrap();
    assert_eq!(fs::read(out).unwrap(), vec![b'B'; 47]);
    let err = f.provider.delete_file(&selected, None).unwrap_err();
    assert_eq!(err.code(), CloudCommandErrorCode::Unsupported);
    assert!(f.requests.lock().unwrap().is_empty());
    assert_eq!(f.rows.lock().unwrap()["fileB"]["trashed"], false);
}

#[test]
fn drive_references_cannot_scope_writes_on_another_provider() {
    let f = Fixture::new();
    set_cloud_provider_kind_override_for_tests("Generated", CloudProviderKind::Onedrive);
    let dir = f.folder("folderA");
    let new_dir = dir.child_path("new-folder").unwrap();
    for result in [
        f.provider.ensure_drive_tree_unambiguous(&dir, None),
        f.provider
            .ensure_drive_file_destination_unambiguous(&f.file("fileA"), None),
        f.provider.mkdir(&new_dir, None),
    ] {
        assert_eq!(
            result.unwrap_err().code(),
            CloudCommandErrorCode::InvalidPath
        );
    }
    assert!(f.requests.lock().unwrap().is_empty());
}

#[test]
fn stale_selected_destination_never_creates_a_replacement_by_name() {
    let f = Fixture::new();
    let stale = f
        .folder("folderA")
        .child_path("old-name.txt")
        .unwrap()
        .with_drive_id("fileB")
        .unwrap();
    let err = f
        .provider
        .ensure_drive_file_destination_unambiguous(&stale, None)
        .unwrap_err();
    assert_eq!(err.code(), CloudCommandErrorCode::NotFound);
    let missing = f.file("gone");
    let err = f
        .provider
        .copy_entry(&f.file("fileA"), &missing, true, false, None)
        .unwrap_err();
    assert_eq!(err.code(), CloudCommandErrorCode::NotFound);
    assert!(f
        .requests
        .lock()
        .unwrap()
        .iter()
        .all(|(method, _)| method == "GET"));
}

#[test]
fn duplicate_source_overwrite_is_refused_before_creating_another_destination_object() {
    let f = Fixture::new();
    set_cloud_provider_kind_override_for_tests("Other", CloudProviderKind::Onedrive);
    let destination = CloudPath::parse("rclone://Other/destination.txt").unwrap();
    let err = f
        .provider
        .copy_entry(&f.file("fileA"), &destination, true, false, None)
        .unwrap_err();
    assert_eq!(err.code(), CloudCommandErrorCode::Unsupported);
    assert!(f
        .requests
        .lock()
        .unwrap()
        .iter()
        .all(|(method, _)| method == "GET"));
    assert_eq!(f.rows.lock().unwrap()["fileA"]["name"], "same.txt");
}

#[test]
fn folder_names_decode_rclone_standard_escaping_without_decoding_quoted_symbols() {
    assert_eq!(decode_standard_name("Bilder æøå／2026"), "Bilder æøå/2026");
    assert_eq!(decode_standard_name("‛／ ‛‛ ‛␡ ‛␁"), "／ ‛ ␡ ␁");
    assert_eq!(decode_standard_name("␁␡"), "\u{1}\u{7f}");
    assert_eq!(decode_standard_name("‛．"), "．");
    assert_eq!(default_drive_name("‛／ ‛‛ ‛␀"), "／ ‛‛ ‛␀");
    assert_eq!(default_drive_name("␀"), "␀");
}
