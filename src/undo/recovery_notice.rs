//! Diagnostic acknowledgement only. Never clears recovery markers or authorizes deletion.
//! The caller holds the session lock, serializing notices across Browsey instances.
use std::{
    fs::{self, File, OpenOptions},
    io::{self, Read, Write},
    path::{Path, PathBuf},
    time::UNIX_EPOCH,
};

use super::backup::RECOVERY_SUFFIX;

const HEADER: &[u8] = b"Browsey recovery notice v1\n";
const SIZE: usize = HEADER.len() + 32;

pub(super) enum RecoveryScan {
    Clean,
    Marked([u8; 32]),
    Uncertain,
}

pub(super) fn scan(directory: &Path) -> RecoveryScan {
    fn fingerprint(directory: &Path) -> Option<Vec<[u8; 32]>> {
        crate::path_guard::ensure_no_symlink_components_existing_prefix(directory).ok()?;
        let mut markers = Vec::new();
        for entry in fs::read_dir(directory).ok()? {
            let entry = entry.ok()?;
            let name = entry.file_name();
            if !name
                .as_encoded_bytes()
                .ends_with(RECOVERY_SUFFIX.as_bytes())
            {
                continue;
            }
            let metadata = fs::symlink_metadata(entry.path()).ok()?;
            if !metadata.is_file() {
                return None;
            }
            let mut hash = blake3::Hasher::new();
            hash.update(name.as_encoded_bytes());
            hash.update(&metadata.len().to_le_bytes());
            hash.update(
                &metadata
                    .modified()
                    .ok()?
                    .duration_since(UNIX_EPOCH)
                    .ok()?
                    .as_nanos()
                    .to_le_bytes(),
            );
            #[cfg(unix)]
            {
                use std::os::unix::fs::MetadataExt;
                for value in [metadata.dev(), metadata.ino(), metadata.mode() as u64] {
                    hash.update(&value.to_le_bytes());
                }
                hash.update(&metadata.ctime().to_le_bytes());
                hash.update(&metadata.ctime_nsec().to_le_bytes());
            }
            markers.push(*hash.finalize().as_bytes());
        }
        Some(markers)
    }
    match fingerprint(directory) {
        None => RecoveryScan::Uncertain,
        Some(markers) if markers.is_empty() => RecoveryScan::Clean,
        Some(mut markers) => {
            // Directory enumeration order must not create another warning.
            markers.sort_unstable();
            let mut hash = blake3::Hasher::new();
            for marker in markers {
                hash.update(&marker);
            }
            RecoveryScan::Marked(*hash.finalize().as_bytes())
        }
    }
}

fn notice_path(directory: &Path) -> PathBuf {
    let mut name = directory.file_name().unwrap_or_default().to_os_string();
    name.push(".recovery-notice");
    directory.with_file_name(name)
}

fn private_file(file: &File) -> io::Result<()> {
    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.len() > SIZE as u64 {
        return Err(io::Error::other("Invalid undo recovery notice"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if metadata.uid() != unsafe { libc::geteuid() }
            || metadata.mode() & 0o077 != 0
            || metadata.nlink() != 1
        {
            return Err(io::Error::other("Undo recovery notice must be private"));
        }
    }
    Ok(())
}

fn options() -> OpenOptions {
    #[cfg_attr(not(unix), allow(unused_mut))]
    let mut options = OpenOptions::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    options
}

pub(super) fn notify_changed(directory: &Path, fingerprint: &[u8; 32]) -> bool {
    let path = notice_path(directory);
    let read = || -> io::Result<bool> {
        let mut file = options().read(true).open(&path)?;
        private_file(&file)?;
        let mut saved = [0; SIZE];
        file.read_exact(&mut saved)?;
        Ok(saved[..HEADER.len()] == *HEADER && saved[HEADER.len()..] == *fingerprint)
    };
    if read().is_ok_and(|unchanged| unchanged) {
        return false;
    }
    let write = || -> io::Result<()> {
        let mut file = options()
            .write(true)
            .create(true)
            .truncate(false)
            .open(&path)?;
        private_file(&file)?;
        // A torn notice is retried and warned about; it never affects retention.
        file.set_len(0)?;
        file.write_all(HEADER)?;
        file.write_all(fingerprint)?;
        file.sync_data()
    };
    if let Err(error) = write() {
        tracing::debug!(%error, notice = %path.display(), "Cannot remember undo recovery warning; keep warning enabled");
    }
    true
}

pub(super) fn remove(directory: &Path) {
    let _ = fs::remove_file(notice_path(directory));
}
