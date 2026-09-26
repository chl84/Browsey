use super::*;
use std::io::{self, Write};
use std::time::{SystemTime, UNIX_EPOCH};

struct Fixture(PathBuf);
impl Fixture {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "browsey-archive-regression-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn extract_password(path: &Path, password: Option<&str>) -> DecompressResult<ExtractResult> {
    do_extract_with_password(
        None,
        CancelState::default(),
        UndoState::default(),
        path.to_string_lossy().into_owned(),
        None,
        None,
        None,
        None,
        password,
    )
}

fn expect_password_error(path: &Path, password: Option<&str>, code: &str) {
    let error = extract_password(path, password).err().expect("must fail");
    assert_eq!(error.code_str(), code, "{error}");
    assert_eq!(
        fs::read_dir(path.parent().unwrap()).unwrap().count(),
        1,
        "failed extraction left outputs"
    );
}

#[test]
fn encrypted_zip_missing_wrong_correct_and_legacy_passwords() {
    use zip::{unstable::write::FileOptionsExt, write::SimpleFileOptions};
    for legacy in [false, true] {
        let root = Fixture::new();
        let path = root.0.join("protected.zip");
        let mut writer = zip::ZipWriter::new(File::create(&path).unwrap());
        // A plaintext entry first verifies that rollback removes partial output on a later password failure.
        writer
            .start_file("plain.txt", SimpleFileOptions::default())
            .unwrap();
        writer.write_all(b"public").unwrap();
        let opts = if legacy {
            SimpleFileOptions::default()
                .with_deprecated_encryption(b"pass")
                .unwrap()
        } else {
            SimpleFileOptions::default().with_aes_encryption(zip::AesMode::Aes256, "pass")
        };
        writer.start_file("secret.txt", opts).unwrap();
        writer.write_all(b"secret").unwrap();
        writer.finish().unwrap();
        expect_password_error(&path, None, "archive_password_required");
        expect_password_error(&path, Some("wrong"), "archive_invalid_password");
        let result = extract_password(&path, Some("pass")).unwrap();
        assert_eq!(
            fs::read(Path::new(&result.destination).join("secret.txt")).unwrap(),
            b"secret"
        );
        assert_eq!(
            fs::read(Path::new(&result.destination).join("plain.txt")).unwrap(),
            b"public"
        );
    }
}

#[test]
fn damaged_aes_zip_authentication_is_not_reported_as_success() {
    let root = Fixture::new();
    let path = root.0.join("damaged.zip");
    let mut writer = zip::ZipWriter::new(File::create(&path).unwrap());
    writer
        .start_file(
            "secret.txt",
            zip::write::SimpleFileOptions::default()
                .with_aes_encryption(zip::AesMode::Aes256, "pass"),
        )
        .unwrap();
    writer.write_all(b"secret").unwrap();
    writer.finish().unwrap();
    let mut archive = zip::ZipArchive::new(File::open(&path).unwrap()).unwrap();
    let entry = archive.by_index_raw(0).unwrap();
    let auth_end = (entry.data_start().unwrap() + entry.compressed_size() - 1) as usize;
    drop(entry);
    drop(archive);
    let mut bytes = fs::read(&path).unwrap();
    bytes[auth_end] ^= 1;
    fs::write(&path, bytes).unwrap();
    expect_password_error(&path, Some("pass"), "archive_password_or_corrupt");
}

#[test]
fn encrypted_sevenz_supports_visible_and_encrypted_headers() {
    use sevenz_rust2::{
        encoder_options::{AesEncoderOptions, Lzma2Options},
        ArchiveEntry, ArchiveWriter, Password,
    };
    for encrypt_header in [false, true] {
        let root = Fixture::new();
        let path = root.0.join("protected.7z");
        let password = " blåbær🔑 ";
        let mut writer = ArchiveWriter::new(File::create(&path).unwrap()).unwrap();
        writer.set_encrypt_header(encrypt_header);
        writer.set_content_methods(vec![
            AesEncoderOptions::new(Password::new(password)).into(),
            Lzma2Options::default().into(),
        ]);
        writer
            .push_archive_entry(
                ArchiveEntry::new_file("secret.txt"),
                Some(b"secret".as_slice()),
            )
            .unwrap();
        writer.finish().unwrap();
        expect_password_error(&path, None, "archive_password_required");
        expect_password_error(&path, Some("wrong"), "archive_password_or_corrupt");
        let result = extract_password(&path, Some(password)).unwrap();
        assert_eq!(
            fs::read(Path::new(&result.destination).join("secret.txt")).unwrap(),
            b"secret"
        );
    }
}

