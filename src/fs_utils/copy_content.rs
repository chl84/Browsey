//! Compare delivered bytes with the actual written stream, not metadata alone.
//! This is a bounded readback through the existing handle, not a transaction.
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::Path;

use super::FileState;

pub(crate) struct DigestWriter<W> {
    inner: W,
    hash: blake3::Hasher,
}

impl<W> DigestWriter<W> {
    pub(crate) fn new(inner: W) -> Self {
        Self {
            inner,
            hash: blake3::Hasher::new(),
        }
    }

    pub(crate) fn digest(&self) -> blake3::Hash {
        self.hash.finalize()
    }
}

impl<W: Write> Write for DigestWriter<W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let written = self.inner.write(bytes)?;
        self.hash.update(&bytes[..written]);
        Ok(written)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

pub(crate) fn verify_copy_content(
    file: &File,
    path: &Path,
    state: &FileState,
    copied: u64,
    expected: &blake3::Hash,
    mut check: impl FnMut() -> io::Result<()>,
) -> io::Result<()> {
    check()?;
    state.verify_copied_file(file, path, copied)?;
    let mut reader = file;
    reader.seek(SeekFrom::Start(0))?;
    let mut buf = vec![0_u8; 256 * 1024];
    let mut hash = blake3::Hasher::new();
    let mut read = 0_u64;
    loop {
        check()?;
        #[cfg(test)]
        super::copy_test_hooks::hit(path, path, super::copy_test_hooks::Phase::Readback, read)?;
        check()?;
        // One byte beyond the expected length detects growth without an
        // unbounded scan of a concurrently growing file.
        let len = copied
            .saturating_sub(read)
            .saturating_add(1)
            .min(buf.len() as u64) as usize;
        let n = match reader.read(&mut buf[..len]) {
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            result => result?,
        };
        if n == 0 {
            break;
        }
        read = read
            .checked_add(n as u64)
            .ok_or_else(|| io::Error::other("Readback length overflow"))?;
        if read > copied {
            return Err(io::Error::other(
                "Copied content length changed during verification",
            ));
        }
        hash.update(&buf[..n]);
    }
    check()?;
    state.verify_copied_file(file, path, copied)?;
    if read != copied || hash.finalize() != *expected {
        return Err(io::Error::other(
            "Copied content does not match the written stream",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn digest_writer_hashes_only_successful_short_writes() {
        struct ShortWriter(Vec<u8>);
        impl Write for ShortWriter {
            fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
                if self.0.len() == 4 {
                    return Err(io::Error::other("injected write failure"));
                }
                let n = bytes.len().min(2);
                self.0.extend_from_slice(&bytes[..n]);
                Ok(n)
            }
            fn flush(&mut self) -> io::Result<()> {
                Err(io::Error::other("injected flush failure"))
            }
        }
        let mut writer = DigestWriter::new(ShortWriter(Vec::new()));
        assert!(writer.write_all(b"abcdef").is_err());
        assert_eq!(writer.inner.0, b"abcd");
        assert_eq!(writer.digest(), blake3::hash(b"abcd"));
        assert!(writer.flush().is_err());
    }

    #[test]
    fn content_verification_handles_empty_short_and_multichunk_files() {
        let root = std::env::temp_dir().join(format!(
            "browsey-content-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&root).unwrap();
        for size in [0, 1, 256 * 1024 + 17] {
            let path = root.join(size.to_string());
            let mut file = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .create_new(true)
                .open(&path)
                .unwrap();
            let bytes = vec![0x52; size];
            file.write_all(&bytes).unwrap();
            file.sync_all().unwrap();
            let state = FileState::from_file(&file).unwrap();
            verify_copy_content(
                &file,
                &path,
                &state,
                size as u64,
                &blake3::hash(&bytes),
                || Ok(()),
            )
            .unwrap();
            let error = verify_copy_content(
                &file,
                &path,
                &state,
                size as u64,
                &blake3::hash(b"incorrect expected contents"),
                || Ok(()),
            )
            .unwrap_err();
            assert!(error.to_string().contains("does not match"));
            assert_eq!(std::fs::read(path).unwrap(), bytes);
        }
        std::fs::remove_dir_all(root).unwrap();
    }
}
