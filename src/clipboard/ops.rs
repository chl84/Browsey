use crate::{
    runtime_lifecycle,
    undo::{move_with_fallback, temp_backup_path, Action},
};
#[cfg(test)]
use std::cell::RefCell;
use std::{
    fs,
    io::{ErrorKind, Read, Write},
    path::Path,
    sync::atomic::AtomicBool,
};

use super::{
    error::{ClipboardError, ClipboardErrorCode, ClipboardResult},
    owned_copy_paths::OwnedCopyPaths,
    progress::CopyProgress,
    ClipboardMode, CopyProgressPayload,
};

#[cfg(test)]
type AfterMergeItemTestHook = Box<dyn FnMut(&Path)>;

#[cfg(test)]
thread_local! {
    static AFTER_MERGE_ITEM_TEST_HOOK: RefCell<Option<AfterMergeItemTestHook>> = RefCell::new(None);
    static BEFORE_MOVE_RENAME_TEST_HOOK: RefCell<Option<Box<dyn FnOnce()>>> = RefCell::new(None);
}

#[cfg(test)]
pub(super) fn set_before_move_rename_test_hook(callback: Option<Box<dyn FnOnce()>>) {
    BEFORE_MOVE_RENAME_TEST_HOOK.with(|hook| *hook.borrow_mut() = callback);
}

#[cfg(test)]
fn run_after_merge_item_test_hook(path: &Path) {
    AFTER_MERGE_ITEM_TEST_HOOK.with(|hook| {
        if let Some(callback) = hook.borrow_mut().as_mut() {
            callback(path);
        }
    });
}

#[cfg(test)]
pub(super) fn set_after_merge_item_test_hook(callback: Option<AfterMergeItemTestHook>) {
    AFTER_MERGE_ITEM_TEST_HOOK.with(|hook| {
        *hook.borrow_mut() = callback;
    });
}

fn ensure_not_child(src: &Path, dest: &Path) -> ClipboardResult<()> {
    if dest.starts_with(src) {
        return Err(ClipboardError::invalid_input(
            "Cannot paste a directory into itself",
        ));
    }
    Ok(())
}

pub(super) fn transfer_cancelled(
    cancel: Option<&AtomicBool>,
    app: Option<&tauri::AppHandle>,
) -> bool {
    cancel
        .map(|c| c.load(std::sync::atomic::Ordering::Relaxed))
        .unwrap_or(false)
        || app
            .map(runtime_lifecycle::is_shutting_down)
            .unwrap_or(false)
}

fn emit_copy_progress(event: Option<&CopyProgress<'_>>, payload: CopyProgressPayload) {
    if let Some(progress) = event {
        progress.report(payload);
    }
}

fn preserve_copy_permissions(
    _src: &Path,
    _dest: &Path,
    file: &fs::File,
    permissions: fs::Permissions,
    context: &str,
) -> ClipboardResult<()> {
    #[cfg(test)]
    let result = crate::fs_utils::copy_test_hooks::hit(
        _src,
        _dest,
        crate::fs_utils::copy_test_hooks::Phase::SetPermissions,
        0,
    )
    .and_then(|()| file.set_permissions(permissions));
    #[cfg(not(test))]
    let result = file.set_permissions(permissions);
    // MTP and other filesystems may have no Unix permission capability. Preserve
    // modes where supported, but do not undo copied bytes for this refusal.
    // Permission denial and all other errors still fail the operation.
    match result {
        Err(error) if error.kind() == ErrorKind::Unsupported => {
            tracing::debug!("Copy destination does not support permission preservation");
            Ok(())
        }
        result => result.map_err(|error| {
            ClipboardError::from_io_error(ClipboardErrorCode::IoError, context, error)
        }),
    }
}

fn copy_dir(
    src: &Path,
    dest: &Path,
    app: Option<&tauri::AppHandle>,
    progress_event: Option<&CopyProgress<'_>>,
    cancel: Option<&AtomicBool>,
    require_receipt: bool,
) -> ClipboardResult<crate::undo::CopyReceipt> {
    let mut outputs = OwnedCopyPaths::default();
    let result = copy_dir_tracked(
        src,
        dest,
        app,
        progress_event,
        cancel,
        &mut outputs,
        require_receipt,
    );
    if let Err(error) = result {
        let retained = outputs.cleanup();
        return Err(if retained.is_empty() {
            error
        } else {
            error.with_context(format!(
                "Copy cleanup retained paths: {}",
                retained.join("; ")
            ))
        });
    }
    Ok(outputs.receipt(dest))
}

