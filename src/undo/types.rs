use std::collections::VecDeque;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::MetadataExt;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::undo::{UndoError, UndoResult};

const MAX_HISTORY: usize = 50;

/// In-memory evidence of the paths actually produced by a completed copy.
/// Missing evidence (for example a GIO-owned writer) is not permission to delete.
#[derive(Debug, Clone, Default)]
pub struct CopyReceipt {
    pub(super) snapshot: Option<crate::fs_utils::TreeSnapshot>,
    pub(super) backup: Option<super::copy_recovery::CopyBackup>,
}

impl CopyReceipt {
    pub(crate) fn from_snapshot(snapshot: crate::fs_utils::TreeSnapshot) -> Self {
        Self {
            snapshot: snapshot.has_root().then_some(snapshot),
            backup: None,
        }
    }

    pub(super) fn snapshot(
        &self,
        path: &std::path::Path,
    ) -> UndoResult<&crate::fs_utils::TreeSnapshot> {
        self.snapshot.as_ref().ok_or_else(|| {
            UndoError::invalid_input(format!(
                "Cannot verify copy target ownership; retained {}",
                path.display()
            ))
        })
    }

    pub(crate) fn verify(&self, path: &std::path::Path) -> UndoResult<()> {
        self.verify_with_check(path, || Ok(()))
    }

    pub(crate) fn verify_with_check(
        &self,
        path: &std::path::Path,
        check: impl FnMut() -> std::io::Result<()>,
    ) -> UndoResult<()> {
        self.snapshot(path)?
            .verify_with_check(path, check)
            .map_err(|error| {
                UndoError::from_io_error(
                    format!(
                        "Copy target changed or could not be verified; retained {}",
                        path.display()
                    ),
                    error,
                )
            })
    }

    pub(crate) fn remove(&self, path: &std::path::Path) -> UndoResult<()> {
        self.snapshot(path)?.remove_recorded(path).map_err(|error| {
            UndoError::from_io_error(
                format!(
                    "Copy target changed or could not be removed safely; remaining paths retained at {}",
                    path.display()
                ),
                error,
            )
        })
    }
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum Action {
    Rename {
        from: PathBuf,
        to: PathBuf,
    },
    Move {
        from: PathBuf,
        to: PathBuf,
    },
    Copy {
        from: PathBuf,
        to: PathBuf,
        receipt: CopyReceipt,
    },
    /// Represents a newly created path. Undo (Backward) moves the path to a
    /// backup location (effectively deleting it while retaining data); redo
    /// (Forward) moves it back.
    Create {
        path: PathBuf,
        backup: PathBuf,
    },
    Delete {
        path: PathBuf,
        backup: PathBuf,
    },
    #[cfg(target_os = "windows")]
    SetHidden {
        path: PathBuf,
        hidden: bool,
    },
    CreateFolder {
        path: PathBuf,
    },
    Batch(Vec<Action>),
}

#[derive(Debug, Clone, Copy)]
#[allow(dead_code)]
pub enum Direction {
    Forward,
    Backward,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct PermissionsSnapshot {
    pub readonly: bool,
    #[cfg(unix)]
    pub mode: u32,
    #[cfg(target_os = "windows")]
    pub dacl: Option<Vec<u8>>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct OwnershipSnapshot {
    #[cfg(unix)]
    pub uid: u32,
    #[cfg(unix)]
    pub gid: u32,
}

#[derive(Default)]
#[allow(dead_code)]
pub struct UndoManager {
    undo_stack: VecDeque<Action>,
    redo_stack: VecDeque<Action>,
}

impl UndoManager {
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self {
            undo_stack: VecDeque::new(),
            redo_stack: VecDeque::new(),
        }
    }

    #[allow(dead_code)]
    pub fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    #[allow(dead_code)]
    pub fn can_redo(&self) -> bool {
        !self.redo_stack.is_empty()
    }

    /// Apply a new action and push it onto the undo stack. Clears redo history.
    #[allow(dead_code)]
    pub fn apply(&mut self, mut action: Action) -> UndoResult<()> {
        super::engine::execute_action(&mut action, Direction::Forward)?;
        super::engine::finalize_action(&mut action);
        self.undo_stack.push_back(action);
        self.redo_stack.clear();
        self.trim();
        Ok(())
    }

    pub fn undo(&mut self) -> UndoResult<()> {
        let mut action = self
            .undo_stack
            .pop_back()
            .ok_or_else(UndoError::undo_unavailable)?;
        match super::engine::execute_action(&mut action, Direction::Backward) {
            Ok(_) => {
                super::engine::finalize_action(&mut action);
                self.redo_stack.push_back(action);
                Ok(())
            }
            Err(err) => {
                self.undo_stack.push_back(action);
                Err(err)
            }
        }
    }

    pub fn redo(&mut self) -> UndoResult<()> {
        let mut action = self
            .redo_stack
            .pop_back()
            .ok_or_else(UndoError::redo_unavailable)?;
        match super::engine::execute_action(&mut action, Direction::Forward) {
            Ok(_) => {
                super::engine::finalize_action(&mut action);
                self.undo_stack.push_back(action);
                self.trim();
                Ok(())
            }
            Err(err) => {
                self.redo_stack.push_back(action);
                Err(err)
            }
        }
    }

    #[allow(dead_code)]
    pub fn clear(&mut self) {
        self.undo_stack.clear();
        self.redo_stack.clear();
    }

    pub fn record_applied(&mut self, action: Action) {
        self.undo_stack.push_back(action);
        self.redo_stack.clear();
        self.trim();
    }

    fn trim(&mut self) {
        while self.undo_stack.len() > MAX_HISTORY {
            let _ = self.undo_stack.pop_front();
        }
    }
}

#[derive(Clone, Default)]
pub struct UndoState {
    inner: Arc<Mutex<UndoManager>>,
}

impl UndoState {
    pub fn clone_inner(&self) -> Arc<Mutex<UndoManager>> {
        self.inner.clone()
    }

