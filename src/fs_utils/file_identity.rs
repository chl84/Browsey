//! Stable output ownership shared by archive extraction and local copies.
//! Size and modification time are excluded: they change as we write.
use std::fs::File;
use std::path::Path;

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct FileIdentity {
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
    #[cfg(windows)]
    volume: u32,
    #[cfg(windows)]
    index: u64,
    #[cfg(windows)]
    directory: bool,
}

impl FileIdentity {
    pub(crate) fn from_file(file: &File) -> Option<Self> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let meta = file.metadata().ok()?;
            if meta.file_type().is_symlink() {
                return None;
            }
            Some(Self {
                device: meta.dev(),
                inode: meta.ino(),
            })
        }
        #[cfg(windows)]
        {
            use std::os::windows::io::AsRawHandle;
            use windows_sys::Win32::Storage::FileSystem::{
                GetFileInformationByHandle, BY_HANDLE_FILE_INFORMATION, FILE_ATTRIBUTE_DIRECTORY,
                FILE_ATTRIBUTE_REPARSE_POINT,
            };
            let mut info = std::mem::MaybeUninit::<BY_HANDLE_FILE_INFORMATION>::uninit();
            if unsafe { GetFileInformationByHandle(file.as_raw_handle(), info.as_mut_ptr()) } == 0 {
                return None;
            }
            let info = unsafe { info.assume_init() };
            if info.dwFileAttributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
                || (info.nFileIndexHigh == 0 && info.nFileIndexLow == 0)
            {
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
            let _ = file;
            None
        }
    }

    pub(crate) fn capture(path: &Path) -> Option<Self> {
        super::check_no_symlink_components(path).ok()?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let meta = std::fs::symlink_metadata(path).ok()?;
            if meta.file_type().is_symlink() {
                return None;
            }
            Some(Self {
                device: meta.dev(),
                inode: meta.ino(),
            })
        }
        #[cfg(windows)]
        {
            use std::os::windows::fs::OpenOptionsExt;
            use windows_sys::Win32::Storage::FileSystem::{
                FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT, FILE_READ_ATTRIBUTES,
            };
            let file = File::options()
                .access_mode(FILE_READ_ATTRIBUTES)
                .custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT)
                .open(path)
                .ok()?;
            Self::from_file(&file)
        }
        #[cfg(not(any(unix, windows)))]
        {
            let _ = path;
            None
        }
    }

    pub(crate) fn matches(&self, path: &Path) -> bool {
        Self::capture(path).as_ref() == Some(self)
    }
}
