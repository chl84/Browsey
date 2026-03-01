use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc,
};
use std::time::{Duration, Instant};

use tauri::{Emitter, Manager};
use tracing::debug;

#[derive(Default)]
pub struct RuntimeLifecycle {
    inner: Arc<RuntimeLifecycleInner>,
}

#[derive(Default)]
struct RuntimeLifecycleInner {
    shutting_down: AtomicBool,
    active_background_jobs: AtomicUsize,
}

#[derive(Clone)]
pub struct RuntimeLifecycleHandle {
    inner: Arc<RuntimeLifecycleInner>,
}

pub struct BackgroundActivityGuard {
    inner: Arc<RuntimeLifecycleInner>,
}

impl Drop for BackgroundActivityGuard {
    fn drop(&mut self) {
        self.inner
            .active_background_jobs
            .fetch_sub(1, Ordering::SeqCst);
    }
}

impl RuntimeLifecycle {
    pub fn handle(&self) -> RuntimeLifecycleHandle {
        RuntimeLifecycleHandle {
            inner: self.inner.clone(),
        }
    }
}

impl RuntimeLifecycleHandle {
    pub fn begin_shutdown(&self) {
        self.inner.shutting_down.store(true, Ordering::SeqCst);
    }

    pub fn is_shutting_down(&self) -> bool {
        self.inner.shutting_down.load(Ordering::SeqCst)
    }

    pub fn try_enter_background_job(&self) -> Option<BackgroundActivityGuard> {
        if self.is_shutting_down() {
            return None;
        }

        self.inner
            .active_background_jobs
            .fetch_add(1, Ordering::SeqCst);
        if self.is_shutting_down() {
            self.inner
                .active_background_jobs
                .fetch_sub(1, Ordering::SeqCst);
            return None;
        }

        Some(BackgroundActivityGuard {
            inner: self.inner.clone(),
        })
    }

    pub fn wait_for_background_jobs(&self, timeout: Duration) {
        let deadline = Instant::now() + timeout;
        while self.inner.active_background_jobs.load(Ordering::SeqCst) > 0 {
            if Instant::now() >= deadline {
                break;
            }
            std::thread::sleep(Duration::from_millis(5));
        }
    }
}

pub fn handle_from_app<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> Option<RuntimeLifecycleHandle> {
    app.try_state::<RuntimeLifecycle>()
        .map(|state| state.handle())
}

pub fn begin_shutdown_from_app<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    if let Some(handle) = handle_from_app(app) {
        handle.begin_shutdown();
    }
}

pub fn is_shutting_down<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> bool {
    handle_from_app(app)
        .map(|handle| handle.is_shutting_down())
        .unwrap_or(false)
}

pub fn try_enter_background_job_from_app<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
) -> Option<BackgroundActivityGuard> {
    handle_from_app(app).and_then(|handle| handle.try_enter_background_job())
}

pub fn wait_for_background_jobs_from_app<R: tauri::Runtime>(
    app: &tauri::AppHandle<R>,
    timeout: Duration,
) {
    if let Some(handle) = handle_from_app(app) {
        handle.wait_for_background_jobs(timeout);
    }
}

pub fn emit_if_running<R: tauri::Runtime, S: serde::Serialize + Clone>(
    app: &tauri::AppHandle<R>,
    event: &str,
    payload: S,
) -> bool {
    if is_shutting_down(app) {
        debug!(event, "dropping runtime event during shutdown");
        return false;
    }
    // Best effort by design: during shutdown or transient frontend teardown we
    // prefer dropping the event over turning coordination helpers into
    // fallible plumbing everywhere.
    match app.emit(event, payload) {
        Ok(()) => true,
        Err(error) => {
            debug!(event, %error, "failed to emit runtime event");
            false
        }
    }
}
