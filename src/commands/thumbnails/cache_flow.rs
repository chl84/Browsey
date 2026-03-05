use super::{ThumbnailResponse, ThumbnailResult};
use once_cell::sync::Lazy;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use tokio::sync::oneshot;

type InflightWaiters = Vec<oneshot::Sender<ThumbnailResult<ThumbnailResponse>>>;
type InflightMap = HashMap<String, InflightWaiters>;

static INFLIGHT: Lazy<std::sync::Mutex<InflightMap>> =
    Lazy::new(|| std::sync::Mutex::new(HashMap::new()));
static TRIM_COUNTER: Lazy<std::sync::Mutex<u32>> = Lazy::new(|| std::sync::Mutex::new(0));

pub(super) fn register_or_wait(
    key: &str,
) -> Option<oneshot::Receiver<ThumbnailResult<ThumbnailResponse>>> {
    let mut map = INFLIGHT.lock().expect("inflight poisoned");
    if let Some(waiters) = map.get_mut(key) {
        let (tx, rx) = oneshot::channel::<ThumbnailResult<ThumbnailResponse>>();
        waiters.push(tx);
        return Some(rx);
    }
    map.insert(key.to_string(), Vec::new());
    None
}

pub(super) fn notify_waiters(key: &str, result: ThumbnailResult<ThumbnailResponse>) {
    let waiters = {
        let mut map = INFLIGHT.lock().expect("inflight poisoned");
        map.remove(key)
    };
    if let Some(waiters) = waiters {
        for tx in waiters {
            let _ = tx.send(result.clone());
        }
    }
}

pub(super) fn bump_trim_counter_should_trim() -> bool {
    let mut counter = TRIM_COUNTER.lock().expect("trim counter poisoned");
    *counter = counter.wrapping_add(1);
    (*counter).is_multiple_of(100)
}

pub(super) fn trim_cache(dir: &Path, max_bytes: u64, max_files: usize) {
    let mut entries: Vec<(PathBuf, u64, std::time::SystemTime)> = Vec::new();
    if let Ok(read_dir) = fs::read_dir(dir) {
        for entry in read_dir.flatten() {
            if let Ok(md) = entry.metadata() {
                let modified = md.modified().unwrap_or(std::time::UNIX_EPOCH);
                entries.push((entry.path(), md.len(), modified));
            }
        }
    }

    let total_bytes: u64 = entries.iter().map(|e| e.1).sum();
    let total_files = entries.len();
    if total_bytes <= max_bytes && total_files <= max_files {
        return;
    }

    entries.sort_by_key(|e| e.2);
    let mut bytes = total_bytes;
    let mut files = total_files;
    for (path, size, _) in entries {
        if bytes <= max_bytes && files <= max_files {
            break;
        }
        if fs::remove_file(&path).is_ok() {
            bytes = bytes.saturating_sub(size);
            files -= 1;
        }
    }
}