fn copy_dir_tracked(
    src: &Path,
    dest: &Path,
    app: Option<&tauri::AppHandle>,
    progress_event: Option<&CopyProgress<'_>>,
    cancel: Option<&AtomicBool>,
    outputs: &mut OwnedCopyPaths,
    require_receipt: bool,
) -> ClipboardResult<()> {
    tracing::info!(
        op = "copy",
        backend = "filesystem-directory",
        kind = "directory",
        staging = "direct",
        "transfer dispatch"
    );
    let source_permissions = fs::metadata(src)
        .map_err(|e| {
            ClipboardError::from_io_error(ClipboardErrorCode::IoError, "Read source permissions", e)
        })?
        .permissions();
    let mut builder = fs::DirBuilder::new();
    builder.recursive(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(dest).map_err(|e| {
        ClipboardError::from_io_error(
            ClipboardErrorCode::IoError,
            &format!("Failed to create dir {}", dest.display()),
            e,
        )
    })?;
    outputs.record_dir(dest);
    for entry in fs::read_dir(src).map_err(|e| {
        ClipboardError::from_io_error(
            ClipboardErrorCode::IoError,
            &format!("Failed to read dir {}", src.display()),
            e,
        )
    })? {
        let entry = entry.map_err(|e| {
            ClipboardError::from_io_error(
                ClipboardErrorCode::IoError,
                "Failed to read dir entry",
                e,
            )
        })?;
        let path = entry.path();
        let meta = fs::symlink_metadata(&path).map_err(|e| {
            ClipboardError::from_io_error(
                ClipboardErrorCode::IoError,
                &format!("Failed to read metadata for {}", path.display()),
                e,
            )
        })?;
        if transfer_cancelled(cancel, app) {
            return Err(ClipboardError::cancelled());
        }
        if meta.file_type().is_symlink() {
            return Err(ClipboardError::new(
                ClipboardErrorCode::SymlinkUnsupported,
                "Refusing to copy symlinks",
            ));
        }
        let target = dest.join(entry.file_name());
        if meta.is_dir() {
            ensure_not_child(&path, &target)?;
            copy_dir_tracked(
                &path,
                &target,
                app,
                progress_event,
                cancel,
                outputs,
                require_receipt,
            )?;
        } else {
            copy_file_tracked(
                &path,
                &target,
                app,
                progress_event,
                cancel,
                None,
                Some(outputs),
                require_receipt,
            )?;
        }
    }
    if !outputs.directory_matches(dest) {
        return Err(ClipboardError::new(
            ClipboardErrorCode::IoError,
            "Copy destination directory changed; retained outputs",
        ));
    }
    let mut directory_options = fs::File::options();
    directory_options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        directory_options.custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        use windows_sys::Win32::Storage::FileSystem::{
            FILE_FLAG_BACKUP_SEMANTICS, FILE_FLAG_OPEN_REPARSE_POINT,
        };
        directory_options.custom_flags(FILE_FLAG_BACKUP_SEMANTICS | FILE_FLAG_OPEN_REPARSE_POINT);
    }
    let directory = directory_options.open(dest).map_err(|error| {
        ClipboardError::from_io_error(ClipboardErrorCode::IoError, "Open copied directory", error)
    })?;
    if !outputs.directory_handle_matches(dest, &directory) {
        return Err(ClipboardError::new(
            ClipboardErrorCode::IoError,
            "Copy destination directory changed; retained outputs",
        ));
    }
    preserve_copy_permissions(
        src,
        dest,
        &directory,
        source_permissions,
        "Set directory permissions",
    )?;
    Ok(())
}

