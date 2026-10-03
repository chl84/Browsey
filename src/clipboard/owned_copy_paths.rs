//! Failure cleanup for paths created by this copy, never whole directory trees.
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::path::{Path, PathBuf};

use crate::fs_utils::{FileIdentity, FileState, TreeSnapshot};

#[derive(Default)]
pub(super) struct OwnedCopyPaths {
    dirs: BTreeMap<PathBuf, Option<FileIdentity>>,
    files: Vec<(PathBuf, FileState)>,
}

impl OwnedCopyPaths {
    pub(super) fn receipt(&self, root: &Path) -> crate::undo::CopyReceipt {
        let mut snapshot = TreeSnapshot::default();
        for (path, identity) in &self.dirs {
            let Some(identity) = identity else {
                return crate::undo::CopyReceipt::default();
            };
            let Ok(relative) = path.strip_prefix(root) else {
                return crate::undo::CopyReceipt::default();
            };
            snapshot.record_directory(relative.into(), identity.clone());
        }
        for (path, state) in &self.files {
            let Ok(relative) = path.strip_prefix(root) else {
                return crate::undo::CopyReceipt::default();
            };
            snapshot.record_file(relative.into(), state.clone());
        }
        crate::undo::CopyReceipt::from_snapshot(snapshot)
    }

    pub(super) fn record_dir(&mut self, path: &Path) {
        self.dirs.insert(path.into(), FileIdentity::capture(path));
    }

    pub(super) fn record_file(&mut self, path: &Path, file: &File) -> std::io::Result<()> {
        self.files.push((path.into(), FileState::from_file(file)?));
        Ok(())
    }

    fn parents_match(&self, path: &Path) -> bool {
        path.ancestors()
            .filter_map(|dir| self.dirs.get(dir).map(|identity| (dir, identity)))
            .all(|(dir, identity)| {
                identity
                    .as_ref()
                    .is_some_and(|identity| identity.matches(dir))
            })
    }

    pub(super) fn directory_matches(&self, path: &Path) -> bool {
        self.dirs.contains_key(path) && self.parents_match(path)
    }

    pub(super) fn directory_handle_matches(&self, path: &Path, file: &File) -> bool {
        let handle = FileIdentity::from_file(file);
        self.parents_match(path)
            && handle.is_some()
            && self
                .dirs
                .get(path)
                .is_some_and(|identity| identity == &handle)
    }

    /// Return retained/error paths so the primary operation error stays visible.
    pub(super) fn cleanup(&self) -> Vec<String> {
        let mut retained = Vec::new();
        #[cfg(target_os = "linux")]
        for dir in self.dirs.keys() {
            if self.parents_match(dir) {
                // A completed child may already have restrictive source modes.
                // Use the existing no-follow permission helper before cleanup.
                let _ = crate::undo::set_unix_mode_nofollow(dir, 0o700);
            }
        }
        for (path, state) in self.files.iter().rev() {
            if !path.try_exists().unwrap_or(true) {
                continue;
            }
            if !self.parents_match(path) || !state.matches(path) {
                retained.push(path.display().to_string());
                continue;
            }
            if let Err(error) = fs::remove_file(path) {
                if error.kind() != std::io::ErrorKind::NotFound {
                    retained.push(format!("{}: {error}", path.display()));
                }
            }
        }
        for path in self.dirs.keys().rev() {
            if !path.try_exists().unwrap_or(true) {
                continue;
            }
            if !self.parents_match(path) {
                retained.push(path.display().to_string());
                continue;
            }
            // Untracked, replaced or edited children keep the directory nonempty.
            if let Err(error) = fs::remove_dir(path) {
                if error.kind() != std::io::ErrorKind::NotFound {
                    retained.push(format!("{}: {error}", path.display()));
                }
            }
        }
        retained
    }
}
