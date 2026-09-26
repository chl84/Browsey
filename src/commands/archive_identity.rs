//! Stable identities for archive output ownership; size/mtime change as we write.
use std::path::Path;

#[derive(Clone, PartialEq, Eq)]
pub(super) struct ArchiveIdentity {
    #[cfg(unix)]
    snapshot: crate::undo::PathSnapshot,
    #[cfg(windows)]
    volume: u32,
    #[cfg(windows)]
    index: u64,
    #[cfg(windows)]
    directory: bool,
}

impl ArchiveIdentity {
    pub(super) fn capture(path: &Path) -> Option<Self> {
        #[cfg(unix)]
        {
            Some(Self {
                snapshot: crate::undo::snapshot_existing_path(path).ok()?,
            })
        }
        #[cfg(windows)]
        {
            use std::os::windows::{fs::OpenOptionsExt, io::AsRawHandle};
            use windows_sys::Win32::Storage::FileSystem::{
                GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION, FILE_ATTRIBUTE_DIRECTORY,
                FILE_ATTRIBUTE_REPARSE_POINT, FILE_FLAG_BACKUP_SEMANTICS,
                FILE_FLAG_OPEN_REPARSE_POINT, FILE_READ_ATTRIBUTES,
            };
            crate::fs_utils::check_no_symlink_components(path).ok()?;
            let file = std::fs::OpenOptions::new()
                .access_mode(FILE_READ_ATTRIBUTES)
                .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
                .open(path)
                .ok()?;
            let mut info = std::mem::MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::uninit();
            if unsafe { GetFileInformationByHandle(file.as_raw_handle(), info.as_mut_ptr()) } == 0 {
                return None;
            }
            let info = unsafe { info.assume_init() };
            if info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0 {
                return None;
            }
            if info.nFileIndexHigh == 0 && info.nFileIndexLow == 0 {
                // No usable identity: leave output behind rather than delete
                // an unrelated replacement on filesystems lacking file IDs.
                return None;
            }
            Some(Self {
                volume: info.dwVolumeSerialNumber,
                index: ((info.nFileIndexHigh as u64) << 32) | info.nFileIndexLow as u64,
                directory: info.dwFileAttributes & FILE_ATTRIBUTE_DIRECTORY != 0,
            })
        }
        #[cfg(not(any(unix, windows)))]
        {
            let _ = path;
            None
        }
    }

    pub(super) fn matches(&self, path: &Path) -> bool {
        Self::capture(path).as_ref() == Some(self)
    }
}
