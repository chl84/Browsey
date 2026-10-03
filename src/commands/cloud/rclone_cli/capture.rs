//! Drain both pipes during execution; never wait for a pipe-blocked child first.
use std::io::{self, Read};
use std::process::Child;
use std::thread::{self, JoinHandle};

pub(super) const STDOUT_LIMIT: usize = 128 * 1024 * 1024;
pub(super) const STDERR_LIMIT: usize = 8 * 1024 * 1024;

pub(super) struct Captured {
    pub bytes: Vec<u8>,
    pub overflow: bool,
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
        let retained = count.min(limit.saturating_sub(result.bytes.len()));
        result.bytes.extend_from_slice(&buffer[..retained]);
        result.overflow |= retained < count;
        // Continue draining after the memory limit, so the child can exit.
    }
}

fn reader(
    source: impl Read + Send + 'static,
    limit: usize,
) -> io::Result<JoinHandle<io::Result<Captured>>> {
    thread::Builder::new()
        .name("browsey-rclone-capture".into())
        .spawn(move || read_bounded(source, limit))
}

pub(super) struct Pipes {
    stdout: JoinHandle<io::Result<Captured>>,
    stderr: JoinHandle<io::Result<Captured>>,
}

impl Pipes {
    pub fn start(child: &mut Child) -> io::Result<Self> {
        let stdout = reader(
            child
                .stdout
                .take()
                .ok_or_else(|| io::Error::other("missing rclone stdout pipe"))?,
            STDOUT_LIMIT,
        )?;
        let stderr = child
            .stderr
            .take()
            .ok_or_else(|| io::Error::other("missing rclone stderr pipe"))
            .and_then(|pipe| reader(pipe, STDERR_LIMIT));
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
