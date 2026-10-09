//! Persist the original destination separately from backup contents and failure
//! diagnostics. Older backups without this record require a chosen destination.
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::{Component, Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use super::{UndoError, UndoResult};

const MAX_RECORD_BYTES: u64 = 16 * 1024;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Origin {
    version: u8,
    backup_name: String,
    original_path: PathBuf,
}

fn record_path(backup: &Path) -> UndoResult<PathBuf> {
    let bucket = backup
        .parent()
        .ok_or_else(|| UndoError::invalid_input("Invalid backup bucket"))?;
    let mut name = bucket
        .file_name()
        .ok_or_else(|| UndoError::invalid_input("Invalid backup bucket name"))?
        .to_os_string();
    name.push(".origin.json");
    Ok(bucket.with_file_name(name))
}

fn valid_original(path: &Path) -> bool {
    path.is_absolute()
        && path.file_name().is_some()
        && !path
            .components()
            .any(|part| matches!(part, Component::ParentDir))
}

pub(super) fn record(backup: &Path, original: &Path) -> UndoResult<()> {
    let Some(name) = backup.file_name().and_then(|name| name.to_str()) else {
        return Ok(());
    };
    // Preserve existing support for non-UTF-8 and relative paths. Such backups
    // remain recoverable using an explicitly chosen local folder.
    if !valid_original(original) || original.to_str().is_none() {
        return Ok(());
    }
    let data = serde_json::to_vec(&Origin {
        version: 1,
        backup_name: name.to_owned(),
        original_path: original.to_path_buf(),
    })
    .map_err(|error| UndoError::invalid_input(format!("Encode backup origin: {error}")))?;
    if data.len() as u64 > MAX_RECORD_BYTES {
        return Ok(());
    }
    let path = record_path(backup)?;
    crate::path_guard::ensure_no_symlink_components_existing_prefix(&path)
        .map_err(|error| UndoError::invalid_input(format!("Unsafe backup origin: {error}")))?;
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600).custom_flags(libc::O_NOFOLLOW);
    }
    let mut file = options
        .open(&path)
        .map_err(|error| UndoError::from_io_error("Create backup origin", error))?;
    file.write_all(&data)
        .and_then(|()| file.sync_all())
        .map_err(|error| UndoError::from_io_error("Save backup origin", error))
}

pub(super) fn read(backup: &Path) -> UndoResult<PathBuf> {
    let path = record_path(backup)?;
    crate::fs_utils::check_no_symlink_components(&path)?;
    let file: File = crate::fs_utils::open_regular_file_nofollow(&path)
        .map_err(|error| UndoError::from_io_error("Read backup origin", error))?;
    let meta = file
        .metadata()
        .map_err(|error| UndoError::from_io_error("Inspect backup origin", error))?;
    if meta.len() > MAX_RECORD_BYTES {
        return Err(UndoError::invalid_input("Backup origin is too large"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if meta.uid() != unsafe { libc::geteuid() } || meta.mode() & 0o077 != 0 || meta.nlink() != 1
        {
            return Err(UndoError::invalid_input("Backup origin must be private"));
        }
    }
    let identity = crate::fs_utils::FileIdentity::from_file(&file)
        .ok_or_else(|| UndoError::invalid_input("Cannot verify backup origin"))?;
    let mut data = Vec::new();
    file.take(MAX_RECORD_BYTES + 1)
        .read_to_end(&mut data)
        .map_err(|error| UndoError::from_io_error("Read backup origin", error))?;
    if data.len() as u64 > MAX_RECORD_BYTES || !identity.matches(&path) {
        return Err(UndoError::invalid_input("Backup origin changed"));
    }
    let origin: Origin = serde_json::from_slice(&data)
        .map_err(|error| UndoError::invalid_input(format!("Invalid backup origin: {error}")))?;
    if origin.version != 1
        || backup.file_name().and_then(|name| name.to_str()) != Some(&origin.backup_name)
        || !valid_original(&origin.original_path)
    {
        return Err(UndoError::invalid_input("Invalid original backup location"));
    }
    Ok(origin.original_path)
}
