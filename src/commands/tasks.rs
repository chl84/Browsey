use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};

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
    pub fn register(&self, id: String) -> Result<CancelGuard, String> {
        let flag = Arc::new(AtomicBool::new(false));
        let mut map = self
            .inner
            .lock()
            .map_err(|_| "Failed to lock cancel registry".to_string())?;
        map.insert(id.clone(), flag.clone());
        Ok(CancelGuard {
            id,
            flag,
            state: self.clone(),
        })
    }

    pub fn cancel(&self, id: &str) -> Result<bool, String> {
        let map = self
            .inner
            .lock()
            .map_err(|_| "Failed to lock cancel registry".to_string())?;
        if let Some(flag) = map.get(id) {
            flag.store(true, Ordering::Relaxed);
            Ok(true)
        } else {
            Ok(false)
        }
    }

    fn remove(&self, id: &str) {
        if let Ok(mut map) = self.inner.lock() {
            map.remove(id);
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
pub fn cancel_task(state: tauri::State<CancelState>, id: String) -> Result<(), String> {
    match state.cancel(&id) {
        Ok(true) => Ok(()),
        Ok(false) => Err("Task not found or already finished".into()),
        Err(e) => Err(e),
    }
}
