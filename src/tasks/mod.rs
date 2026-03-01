use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};
use tracing::warn;

mod error;

use crate::errors::api_error::ApiResult;
use error::map_api_result;
pub use error::{TaskError, TaskErrorCode, TaskResult};

#[derive(Clone, Default)]
pub struct CancelState {
    inner: Arc<Mutex<HashMap<String, Arc<AtomicBool>>>>,
}

#[derive(Clone)]
pub struct CancelGuard {
    id: String,
    flag: Arc<AtomicBool>,
    state: CancelState,
}

impl CancelState {
    pub fn register(&self, id: String) -> TaskResult<CancelGuard> {
        let flag = Arc::new(AtomicBool::new(false));
        let mut map = self.inner.lock().map_err(|_| {
            TaskError::new(
                TaskErrorCode::RegistryLockFailed,
                "Failed to lock cancel registry",
            )
        })?;
        map.insert(id.clone(), flag.clone());
        Ok(CancelGuard {
            id,
            flag,
            state: self.clone(),
        })
    }

    pub fn cancel(&self, id: &str) -> TaskResult<bool> {
        let map = self.inner.lock().map_err(|_| {
            TaskError::new(
                TaskErrorCode::RegistryLockFailed,
                "Failed to lock cancel registry",
            )
        })?;
        if let Some(flag) = map.get(id) {
            flag.store(true, Ordering::Relaxed);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    pub fn cancel_all(&self) -> TaskResult<usize> {
        let map = self.inner.lock().map_err(|_| {
            TaskError::new(
                TaskErrorCode::RegistryLockFailed,
                "Failed to lock cancel registry",
            )
        })?;
        for flag in map.values() {
            flag.store(true, Ordering::Relaxed);
        }
        Ok(map.len())
    }

    fn remove(&self, id: &str) {
        match self.inner.lock() {
            Ok(mut map) => {
                map.remove(id);
            }
            Err(_) => {
                // Best effort during guard drop: the task is already ending and
                // callers cannot recover here, but we should leave a signal.
                warn!(task_id = id, "failed to remove task from cancel registry");
            }
        }
    }
}

impl CancelGuard {
    pub fn token(&self) -> Arc<AtomicBool> {
        self.flag.clone()
    }
}

impl Drop for CancelGuard {
    fn drop(&mut self) {
        self.state.remove(&self.id);
    }
}

#[tauri::command]
pub fn cancel_task(state: tauri::State<CancelState>, id: String) -> ApiResult<()> {
    map_api_result(cancel_task_impl(state, id))
}

fn cancel_task_impl(state: tauri::State<CancelState>, id: String) -> TaskResult<()> {
    match state.cancel(&id)? {
        true => Ok(()),
        false => Err(TaskError::new(
            TaskErrorCode::TaskNotFound,
            "Task not found or already finished",
        )),
    }
}
