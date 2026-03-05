use std::{
    fs,
    path::PathBuf,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};

use crate::fs_utils::debug_log;

mod budget;
mod path_ops;
mod stream_io;

pub(super) const CHUNK: usize = 4 * 1024 * 1024;
pub(super) const EXTRACT_TOTAL_BYTES_CAP: u64 = 100_000_000_000; // 100 GB
pub(super) const EXTRACT_TOTAL_ENTRIES_CAP: u64 = 2_000_000; // 2 million entries
pub(super) const EXTRACT_MIN_FREE_DISK_RESERVE: u64 = 1_073_741_824; // 1 GiB
pub(super) const EXTRACT_DISK_CHECK_INTERVAL_BYTES: u64 = 256 * 1024 * 1024; // 256 MiB

pub(super) use budget::{
    available_disk_bytes, effective_extract_bytes_cap, DiskSpaceGuard, ExtractBudget,
};
pub(super) use path_ops::{
    clean_relative_path, create_unique_dir_nofollow, ensure_dir_nofollow, first_component,
    open_unique_file, path_exists_nofollow, strip_known_suffixes,
};
pub(super) use stream_io::{
    check_cancel, copy_with_progress, is_cancelled, map_copy_err, map_io, open_buffered_file,
    ProgressEmitter,
};

#[derive(Default, Clone)]
pub(super) struct SkipStats {
    pub(super) symlinks: Arc<AtomicUsize>,
    pub(super) unsupported: Arc<AtomicUsize>,
}

impl SkipStats {
    pub(super) fn skip_symlink(&self, path: &str) {
        self.symlinks.fetch_add(1, Ordering::Relaxed);
        debug_log(&format!("Skipping symlink entry while extracting: {path}"));
    }

    pub(super) fn skip_unsupported(&self, path: &str, reason: &str) {
        self.unsupported.fetch_add(1, Ordering::Relaxed);
        debug_log(&format!("Skipping unsupported entry {path}: {reason}"));
    }
}

pub(super) struct CreatedPaths {
    pub(super) files: Vec<PathBuf>,
    pub(super) dirs: Vec<PathBuf>,
    active: bool,
}

impl Default for CreatedPaths {
    fn default() -> Self {
        Self {
            files: Vec::new(),
            dirs: Vec::new(),
            active: true,
        }
    }
}

impl CreatedPaths {
    pub(super) fn record_file(&mut self, path: PathBuf) {
        self.files.push(path);
    }

    pub(super) fn record_dir(&mut self, path: PathBuf) {
        self.dirs.push(path);
    }

    pub(super) fn disarm(&mut self) {
        self.active = false;
    }
}

impl Drop for CreatedPaths {
    fn drop(&mut self) {
        if !self.active {
            return;
        }
        // Remove files first, then dirs in reverse to clean up partially extracted content.
        for file in self.files.iter().rev() {
            let _ = fs::remove_file(file);
        }
        for dir in self.dirs.iter().rev() {
            let _ = fs::remove_dir_all(dir);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{copy_with_progress, open_unique_file, CreatedPaths, ExtractBudget, CHUNK};
    use crate::errors::domain::DomainError;
    use std::fs;
    use std::io::{self, Read};
    use std::path::PathBuf;
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_dir(label: &str) -> PathBuf {
        let unique = format!(
            "browsey-decompress-util-{label}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("time")
                .as_nanos()
        );
        let path = std::env::temp_dir().join(unique);
        fs::create_dir_all(&path).expect("create temp dir");
        path
    }

    struct CancelAfterReads {
        remaining: usize,
        chunk_size: usize,
        reads: usize,
        cancel_after_reads: usize,
        cancel: Arc<AtomicBool>,
    }

    impl Read for CancelAfterReads {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            if self.remaining == 0 {
                return Ok(0);
            }
            let n = self.remaining.min(self.chunk_size).min(buf.len());
            for byte in &mut buf[..n] {
                *byte = b'a';
            }
            self.remaining -= n;
            self.reads = self.reads.saturating_add(1);
            if self.reads == self.cancel_after_reads {
                self.cancel.store(true, Ordering::Relaxed);
            }
            Ok(n)
        }
    }

    struct DisappearingSourceReader {
        remaining: usize,
        chunk_size: usize,
        reads: usize,
        fail_after_reads: usize,
    }

    impl Read for DisappearingSourceReader {
        fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
            if self.reads >= self.fail_after_reads {
                return Err(io::Error::new(
                    io::ErrorKind::NotFound,
                    "source disappeared during extraction",
                ));
            }
            if self.remaining == 0 {
                return Ok(0);
            }
            let n = self.remaining.min(self.chunk_size).min(buf.len());
            for byte in &mut buf[..n] {
                *byte = b'b';
            }
            self.remaining -= n;
            self.reads = self.reads.saturating_add(1);
            Ok(n)
        }
    }

    struct DestinationUnavailableWriter {
        writes: usize,
        fail_on_write: usize,
        data: Vec<u8>,
    }