pub(super) fn backup_existing_target(
    target: &Path,
    actions: &mut Vec<Action>,
) -> ClipboardResult<()> {
    let backup = temp_backup_path(target).map_err(ClipboardError::from)?;
    let parent = backup
        .parent()
        .ok_or_else(|| ClipboardError::invalid_input("Invalid backup path"))?;
    fs::create_dir_all(parent).map_err(|e| {
        ClipboardError::from_io_error(
            ClipboardErrorCode::IoError,
            &format!("Failed to create backup parent {}", parent.display()),
            e,
        )
    })?;
    let mut protection = crate::undo::BackupProtection::create(&backup, target)
        .map_err(|error| ClipboardError::from(error).with_context(format!(
            "Overwrite not started; original target not moved. Could not protect backup candidate at {}", backup.display()
        )))?;
    let result = (|| {
        #[cfg(test)]
        crate::fs_utils::copy_test_hooks::hit(
            target,
            &backup,
            crate::fs_utils::copy_test_hooks::Phase::OverwritePrepared,
            0,
        )
        .map_err(|error| {
            crate::undo::UndoError::from_io_error("Prepare overwrite backup", error)
        })?;
        protection.ensure(&backup, target)?;
        move_with_fallback(target, &backup)?;
        #[cfg(test)]
        crate::fs_utils::copy_test_hooks::hit(
            target,
            &backup,
            crate::fs_utils::copy_test_hooks::Phase::OverwriteBackedUp,
            0,
        )
        .map_err(|error| crate::undo::UndoError::from_io_error("Finish overwrite backup", error))?;
        Ok::<_, crate::undo::UndoError>(())
    })();
    result.map_err(|error| ClipboardError::from(error).with_context(format!(
        "Overwrite backup incomplete; protected candidate retained at {}; inspect original target {} before retrying",
        backup.display(), target.display()
    )))?;
    actions.push(Action::Delete {
        path: target.to_path_buf(),
        backup,
        protection: Some(protection),
    });
    Ok(())
}

