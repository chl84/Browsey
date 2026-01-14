use chrono::{DateTime, Local};
use serde::Serialize;
use std::collections::HashMap;
use std::fs::{self, Metadata};
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime};

use crate::icons::icon_for;

#[cfg(target_os = "windows")]
use std::path::{Component, Prefix};
#[cfg(target_os = "windows")]
use std::iter;
#[cfg(target_os = "windows")]
use windows_sys::Win32::Storage::FileSystem::GetDriveTypeW;
#[cfg(target_os = "windows")]
const DRIVE_REMOTE: u32 = 4;

#[derive(Clone)]
pub struct CachedMeta {
    pub is_dir: bool,
    pub is_link: bool,
    pub size: Option<u64>,
    pub modified: Option<String>,
    pub icon: String,
    stored: SystemTime,
}

static META_CACHE: OnceLock<Mutex<HashMap<String, CachedMeta>>> = OnceLock::new();

fn meta_cache() -> &'static Mutex<HashMap<String, CachedMeta>> {
    META_CACHE.get_or_init(Default::default)
}

#[derive(Serialize, Clone)]
pub struct FsEntry {
    pub name: String,
    pub path: String,
    pub kind: String,
    pub ext: Option<String>,
    pub size: Option<u64>,
    pub items: Option<u64>,
    pub modified: Option<String>,
    pub icon: String,
    pub starred: bool,
}

#[derive(Serialize, Clone)]
pub struct EntryTimes {
    pub accessed: Option<String>,
    pub created: Option<String>,
    pub modified: Option<String>,
}

fn dir_item_count(path: &Path) -> Option<u64> {
    match fs::read_dir(path) {
        Ok(iter) => {
            let mut count: u64 = 0;
            for entry in iter {
                if entry.is_ok() {
                    count = count.saturating_add(1);
                }
            }
            Some(count)
        }
        Err(_) => None,
    }
}

fn fmt_time(value: Option<SystemTime>) -> Option<String> {
    value.and_then(|t| {
        DateTime::<Local>::from(t)
            .format("%Y-%m-%d %H:%M")
            .to_string()
            .into()
    })
}

#[cfg(target_os = "windows")]
static DRIVE_REMOTE_CACHE: OnceLock<Mutex<HashMap<u8, bool>>> = OnceLock::new();

#[cfg(target_os = "windows")]
pub fn is_network_location(path: &Path) -> bool {
    match path.components().next() {
        Some(Component::Prefix(prefix)) => match prefix.kind() {
            Prefix::UNC(..) | Prefix::VerbatimUNC(..) => true,
            Prefix::Disk(letter) | Prefix::VerbatimDisk(letter) => {
                if let Some(cache) = DRIVE_REMOTE_CACHE.get_or_init(Default::default).lock().ok() {
                    if let Some(&cached) = cache.get(&letter) {
                        return cached;
                    }
                }
                let drive = format!("{}:\\", letter as char);
                let wide: Vec<u16> = drive.encode_utf16().chain(iter::once(0)).collect();
                let drive_type = unsafe { GetDriveTypeW(wide.as_ptr()) };
                let is_remote = drive_type == DRIVE_REMOTE;
                if let Some(mut cache) = DRIVE_REMOTE_CACHE.get_or_init(Default::default).lock().ok() {
                    cache.insert(letter, is_remote);
                }
                is_remote
            }
            _ => false,
        },
        _ => false,
    }
}

#[cfg(not(target_os = "windows"))]
pub fn is_network_location(_path: &Path) -> bool {
    false
}

pub fn build_entry(path: &Path, meta: &Metadata, is_link: bool, starred: bool) -> FsEntry {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string());

    let kind = if is_link {
        "link"
    } else if meta.is_dir() {
        "dir"
    } else {
        "file"
    }
    .to_string();

    let size = if meta.is_file() {
        Some(meta.len())
    } else {
        None
    };
    let items = if meta.is_dir() {
        if is_network_location(path) {
            None
        } else {
            dir_item_count(path)
        }
    } else {
        None
    };
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_string());
    let modified = fmt_time(meta.modified().ok());

    FsEntry {
        name,
        path: path.to_string_lossy().into_owned(),
        kind,
        ext,
        size,
        items,
        modified,
        icon: icon_for(path, meta, is_link).to_string(),
        starred,
    }
}

pub fn entry_times(path: &Path) -> Result<EntryTimes, String> {
    let meta = fs::symlink_metadata(path).map_err(|e| format!("Failed to read metadata: {e}"))?;
    Ok(EntryTimes {
        accessed: fmt_time(meta.accessed().ok()),
        created: fmt_time(meta.created().ok()),
        modified: fmt_time(meta.modified().ok()),
    })
}

pub fn store_cached_meta(path: &Path, meta: &Metadata, is_link: bool) {
    let key = path.to_string_lossy().into_owned();
    let cached = CachedMeta {
        is_dir: meta.is_dir(),
        is_link,
        size: if meta.is_file() { Some(meta.len()) } else { None },
        modified: fmt_time(meta.modified().ok()),
        icon: icon_for(path, meta, is_link).to_string(),
        stored: SystemTime::now(),
    };
    if let Ok(mut map) = meta_cache().lock() {
        map.insert(key, cached);
    }
}

pub fn get_cached_meta(path: &Path, ttl: Duration) -> Option<CachedMeta> {
    let key = path.to_string_lossy().into_owned();
    let now = SystemTime::now();
    let map = meta_cache().lock().ok()?;
    let cached = map.get(&key)?;
    if let Ok(age) = now.duration_since(cached.stored) {
        if age <= ttl {
            return Some(cached.clone());
        }
    }
    None
}
