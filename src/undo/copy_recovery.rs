//! Copy-before-remove recovery. No persistent history or atomic transaction claim.
use std::fs;
use std::path::{Path, PathBuf};

use crate::fs_utils::TreeSnapshot;

use super::backup::RecoveryMarker;
use super::path_ops::copy_entry_recorded;
use super::{temp_backup_path, CopyReceipt, UndoError, UndoResult};

#[derive(Debug, Clone)]
pub(super) struct CopyBackup {
    path: PathBuf,
    snapshot: TreeSnapshot,
    undone: bool,
    marker: Option<RecoveryMarker>,
}

impl CopyBackup {
    fn protect(&mut self, target: &Path) -> UndoResult<()> {
        if let Some(marker) = &self.marker {
            marker.verify()
        } else {
            self.marker = Some(RecoveryMarker::create(&self.path, target)?);
            Ok(())
        }
    }

    fn verify(&self) -> UndoResult<()> {
        self.snapshot.verify(&self.path).map_err(|error| {
            UndoError::from_io_error(
                format!(
                    "Recovery copy changed or could not be verified at {}",
                    self.path.display()
                ),
                error,
            )
        })
    }
}

impl CopyReceipt {
    pub(super) fn preflight_undo(&self, target: &Path) -> UndoResult<()> {
        self.preflight_undo_inner(target).map_err(|error| {
            if let Some(backup) = &self.backup {
                error.with_context(format!(
                    "Recovery copy retained at {}; inspect before retrying",
                    backup.path.display()
                ))
            } else {
                error
            }
        })
    }

    fn preflight_undo_inner(&self, target: &Path) -> UndoResult<()> {
        if let Some(backup) = &self.backup {
            backup.verify()?;
            if backup.undone {
                crate::path_guard::ensure_no_symlink_components_existing_prefix(target).map_err(
                    |error| UndoError::invalid_input(format!("Unsafe copy target: {error}")),
                )?;
                return match fs::symlink_metadata(target) {
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
                    Err(error) => Err(UndoError::from_io_error(
                        "Check already undone copy target",
                        error,
                    )),
                    Ok(_) => Err(UndoError::target_exists(format!(
                        "Already undone copy target is occupied; retained {}",
                        target.display()
                    ))),
                };
            }
        }
        self.verify(target)
    }

    pub(super) fn execute_backward(&mut self, target: &Path) -> UndoResult<()> {
        self.preflight_undo(target)?;
        if self.backup.as_ref().is_some_and(|backup| backup.undone) {
            return Ok(());
        }
        if self.backup.is_none() {
            let path = temp_backup_path(target)?;
            let result = (|| {
                let parent = path
                    .parent()
                    .ok_or_else(|| UndoError::invalid_input("Invalid copy backup path"))?;
                let mut builder = fs::DirBuilder::new();
                builder.recursive(true);
                #[cfg(unix)]
                {
                    use std::os::unix::fs::DirBuilderExt;
                    builder.mode(0o700);
                }
                builder.create(parent).map_err(|error| {
                    UndoError::from_io_error("Create copy backup directory", error)
                })?;
                let backup_receipt = copy_entry_recorded(target, &path)?;
                backup_receipt.verify(&path)?;
                // Never remove a changed target or cache a potentially mixed
                // backup as the original copied bytes after a concurrent edit.
                self.verify(target)?;
                Ok(backup_receipt.snapshot(&path)?.clone())
            })();
            let snapshot = result.map_err(|error: UndoError| {
                error.with_context(format!(
                    "Copy undo backup failed; target not removed. Backup candidate retained at {}",
                    path.display()
                ))
            })?;
            self.backup = Some(CopyBackup {
                path,
                snapshot,
                undone: false,
                marker: None,
            });
        }
        let path = self
            .backup
            .as_ref()
            .expect("copy backup prepared")
            .path
            .clone();
        let result = (|| {
            let backup = self.backup.as_mut().unwrap();
            backup.protect(target)?;
            backup.verify()?;
            self.remove(target)?;
            self.backup.as_mut().unwrap().undone = true;
            Ok(())
        })();
        result.map_err(|error: UndoError| {
            error.with_context(format!(
                "Recovery copy retained at {}. Inspect the affected paths before retrying",
                path.display()
            ))
        })
    }

    pub(super) fn execute_forward(&mut self, source: &Path, target: &Path) -> UndoResult<()> {
        let Some(backup) = self.backup.as_ref() else {
            *self = copy_entry_recorded(source, target)?;
            return Ok(());
        };
        if !backup.undone {
            // A previous batch compensation may already have restored this
            // member. Do not duplicate it or silently overwrite current data.
            return self.verify(target);
        }
        let path = backup.path.clone();
        let result = (|| {
            let backup = self.backup.as_mut().unwrap();
            backup.protect(target)?;
            backup.verify()?;
            let restored = copy_entry_recorded(&path, target)?;
            backup.verify()?;
            restored.verify(target)?;
            self.snapshot = restored.snapshot;
            self.backup.as_mut().unwrap().undone = false;
            Ok(())
        })();
        result.map_err(|error: UndoError| error.with_context(format!(
            "Copy restore failed; recovery copy retained at {}. Target may contain partial output; inspect before retrying", path.display()
        )))
    }

    pub(super) fn finalize_recovery(&mut self) {
        if let Some(backup) = self.backup.as_mut() {
            if let Some(marker) = &backup.marker {
                match marker.clear() {
                    Ok(()) => backup.marker = None,
                    Err(error) => {
                        tracing::warn!(%error, backup = %backup.path.display(), "Keep copy recovery marker after completed operation")
                    }
                }
            }
        }
    }

    pub(super) fn recovery_path(&self) -> Option<&Path> {
        self.backup
            .as_ref()
            .filter(|backup| backup.marker.is_some())
            .map(|backup| backup.path.as_path())
    }
}