pub(super) fn merge_dir(
    src: &Path,
    dest: &Path,
    mode: ClipboardMode,
    actions: &mut Vec<Action>,
    app: Option<&tauri::AppHandle>,
    progress_event: Option<&CopyProgress<'_>>,
    cancel: Option<&AtomicBool>,
) -> ClipboardResult<()> {
    ensure_not_child(src, dest)?;
    // Ensure both exist and are directories.
    let src_meta = fs::symlink_metadata(src).map_err(|e| {
        ClipboardError::from_io_error(
            ClipboardErrorCode::IoError,
            &format!("Failed to read source metadata for {}", src.display()),
            e,
        )
    })?;
    let dest_meta = fs::symlink_metadata(dest).map_err(|e| {
        ClipboardError::from_io_error(
            ClipboardErrorCode::IoError,
            &format!("Failed to read target metadata for {}", dest.display()),
            e,
        )
    })?;
    if !src_meta.is_dir() || !dest_meta.is_dir() {
        return Err(ClipboardError::new(
            ClipboardErrorCode::InvalidInput,
            "Merge requires both source and target to be directories",
        ));
    }

    for entry in fs::read_dir(src).map_err(|e| {
        ClipboardError::from_io_error(
            ClipboardErrorCode::IoError,
            &format!("Failed to read dir {}", src.display()),
            e,
        )
    })? {
        let entry = entry.map_err(|e| {
            ClipboardError::from_io_error(
                ClipboardErrorCode::IoError,
                "Failed to read dir entry",
                e,
            )
        })?;
        let path = entry.path();
        let meta = fs::symlink_metadata(&path).map_err(|e| {
            ClipboardError::from_io_error(
                ClipboardErrorCode::IoError,
                &format!("Failed to read metadata for {}", path.display()),
                e,
            )
        })?;
        if meta.file_type().is_symlink() {
            return Err(ClipboardError::new(
                ClipboardErrorCode::SymlinkUnsupported,
                "Refusing to copy symlinks",
            ));
        }
        if transfer_cancelled(cancel, app) {
            return Err(ClipboardError::cancelled());
        }
        let target = dest.join(entry.file_name());
        let target_meta = metadata_if_exists_nofollow(&target)?;
        if meta.is_dir() {
            if matches!(target_meta, Some(ref m) if m.file_type().is_symlink()) {
                return Err(ClipboardError::new(
                    ClipboardErrorCode::SymlinkUnsupported,
                    "Refusing to overwrite symlinks",
                ));
            }
            if matches!(target_meta, Some(ref m) if m.is_dir()) {
                merge_dir(&path, &target, mode, actions, app, progress_event, cancel)?;
            } else {
                if target_meta.is_some() {
                    backup_existing_target(&target, actions)?;
                }
                match mode {
                    ClipboardMode::Copy => {
                        let receipt = copy_dir(&path, &target, app, progress_event, cancel, false)?;
                        actions.push(Action::Copy {
                            from: path.clone(),
                            to: target.clone(),
                            receipt,
                        });
                    }
                    ClipboardMode::Cut => {
                        move_entry(&path, &target, app, progress_event, cancel)?;
                        actions.push(Action::Move {
                            from: path.clone(),
                            to: target.clone(),
                        });
                    }
                }
            }
        } else {
            if matches!(target_meta, Some(ref m) if m.file_type().is_symlink()) {
                return Err(ClipboardError::new(
                    ClipboardErrorCode::SymlinkUnsupported,
                    "Refusing to overwrite symlinks",
                ));
            }
            if target_meta.is_some() {
                backup_existing_target(&target, actions)?;
            }
            match mode {
                ClipboardMode::Copy => {
                    let receipt = copy_entry(&path, &target, app, progress_event, cancel)?;
                    actions.push(Action::Copy {
                        from: path.clone(),
                        to: target.clone(),
                        receipt,
                    });
                }
                ClipboardMode::Cut => {
                    move_entry(&path, &target, app, progress_event, cancel)?;
                    actions.push(Action::Move {
                        from: path.clone(),
                        to: target.clone(),
                    });
                }
            }
        }
        #[cfg(test)]
        run_after_merge_item_test_hook(&path);
    }

    if let ClipboardMode::Cut = mode {
        // Remove source directory but keep an empty backup so undo can recreate it
        // before moving items back.
        let backup = temp_backup_path(src).map_err(ClipboardError::from)?;
        if let Some(parent) = backup.parent() {
            fs::create_dir_all(parent).map_err(|e| {
                ClipboardError::from_io_error(
                    ClipboardErrorCode::IoError,
                    &format!("Failed to create backup parent {}", parent.display()),
                    e,
                )
            })?;
        }
        fs::create_dir_all(&backup).map_err(|e| {
            ClipboardError::from_io_error(
                ClipboardErrorCode::IoError,
                &format!("Failed to create backup dir {}", backup.display()),
                e,
            )
        })?;
        fs::set_permissions(&backup, src_meta.permissions()).map_err(|e| {
            ClipboardError::from_io_error(
                ClipboardErrorCode::IoError,
                "Set backup directory permissions",
                e,
            )
        })?;
        // Never delete entries that another process created during the merge.
        fs::remove_dir(src).map_err(|e| {
            ClipboardError::from_io_error(
                ClipboardErrorCode::IoError,
                &format!("Failed to remove source dir {}", src.display()),
                e,
            )
        })?;
        actions.push(Action::Delete {
            path: src.to_path_buf(),
            backup,
            protection: None,
        });
    }
    Ok(())
}

pub(super) fn copy_entry(
    src: &Path,
    dest: &Path,
    app: Option<&tauri::AppHandle>,
    progress_event: Option<&CopyProgress<'_>>,
    cancel: Option<&AtomicBool>,
) -> ClipboardResult<crate::undo::CopyReceipt> {
    copy_entry_with_receipt(src, dest, app, progress_event, cancel, false)
}

