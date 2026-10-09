//! Remember successful recovery without removing data used by undo/redo.
use std::{
    fs::{File, OpenOptions},
    io::{self, Read, Seek, SeekFrom, Write},
    path::{Path, PathBuf},
};

use crate::fs_utils::{FileIdentity, TreeSnapshot};

const HEADER: &[u8] = b"BROWSEY_RECOVERED_V1\n";

pub(super) fn fingerprint(snapshot: &TreeSnapshot) -> [u8; 32] {
    // TreeSnapshot's ordered names, identities and file versions are the same
    // observations verified by the copy engine. No backup file bytes are read.
    let mut hash = blake3::Hasher::new();
    write!(&mut hash, "{snapshot:?}").expect("Hasher writes are infallible");
    *hash.finalize().as_bytes()
}

fn state_path(source: &Path, version: &str) -> io::Result<PathBuf> {
    let bucket = source
        .parent()
        .ok_or_else(|| io::Error::other("Missing backup bucket"))?;
    let mut hash = blake3::Hasher::new();
    hash.update(
        source
            .file_name()
            .ok_or_else(|| io::Error::other("Missing backup name"))?
            .as_encoded_bytes(),
    );
    hash.update(version.as_bytes());
    let mut name = bucket
        .file_name()
        .ok_or_else(|| io::Error::other("Missing backup bucket name"))?
        .to_os_string();
    name.push(format!(".recovered-{}", hash.finalize().to_hex()));
    Ok(bucket.with_file_name(name))
}

fn load(source: &Path, version: &str, writable: bool) -> Option<(File, [u8; 32])> {
    let path = state_path(source, version).ok()?;
    crate::fs_utils::check_no_symlink_components(&path).ok()?;
    let mut options = OpenOptions::new();
    options.read(true).write(writable);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.custom_flags(windows_sys::Win32::Storage::FileSystem::FILE_FLAG_OPEN_REPARSE_POINT);
    }
    let mut file = options.open(&path).ok()?;
    let meta = file.metadata().ok()?;
    if !meta.is_file() || meta.len() != (HEADER.len() + 32) as u64 {
        return None;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if meta.uid() != unsafe { libc::geteuid() } || meta.mode() & 0o077 != 0 || meta.nlink() != 1
        {
            return None;
        }
    }
    let identity = FileIdentity::from_file(&file)?;
    let mut header = vec![0; HEADER.len()];
    let mut fingerprint = [0; 32];
    file.read_exact(&mut header).ok()?;
    file.read_exact(&mut fingerprint).ok()?;
    (header == HEADER && identity.matches(&path)).then_some((file, fingerprint))
}

pub(super) fn read(source: &Path, version: &str) -> Option<[u8; 32]> {
    load(source, version, false).map(|(_, fingerprint)| fingerprint)
}

pub(super) fn record(source: &Path, version: &str, fingerprint: &[u8; 32]) -> io::Result<()> {
    if read(source, version).as_ref() == Some(fingerprint) {
        return Ok(());
    }
    let path = state_path(source, version)?;
    crate::path_guard::ensure_no_symlink_components_existing_prefix(&path)
        .map_err(io::Error::other)?;
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
    }
    let mut file = match options.open(&path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => load(source, version, true)
            .map(|(file, _)| file)
            .ok_or(error)?,
        Err(error) => return Err(error),
    };
    file.seek(SeekFrom::Start(0))?;
    file.write_all(HEADER)?;
    file.write_all(fingerprint)?;
    file.sync_all()?;
    if read(source, version).as_ref() != Some(fingerprint) {
        return Err(io::Error::other("Cannot verify recovered backup status"));
    }
    Ok(())
}
