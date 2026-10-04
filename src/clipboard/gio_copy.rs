//! Numeric GIO progress and cancellation, with no CLI parsing or mutation retry.
use gio::prelude::*;
use std::{
    path::Path,
    sync::mpsc,
    time::{Duration, Instant},
};

use super::{
    error::{ClipboardError, ClipboardErrorCode, ClipboardResult},
    CopyProgressPayload,
};

#[cfg(test)]
mod tests;

// The watcher is scoped and joined, including on panic. It cancels blocked GIO
// work even when the backend has stopped calling its progress callback.
fn with_cancellable<T>(
    abort: impl Fn() -> bool + Sync,
    operation: impl FnOnce(&gio::Cancellable) -> T,
) -> T {
    let cancellable = gio::Cancellable::new();
    std::thread::scope(|scope| {
        let (done, stopped) = mpsc::channel();
        let watch_cancel = &cancellable;
        let watch_abort = &abort;
        scope.spawn(move || loop {
            if watch_abort() {
                watch_cancel.cancel();
                break;
            }
            match stopped.recv_timeout(Duration::from_millis(25)) {
                Err(mpsc::RecvTimeoutError::Timeout) => {}
                _ => break,
            }
        });
        let result = operation(&cancellable);
        let _ = done.send(());
        result
    })
}

fn gio_error(error: gio::glib::Error) -> ClipboardError {
    let code = if error.matches(gio::IOErrorEnum::Cancelled) {
        ClipboardErrorCode::Cancelled
    } else if error.matches(gio::IOErrorEnum::Exists) {
        ClipboardErrorCode::DestinationExists
    } else if error.matches(gio::IOErrorEnum::NotFound) {
        ClipboardErrorCode::NotFound
    } else {
        ClipboardErrorCode::IoError
    };
    ClipboardError::new(code, format!("Network copy failed: {error}"))
}

fn file_for_path(path: &Path) -> ClipboardResult<gio::File> {
    // GVFS maps its FUSE path to the mounted URI, preserving account and port.
    let file = gio::File::for_path(path);
    if file.is_native()
        && dirs_next::runtime_dir().is_some_and(|root| path.starts_with(root.join("gvfs")))
    {
        return Err(ClipboardError::new(
            ClipboardErrorCode::NotFound,
            "The network mount is unavailable. Reconnect before copying.",
        ));
    }
    Ok(file)
}

pub(super) fn estimate_size(
    path: &Path,
    abort: impl Fn() -> bool + Sync,
) -> ClipboardResult<Option<u64>> {
    with_cancellable(&abort, |cancel| {
        let mut pending = vec![(file_for_path(path)?, None)];
        let mut total = 0u64;
        while let Some((file, enumerated_info)) = pending.pop() {
            if abort() || cancel.is_cancelled() {
                return Err(ClipboardError::cancelled());
            }
            // Reuse metadata from the directory listing instead of making a
            // separate server request for every file during size preflight.
            let info = match enumerated_info {
                Some(info) => info,
                None => file
                    .query_info(
                        "standard::type,standard::size",
                        gio::FileQueryInfoFlags::NOFOLLOW_SYMLINKS,
                        Some(cancel),
                    )
                    .map_err(gio_error)?,
            };
            match info.file_type() {
                gio::FileType::Regular => {
                    if !info.has_attribute("standard::size") {
                        return Ok(None);
                    }
                    let Some(size) = u64::try_from(info.size())
                        .ok()
                        .and_then(|size| total.checked_add(size))
                    else {
                        return Ok(None);
                    };
                    total = size;
                }
                gio::FileType::Directory => {
                    let children = file
                        .enumerate_children(
                            "standard::name,standard::type,standard::size",
                            gio::FileQueryInfoFlags::NOFOLLOW_SYMLINKS,
                            Some(cancel),
                        )
                        .map_err(gio_error)?;
                    while let Some(child) = children.next_file(Some(cancel)).map_err(gio_error)? {
                        if abort() || cancel.is_cancelled() {
                            return Err(ClipboardError::cancelled());
                        }
                        pending.push((file.child(child.name()), Some(child)));
                    }
                    children.close(Some(cancel)).map_err(gio_error)?;
                }
                _ => return Ok(None),
            }
        }
        Ok(Some(total))
    })
}

