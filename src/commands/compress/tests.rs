use super::*;
use crate::errors::domain::DomainError;

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "browsey-compress-regression-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn compress(&self, input: &Path) -> CompressResult<String> {
        do_compress(
            None,
            CancelState::default(),
            UndoState::default(),
            vec![input.to_string_lossy().into_owned()],
            Some("result.zip".into()),
            Some(6),
            None,
        )
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn creates_readable_zip_with_empty_and_unicode_files() {
    let root = Fixture::new();
    let input = root.0.join("folder");
    fs::create_dir(&input).unwrap();
    fs::write(input.join("æ #.txt"), b"payload").unwrap();
    fs::write(input.join("empty"), b"").unwrap();
    let dest = root.compress(&input).unwrap();
    let mut zip = zip::ZipArchive::new(File::open(dest).unwrap()).unwrap();
    let mut content = String::new();
    zip.by_name("folder/æ #.txt")
        .unwrap()
        .read_to_string(&mut content)
        .unwrap();
    assert_eq!(content, "payload");
    assert_eq!(zip.by_name("folder/empty").unwrap().size(), 0);
}

#[test]
fn cancel_is_registered_before_collection_and_removes_output() {
    let root = Fixture::new();
    let file = root.0.join("input.txt");
    fs::write(&file, b"data").unwrap();
    let state = CancelState::default();
    let cancel = state.clone();
    BEFORE_COLLECT.with(|hook| {
        *hook.borrow_mut() = Some(Box::new(move || {
            assert!(cancel.cancel("scan-cancel-test").unwrap());
        }))
    });
    let err = do_compress(
        None,
        state,
        UndoState::default(),
        vec![file.to_string_lossy().into_owned()],
        Some("result.zip".into()),
        None,
        Some("scan-cancel-test".into()),
    )
    .unwrap_err();
    assert_eq!(err.code_str(), "cancelled");
    assert!(!root.0.join("result.zip").exists());
}

#[cfg(unix)]
#[test]
fn special_files_are_rejected_without_blocking_or_leaving_an_archive() {
    use std::os::unix::ffi::OsStrExt;
    let root = Fixture::new();
    let folder = root.0.join("folder");
    fs::create_dir(&folder).unwrap();
    let fifo = folder.join("pipe");
    let cpath = std::ffi::CString::new(fifo.as_os_str().as_bytes()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(cpath.as_ptr(), 0o600) }, 0);
    assert!(open_regular_input(&fifo).is_err());
    assert!(root
        .compress(&folder)
        .unwrap_err()
        .to_string()
        .contains("Unsupported file type"));
    assert!(!root.0.join("result.zip").exists());
}

#[cfg(unix)]
#[test]
fn unrepresentable_names_fail_without_silent_renaming_or_leftovers() {
    use std::os::unix::ffi::OsStringExt;
    for name in [
        std::ffi::OsString::from("a\\b.txt"),
        std::ffi::OsString::from_vec(vec![0xff, b'x']),
    ] {
        let root = Fixture::new();
        let folder = root.0.join("folder");
        fs::create_dir(&folder).unwrap();
        let file = folder.join(name);
        fs::write(&file, b"original").unwrap();
        assert!(root.compress(&folder).is_err());
        assert_eq!(fs::read(&file).unwrap(), b"original");
        assert!(!root.0.join("result.zip").exists());
    }
}

#[cfg(unix)]
#[test]
fn preserves_literal_backslashes_in_symlink_targets() {
    use std::os::unix::fs::symlink;
    let root = Fixture::new();
    let link = root.0.join("link");
    symlink("literal\\target", &link).unwrap();
    let dest = root.compress(&link).unwrap();
    let mut zip = zip::ZipArchive::new(File::open(dest).unwrap()).unwrap();
    let mut content = String::new();
    zip.by_name("link")
        .unwrap()
        .read_to_string(&mut content)
        .unwrap();
    assert_eq!(content, "literal\\target");
}

#[cfg(unix)]
#[test]
fn input_replaced_with_symlink_is_not_followed() {
    use std::os::unix::fs::symlink;
    let root = Fixture::new();
    let file = root.0.join("target");
    fs::write(&file, b"secret").unwrap();
    let link = root.0.join("replacement");
    symlink(file, &link).unwrap();
    assert!(open_regular_input(&link).is_err());
}