fn copy_entry_with_receipt(
    src: &Path,
    dest: &Path,
    app: Option<&tauri::AppHandle>,
    progress_event: Option<&CopyProgress<'_>>,
    cancel: Option<&AtomicBool>,
    require_receipt: bool,
) -> ClipboardResult<crate::undo::CopyReceipt> {
    #[cfg(feature = "native-test")]
    crate::native_test::probes::checkpoint(
        &src.to_string_lossy(),
        &dest.to_string_lossy(),
        "start",
        0,
        || transfer_cancelled(cancel, app),
    );
    let meta = fs::symlink_metadata(src).map_err(|e| {
        ClipboardError::from_io_error(
            ClipboardErrorCode::IoError,
            &format!("Failed to read metadata for {}", src.display()),
            e,
        )
    })?;
    if meta.file_type().is_symlink() {
        return Err(ClipboardError::new(
            ClipboardErrorCode::SymlinkUnsupported,
            "Refusing to copy symlinks",
        ));
    }
    let result = if meta.is_dir() {
        ensure_not_child(src, dest)?;
        copy_dir(src, dest, app, progress_event, cancel, require_receipt)
    } else {
        if transfer_cancelled(cancel, app) {
            return Err(ClipboardError::cancelled());
        }
        let size_hint = Some(meta.len());
        let mut outputs = OwnedCopyPaths::default();
        copy_file_tracked(
            src,
            dest,
            app,
            progress_event,
            cancel,
            size_hint,
            Some(&mut outputs),
            require_receipt,
        )?;
        Ok(outputs.receipt(dest))
    };
    #[cfg(feature = "native-test")]
    if result.is_ok() {
        crate::native_test::probes::checkpoint(
            &src.to_string_lossy(),
            &dest.to_string_lossy(),
            "finalize",
            if meta.is_file() { meta.len() } else { 0 },
            || transfer_cancelled(cancel, app),
        );
    }
    result
}

#[cfg(test)]
pub(super) fn copy_file_best_effort(
    src: &Path,
    dest: &Path,
    app: Option<&tauri::AppHandle>,
    progress_event: Option<&CopyProgress<'_>>,
    cancel: Option<&AtomicBool>,
    total_hint: Option<u64>,
) -> ClipboardResult<u64> {
    copy_file_tracked(
        src,
        dest,
        app,
        progress_event,
        cancel,
        total_hint,
        None,
        false,
    )
}

