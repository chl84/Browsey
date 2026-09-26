use super::{check_cancel, EXTRACT_TOTAL_BYTES_CAP, EXTRACT_TOTAL_ENTRIES_CAP};
use crate::commands::decompress::error::{DecompressError, DecompressResult};
use std::{
    io::{self, Read},
    sync::atomic::AtomicBool,
};

/// Preflight has its own byte budget; it must not spend the output budget twice.
#[derive(Clone, Copy)]
pub(crate) struct ScanControl<'a> {
    pub(crate) cancel: Option<&'a AtomicBool>,
    pub(crate) max_bytes: u64,
    pub(crate) password: Option<&'a str>,
}

impl Default for ScanControl<'_> {
    fn default() -> Self {
        Self {
            cancel: None,
            max_bytes: EXTRACT_TOTAL_BYTES_CAP,
            password: None,
        }
    }
}

impl<'a> ScanControl<'a> {
    pub(crate) fn check(self) -> DecompressResult<()> {
        check_cancel(self.cancel).map_err(|e| {
            DecompressError::from_external_message(format!("Extraction cancelled: {e}"))
        })
    }

    pub(crate) fn reader<R: Read>(self, inner: R) -> ScanReader<'a, R> {
        ScanReader {
            inner,
            control: self,
            read: 0,
        }
    }
}

pub(crate) struct ScanReader<'a, R> {
    inner: R,
    control: ScanControl<'a>,
    read: u64,
}

impl<R: Read> Read for ScanReader<'_, R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        // Read::read_exact/io::copy retry Interrupted indefinitely. Cancellation
        // inside a decoder must instead be a terminal error with a clear message.
        check_cancel(self.control.cancel).map_err(|_| io::Error::other("Extraction cancelled"))?;
        if buf.is_empty() {
            return Ok(0);
        }
        // Include TAR headers/padding, with a fixed upper bound even for
        // skipped entries and extension records consumed by the TAR parser.
        let cap = self
            .control
            .max_bytes
            .saturating_add(EXTRACT_TOTAL_ENTRIES_CAP * 1024);
        let limit =
            (cap.saturating_sub(self.read).saturating_add(1)).min(buf.len() as u64) as usize;
        let n = self.inner.read(&mut buf[..limit])?;
        self.read = self.read.saturating_add(n as u64);
        if self.read > cap {
            return Err(io::Error::other(
                "Archive preflight exceeds extraction size cap",
            ));
        }
        check_cancel(self.control.cancel).map_err(|_| io::Error::other("Extraction cancelled"))?;
        Ok(n)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::Ordering;

    #[test]
    fn scan_cancellation_is_terminal_even_for_read_exact() {
        let cancel = AtomicBool::new(true);
        let mut reader = ScanControl {
            cancel: Some(&cancel),
            max_bytes: 8,
            ..ScanControl::default()
        }
        .reader(io::repeat(0));
        let error = reader.read_exact(&mut [0; 8]).unwrap_err();
        assert_ne!(error.kind(), io::ErrorKind::Interrupted);
        assert!(error.to_string().contains("cancelled"));
        cancel.store(false, Ordering::Relaxed);
        reader.read_exact(&mut [0; 8]).unwrap();
        cancel.store(true, Ordering::Relaxed);
        assert!(reader.read(&mut [0; 8]).is_err());
    }

    #[test]
    fn decoded_scan_bytes_are_bounded_even_for_skipped_content() {
        let control = ScanControl {
            cancel: None,
            max_bytes: 8,
            ..ScanControl::default()
        };
        let mut reader = control.reader(io::repeat(0));
        reader.read = control.max_bytes + EXTRACT_TOTAL_ENTRIES_CAP * 1024;
        assert!(reader
            .read(&mut [0; 8])
            .unwrap_err()
            .to_string()
            .contains("size cap"));
    }
}