    impl io::Write for DestinationUnavailableWriter {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            if self.writes >= self.fail_on_write {
                return Err(io::Error::new(
                    io::ErrorKind::BrokenPipe,
                    "destination unavailable during extraction",
                ));
            }
            self.writes = self.writes.saturating_add(1);
            self.data.extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn open_unique_file_uses_suffix_when_destination_already_exists() {
        let root = unique_temp_dir("open-unique-file");
        let target = root.join("extracted");
        fs::write(&target, b"existing").expect("seed existing file");

        let (file, chosen_path) = open_unique_file(&target).expect("open unique file");
        drop(file);

        assert_eq!(
            chosen_path.file_name().and_then(|name| name.to_str()),
            Some("extracted-1")
        );
        assert_eq!(
            fs::read_to_string(&target).expect("read original"),
            "existing",
            "existing destination must remain untouched"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[cfg(unix)]
    #[test]
    fn open_unique_file_reports_permission_denied_in_read_only_directory() {
        if unsafe { libc::geteuid() } == 0 {
            return;
        }

        use std::os::unix::fs::PermissionsExt;

        let root = unique_temp_dir("open-unique-perms");
        let locked_dir = root.join("locked");
        fs::create_dir_all(&locked_dir).expect("create locked dir");
        let mut perms = fs::metadata(&locked_dir)
            .expect("locked dir metadata")
            .permissions();
        perms.set_mode(0o555);
        fs::set_permissions(&locked_dir, perms).expect("set locked perms");

        let err = open_unique_file(&locked_dir.join("file.txt"))
            .expect_err("read-only directory should reject destination creation");
        assert_eq!(err.code_str(), "permission_denied");

        let mut restore = fs::metadata(&locked_dir)
            .expect("locked dir metadata")
            .permissions();
        restore.set_mode(0o755);
        fs::set_permissions(&locked_dir, restore).expect("restore perms");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn copy_with_progress_stops_when_cancel_is_triggered_during_large_copy() {
        let cancel = Arc::new(AtomicBool::new(false));
        let mut reader = CancelAfterReads {
            remaining: CHUNK * 5,
            chunk_size: CHUNK,
            reads: 0,
            cancel_after_reads: 2,
            cancel: cancel.clone(),
        };
        let mut writer = Vec::<u8>::new();
        let budget = ExtractBudget::new((CHUNK * 10) as u64, 16);
        let mut buf = vec![0u8; CHUNK];

        let err = copy_with_progress(
            &mut reader,
            &mut writer,
            None,
            Some(cancel.as_ref()),
            &budget,
            &mut buf,
        )
        .expect_err("copy should stop once cancellation is observed");

        assert_eq!(err.kind(), io::ErrorKind::Interrupted);
        assert_eq!(
            writer.len(),
            CHUNK * 4,
            "copy should stop after the first cancellation check boundary"
        );
    }

    #[test]
    fn created_paths_drop_rolls_back_partial_outputs_unless_disarmed() {
        let root = unique_temp_dir("created-paths");

        let rollback_dir = root.join("rollback");
        fs::create_dir_all(&rollback_dir).expect("create rollback dir");
        let rollback_file = rollback_dir.join("file.txt");
        fs::write(&rollback_file, b"payload").expect("write rollback file");
        {
            let mut created = CreatedPaths::default();
            created.record_file(rollback_file.clone());
            created.record_dir(rollback_dir.clone());
        }
        assert!(
            !rollback_file.exists() && !rollback_dir.exists(),
            "drop rollback should remove recorded partial outputs"
        );

        let keep_dir = root.join("keep");
        fs::create_dir_all(&keep_dir).expect("create keep dir");
        let keep_file = keep_dir.join("file.txt");
        fs::write(&keep_file, b"payload").expect("write keep file");
        {
            let mut created = CreatedPaths::default();
            created.record_file(keep_file.clone());
            created.record_dir(keep_dir.clone());
            created.disarm();
        }
        assert!(
            keep_file.exists() && keep_dir.exists(),
            "disarmed created paths should preserve outputs"
        );
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn copy_with_progress_surfaces_source_disappeared_error() {
        let mut reader = DisappearingSourceReader {
            remaining: CHUNK * 2,
            chunk_size: CHUNK,
            reads: 0,
            fail_after_reads: 1,
        };
        let mut writer = Vec::<u8>::new();
        let budget = ExtractBudget::new((CHUNK * 4) as u64, 16);
        let mut buf = vec![0u8; CHUNK];

        let err = copy_with_progress(&mut reader, &mut writer, None, None, &budget, &mut buf)
            .expect_err("source disappearance should fail copy loop");

        assert_eq!(err.kind(), io::ErrorKind::NotFound);
        assert_eq!(writer.len(), CHUNK);
    }

    #[test]
    fn copy_with_progress_surfaces_destination_unavailable_error() {
        let mut reader = io::repeat(b'c').take((CHUNK * 3) as u64);
        let mut writer = DestinationUnavailableWriter {
            writes: 0,
            fail_on_write: 1,
            data: Vec::new(),
        };
        let budget = ExtractBudget::new((CHUNK * 4) as u64, 16);
        let mut buf = vec![0u8; CHUNK];

        let err = copy_with_progress(&mut reader, &mut writer, None, None, &budget, &mut buf)
            .expect_err("destination unavailability should fail copy loop");

        assert_eq!(err.kind(), io::ErrorKind::BrokenPipe);
        assert_eq!(writer.data.len(), CHUNK);
    }
}
