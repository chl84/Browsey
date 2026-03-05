use super::{
    CloudCommandError, CloudCommandErrorCode, CloudCommandResult, CloudMaterializeSnapshot,
};
use blake3::Hasher;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::Mutex;
use std::time::Duration;
use tracing::debug;

#[cfg(not(test))]
const MATERIALIZE_WAIT_TIMEOUT: Duration = Duration::from_secs(300);
#[cfg(test)]
const MATERIALIZE_WAIT_TIMEOUT: Duration = Duration::from_millis(50);

type MaterializeWaiters = Vec<mpsc::Sender<CloudCommandResult<PathBuf>>>;
static MATERIALIZE_INFLIGHT: once_cell::sync::Lazy<Mutex<HashMap<String, MaterializeWaiters>>> =
    once_cell::sync::Lazy::new(|| Mutex::new(HashMap::new()));

pub(super) fn materialize_with_inflight_dedupe<F>(
    path: &crate::commands::cloud::path::CloudPath,
    snapshot: &CloudMaterializeSnapshot,
    do_materialize: F,
) -> CloudCommandResult<PathBuf>
where
    F: FnOnce() -> CloudCommandResult<PathBuf>,
{
    let key = materialize_inflight_key(path, snapshot.size, snapshot.modified.as_deref());
    if let Some(rx) = register_materialize_waiter(&key) {
        return wait_for_materialize_result(path, &key, rx);
    }
    let result = do_materialize();
    notify_materialize_waiters(&key, result.clone());
    result
}

fn materialize_inflight_key(
    path: &crate::commands::cloud::path::CloudPath,
    size: Option<u64>,
    modified: Option<&str>,
) -> String {
    let mut hasher = Hasher::new();
    hasher.update(path.to_string().as_bytes());
    match size {
        Some(value) => {
            hasher.update(&[1]);
            hasher.update(&value.to_le_bytes());
        }
        None => {
            hasher.update(&[0]);
        }
    }
    match modified {
        Some(value) => {
            hasher.update(&[1]);
            hasher.update(value.as_bytes());
        }
        None => {
            hasher.update(&[0]);
        }
    }
    hasher.finalize().to_hex().to_string()
}

pub(super) fn register_materialize_waiter(
    key: &str,
) -> Option<mpsc::Receiver<CloudCommandResult<PathBuf>>> {
    let mut inflight = MATERIALIZE_INFLIGHT
        .lock()
        .expect("materialize inflight mutex poisoned");
    if let Some(waiters) = inflight.get_mut(key) {
        let (tx, rx) = mpsc::channel();
        waiters.push(tx);
        return Some(rx);
    }
    inflight.insert(key.to_string(), Vec::new());
    None
}

pub(super) fn wait_for_materialize_result(
    path: &crate::commands::cloud::path::CloudPath,
    key: &str,
    rx: mpsc::Receiver<CloudCommandResult<PathBuf>>,
) -> CloudCommandResult<PathBuf> {
    match rx.recv_timeout(MATERIALIZE_WAIT_TIMEOUT) {
        Ok(result) => result,
        Err(mpsc::RecvTimeoutError::Timeout) => {
            debug!(
                op = "cloud_materialize_wait",
                key,
                path = %path,
                timeout_ms = MATERIALIZE_WAIT_TIMEOUT.as_millis() as u64,
                "cloud materialization waiter timed out"
            );
            Err(CloudCommandError::new(
                CloudCommandErrorCode::Timeout,
                "Cloud materialization wait timed out",
            ))
        }
        Err(mpsc::RecvTimeoutError::Disconnected) => Err(CloudCommandError::new(
            CloudCommandErrorCode::TaskFailed,
            "Cloud file materialization task was cancelled",
        )),
    }
}

pub(super) fn notify_materialize_waiters(key: &str, result: CloudCommandResult<PathBuf>) {
    let waiters = {
        let mut inflight = MATERIALIZE_INFLIGHT
            .lock()
            .expect("materialize inflight mutex poisoned");
        inflight.remove(key)
    };
    if let Some(waiters) = waiters {
        for tx in waiters {
            let _ = tx.send(result.clone());
        }
    }
}