#[allow(clippy::too_many_arguments)]
fn copy_file_tracked(
    src: &Path,
    dest: &Path,
    app: Option<&tauri::AppHandle>,
    progress_event: Option<&CopyProgress<'_>>,
    cancel: Option<&AtomicBool>,
    total_hint: Option<u64>,
    outputs: Option<&mut OwnedCopyPaths>,
    _require_receipt: bool,
) -> ClipboardResult<u64> {
    if transfer_cancelled(cancel, app) {
        return Err(ClipboardError::cancelled());
    }
    #[cfg(not(target_os = "windows"))]
    {
        if !_require_receipt && (is_gvfs_path(src) || is_gvfs_path(dest)) {
            // GIO owns the output handle; a later path lookup is not an ownership
            // receipt. Retain uncertain outputs and never retry through local I/O.
            tracing::info!(
                op = "copy",
                backend = "gio",
                kind = "file",
                staging = "direct",
                "transfer dispatch"
            );
            return super::gio_copy::copy(
                src,
                dest,
                total_hint,
                || transfer_cancelled(cancel, app),
                |payload| emit_copy_progress(progress_event, payload),
            );
        }
    }

    tracing::info!(
        op = "copy",
        backend = if _require_receipt {
            "owned-stream-receipt"
        } else {
            "owned-stream"
        },
        kind = "file",
        staging = "direct",
        "transfer dispatch"
    );
    // Fallback: manual chunked copy with progress
    #[cfg_attr(test, allow(unused_mut))]
    let mut reader = crate::fs_utils::open_regular_file_nofollow(src).map_err(|e| {
        ClipboardError::from_io_error(
            ClipboardErrorCode::IoError,
            &format!("Failed to open source for copy {}", src.display()),
            e,
        )
    })?;
    let permissions = reader
        .metadata()
        .map_err(|e| {
            ClipboardError::from_io_error(ClipboardErrorCode::IoError, "Read source permissions", e)
        })?
        .permissions();
    let source_state = crate::fs_utils::FileState::from_file(&reader).map_err(|error| {
        ClipboardError::from_io_error(
            ClipboardErrorCode::IoError,
            "Snapshot open copy source",
            error,
        )
    })?;
    let mut options = fs::OpenOptions::new();
    options.read(true).write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        options.mode(permissions.mode() & 0o777);
    }
    let writer = options.open(dest).map_err(|e| {
        ClipboardError::from_io_error(
            ClipboardErrorCode::IoError,
            &format!("Failed to open target for copy {}", dest.display()),
            e,
        )
    })?;
    // Capture ownership from the open handle, never from a replaceable path.
    let target_identity = crate::fs_utils::FileIdentity::from_file(&writer);
    #[cfg(test)]
    let mut reader = crate::fs_utils::copy_test_hooks::TestFile::new(reader, src, dest);
    #[cfg_attr(test, allow(unused_mut))]
    let mut writer = writer;
    #[cfg(test)]
    let mut writer = crate::fs_utils::copy_test_hooks::TestFile::new(writer, src, dest);
    let result: ClipboardResult<u64> = (|| {
        let mut buf = vec![0u8; 512 * 1024];
        let mut digest = blake3::Hasher::new();
        let mut done: u64 = 0;
        let total = total_hint
            .or_else(|| progress_event.and_then(|_| fs::metadata(src).ok().map(|m| m.len())));
        let mut last_emit = 0u64;
        let mut last_time = std::time::Instant::now();
        loop {
            if transfer_cancelled(cancel, app) {
                emit_copy_progress(
                    progress_event,
                    CopyProgressPayload {
                        bytes: done,
                        total: total.unwrap_or(done),
                        finished: false,
                    },
                );
                return Err(ClipboardError::cancelled());
            }
            #[cfg(feature = "native-test")]
            let chunk = if crate::native_test::probes::selected(
                &src.to_string_lossy(),
                &dest.to_string_lossy(),
            ) {
                buf.len().min(16384)
            } else {
                buf.len()
            };
            #[cfg(not(feature = "native-test"))]
            let chunk = buf.len();
            let n = reader.read(&mut buf[..chunk]).map_err(|e| {
                ClipboardError::from_io_error(ClipboardErrorCode::IoError, "Read failed", e)
            })?;
            if n == 0 {
                break;
            }
            writer.write_all(&buf[..n]).map_err(|e| {
                ClipboardError::from_io_error(ClipboardErrorCode::IoError, "Write failed", e)
            })?;
            digest.update(&buf[..n]);
            done = done.saturating_add(n as u64);
            if progress_event.is_some() {
                let elapsed = last_time.elapsed();
                #[cfg(feature = "native-test")]
                let observed = crate::native_test::probes::selected(
                    &src.to_string_lossy(),
                    &dest.to_string_lossy(),
                );
                #[cfg(not(feature = "native-test"))]
                let observed = false;
                if observed
                    || done.saturating_sub(last_emit) >= 64 * 1024
                    || elapsed >= std::time::Duration::from_millis(200)
                {
                    emit_copy_progress(
                        progress_event,
                        CopyProgressPayload {
                            bytes: done,
                            total: total.unwrap_or(0),
                            finished: false,
                        },
                    );
                    last_emit = done;
                    last_time = std::time::Instant::now();
                }
            }
            #[cfg(feature = "native-test")]
            crate::native_test::probes::checkpoint(
                &src.to_string_lossy(),
                &dest.to_string_lossy(),
                "written",
                done,
                || transfer_cancelled(cancel, app),
            );
        }
        preserve_copy_permissions(src, dest, &writer, permissions, "Set file permissions")?;
        let completed_state = crate::fs_utils::FileState::from_file(&writer).map_err(|error| {
            ClipboardError::from_io_error(
                ClipboardErrorCode::IoError,
                "Snapshot written copy target",
                error,
            )
        })?;
        let sync_result = (|| {
            #[cfg(test)]
            crate::fs_utils::copy_test_hooks::hit(
                src,
                dest,
                crate::fs_utils::copy_test_hooks::Phase::Sync,
                done,
            )?;
            writer.sync_all()?;
            #[cfg(test)]
            crate::fs_utils::copy_test_hooks::hit(
                src,
                dest,
                crate::fs_utils::copy_test_hooks::Phase::Synced,
                done,
            )?;
            Ok::<_, std::io::Error>(())
        })();
        sync_result.map_err(|e| {
            ClipboardError::from_io_error(ClipboardErrorCode::IoError, "Flush copied file", e)
        })?;
        if !target_identity
            .as_ref()
            .is_some_and(|identity| identity.matches(dest))
        {
            return Err(ClipboardError::new(
                ClipboardErrorCode::IoError,
                format!(
                    "Cannot verify copied target {}; source retained",
                    dest.display()
                ),
            ));
        }
        completed_state
            .verify_copied_file(&writer, dest, done)
            .map_err(|error| {
                ClipboardError::from_io_error(
                    ClipboardErrorCode::IoError,
                    "Copied target changed during finalization; source retained",
                    error,
                )
            })?;
        emit_copy_progress(
            progress_event,
            CopyProgressPayload {
                bytes: done,
                total: total.unwrap_or(done),
                finished: false,
            },
        );
        crate::fs_utils::verify_copy_content(
            &writer,
            dest,
            &completed_state,
            done,
            &digest.finalize(),
            || {
                if transfer_cancelled(cancel, app) {
                    Err(std::io::Error::new(
                        ErrorKind::Interrupted,
                        "Copy verification cancelled",
                    ))
                } else {
                    Ok(())
                }
            },
        )
        .map_err(|error| {
            if error.kind() == ErrorKind::Interrupted && transfer_cancelled(cancel, app) {
                ClipboardError::cancelled()
            } else {
                ClipboardError::from_io_error(
                    ClipboardErrorCode::IoError,
                    "Copied content verification failed; source retained",
                    error,
                )
            }
        })?;
        source_state
            .verify_copied_file(&reader, src, done)
            .map_err(|error| {
                ClipboardError::from_io_error(
                    ClipboardErrorCode::IoError,
                    "Source changed during copy; output retained for inspection",
                    error,
                )
            })?;
        if let Some(outputs) = outputs {
            // Use the pre-sync version, never adopt an edit after verification.
            outputs.record_file(dest, completed_state);
        }
        emit_copy_progress(
            progress_event,
            CopyProgressPayload {
                bytes: done,
                total: total.unwrap_or(done),
                finished: true,
            },
        );
        Ok(done)
    })();
    if let Err(error) = result {
        // Our own writes can mask another writer's metadata changes. Even a
        // matching inode or post-write snapshot cannot justify unlinking this
        // failed output. Do not race an active writer during failure cleanup.
        if !target_identity
            .as_ref()
            .is_some_and(|identity| identity.matches(dest))
        {
            return Err(error.with_context(format!(
                "Partial target {} is missing, replaced, or unverifiable; no cleanup attempted",
                dest.display()
            )));
        }
        return Err(error.with_context(format!(
            "Uncertain copied output retained at {}; it may be incomplete or edited by another program; inspect affected paths before retrying; no cleanup attempted",
            dest.display()
        )));
    }
    result
}

