//! Drain both pipes during execution; never wait for a pipe-blocked child first.
use super::progress::TransferActivity;
use std::io::{self, Read};
use std::process::Child;
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::Instant;

pub(super) const STDOUT_LIMIT: usize = 128 * 1024 * 1024;
pub(super) const STDERR_LIMIT: usize = 8 * 1024 * 1024;

pub(super) struct Captured {
    pub bytes: Vec<u8>,
    pub overflow: bool,
}

impl Captured {
    fn append(&mut self, bytes: &[u8], limit: usize) {
        let retained = bytes.len().min(limit.saturating_sub(self.bytes.len()));
        self.bytes.extend_from_slice(&bytes[..retained]);
        self.overflow |= retained < bytes.len();
    }
}

fn read_bounded(mut source: impl Read, limit: usize) -> io::Result<Captured> {
    let mut result = Captured {
        bytes: Vec::new(),
        overflow: false,
    };
    let mut buffer = [0; 8192];
    loop {
        let count = match source.read(&mut buffer) {
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            value => value?,
        };
        if count == 0 {
            return Ok(result);
        }
        result.append(&buffer[..count], limit);
        // Continue draining after the memory limit, so the child can exit.
    }
}

fn read_transfer_stderr(
    mut source: impl Read,
    limit: usize,
    activity: &Mutex<TransferActivity>,
) -> io::Result<Captured> {
    const LINE_LIMIT: usize = 64 * 1024;
    let mut result = Captured {
        bytes: Vec::new(),
        overflow: false,
    };
    let mut buffer = [0; 8192];
    let mut line = Vec::new();
    let mut oversized = false;
    let process_line = |line: &[u8], result: &mut Captured| {
        if let Ok(record) = serde_json::from_slice::<serde_json::Value>(line) {
            if let Some(stats) = record.get("stats").filter(|stats| stats.is_object()) {
                activity
                    .lock()
                    .unwrap_or_else(|poisoned| poisoned.into_inner())
                    .observe(stats, Instant::now());
                // Do not retain periodic stats: a long successful transfer must
                // not overflow the diagnostic memory cap. Preserve error logs.
                if matches!(
                    record.get("level").and_then(serde_json::Value::as_str),
                    Some("notice" | "info")
                ) {
                    return;
                }
            }
        }
        result.append(line, limit);
    };
    loop {
        let count = match source.read(&mut buffer) {
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            value => value?,
        };
        if count == 0 {
            if !line.is_empty() {
                process_line(&line, &mut result);
            }
            return Ok(result);
        }
        for part in buffer[..count].split_inclusive(|byte| *byte == b'\n') {
            if !oversized && line.len() + part.len() > LINE_LIMIT {
                result.append(&line, limit);
                line.clear();
                oversized = true;
            }
            if oversized {
                result.append(part, limit);
            } else {
                line.extend_from_slice(part);
            }
            if part.last() == Some(&b'\n') {
                if !oversized {
                    process_line(&line, &mut result);
                    line.clear();
                }
                oversized = false;
            }
        }
    }
}

fn reader(
    source: impl Read + Send + 'static,
    limit: usize,
    activity: Option<Arc<Mutex<TransferActivity>>>,
) -> io::Result<JoinHandle<io::Result<Captured>>> {
    thread::Builder::new()
        .name("browsey-rclone-capture".into())
        .spawn(move || match activity {
            Some(activity) => read_transfer_stderr(source, limit, &activity),
            None => read_bounded(source, limit),
        })
}

pub(super) struct Pipes {
    stdout: JoinHandle<io::Result<Captured>>,
    stderr: JoinHandle<io::Result<Captured>>,
}

impl Pipes {
    pub fn start(
        child: &mut Child,
        activity: Option<Arc<Mutex<TransferActivity>>>,
    ) -> io::Result<Self> {
        let stdout = reader(
            child
                .stdout
                .take()
                .ok_or_else(|| io::Error::other("missing rclone stdout pipe"))?,
            STDOUT_LIMIT,
            None,
        )?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| io::Error::other("missing rclone stderr pipe"))
            .and_then(|pipe| reader(pipe, STDERR_LIMIT, activity));
        match stderr {
            Ok(stderr) => Ok(Self { stdout, stderr }),
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                let _ = stdout.join();
                Err(error)
            }
        }
    }

    pub fn finish(self) -> io::Result<(Captured, Captured)> {
        // Join both even if one failed; do not leave a detached reader behind.
        let stdout = self.stdout.join();
        let stderr = self.stderr.join();
        let joined = |value: thread::Result<io::Result<Captured>>| {
            value.map_err(|_| io::Error::other("rclone capture worker panicked"))?
        };
        Ok((joined(stdout)?, joined(stderr)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn transfer_stats_are_drained_without_losing_failure_diagnostics() {
        let activity = Mutex::new(TransferActivity::new(
            Instant::now() - std::time::Duration::from_secs(600),
        ));
        let stats = b"{\"level\":\"notice\",\"stats\":{\"bytes\":1024,\"transfers\":1}}\n";
        let error = b"{\"level\":\"error\",\"msg\":\"quota exceeded\"}\n";
        let mut input = stats.repeat(200_000);
        input.extend_from_slice(error);
        let result = read_transfer_stderr(io::Cursor::new(input), 1024, &activity).unwrap();
        assert!(!result.overflow);
        assert_eq!(result.bytes, error);
        assert!(
            activity.lock().unwrap().idle_for(Instant::now()) < std::time::Duration::from_secs(30)
        );
    }

    #[test]
    fn oversized_or_malformed_lines_stay_bounded_and_do_not_mask_later_stats() {
        let activity = Mutex::new(TransferActivity::new(
            Instant::now() - std::time::Duration::from_secs(600),
        ));
        let mut input = vec![b'x'; 100_000];
        input
            .extend_from_slice(b"\nnot json\n{\"level\":\"notice\",\"stats\":{\"transfers\":1}}\n");
        let mut source = io::Cursor::new(input);
        let result = read_transfer_stderr(&mut source, 1024, &activity).unwrap();
        assert!(result.overflow);
        assert_eq!(result.bytes, vec![b'x'; 1024]);
        assert_eq!(source.position(), source.get_ref().len() as u64);
        assert!(
            activity.lock().unwrap().idle_for(Instant::now()) < std::time::Duration::from_secs(30)
        );
    }
    #[test]
    fn capture_limits_memory_but_drains_remaining_bytes() {
        let mut source = io::Cursor::new(vec![b'x'; 100_000]);
        let result = read_bounded(&mut source, 1024).unwrap();
        assert!(result.overflow);
        assert_eq!(result.bytes, vec![b'x'; 1024]);
        assert_eq!(source.position(), 100_000);
        let exact = read_bounded(io::Cursor::new(b"data"), 4).unwrap();
        assert!(!exact.overflow);
        assert_eq!(exact.bytes, b"data");
    }
}
