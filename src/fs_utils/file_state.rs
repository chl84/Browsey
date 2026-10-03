//! Metadata change detection, not a content hash or an atomic filesystem lock.
use std::collections::BTreeMap;
use std::fs::{self, File, Metadata};
use std::io;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use super::FileIdentity;

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct FileState {
    identity: FileIdentity,
    len: u64,
    modified: SystemTime,
    #[cfg(unix)]
    changed: (i64, i64),
}

impl FileState {
    fn from_metadata(identity: FileIdentity, meta: &Metadata) -> io::Result<Self> {
        #[cfg(unix)]
        use std::os::unix::fs::MetadataExt;
        Ok(Self {
            identity,
            len: meta.len(),
            modified: meta.modified()?,
            #[cfg(unix)]
            changed: (meta.ctime(), meta.ctime_nsec()),
        })
    }

    pub(crate) fn from_file(file: &File) -> io::Result<Self> {
        let identity = FileIdentity::from_file(file)
            .ok_or_else(|| io::Error::other("Cannot verify open file identity"))?;
        Self::from_metadata(identity, &file.metadata()?)
    }

    fn capture(path: &Path) -> io::Result<Self> {
        let meta = fs::symlink_metadata(path)?;
        let identity = FileIdentity::capture(path)
            .ok_or_else(|| io::Error::other("Cannot verify file identity"))?;
        Self::from_metadata(identity, &meta)
    }

    pub(crate) fn matches(&self, path: &Path) -> bool {
        Self::capture(path)
            .as_ref()
            .is_ok_and(|current| current == self)
    }
}

#[derive(PartialEq, Eq)]
enum TreeEntry {
    Directory(FileIdentity),
    File(FileState),
}

/// Capture names, identities and regular-file versions before destructive
/// fallback deletion. Concurrent mutation after the final check is still possible.
#[derive(PartialEq, Eq)]
pub(crate) struct TreeSnapshot(BTreeMap<PathBuf, TreeEntry>);

impl TreeSnapshot {
    pub(crate) fn capture(root: &Path) -> io::Result<Self> {
        Self::capture_with_check(root, || Ok(()))
    }

    pub(crate) fn capture_with_check(
        root: &Path,
        mut check: impl FnMut() -> io::Result<()>,
    ) -> io::Result<Self> {
        fn visit(
            root: &Path,
            path: &Path,
            entries: &mut BTreeMap<PathBuf, TreeEntry>,
            check: &mut impl FnMut() -> io::Result<()>,
        ) -> io::Result<()> {
            check()?;
            let meta = fs::symlink_metadata(path)?;
            if meta.file_type().is_symlink() || (!meta.is_file() && !meta.is_dir()) {
                return Err(io::Error::new(
                    io::ErrorKind::Unsupported,
                    "Cannot snapshot a symlink or special input",
                ));
            }
            let entry = if meta.is_dir() {
                TreeEntry::Directory(
                    FileIdentity::capture(path)
                        .ok_or_else(|| io::Error::other("Cannot verify directory identity"))?,
                )
            } else {
                TreeEntry::File(FileState::capture(path)?)
            };
            entries.insert(path.strip_prefix(root).unwrap().to_path_buf(), entry);
            if meta.is_dir() {
                for child in fs::read_dir(path)? {
                    visit(root, &child?.path(), entries, check)?;
                }
            }
            Ok(())
        }
        let mut entries = BTreeMap::new();
        visit(root, root, &mut entries, &mut check)?;
        Ok(Self(entries))
    }

    pub(crate) fn verify(&self, root: &Path) -> io::Result<()> {
        self.verify_with_check(root, || Ok(()))
    }

    pub(crate) fn verify_with_check(
        &self,
        root: &Path,
        check: impl FnMut() -> io::Result<()>,
    ) -> io::Result<()> {
        if &Self::capture_with_check(root, check)? == self {
            Ok(())
        } else {
            Err(io::Error::other(
                "Source changed during copy; source and destination retained",
            ))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture(label: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "browsey-tree-state-{label}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(root.join("deep")).unwrap();
        fs::write(root.join("deep/file.bin"), b"first").unwrap();
        root
    }

    #[test]
    fn tree_snapshot_detects_added_removed_and_renamed_children() {
        for change in ["add", "remove", "rename"] {
            let root = fixture(change);
            let snapshot = TreeSnapshot::capture(&root).unwrap();
            snapshot.verify(&root).unwrap();
            match change {
                "add" => fs::write(root.join("deep/new.bin"), b"new").unwrap(),
                "remove" => fs::remove_file(root.join("deep/file.bin")).unwrap(),
                _ => fs::rename(root.join("deep/file.bin"), root.join("deep/renamed.bin")).unwrap(),
            }
            let result = snapshot.verify(&root);
            fs::remove_dir_all(root).unwrap();
            assert!(result.is_err());
        }
    }

    #[test]
    fn tree_snapshot_detects_same_size_edits_with_changed_timestamp() {
        let root = fixture("same-size");
        let path = root.join("deep/file.bin");
        let file = File::options().write(true).open(&path).unwrap();
        file.set_times(
            fs::FileTimes::new()
                .set_modified(SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(100)),
        )
        .unwrap();
        let snapshot = TreeSnapshot::capture(&root).unwrap();
        fs::write(&path, b"other").unwrap();
        file.set_times(
            fs::FileTimes::new()
                .set_modified(SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(200)),
        )
        .unwrap();
        let result = snapshot.verify(&root);
        drop(file);
        fs::remove_dir_all(root).unwrap();
        assert!(result.is_err(), "length alone is not sufficient");
    }

    #[test]
    fn tree_snapshot_cancellation_stops_scanning_without_mutation() {
        let root = fixture("cancel");
        let mut checks = 0;
        let result = TreeSnapshot::capture_with_check(&root, || {
            checks += 1;
            if checks == 2 {
                Err(io::ErrorKind::Interrupted.into())
            } else {
                Ok(())
            }
        });
        let data = fs::read(root.join("deep/file.bin")).unwrap();
        fs::remove_dir_all(root).unwrap();
        assert_eq!(result.err().unwrap().kind(), io::ErrorKind::Interrupted);
        assert_eq!(checks, 2);
        assert_eq!(data, b"first");
    }
}
