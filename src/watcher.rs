use std::path::PathBuf;
use std::sync::Mutex;

use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};

use crate::errors::domain::{DomainError, ErrorCode};
use crate::runtime_lifecycle;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WatcherErrorCode {
    WatcherCreateFailed,
    WatchPathFailed,
}

impl ErrorCode for WatcherErrorCode {
    fn as_code_str(self) -> &'static str {
        match self {
            Self::WatcherCreateFailed => "watcher_create_failed",
            Self::WatchPathFailed => "watch_path_failed",
        }
    }
}

#[derive(Debug, Clone)]
pub struct WatcherError {
    code: WatcherErrorCode,
    message: String,
}

impl WatcherError {
    pub fn new(code: WatcherErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

impl fmt::Display for WatcherError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for WatcherError {}

impl DomainError for WatcherError {
    fn code_str(&self) -> &'static str {
        self.code.as_code_str()
    }

    fn message(&self) -> &str {
        &self.message
    }
}

impl From<WatcherError> for String {
    fn from(error: WatcherError) -> Self {
        error.to_string()
    }
}

pub type WatcherResult<T> = Result<T, WatcherError>;

#[derive(Default)]
pub struct WatchState {
    inner: Mutex<Option<RecommendedWatcher>>,
}

impl WatchState {
    pub fn replace(&self, watcher: Option<RecommendedWatcher>) {
        let mut guard = self.inner.lock().expect("watch mutex poisoned");
        *guard = watcher;
    }

    pub fn stop_all(&self) {
        self.replace(None);
    }
}

pub fn start_watch(app: tauri::AppHandle, path: PathBuf, state: &WatchState) -> WatcherResult<()> {
    let watched_path = path.to_string_lossy().to_string();
    let mut watcher = notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
        if runtime_lifecycle::is_shutting_down(&app) {
            return;
        }
        if let Ok(event) = res {
            match event.kind {
                EventKind::Create(_)
                | EventKind::Modify(_)
                | EventKind::Remove(_)
                | EventKind::Any
                | EventKind::Other => {
                    let _ = runtime_lifecycle::emit_if_running(
                        &app,
                        "dir-changed",
                        watched_path.clone(),
                    );
                }
                _ => {}
            }
        }
    })
    .map_err(|error| {
        WatcherError::new(
            WatcherErrorCode::WatcherCreateFailed,
            format!("Failed to create watcher: {error}"),
        )
    })?;

    watcher
        .watch(&path, RecursiveMode::NonRecursive)
        .map_err(|error| {
            WatcherError::new(
                WatcherErrorCode::WatchPathFailed,
                format!("Failed to watch path {}: {error}", path.display()),
            )
        })?;

    state.replace(Some(watcher));
    Ok(())
}
