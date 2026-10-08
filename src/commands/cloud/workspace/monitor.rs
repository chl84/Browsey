//! One lazy watcher, scoped to editor copy directories. Idle sessions perform
//! no hashing or cloud polling. Atomic editor replaces trigger the same debounce.
use super::*;
use crate::{commands::cloud::providers::rclone::RcloneCloudProvider, runtime_lifecycle};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::{
    collections::{HashMap, HashSet},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, SyncSender},
        Arc,
    },
    time::{Duration, Instant},
};
use tauri::Manager;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CloudWritebackStatus {
    pub id: String,
    pub name: String,
    pub source_path: String,
    pub status: CloudSaveStatus,
    pub message: Option<String>,
    pub bytes: u64,
    pub total: u64,
    pub sequence: u64,
}
pub(super) fn status(copy: &CloudWorkingCopy, bytes: u64, total: u64) -> CloudWritebackStatus {
    static SEQUENCE: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
    CloudWritebackStatus {
        id: copy.id.clone(),
        name: Path::new(&copy.local_path)
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
        source_path: copy.source_path.clone(),
        status: copy.save_status,
        message: copy.save_message.clone(),
        bytes,
        total,
        sequence: SEQUENCE.fetch_add(1, Ordering::Relaxed),
    }
}
enum Message {
    Register(Box<CloudWorkingCopy>),
    Changed(Vec<PathBuf>),
    Wake,
    Stop,
}
struct Running {
    sender: SyncSender<Message>,
    stop: Arc<AtomicBool>,
}
#[derive(Default)]
struct CloudWritebackState {
    running: Mutex<Option<Running>>,
    statuses: Arc<Mutex<HashMap<String, CloudWritebackStatus>>>,
    removed: Mutex<HashSet<String>>,
}
fn is_removed(app: &tauri::AppHandle, id: &str) -> bool {
    app.try_state::<CloudWritebackState>().is_some_and(|state| {
        state
            .removed
            .lock()
            .map_or(true, |removed| removed.contains(id))
    })
}
pub(super) fn forget(app: &tauri::AppHandle, id: &str) {
    if let Some(state) = app.try_state::<CloudWritebackState>() {
        // Same lock order as status publication, so queued progress cannot
        // restore a deleted copy's badge or watcher registration.
        if let Ok(mut removed) = state.removed.lock() {
            removed.insert(id.to_owned());
            if let Ok(mut statuses) = state.statuses.lock() {
                statuses.remove(id);
            }
        }
        if let Ok(running) = state.running.lock() {
            if let Some(running) = running.as_ref() {
                let _ = running.sender.try_send(Message::Wake);
            }
        }
    }
    runtime_lifecycle::emit_if_running(app, "cloud-working-copy-removed", id.to_owned());
}
pub(super) fn callback(app: &tauri::AppHandle) -> StatusCallback {
    let app = app.clone();
    Arc::new(move |copy, bytes, total| {
        let value = status(copy, bytes, total);
        if let Some(state) = app.try_state::<CloudWritebackState>() {
            let Ok(removed) = state.removed.lock() else {
                return;
            };
            if removed.contains(&copy.id) {
                return;
            }
            if let Ok(mut statuses) = state.statuses.lock() {
                statuses.insert(copy.id.clone(), value.clone());
            }
        }
        runtime_lifecycle::emit_if_running(&app, "cloud-writeback", value);
    })
}
#[tauri::command]
pub fn cloud_writeback_statuses(
    app: tauri::AppHandle,
) -> crate::errors::api_error::ApiResult<Vec<CloudWritebackStatus>> {
    let Some(state) = app.try_state::<CloudWritebackState>() else {
        return Ok(Vec::new());
    };
    map_api_result(
        state
            .statuses
            .lock()
            .map(|s| s.values().cloned().collect())
            .map_err(|_| {
                CloudCommandError::new(
                    CloudCommandErrorCode::TaskFailed,
                    "Cloud saving status is unavailable",
                )
            }),
    )
}
pub(crate) fn start_cloud_writeback(app: &tauri::AppHandle) {
    app.manage(CloudWritebackState::default());
    // Only inspect manifest metadata at startup. Old copies stay manual and
    // large local copies are hashed once, only when their own save is due.
    let Ok(base) = root() else {
        return;
    };
    if !base.is_dir() {
        return;
    }
    let app = app.clone();
    std::thread::spawn(move || {
        let Ok(entries) = fs::read_dir(&base) else {
            return;
        };
        for entry in entries.flatten() {
            if runtime_lifecycle::is_shutting_down(&app) {
                return;
            }
            let Some(id) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            if let Ok(mut copy) = load_manifest_at(&base, &id, false) {
                if copy.auto_save {
                    if schedule(&copy) {
                        copy.save_status = CloudSaveStatus::Pending;
                    }
                    if let Err(error) = register(&app, &copy) {
                        let mut copy = copy;
                        copy.save_status = CloudSaveStatus::Error;
                        copy.save_message = Some(error.message().into());
                        let _ = sync::persist(&base, &copy);
                        callback(&app)(&copy, 0, 0);
                    }
                }
            }
        }
    });
}
pub(crate) fn stop_cloud_writeback(app: &tauri::AppHandle) {
    if let Some(state) = app.try_state::<CloudWritebackState>() {
        if let Ok(running) = state.running.lock() {
            if let Some(running) = running.as_ref() {
                running.stop.store(true, Ordering::SeqCst);
                let _ = running.sender.try_send(Message::Stop);
            }
        }
    }
}
pub(super) fn register(app: &tauri::AppHandle, copy: &CloudWorkingCopy) -> CloudCommandResult<()> {
    if is_removed(app, &copy.id) {
        return Ok(());
    }
    let Some(state) = app.try_state::<CloudWritebackState>() else {
        // Scoped native-test builds deliberately do not start background uploads.
        return Ok(());
    };
    callback(app)(copy, 0, 0);
    let mut running = state.running.lock().map_err(|_| {
        CloudCommandError::new(
            CloudCommandErrorCode::TaskFailed,
            "Cloud saving watcher is unavailable",
        )
    })?;
    if running.is_none() {
        let (sender, receiver) = mpsc::sync_channel(128);
        let overflow = Arc::new(AtomicBool::new(false));
        let callback_sender = sender.clone();
        let callback_overflow = overflow.clone();
        let watcher = notify::recommended_watcher(move |event: Result<Event, notify::Error>| {
            let paths = match event {
                Ok(event) if event.need_rescan() => {
                    callback_overflow.store(true, Ordering::Relaxed);
                    event.paths
                }
                Ok(event)
                    if matches!(
                        event.kind,
                        EventKind::Create(_)
                            | EventKind::Modify(_)
                            | EventKind::Remove(_)
                            | EventKind::Any
                            | EventKind::Other
                    ) =>
                {
                    event.paths
                }
                Ok(_) => return,
                Err(_) => {
                    callback_overflow.store(true, Ordering::Relaxed);
                    Vec::new()
                }
            };
            if callback_sender.try_send(Message::Changed(paths)).is_err() {
                callback_overflow.store(true, Ordering::Relaxed);
                let _ = callback_sender.try_send(Message::Wake);
            }
        })
        .map_err(|_| {
            CloudCommandError::new(
                CloudCommandErrorCode::TaskFailed,
                "File watching is unavailable; save your cloud copy manually",
            )
        })?;
        let stop = Arc::new(AtomicBool::new(false));
        let thread_stop = stop.clone();
        let thread_app = app.clone();
        std::thread::Builder::new()
            .name("cloud-edit-save".into())
            .spawn(move || {
                run(thread_app, watcher, receiver, overflow, thread_stop);
            })
            .map_err(|_| {
                CloudCommandError::new(
                    CloudCommandErrorCode::TaskFailed,
                    "Cannot start cloud saving worker",
                )
            })?;
        *running = Some(Running { sender, stop });
    }
    // Registration is infrequent, unlike watcher callbacks. Keep all sessions
    // even when a burst has filled the bounded event queue.
    let sender = running
        .as_ref()
        .ok_or_else(|| {
            CloudCommandError::new(
                CloudCommandErrorCode::TaskFailed,
                "Cloud saving worker stopped",
            )
        })?
        .sender
        .clone();
    // Shutdown must be able to set the stop flag even if a registration waits
    // behind a full queue while the worker is handling a network request.
    drop(running);
    sender
        .send(Message::Register(Box::new(copy.clone())))
        .map_err(|_| {
            CloudCommandError::new(
                CloudCommandErrorCode::TaskFailed,
                "Cloud saving worker stopped",
            )
        })
}
fn schedule(copy: &CloudWorkingCopy) -> bool {
    copy.auto_save
        && !matches!(
            copy.save_status,
            CloudSaveStatus::Conflict | CloudSaveStatus::Unsupported | CloudSaveStatus::Paused
        )
}
fn run(
    app: tauri::AppHandle,
    mut watcher: RecommendedWatcher,
    receiver: mpsc::Receiver<Message>,
    overflow: Arc<AtomicBool>,
    stop: Arc<AtomicBool>,
) {
    let mut copies: HashMap<String, CloudWorkingCopy> = HashMap::new();
    let mut watched: HashSet<PathBuf> = HashSet::new();
    let mut pending: HashMap<String, Instant> = HashMap::new();
    let mut attempts: HashMap<String, u32> = HashMap::new();
    let on_status = callback(&app);
    while !stop.load(Ordering::Relaxed) && !runtime_lifecycle::is_shutting_down(&app) {
        // Cleanup wakes the worker; a full queue also reaches this branch on
        // its next iteration. No extra timer or periodic disk scan is needed.
        let removed_ids: Vec<_> = app
            .try_state::<CloudWritebackState>()
            .and_then(|state| {
                state.removed.lock().ok().map(|removed| {
                    if removed.is_empty() {
                        return Vec::new();
                    }
                    copies
                        .keys()
                        .filter(|id| removed.contains(*id))
                        .cloned()
                        .collect()
                })
            })
            .unwrap_or_default();
        for id in removed_ids {
            if let Some(copy) = copies.remove(&id) {
                if let Some(directory) = Path::new(&copy.local_path).parent() {
                    let _ = watcher.unwatch(directory);
                    watched.remove(directory);
                }
            }
            pending.remove(&id);
            attempts.remove(&id);
        }
        let wait = pending
            .values()
            .min()
            .map(|at| at.saturating_duration_since(Instant::now()))
            .unwrap_or(Duration::from_secs(30));
        match receiver.recv_timeout(wait) {
            Ok(Message::Stop) | Err(mpsc::RecvTimeoutError::Disconnected) => break,
            Ok(Message::Register(copy)) => {
                let mut copy = *copy;
                if is_removed(&app, &copy.id) {
                    continue;
                }
                let directory = Path::new(&copy.local_path).parent().map(Path::to_path_buf);
                if copy.auto_save {
                    if let Some(directory) = directory {
                        if !watched.contains(&directory) {
                            if watcher
                                .watch(&directory, RecursiveMode::NonRecursive)
                                .is_err()
                            {
                                copy.save_status = CloudSaveStatus::Error;
                                copy.save_message=Some("File watching failed. Your local copy is kept; use Save to original.".into());
                                copy.auto_save = false;
                                if let Ok(base) = root() {
                                    let _ = sync::persist(&base, &copy);
                                }
                                on_status(&copy, 0, 0);
                            } else {
                                watched.insert(directory);
                            }
                        }
                    }
                } else if let Some(directory) = directory {
                    let _ = watcher.unwatch(&directory);
                    watched.remove(&directory);
                    pending.remove(&copy.id);
                }
                if schedule(&copy) {
                    pending.insert(
                        copy.id.clone(),
                        Instant::now() + Duration::from_millis(1500),
                    );
                }
                copies.insert(copy.id.clone(), copy);
            }
            Ok(Message::Changed(paths)) => {
                for copy in copies.values_mut().filter(|copy| schedule(copy)) {
                    let local = Path::new(&copy.local_path);
                    if paths
                        .iter()
                        .any(|p| p == local || Some(p.as_path()) == local.parent())
                    {
                        pending.insert(
                            copy.id.clone(),
                            Instant::now() + Duration::from_millis(1500),
                        );
                        attempts.remove(&copy.id);
                        if copy.save_status != CloudSaveStatus::Pending {
                            copy.save_status = CloudSaveStatus::Pending;
                            on_status(copy, 0, 0);
                        }
                    }
                }
            }
            Ok(Message::Wake) | Err(mpsc::RecvTimeoutError::Timeout) => {}
        }
        if overflow.swap(false, Ordering::Relaxed) {
            for copy in copies.values().filter(|c| schedule(c)) {
                pending.insert(
                    copy.id.clone(),
                    Instant::now() + Duration::from_millis(1500),
                );
            }
        }
        let due: Vec<_> = pending
            .iter()
            .filter(|(_, at)| **at <= Instant::now())
            .map(|(id, _)| id.clone())
            .collect();
        for id in due {
            pending.remove(&id);
            if stop.load(Ordering::Relaxed) || runtime_lifecycle::is_shutting_down(&app) {
                break;
            }
            let Some(previous) = copies.get(&id).cloned() else {
                continue;
            };
            if is_removed(&app, &id) {
                continue;
            }
            let loaded = root().and_then(|base| load_manifest_at(&base, &id, false));
            // A detached/removed working copy must never fall back to a stale
            // manifest and trigger cloud I/O or recreate save-error status.
            if loaded
                .as_ref()
                .is_err_and(|error| error.code() == CloudCommandErrorCode::NotFound)
            {
                continue;
            }
            let copy = loaded.unwrap_or(previous);
            copies.insert(id.clone(), copy.clone());
            if !schedule(&copy) {
                continue;
            }
            let Some(_activity) = runtime_lifecycle::try_enter_background_job_from_app(&app) else {
                break;
            };
            let result = (|| {
                super::super::ensure_cloud_enabled()?;
                let base = root()?;
                let source = CloudPath::parse(&copy.source_path)
                    .map_err(super::super::map_cloud_path_error)?;
                super::super::limits::with_cloud_remote_permits(
                    vec![source.remote().into()],
                    || {
                        let provider: RcloneCloudProvider =
                            super::super::configured_rclone_provider()
                                .map_err(CloudCommandError::from)?;
                        sync::save_original_at(
                            &base,
                            &id,
                            &provider,
                            stop.clone(),
                            on_status.clone(),
                        )
                    },
                )
            })();
            match result {
                Ok(copy) => {
                    attempts.remove(&id);
                    if copy.dirty {
                        pending.insert(id.clone(), Instant::now() + Duration::from_millis(1500));
                    }
                    copies.insert(id, copy);
                }
                Err(error) => {
                    if is_removed(&app, &id) || error.code() == CloudCommandErrorCode::NotFound {
                        continue;
                    }
                    let loaded = root().and_then(|base| load_manifest_at(&base, &id, false));
                    let mut copy = loaded.unwrap_or(copy);
                    if !matches!(
                        copy.save_status,
                        CloudSaveStatus::Conflict | CloudSaveStatus::Unsupported
                    ) {
                        copy.save_status = if error.code() == CloudCommandErrorCode::Cancelled {
                            CloudSaveStatus::Pending
                        } else {
                            CloudSaveStatus::Error
                        };
                        copy.save_message = Some(error.message().into());
                        if let Ok(base) = root() {
                            let _ = sync::persist(&base, &copy);
                        }
                        on_status(&copy, 0, 0);
                    }
                    let retry = matches!(
                        error.code(),
                        CloudCommandErrorCode::NetworkError
                            | CloudCommandErrorCode::Timeout
                            | CloudCommandErrorCode::RateLimited
                            | CloudCommandErrorCode::TaskFailed
                            | CloudCommandErrorCode::Cancelled
                    );
                    if retry && schedule(&copy) {
                        let attempt = attempts.entry(id.clone()).or_default();
                        *attempt = attempt.saturating_add(1);
                        let seconds = if error.code() == CloudCommandErrorCode::Cancelled {
                            2
                        } else {
                            5u64.saturating_mul(3u64.saturating_pow((*attempt).min(5)))
                                .min(300)
                        };
                        pending.insert(id.clone(), Instant::now() + Duration::from_secs(seconds));
                    }
                    copies.insert(id, copy);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn conflicts_and_paused_sessions_do_not_retry_on_editor_events() {
        let base =
            std::env::temp_dir().join(format!("browsey-monitor-policy-{}", std::process::id()));
        fs::create_dir_all(&base).unwrap();
        let cached = base.join("cached");
        fs::write(&cached, b"original").unwrap();
        let mut copy = create_at(
            &base.join("workspaces"),
            &CloudPath::parse("rclone://work/file.txt").unwrap(),
            &cached,
            None,
            None,
        )
        .unwrap();
        assert!(!schedule(&copy));
        copy.auto_save = true;
        assert!(schedule(&copy));
        for state in [
            CloudSaveStatus::Conflict,
            CloudSaveStatus::Paused,
            CloudSaveStatus::Unsupported,
        ] {
            copy.save_status = state;
            assert!(!schedule(&copy));
        }
        copy.save_status = CloudSaveStatus::Error;
        assert!(schedule(&copy));
        fs::remove_dir_all(base).unwrap();
    }
}
