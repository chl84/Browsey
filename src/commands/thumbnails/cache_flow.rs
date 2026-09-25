use super::{control::Control, ThumbnailResult};
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{
    atomic::{AtomicBool, AtomicU32, Ordering},
    Arc, Mutex, Weak,
};
use std::time::{Duration, SystemTime};

static LOCKS: Lazy<Mutex<HashMap<String, Weak<Mutex<()>>>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));
static TRIM_COUNTER: AtomicU32 = AtomicU32::new(0);
static TRIMMING: AtomicBool = AtomicBool::new(false);

pub(super) fn with_key_lock<T>(
    key: &str,
    control: &Control,
    work: impl FnOnce() -> ThumbnailResult<T>,
) -> ThumbnailResult<T> {
    let lock = {
        let mut locks = LOCKS.lock().expect("thumbnail locks poisoned");
        locks.retain(|_, lock| lock.strong_count() > 0);
        if let Some(lock) = locks.get(key).and_then(Weak::upgrade) {
            lock
        } else {
            let lock = Arc::new(Mutex::new(()));
            locks.insert(key.to_owned(), Arc::downgrade(&lock));
            lock
        }
    };
    loop {
        control.check()?;
        if let Ok(_guard) = lock.try_lock() {
            return work();
        }
        std::thread::sleep(Duration::from_millis(10));
    }
}

pub(super) struct PendingFile(pub(super) PathBuf);
pub(super) fn pending_path(cache: &Path) -> PathBuf {
    static SEQUENCE: AtomicU32 = AtomicU32::new(0);
    cache.with_extension(format!(
        "pending-{}-{}.png",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ))
}
impl Drop for PendingFile {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

pub(super) fn touch_cache_entry(path: &Path) {
    // Cache mtime is last-use time, not source mtime. Throttle writes on hot hits.
    let needs_touch = fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.elapsed().ok())
        .is_some_and(|age| age >= Duration::from_secs(60));
    if needs_touch {
        if let Ok(file) = fs::OpenOptions::new().write(true).open(path) {
            let _ = file.set_times(fs::FileTimes::new().set_modified(SystemTime::now()));
        }
    }
}

pub(super) fn schedule_trim(dir: PathBuf, max_bytes: u64) {
    if !TRIM_COUNTER
        .fetch_add(1, Ordering::Relaxed)
        .is_multiple_of(100)
        || TRIMMING.swap(true, Ordering::AcqRel)
    {
        return;
    }
    tauri::async_runtime::spawn_blocking(move || {
        struct Reset;
        impl Drop for Reset {
            fn drop(&mut self) {
                TRIMMING.store(false, Ordering::Release);
            }
        }
        let _reset = Reset;
        trim_cache(&dir, max_bytes);
    });
}

pub(super) fn trim_cache(dir: &Path, max_bytes: u64) {
    let mut entries = Vec::new();
    if let Ok(read_dir) = fs::read_dir(dir) {
        for entry in read_dir.flatten() {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            // Ignore in-progress outputs, symlinks and unrelated files.
            if !name.ends_with(".png") || name.contains(".pending") || name.ends_with(".tmp.png") {
                continue;
            }
            if let Ok(md) = fs::symlink_metadata(entry.path()) {
                if !md.is_file() {
                    continue;
                }
                entries.push((
                    entry.path(),
                    md.len().max(4096),
                    md.modified().unwrap_or(SystemTime::UNIX_EPOCH),
                ));
            }
        }
    }
    let mut bytes: u64 = entries.iter().map(|e| e.1).sum();
    entries.sort_by_key(|e| e.2);
    for (path, size, last_used) in entries {
        if bytes <= max_bytes {
            break;
        }
        // A hit during enumeration protects the entry from this trim pass.
        if fs::metadata(&path).and_then(|m| m.modified()).ok() != Some(last_used) {
            continue;
        }
        if fs::remove_file(&path).is_ok() {
            bytes = bytes.saturating_sub(size);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> PathBuf {
        static SEQUENCE: AtomicU32 = AtomicU32::new(0);
        let path = std::env::temp_dir().join(format!(
            "browsey-thumb-cache-{}-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed),
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&path).unwrap();
        path
    }
    #[test]
    fn keeps_more_than_two_thousand_small_thumbnails() {
        let dir = fixture();
        for i in 0..2100 {
            fs::write(dir.join(format!("{i}.png")), b"fixture").unwrap();
        }
        trim_cache(&dir, 50 * 1024 * 1024);
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 2100);
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn recent_hits_survive_eviction_and_pending_files_are_untouched() {
        let dir = fixture();
        let hot = dir.join("hot.png");
        let cold = dir.join("cold.png");
        for path in [&hot, &cold] {
            fs::write(path, b"fixture").unwrap();
            fs::File::options()
                .write(true)
                .open(path)
                .unwrap()
                .set_times(fs::FileTimes::new().set_modified(SystemTime::UNIX_EPOCH))
                .unwrap();
        }
        fs::write(dir.join("active.pending.png"), b"fixture").unwrap();
        touch_cache_entry(&hot);
        trim_cache(&dir, 4096);
        assert!(hot.exists());
        assert!(!cold.exists());
        assert!(dir.join("active.pending.png").exists());
        fs::remove_dir_all(dir).unwrap();
    }
    #[test]
    fn cancelled_key_waiter_does_not_poison_following_requests() {
        let control = Control::new(Arc::new(AtomicBool::new(true)), Duration::from_secs(1));
        assert!(with_key_lock("test-key", &control, || Ok(())).is_err());
        let control = Control::new(Arc::new(AtomicBool::new(false)), Duration::from_secs(1));
        assert_eq!(with_key_lock("test-key", &control, || Ok(42)).unwrap(), 42);
    }
}