#[cfg(not(target_os = "windows"))]
fn is_gvfs_path(path: &Path) -> bool {
    path.to_string_lossy().to_lowercase().contains("/gvfs/")
}

pub(super) fn move_entry(
    src: &Path,
    dest: &Path,
    app: Option<&tauri::AppHandle>,
    progress_event: Option<&CopyProgress<'_>>,
    cancel: Option<&AtomicBool>,
) -> ClipboardResult<()> {
    ensure_not_child(src, dest)?;
    if transfer_cancelled(cancel, app) {
        return Err(ClipboardError::cancelled());
    }
    let source_snapshot = crate::undo::snapshot_existing_path(src).map_err(ClipboardError::from)?;
    if metadata_if_exists_nofollow(dest)?.is_some() {
        return Err(ClipboardError::new(
            ClipboardErrorCode::DestinationExists,
            format!("Destination already exists: {}", dest.display()),
        ));
    }
    #[cfg(test)]
    BEFORE_MOVE_RENAME_TEST_HOOK.with(|hook| {
        let callback = hook.borrow_mut().take();
        if let Some(callback) = callback {
            callback();
        }
    });
    if transfer_cancelled(cancel, app) {
        return Err(ClipboardError::cancelled());
    }
    crate::undo::assert_path_snapshot(src, &source_snapshot).map_err(ClipboardError::from)?;
    // A prior existence check is only advisory: another process can create the
    // destination before rename. Reuse the undo engine's native no-replace move.
    match crate::undo::rename_nofollow_io(src, dest) {
        Ok(_) => {
            tracing::info!(
                op = "move",
                backend = "filesystem-rename",
                kind = "entry",
                staging = "direct",
                "transfer dispatch"
            );
            Ok(())
        }
        Err(error)
            if matches!(
                error.code(),
                crate::undo::UndoErrorCode::CrossDeviceMove
                    | crate::undo::UndoErrorCode::AtomicRenameUnsupported
            ) =>
        {
            tracing::info!(
                op = "move",
                backend = "copy-verify-delete",
                kind = "entry",
                staging = "direct",
                "transfer dispatch"
            );
            let check = || {
                if transfer_cancelled(cancel, app) {
                    Err(std::io::Error::from(ErrorKind::Interrupted))
                } else {
                    Ok(())
                }
            };
            let source_tree = crate::fs_utils::TreeSnapshot::capture_with_check(src, check)
                .map_err(|error| {
                    if error.kind() == ErrorKind::Interrupted {
                        return ClipboardError::cancelled();
                    }
                    ClipboardError::from_io_error(
                        ClipboardErrorCode::IoError,
                        "Snapshot source before fallback copy",
                        error,
                    )
                })?;
            // Choose the owned stream writer before any fallback copy starts.
            // GIO copy does not expose its writer identity; do not adopt a late
            // path lookup as a receipt or retry a write to obtain one.
            let receipt = copy_entry_with_receipt(src, dest, app, progress_event, cancel, true)?;
            if transfer_cancelled(cancel, app) {
                return Err(ClipboardError::cancelled().with_context(format!(
                    "Copy retained at {}; source not removed",
                    dest.display()
                )));
            }
            crate::undo::assert_path_snapshot(src, &source_snapshot)
                .map_err(ClipboardError::from)?;
            #[cfg(test)]
            crate::fs_utils::copy_test_hooks::hit(
                src,
                dest,
                crate::fs_utils::copy_test_hooks::Phase::BeforeSourceDelete,
                0,
            )
            .map_err(|error| {
                ClipboardError::from_io_error(
                    ClipboardErrorCode::IoError,
                    "Prepare fallback source deletion",
                    error,
                )
            })?;
            finish_fallback_move(src, dest, &source_tree, &receipt, check)
        }
        Err(error) => Err(ClipboardError::from(error)),
    }
}

