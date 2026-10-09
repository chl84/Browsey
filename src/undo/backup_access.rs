//! The process-wide session lock protects retention, not individual files.
//! Readers of a completed backup exclude writers of that bucket while copying;
//! unrelated backups in the same running session remain available.
use super::{UndoError, UndoResult};
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{Arc, Condvar, Mutex},
};

#[derive(Default)]
struct Uses {
    readers: usize,
    writers: usize,
}
#[derive(Default)]
pub(super) struct BackupAccess {
    uses: Mutex<HashMap<PathBuf, Uses>>,
    changed: Condvar,
}

pub(super) struct BackupReadGuard {
    access: Arc<BackupAccess>,
    key: PathBuf,
}
pub(crate) struct BackupWriteGuard {
    guards: Vec<(Arc<BackupAccess>, PathBuf)>,
}

fn overlaps(a: &std::path::Path, b: &std::path::Path) -> bool {
    a.as_os_str().is_empty() || b.as_os_str().is_empty() || a == b
}

impl BackupAccess {
    pub(super) fn try_read(self: &Arc<Self>, key: PathBuf) -> UndoResult<Option<BackupReadGuard>> {
        let mut uses = self
            .uses
            .lock()
            .map_err(|_| UndoError::lock_failed("Backup access registry poisoned"))?;
        if uses
            .iter()
            .any(|(other, usage)| overlaps(&key, other) && usage.writers > 0)
        {
            return Ok(None);
        }
        uses.entry(key.clone()).or_default().readers += 1;
        Ok(Some(BackupReadGuard {
            access: self.clone(),
            key,
        }))
    }

    fn write(self: &Arc<Self>, key: &std::path::Path) -> UndoResult<()> {
        let mut uses = self
            .uses
            .lock()
            .map_err(|_| UndoError::lock_failed("Backup access registry poisoned"))?;
        while uses
            .iter()
            .any(|(other, usage)| overlaps(key, other) && usage.readers > 0)
        {
            uses = self
                .changed
                .wait(uses)
                .map_err(|_| UndoError::lock_failed("Backup access registry poisoned"))?;
        }
        // Writers may nest through existing copy/move helpers. Counting them
        // instead of locking recursively preserves that flow without deadlock.
        uses.entry(key.to_path_buf()).or_default().writers += 1;
        Ok(())
    }

    fn release(&self, key: &std::path::Path, reader: bool) {
        let Ok(mut uses) = self.uses.lock() else {
            return;
        };
        if let Some(usage) = uses.get_mut(key) {
            if reader {
                usage.readers -= 1;
            } else {
                usage.writers -= 1;
            }
            if usage.readers == 0 && usage.writers == 0 {
                uses.remove(key);
            }
        }
        self.changed.notify_all();
    }
}

impl Drop for BackupReadGuard {
    fn drop(&mut self) {
        self.access.release(&self.key, true);
    }
}
impl Drop for BackupWriteGuard {
    fn drop(&mut self) {
        for (access, key) in &self.guards {
            access.release(key, false);
        }
    }
}

/// Guards all registered current-session backup buckets touched by a primitive.
/// Paths belonging to ordinary files or another process are unaffected.
pub(crate) fn write_backups(paths: &[&std::path::Path]) -> UndoResult<BackupWriteGuard> {
    let mut guard = BackupWriteGuard { guards: Vec::new() };
    for path in paths {
        if let Some(session) = super::backup::owned_session(path)? {
            let key = session.bucket(path)?;
            if guard
                .guards
                .iter()
                .any(|(access, other)| Arc::ptr_eq(access, &session.access) && *other == key)
            {
                continue;
            }
            session.access.write(&key)?;
            guard.guards.push((session.access, key));
        }
    }
    Ok(guard)
}
