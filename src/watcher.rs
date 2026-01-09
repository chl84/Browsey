use std::path::PathBuf;
use std::sync::Mutex;

use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use tauri::Emitter;

#[derive(Default)]
pub struct WatchState {
    inner: Mutex<Option<RecommendedWatcher>>,
}

impl WatchState {
    pub fn replace(&self, watcher: Option<RecommendedWatcher>) {
        let mut guard = self.inner.lock().expect("watch mutex poisoned");
        *guard = watcher;
    }
}

pub fn start_watch(app: tauri::AppHandle, path: PathBuf, state: &WatchState) -> Result<(), String> {
    let watched_path = path.to_string_lossy().to_string();
    let mut watcher = notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
        if let Ok(event) = res {
            match event.kind {
                EventKind::Create(_) | EventKind::Modify(_) | EventKind::Remove(_) | EventKind::Any | EventKind::Other => {
                    let _ = app.emit("dir-changed", watched_path.clone());
                }
                _ => {}
            }
        }
    })
    .map_err(|e| e.to_string())?;

    watcher
        .watch(&path, RecursiveMode::NonRecursive)
        .map_err(|e| e.to_string())?;

    state.replace(Some(watcher));
    Ok(())
}