pub(super) fn finish_fallback_move(
    src: &Path,
    dest: &Path,
    source_tree: &crate::fs_utils::TreeSnapshot,
    receipt: &crate::undo::CopyReceipt,
    mut check_cancel: impl FnMut() -> std::io::Result<()>,
) -> ClipboardResult<()> {
    // No receipt (for example a GIO-owned writer) is not permission to delete.
    receipt.verify_with_check(dest, &mut check_cancel).map_err(|error| {
        let error = if check_cancel().is_err_and(|error| error.kind() == ErrorKind::Interrupted) {
            ClipboardError::cancelled()
        } else {
            ClipboardError::from(error)
        };
        error.with_context(format!("Fallback target changed or unverifiable; source not removed, copied output retained at {}", dest.display()))
    })?;
    source_tree.remove_recorded_with_check(src, check_cancel).map_err(|error| {
        let context = format!("Copied {} to {}; destination retained because source removal failed or was interrupted; inspect remaining source entries before retrying", src.display(), dest.display());
        if error.kind() == ErrorKind::Interrupted {
            ClipboardError::cancelled().with_context(context)
        } else {
            ClipboardError::from_io_error(ClipboardErrorCode::IoError, &context, error)
        }
    })
}

pub(super) fn metadata_if_exists_nofollow(path: &Path) -> ClipboardResult<Option<fs::Metadata>> {
    match fs::symlink_metadata(path) {
        Ok(meta) => Ok(Some(meta)),
        Err(err) if err.kind() == ErrorKind::NotFound => Ok(None),
        Err(err) => Err(ClipboardError::from_io_error(
            ClipboardErrorCode::IoError,
            &format!("Failed to read metadata for {}", path.display()),
            err,
        )),
    }
}

pub(super) fn is_destination_exists_error(err: &ClipboardError) -> bool {
    err.code() == ClipboardErrorCode::DestinationExists
}