pub(super) fn copy(
    src: &Path,
    dest: &Path,
    total_hint: Option<u64>,
    abort: impl Fn() -> bool + Sync,
    mut progress: impl FnMut(CopyProgressPayload),
) -> ClipboardResult<u64> {
    if abort() {
        return Err(ClipboardError::cancelled());
    }
    with_cancellable(&abort, |cancel| {
        if abort() {
            return Err(ClipboardError::cancelled());
        }
        let source = file_for_path(src)?;
        let destination = file_for_path(dest)?;
        let info = source
            .query_info(
                "standard::type,standard::size",
                gio::FileQueryInfoFlags::NOFOLLOW_SYMLINKS,
                Some(cancel),
            )
            .map_err(gio_error)?;
        if info.file_type() != gio::FileType::Regular {
            return Err(ClipboardError::new(
                ClipboardErrorCode::SymlinkUnsupported,
                "Network copy requires a regular file; symlinks and special files are not copied.",
            ));
        }
        let mut total = u64::try_from(info.size()).ok().or(total_hint).unwrap_or(0);
        let mut bytes = 0;
        let mut last_emit = 0u64;
        let mut last_time = Instant::now();
        progress(CopyProgressPayload {
            bytes: 0,
            total,
            finished: false,
        });
        let result = source.copy(
            &destination,
            gio::FileCopyFlags::NOFOLLOW_SYMLINKS,
            Some(cancel),
            Some(&mut |current, expected| {
                if abort() {
                    cancel.cancel();
                }
                bytes = u64::try_from(current).unwrap_or(0);
                if let Ok(expected) = u64::try_from(expected) {
                    total = expected;
                }
                if (last_emit == 0 && bytes > 0)
                    || last_time.elapsed() >= Duration::from_millis(200)
                {
                    progress(CopyProgressPayload {
                        bytes,
                        total,
                        finished: false,
                    });
                    last_emit = bytes;
                    last_time = Instant::now();
                }
            }),
        );
        if let Err(error) = result {
            let error = gio_error(error);
            return Err(if error.code() == ClipboardErrorCode::DestinationExists {
                error
            } else {
                error.with_context(format!("Any incomplete output at {} is retained for inspection; source not removed; no automatic retry", dest.display()))
            });
        }
        if abort() || cancel.is_cancelled() {
            return Err(ClipboardError::cancelled().with_context(format!(
                "Output retained at {}; source not removed",
                dest.display()
            )));
        }
        let copied = destination
            .query_info(
                "standard::type,standard::size",
                gio::FileQueryInfoFlags::NOFOLLOW_SYMLINKS,
                Some(cancel),
            )
            .map_err(|error| {
                gio_error(error).with_context(format!(
                    "Copied output retained at {}; could not verify final metadata",
                    dest.display()
                ))
            })?;
        if abort() || cancel.is_cancelled() {
            return Err(ClipboardError::cancelled().with_context(format!(
                "Output retained at {}; source not removed",
                dest.display()
            )));
        }
        if copied.file_type() != gio::FileType::Regular
            || u64::try_from(copied.size()).ok() != Some(bytes.max(total))
        {
            return Err(ClipboardError::new(
                ClipboardErrorCode::IoError,
                format!(
                    "Copied output changed or is unverifiable; retained at {}; source not removed",
                    dest.display()
                ),
            ));
        }
        // The aggregate reporter consumes this child completion. Only the paste
        // command emits finished=true to the UI after the entire batch succeeds.
        if bytes == 0 {
            bytes = total;
        }
        progress(CopyProgressPayload {
            bytes,
            total,
            finished: true,
        });
        Ok(bytes)
    })
}