#[test]
fn encrypted_rar_supports_visible_and_encrypted_headers() {
    for (fixture, password) in [
        ("crypted.rar", "unrar"),
        ("comment-hpw-password.rar", "password"),
    ] {
        let root = Fixture::new();
        let path = root.0.join("protected.rar");
        fs::copy(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("tests/fixtures/rar")
                .join(fixture),
            &path,
        )
        .unwrap();
        expect_password_error(&path, None, "archive_password_required");
        let error = extract_password(&path, Some("wrong"))
            .err()
            .expect("wrong password must fail");
        assert!(
            matches!(
                error.code_str(),
                "archive_invalid_password" | "archive_password_or_corrupt"
            ),
            "{error}"
        );
        assert_eq!(fs::read_dir(&root.0).unwrap().count(), 1);
        let result = extract_password(&path, Some(password)).unwrap();
        assert_eq!(
            fs::read(Path::new(&result.destination).join(".gitignore")).unwrap(),
            b"target\nCargo.lock\n"
        );
    }
}

#[test]
fn tar_gzip_preflight_honors_cancellation_and_declared_size_budget() {
    use std::sync::atomic::AtomicBool;
    let root = Fixture::new();
    let archive = root.0.join("scan.tar.gz");
    let encoder =
        flate2::write::GzEncoder::new(File::create(&archive).unwrap(), flate2::Compression::fast());
    let mut tar = tar::Builder::new(encoder);
    let mut header = tar::Header::new_gnu();
    header.set_mode(0o600);
    header.set_size(4096);
    header.set_cksum();
    tar.append_data(&mut header, "root/file", io::repeat(0).take(4096))
        .unwrap();
    tar.into_inner().unwrap().finish().unwrap();
    let cancelled = AtomicBool::new(true);
    let err = single_root_in_tar(
        &archive,
        ArchiveKind::TarGz,
        ScanControl {
            cancel: Some(&cancelled),
            max_bytes: 4096,
            ..ScanControl::default()
        },
    )
    .unwrap_err();
    assert!(is_cancelled_error(&err));
    let err = single_root_in_tar(
        &archive,
        ArchiveKind::TarGz,
        ScanControl {
            cancel: None,
            max_bytes: 32,
            ..ScanControl::default()
        },
    )
    .unwrap_err();
    assert!(err.to_string().contains("size cap"));
}

#[test]
fn sevenz_empty_file_is_not_stripped_as_a_root_directory() {
    // Own fixture generated with bsdtar --format=7zip: one zero-byte empty.txt.
    const EMPTY_7Z: &[u8] = &[
        55, 122, 188, 175, 39, 28, 0, 3, 100, 235, 166, 77, 0, 0, 0, 0, 0, 0, 0, 0, 78, 0, 0, 0, 0,
        0, 0, 0, 250, 14, 29, 145, 1, 5, 1, 14, 1, 128, 15, 1, 128, 17, 21, 0, 101, 0, 109, 0, 112,
        0, 116, 0, 121, 0, 46, 0, 116, 0, 120, 0, 116, 0, 0, 0, 20, 10, 1, 0, 173, 58, 138, 103,
        240, 77, 221, 1, 18, 10, 1, 0, 173, 58, 138, 103, 240, 77, 221, 1, 19, 10, 1, 0, 173, 58,
        138, 103, 240, 77, 221, 1, 21, 6, 1, 0, 32, 128, 164, 129, 0, 0,
    ];
    let root = Fixture::new();
    let archive = root.0.join("empty.7z");
    fs::write(&archive, EMPTY_7Z).unwrap();
    assert_eq!(
        single_root_in_7z(&archive, ScanControl::default()).unwrap(),
        None
    );
    let result = do_extract_impl(
        None,
        CancelState::default(),
        UndoState::default(),
        archive.to_string_lossy().into_owned(),
        None,
        None,
        None,
        None,
    )
    .unwrap();
    let file = PathBuf::from(result.destination).join("empty.txt");
    assert!(file.is_file());
    assert_eq!(fs::metadata(file).unwrap().len(), 0);
}

