//! Explicit deletion is exclusive with complete backup-producing operations,
//! including the gaps between allocation, copying, rollback and history recording.
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard, TryLockError};

use super::{UndoError, UndoResult};

static OPERATIONS: RwLock<()> = RwLock::new(());

pub(crate) fn backup_operation() -> UndoResult<RwLockReadGuard<'static, ()>> {
    OPERATIONS
        .read()
        .map_err(|_| UndoError::lock_failed("Backup operation registry poisoned"))
}

pub(super) fn exclusive() -> UndoResult<RwLockWriteGuard<'static, ()>> {
    OPERATIONS.try_write().map_err(|error| match error {
        TryLockError::WouldBlock => UndoError::lock_failed(
            "Backups are in use. Wait for file operations or recovery to finish, then retry.",
        ),
        TryLockError::Poisoned(_) => UndoError::lock_failed("Backup operation registry poisoned"),
    })
}
