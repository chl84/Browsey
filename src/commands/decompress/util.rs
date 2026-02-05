use std::{
    fs::{self, File},
    io::{self, BufReader, Read, Write},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering},
        Arc,
    },
    time::{SystemTime, UNIX_EPOCH},
};

use serde::Serialize;
use tauri::Emitter;

use crate::fs_utils::{debug_log, unique_path};

pub(super) const CHUNK: usize = 4 * 1024 * 1024;

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

#[derive(Serialize, Clone, Copy)]
struct ExtractProgressPayload {
    bytes: u64,
    total: u64,
    finished: bool,
}

#[derive(Clone)]
pub(super) struct ProgressEmitter {
    app: tauri::AppHandle,
    event: String,
    total: u64,
    done: Arc<AtomicU64>,
    last_emit: Arc<AtomicU64>,
    last_emit_time_ms: Arc<AtomicU64>,
}

impl ProgressEmitter {
    pub(super) fn new(app: tauri::AppHandle, event: String, total: u64) -> Self {
        Self {
            app,
            event,
            total,
            done: Arc::new(AtomicU64::new(0)),
            last_emit: Arc::new(AtomicU64::new(0)),
            last_emit_time_ms: Arc::new(AtomicU64::new(0)),
        }
    }

    pub(super) fn add(&self, delta: u64) {
        let done = self
            .done
            .fetch_add(delta, Ordering::Relaxed)
            .saturating_add(delta);
        let last = self.last_emit.load(Ordering::Relaxed);
        let now_ms = current_millis();
        let last_time = self.last_emit_time_ms.load(Ordering::Relaxed);
        if done != last && now_ms.saturating_sub(last_time) >= 1000 {
            if self
                .last_emit
                .compare_exchange(last, done, Ordering::Relaxed, Ordering::Relaxed)
                .is_ok()
            {
                let _ = self.last_emit_time_ms.compare_exchange(
                    last_time,
                    now_ms,
                    Ordering::Relaxed,
                    Ordering::Relaxed,
                );
                let _ = self.app.emit(
                    &self.event,
                    ExtractProgressPayload {
                        bytes: done,
                        total: self.total,
                        finished: false,
                    },
                );
            }
        }
    }

    pub(super) fn finish(&self) {
        let done = self.done.load(Ordering::Relaxed);
        self.last_emit.store(done, Ordering::Relaxed);
        self.last_emit_time_ms
            .store(current_millis(), Ordering::Relaxed);
        let _ = self.app.emit(
            &self.event,
            ExtractProgressPayload {
                bytes: done,
                total: self.total,
                finished: true,
            },
        );
    }
}

pub(super) fn is_cancelled(cancel: Option<&AtomicBool>) -> bool {
    cancel.map(|c| c.load(Ordering::Relaxed)).unwrap_or(false)
}

pub(super) fn check_cancel(cancel: Option<&AtomicBool>) -> io::Result<()> {
    if is_cancelled(cancel) {
        Err(io::Error::new(io::ErrorKind::Interrupted, "cancelled"))
    } else {
        Ok(())
    }
}

pub(super) fn map_copy_err(context: &str, err: io::Error) -> String {
    if err.kind() == io::ErrorKind::Interrupted {
        "Extraction cancelled".into()
    } else {
        format!("{context}: {err}")
    }
}

pub(super) fn map_io(action: &'static str) -> impl FnOnce(io::Error) -> String {
    move |e| format!("Failed to {action}: {e}")
}

pub(super) fn clean_relative_path(path: &Path) -> Result<PathBuf, String> {
    let mut cleaned = PathBuf::new();
    for comp in path.components() {
        match comp {
            std::path::Component::Normal(p) => cleaned.push(p),
            std::path::Component::CurDir => {}
            _ => return Err("Refusing path with traversal or absolute components".into()),
        }
    }
    Ok(cleaned)
}

pub(super) fn first_component(path: &Path) -> Option<PathBuf> {
    path.components().find_map(|c| match c {
        std::path::Component::Normal(p) => Some(PathBuf::from(p)),
        _ => None,
    })
}

pub(super) fn open_unique_file(dest_path: &Path) -> Result<(File, PathBuf), String> {
    let mut candidate = dest_path.to_path_buf();
    loop {
        match File::options()
            .write(true)
            .create_new(true)
            .open(&candidate)
        {
            Ok(f) => return Ok((f, candidate)),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {
                candidate = unique_path(&candidate);
                continue;
            }
            Err(e) => {
                return Err(format!(
                    "Failed to create file {}: {e}",
                    candidate.display()
                ))
            }
        }
    }
}

pub(super) fn copy_with_progress<R: Read, W: Write>(
    mut reader: R,
    mut writer: W,
    progress: Option<&ProgressEmitter>,
    cancel: Option<&AtomicBool>,
    buf: &mut [u8],
) -> io::Result<u64> {
    let mut written: u64 = 0;
    loop {
        check_cancel(cancel)?;
        let n = reader.read(buf)?;
        if n == 0 {
            break;
        }
        writer.write_all(&buf[..n])?;
        written = written.saturating_add(n as u64);
        if let Some(p) = progress {
            p.add(n as u64);
        }
    }
    Ok(written)
}

pub(super) fn open_buffered_file(path: &Path, action: &'static str) -> Result<BufReader<File>, String> {
    let file = File::open(path).map_err(map_io(action))?;
    Ok(BufReader::with_capacity(CHUNK, file))
}

pub(super) fn strip_known_suffixes(name: &str) -> String {
    let lower = name.to_lowercase();
    for suffix in [
        ".tar.gz", ".tgz", ".tar.bz2", ".tbz2", ".tar.xz", ".txz", ".tar.zst", ".tzst", ".tar",
        ".7z", ".rar",
    ] {
        if lower.ends_with(suffix) && name.len() > suffix.len() {
            return name[..name.len() - suffix.len()].to_string();
        }
    }
    if let Some((stem, _ext)) = name.rsplit_once('.') {
        if !stem.is_empty() {
            return stem.to_string();
        }
    }
    name.to_string()
}

pub(super) fn current_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