#[cfg(unix)]
#[test]
fn zip_restores_private_executable_and_directory_modes_without_special_bits() {
    use std::os::unix::fs::PermissionsExt;
    let root = Fixture::new();
    let archive = root.0.join("modes.zip");
    let mut zip = zip::ZipWriter::new(File::create(&archive).unwrap());
    zip.add_directory(
        "root/",
        zip::write::SimpleFileOptions::default().unix_permissions(0o700),
    )
    .unwrap();
    for (name, mode) in [("private", 0o600), ("run", 0o755), ("unsafe", 0o6755)] {
        zip.start_file(
            format!("root/{name}"),
            zip::write::SimpleFileOptions::default().unix_permissions(mode),
        )
        .unwrap();
        zip.write_all(b"payload").unwrap();
    }
    zip.finish().unwrap();
    let result = do_extract_impl(
        None,
        CancelState::default(),
        UndoState::default(),
        archive.to_string_lossy().into_owned(),
        None,
        None,
        None,
        None,
    )
    .unwrap();
    let dest = PathBuf::from(result.destination);
    let mode = |path: &Path| fs::metadata(path).unwrap().permissions().mode() & 0o7777;
    assert_eq!(mode(&dest), 0o700);
    assert_eq!(mode(&dest.join("private")), 0o600);
    assert_eq!(mode(&dest.join("run")), 0o755);
    assert_eq!(mode(&dest.join("unsafe")), 0o755);
}

#[cfg(unix)]
#[test]
fn tar_restores_modes_and_defers_read_only_directory_until_children_are_written() {
    use std::os::unix::fs::PermissionsExt;
    let root = Fixture::new();
    let archive = root.0.join("modes.tar");
    let mut tar = tar::Builder::new(File::create(&archive).unwrap());
    let mut dir = tar::Header::new_gnu();
    dir.set_entry_type(tar::EntryType::Directory);
    dir.set_mode(0o500);
    dir.set_size(0);
    dir.set_cksum();
    tar.append_data(&mut dir, "root/", io::empty()).unwrap();
    let mut file = tar::Header::new_gnu();
    file.set_mode(0o600);
    file.set_size(3);
    file.set_cksum();
    tar.append_data(&mut file, "root/private", &b"abc"[..])
        .unwrap();
    let mut executable = tar::Header::new_gnu();
    executable.set_mode(0o6755);
    executable.set_size(3);
    executable.set_cksum();
    tar.append_data(&mut executable, "root/run", &b"run"[..])
        .unwrap();
    tar.finish().unwrap();
    drop(tar);
    let result = do_extract_impl(
        None,
        CancelState::default(),
        UndoState::default(),
        archive.to_string_lossy().into_owned(),
        None,
        None,
        None,
        None,
    )
    .unwrap();
    let dest = PathBuf::from(result.destination);
    assert_eq!(fs::read(dest.join("private")).unwrap(), b"abc");
    assert_eq!(
        fs::metadata(&dest).unwrap().permissions().mode() & 0o777,
        0o500
    );
    assert_eq!(
        fs::metadata(dest.join("private"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    assert_eq!(
        fs::metadata(dest.join("run")).unwrap().permissions().mode() & 0o7777,
        0o755
    );
    fs::set_permissions(dest, fs::Permissions::from_mode(0o700)).unwrap();
}
