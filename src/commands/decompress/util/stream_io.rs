use super::{DecompressResult, ExtractBudget, CHUNK};
use crate::runtime_lifecycle;
use serde::Serialize;
use std::{
    fs::File,
    io::{self, BufReader, Read, Write},
    path::Path,
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc,
    },
    time::{SystemTime, UNIX_EPOCH},
};

const EXTRACT_CANCEL_CHECK_INTERVAL_BYTES: u64 = 16 * 1024 * 1024; // 16 MiB

#[derive(Serialize, Clone, Copy)]
struct ExtractProgressPayload {
    bytes: u64,
    total: u64,
    finished: bool,
}

#[derive(Clone)]
pub(crate) struct ProgressEmitter {
    app: tauri::AppHandle,
    event: String,
    total: u64,
    done: Arc<AtomicU64>,
    last_emit: Arc<AtomicU64>,
    last_emit_time_ms: Arc<AtomicU64>,
}

impl ProgressEmitter {
    pub(crate) fn new(app: tauri::AppHandle, event: String, total: u64) -> Self {
        Self {
            app,
            event,
            total,
            done: Arc::new(AtomicU64::new(0)),
            last_emit: Arc::new(AtomicU64::new(0)),
            last_emit_time_ms: Arc::new(AtomicU64::new(0)),
        }
    }

    pub(crate) fn add(&self, delta: u64) {
        let done = self
            .done
            .fetch_add(delta, Ordering::Relaxed)
            .saturating_add(delta);
        let last = self.last_emit.load(Ordering::Relaxed);
        let now_ms = current_millis();
        let last_time = self.last_emit_time_ms.load(Ordering::Relaxed);
        if done != last
            && now_ms.saturating_sub(last_time) >= 1000
            && self
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
            let _ = runtime_lifecycle::emit_if_running(
                &self.app,
                &self.event,
                ExtractProgressPayload {
                    bytes: done,
                    total: self.total,
                    finished: false,
                },
            );
        }
    }

    pub(crate) fn finish(&self) {
        let done = self.done.load(Ordering::Relaxed);
        self.last_emit.store(done, Ordering::Relaxed);
        self.last_emit_time_ms
            .store(current_millis(), Ordering::Relaxed);
        let _ = runtime_lifecycle::emit_if_running(
            &self.app,
            &self.event,
            ExtractProgressPayload {
                bytes: done,
                total: self.total,
                finished: true,
            },
        );
    }
}

pub(crate) fn is_cancelled(cancel: Option<&AtomicBool>) -> bool {
    cancel.map(|c| c.load(Ordering::Relaxed)).unwrap_or(false)
}

pub(crate) fn check_cancel(cancel: Option<&AtomicBool>) -> io::Result<()> {
    if is_cancelled(cancel) {
        Err(io::Error::new(io::ErrorKind::Interrupted, "cancelled"))
    } else {
        Ok(())
    }
}

pub(crate) fn map_copy_err(context: &str, err: io::Error) -> String {
    if err.kind() == io::ErrorKind::Interrupted {
        "Extraction cancelled".into()
    } else {
        format!("{context}: {err}")
    }
}

pub(crate) fn map_io(action: &'static str) -> impl FnOnce(io::Error) -> String {
    move |e| format!("Failed to {action}: {e}")
}

pub(crate) fn copy_with_progress<R: Read, W: Write>(
    mut reader: R,
    mut writer: W,
    progress: Option<&ProgressEmitter>,
    cancel: Option<&AtomicBool>,
    budget: &ExtractBudget,
    buf: &mut [u8],
) -> io::Result<u64> {
    let mut written: u64 = 0;
    let mut since_cancel_check = EXTRACT_CANCEL_CHECK_INTERVAL_BYTES;
    loop {
        if since_cancel_check >= EXTRACT_CANCEL_CHECK_INTERVAL_BYTES {
            check_cancel(cancel)?;
            since_cancel_check = 0;
        }
        let n = reader.read(buf)?;
        if n == 0 {
            break;
        }
        budget.reserve_bytes(n as u64)?;
        writer.write_all(&buf[..n])?;
        since_cancel_check = since_cancel_check.saturating_add(n as u64);
        written = written.saturating_add(n as u64);
        if let Some(p) = progress {
            p.add(n as u64);
        }
    }
    Ok(written)
}

pub(crate) fn open_buffered_file(
    path: &Path,
    action: &'static str,
) -> DecompressResult<BufReader<File>> {
    let file = File::open(path).map_err(map_io(action))?;
    Ok(BufReader::with_capacity(CHUNK, file))
}

pub(crate) fn current_millis() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}
