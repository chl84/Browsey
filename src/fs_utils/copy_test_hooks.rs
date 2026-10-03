//! Deterministic, thread-local I/O faults for real disposable copy fixtures.
//! Not compiled into application builds. Hooks must never touch user paths.

use std::cell::RefCell;
use std::fs::File;
use std::io::{self, Read, Write};
use std::ops::Deref;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Phase {
    Rename,
    Read,
    Write,
    Sync,
    Synced,
    BeforeSourceDelete,
    CopyUndoVerified,
    CopyUndoEntry,
    RecoveryMarker,
    RecoveryMarkerSync,
    OverwritePrepared,
    OverwriteBackedUp,
    Readback,
}

type Hook = Box<dyn FnMut(&Path, &Path, Phase, u64) -> io::Result<()>>;

thread_local! {
    static HOOK: RefCell<Option<Hook>> = RefCell::new(None);
}

// A thread-local scope must also be dropped on its installing thread.
pub(crate) struct Scope(std::marker::PhantomData<std::rc::Rc<()>>);

impl Scope {
    pub(crate) fn new(
        hook: impl FnMut(&Path, &Path, Phase, u64) -> io::Result<()> + 'static,
    ) -> Self {
        HOOK.with(|slot| {
            let mut slot = slot.borrow_mut();
            assert!(slot.is_none(), "copy fault scopes must not nest");
            *slot = Some(Box::new(hook));
        });
        Self(std::marker::PhantomData)
    }
}

impl Drop for Scope {
    fn drop(&mut self) {
        HOOK.with(|slot| *slot.borrow_mut() = None);
    }
}

pub(crate) fn hit(src: &Path, dst: &Path, phase: Phase, bytes: u64) -> io::Result<()> {
    HOOK.with(|slot| match slot.borrow_mut().as_mut() {
        Some(hook) => hook(src, dst, phase, bytes),
        None => Ok(()),
    })
}

/// Wrapping the actual open file lets faults reach the existing error handlers.
/// With a scope active, short reads/writes force deterministic mid-stream edges.
pub(crate) struct TestFile {
    file: File,
    src: PathBuf,
    dst: PathBuf,
    bytes: u64,
}

impl TestFile {
    pub(crate) fn new(file: File, src: &Path, dst: &Path) -> Self {
        Self {
            file,
            src: src.into(),
            dst: dst.into(),
            bytes: 0,
        }
    }

    fn chunk_len(&self, len: usize, limit: usize) -> usize {
        HOOK.with(|slot| {
            if slot.borrow().is_some() {
                len.min(limit)
            } else {
                len
            }
        })
    }
}

impl Deref for TestFile {
    type Target = File;

    fn deref(&self) -> &File {
        &self.file
    }
}

impl Read for TestFile {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        hit(&self.src, &self.dst, Phase::Read, self.bytes)?;
        let len = self.chunk_len(buf.len(), 8192);
        let count = self.file.read(&mut buf[..len])?;
        self.bytes += count as u64;
        Ok(count)
    }
}

impl Write for TestFile {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        hit(&self.src, &self.dst, Phase::Write, self.bytes)?;
        let len = self.chunk_len(buf.len(), 4096);
        let count = self.file.write(&buf[..len])?;
        self.bytes += count as u64;
        Ok(count)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.file.flush()
    }
}