    #[allow(dead_code)]
    pub fn record(&self, action: Action) -> UndoResult<()> {
        let mut mgr = self
            .inner
            .lock()
            .map_err(|_| UndoError::lock_failed("Undo manager poisoned"))?;
        mgr.apply(action)?;
        Ok(())
    }

    pub fn record_applied(&self, action: Action) -> UndoResult<()> {
        let mut mgr = self
            .inner
            .lock()
            .map_err(|_| UndoError::lock_failed("Undo manager poisoned"))?;
        mgr.record_applied(action);
        Ok(())
    }

    pub fn undo(&self) -> UndoResult<()> {
        let mut mgr = self
            .inner
            .lock()
            .map_err(|_| UndoError::lock_failed("Undo manager poisoned"))?;
        mgr.undo()
    }

    pub fn redo(&self) -> UndoResult<()> {
        let mut mgr = self
            .inner
            .lock()
            .map_err(|_| UndoError::lock_failed("Undo manager poisoned"))?;
        mgr.redo()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PathKind {
    File,
    Dir,
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PathSnapshot {
    kind: PathKind,
    #[cfg(unix)]
    dev: u64,
    #[cfg(unix)]
    ino: u64,
    #[cfg(not(unix))]
    len: u64,
    #[cfg(not(unix))]
    modified_nanos: Option<u128>,
}

fn path_kind_from_meta(meta: &fs::Metadata) -> PathKind {
    if meta.is_file() {
        PathKind::File
    } else if meta.is_dir() {
        PathKind::Dir
    } else {
        PathKind::Other
    }
}

pub(crate) fn path_snapshot_from_meta(meta: &fs::Metadata) -> PathSnapshot {
    PathSnapshot {
        kind: path_kind_from_meta(meta),
        #[cfg(unix)]
        dev: meta.dev(),
        #[cfg(unix)]
        ino: meta.ino(),
        #[cfg(not(unix))]
        len: meta.len(),
        #[cfg(not(unix))]
        modified_nanos: meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_nanos()),
    }
}

pub(crate) fn snapshots_match(expected: &PathSnapshot, current: &PathSnapshot) -> bool {
    if expected.kind != current.kind {
        return false;
    }
    #[cfg(unix)]
    {
        expected.dev == current.dev && expected.ino == current.ino
    }
    #[cfg(not(unix))]
    {
        expected.len == current.len && expected.modified_nanos == current.modified_nanos
    }
}
