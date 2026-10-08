//! Linux remote deletion never downloads contents into local undo storage.
use super::error::{map_api_result, FsError, FsErrorCode, FsResult};
use crate::errors::api_error::ApiResult;
use std::path::{Component, Path};

fn validate_path(path: &Path) -> FsResult<()> {
    if !path.is_absolute()
        || path
            .components()
            .any(|part| matches!(part, Component::ParentDir | Component::CurDir))
    {
        return Err(FsError::new(
            FsErrorCode::InvalidPath,
            "Deletion requires an absolute path without traversal components.",
        ));
    }
    Ok(())
}

fn mount_root(path: &Path) -> FsResult<Option<std::path::PathBuf>> {
    validate_path(path)?;
    #[cfg(target_os = "linux")]
    {
        crate::commands::permissions::network_mount_root(path).map_err(|_| {
            FsError::new(
                FsErrorCode::TaskFailed,
                "Could not determine network deletion policy.",
            )
        })
    }
    #[cfg(not(target_os = "linux"))]
    {
        Ok(None)
    }
}

pub(super) fn require_local_backup_path(path: &Path) -> FsResult<()> {
    if mount_root(path)?.is_some() {
        return Err(confirmation_required());
    }
    Ok(())
}

fn confirmation_required() -> FsError {
    FsError::new(FsErrorCode::NetworkConfirmationRequired,
        "This network operation requires the network deletion flow. Permanent deletion has no local undo backup.")
}

#[tauri::command]
pub async fn network_delete_paths(paths: Vec<String>) -> ApiResult<Vec<String>> {
    map_api_result(
        async {
            tauri::async_runtime::spawn_blocking(move || {
                paths
                    .into_iter()
                    .filter_map(|raw| match mount_root(Path::new(&raw)) {
                        Ok(Some(_)) => Some(Ok(raw)),
                        Ok(None) => None,
                        Err(error) => Some(Err(error)),
                    })
                    .collect::<FsResult<Vec<_>>>()
            })
            .await
            .map_err(|_| {
                FsError::new(
                    FsErrorCode::TaskFailed,
                    "Network deletion policy task failed.",
                )
            })?
        }
        .await,
    )
}

#[tauri::command]
pub async fn can_trash_paths(paths: Vec<String>) -> ApiResult<bool> {
    #[cfg(target_os = "linux")]
    {
        use gio::prelude::CancellableExt;
        let cancel = gio::Cancellable::new();
        let check_cancel = cancel.clone();
        let task = tauri::async_runtime::spawn_blocking(move || {
            linux::trash_supported(&paths, &check_cancel)
        });
        // A disconnected server must not hold a drag/drop gesture indefinitely.
        let result = tokio::time::timeout(std::time::Duration::from_secs(3), task).await;
        map_api_result(match result {
            Ok(Ok(result)) => result,
            Ok(Err(_)) => Err(FsError::new(
                FsErrorCode::TaskFailed,
                "Trash capability check failed.",
            )),
            Err(_) => {
                cancel.cancel();
                Ok(false)
            }
        })
    }
    #[cfg(not(target_os = "linux"))]
    {
        map_api_result(
            tauri::async_runtime::spawn_blocking(move || {
                if paths.is_empty() {
                    return Ok(false);
                }
                for raw in paths {
                    let path = Path::new(&raw);
                    validate_path(path)?;
                    // Native Windows network trash is unsupported. Local volumes
                    // still use the existing platform trash operation and undo.
                    if path.parent().is_none()
                        || raw.starts_with("//")
                        || crate::entry::is_network_location(path)
                        || std::fs::symlink_metadata(path).is_err()
                    {
                        return Ok(false);
                    }
                }
                Ok(true)
            })
            .await
            .map_err(|_| FsError::new(FsErrorCode::TaskFailed, "Trash capability check failed."))
            .and_then(|result| result),
        )
    }
}

#[tauri::command]
pub async fn network_delete_entries(
    app: tauri::AppHandle,
    paths: Vec<String>,
    trash: bool,
    confirmed: bool,
    progress_event: Option<String>,
    cancel: tauri::State<'_, super::CancelState>,
    undo: tauri::State<'_, super::UndoState>,
) -> ApiResult<()> {
    #[cfg(target_os = "linux")]
    {
        let cancel = cancel.inner().clone();
        let undo = undo.inner().clone();
        map_api_result(
            async {
                tauri::async_runtime::spawn_blocking(move || {
                    linux::execute(app, paths, trash, confirmed, progress_event, cancel, undo)
                })
                .await
                .map_err(|_| {
                    FsError::new(FsErrorCode::TaskFailed, "Network deletion task failed.")
                })?
            }
            .await,
        )
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = (app, paths, trash, confirmed, progress_event, cancel, undo);
        map_api_result(Err(FsError::new(
            FsErrorCode::DeleteFailed,
            "Native network deletion is only available on Linux.",
        )))
    }
}

#[cfg(target_os = "linux")]
mod linux {
    use super::*;
    use gio::prelude::*;
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };
    use std::time::{Duration, Instant};

    fn resolve_remote(path: &Path) -> FsResult<gio::File> {
        let root = mount_root(path)?
            .ok_or_else(|| FsError::new(FsErrorCode::InvalidPath, "Not a network-mounted path."))?;
        if path == root {
            return Err(FsError::new(
                FsErrorCode::RootForbidden,
                "Refusing to delete a network mount root.",
            ));
        }
        // Check ancestors only: deleting the final symlink must never follow it.
        if let Some(parent) = path.parent() {
            crate::fs_utils::check_no_symlink_components(parent).map_err(FsError::from)?;
        }
        let mounts = gio::VolumeMonitor::get().mounts();
        let mut roots: Vec<_> = mounts
            .into_iter()
            .filter_map(|mount| {
                let remote = mount.root();
                let local = remote.path()?;
                (!remote.is_native() && path.starts_with(&local)).then_some((local, remote))
            })
            .collect();
        roots.sort_by_key(|(local, _)| std::cmp::Reverse(local.as_os_str().len()));
        if let Some((local, remote)) = roots.into_iter().next() {
            let relative = path.strip_prefix(local).unwrap();
            if relative.as_os_str().is_empty() {
                return Err(FsError::new(
                    FsErrorCode::RootForbidden,
                    "Refusing to delete a server root.",
                ));
            }
            // Resolve from the actual GIO mount, preserving account, port and
            // filename encoding. Do not infer a URI from a display label.
            let mut file = remote;
            for component in relative.components() {
                match component {
                    Component::Normal(name) => file = file.child(name),
                    _ => {
                        return Err(FsError::new(
                            FsErrorCode::InvalidPath,
                            "Invalid remote path.",
                        ))
                    }
                }
            }
            return Ok(file);
        }
        // A stale GVFS mount must not fall back to GLocalFile and local trash.
        if root.file_name().is_some_and(|name| name == "gvfs") {
            return Err(FsError::new(
                FsErrorCode::NotFound,
                "The network mount is unavailable. Reconnect before deleting.",
            ));
        }
        // Kernel NFS/CIFS/SSHFS mounts use native filesystem operations on the
        // server-backed mount, including its own trash when GIO supports it.
        Ok(gio::File::for_path(path))
    }

    fn gio_error(error: gio::glib::Error) -> FsError {
        let code = if error.matches(gio::IOErrorEnum::Cancelled) {
            FsErrorCode::Cancelled
        } else if error.matches(gio::IOErrorEnum::PermissionDenied) {
            FsErrorCode::PermissionDenied
        } else {
            FsErrorCode::DeleteFailed
        };
        FsError::new(code, format!("Network operation failed: {error}"))
    }

    fn can_trash(file: &gio::File, cancel: &gio::Cancellable) -> FsResult<bool> {
        let info = file
            .query_info(
                "access::can-trash",
                gio::FileQueryInfoFlags::NOFOLLOW_SYMLINKS,
                Some(cancel),
            )
            .map_err(gio_error)?;
        Ok(info.has_attribute("access::can-trash") && info.boolean("access::can-trash"))
    }

    pub(super) fn trash_supported(paths: &[String], cancel: &gio::Cancellable) -> FsResult<bool> {
        if paths.is_empty() {
            return Ok(false);
        }
        for raw in paths {
            let path = Path::new(raw);
            validate_path(path)?;
            if path.parent().is_none() {
                return Ok(false);
            }
            let file = if mount_root(path)?.is_some() {
                resolve_remote(path)?
            } else {
                gio::File::for_path(path)
            };
            if !can_trash(&file, cancel)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn delete_tree(file: &gio::File, cancel: &gio::Cancellable) -> FsResult<()> {
        // Iterative post-order traversal; no content reads, no symlink traversal,
        // no recursion-stack limit, and cancellable GIO calls for each entry.
        let mut pending = vec![(file.clone(), false)];
        while let Some((file, expanded)) = pending.pop() {
            if cancel.is_cancelled() {
                return Err(FsError::new(
                    FsErrorCode::Cancelled,
                    "Network deletion cancelled.",
                ));
            }
            if !expanded {
                let info = file
                    .query_info(
                        "standard::type",
                        gio::FileQueryInfoFlags::NOFOLLOW_SYMLINKS,
                        Some(cancel),
                    )
                    .map_err(gio_error)?;
                if info.file_type() == gio::FileType::Directory {
                    let children = file
                        .enumerate_children(
                            "standard::name",
                            gio::FileQueryInfoFlags::NOFOLLOW_SYMLINKS,
                            Some(cancel),
                        )
                        .map_err(gio_error)?;
                    pending.push((file.clone(), true));
                    while let Some(info) = children.next_file(Some(cancel)).map_err(gio_error)? {
                        let name = info.name();
                        if name.components().count() != 1
                            || !matches!(name.components().next(), Some(Component::Normal(_)))
                        {
                            return Err(FsError::new(
                                FsErrorCode::InvalidPath,
                                "Invalid server filename; deletion stopped.",
                            ));
                        }
                        pending.push((file.child(name), false));
                    }
                    children.close(Some(cancel)).map_err(gio_error)?;
                    continue;
                }
            }
            file.delete(Some(cancel)).map_err(gio_error)?;
        }
        Ok(())
    }

    trait Backend {
        fn can_trash(&self, index: usize) -> FsResult<bool>;
        fn trash(&self, index: usize) -> FsResult<()>;
        fn delete(&self, index: usize) -> FsResult<()>;
    }

    enum Target {
        Remote(gio::File),
        Local(std::path::PathBuf),
    }

    struct GioBackend<'a> {
        targets: &'a [Target],
        cancel: &'a gio::Cancellable,
        undo: &'a super::super::UndoState,
    }
    impl Backend for GioBackend<'_> {
        fn can_trash(&self, index: usize) -> FsResult<bool> {
            match &self.targets[index] {
                Target::Remote(file) => can_trash(file, self.cancel),
                Target::Local(_) => Ok(true),
            }
        }
        fn trash(&self, index: usize) -> FsResult<()> {
            match &self.targets[index] {
                Target::Remote(file) => file.trash(Some(self.cancel)).map_err(gio_error),
                Target::Local(path) => {
                    let action = super::super::trash::move_single_to_trash_with_system_backend(
                        path.to_str().ok_or_else(|| {
                            FsError::new(FsErrorCode::InvalidPath, "Invalid local path encoding.")
                        })?,
                    )?;
                    let _ = self.undo.record_applied(action);
                    Ok(())
                }
            }
        }
        fn delete(&self, index: usize) -> FsResult<()> {
            match &self.targets[index] {
                Target::Remote(file) => delete_tree(file, self.cancel),
                Target::Local(path) => {
                    let action = super::super::delete_ops::delete_with_backup(path)?;
                    let _ = self.undo.record_applied(action);
                    Ok(())
                }
            }
        }
    }

    fn run(
        backend: &impl Backend,
        count: usize,
        trash: bool,
        confirmed: bool,
        mut abort: impl FnMut() -> bool,
        mut progress: impl FnMut(u64, bool),
    ) -> FsResult<()> {
        if !trash && !confirmed {
            return Err(confirmation_required());
        }
        let mut trashable = Vec::with_capacity(count);
        // Preflight every target before any mutation. Unsupported trash never
        // silently becomes deletion, even when the normal confirmation is off.
        for index in 0..count {
            if abort() {
                return Err(FsError::new(
                    FsErrorCode::Cancelled,
                    "Network deletion cancelled before starting.",
                ));
            }
            let supported = trash && backend.can_trash(index)?;
            if trash && !supported && !confirmed {
                return Err(confirmation_required());
            }
            trashable.push(supported);
        }
        let mut done = 0;
        let result = (|| {
            for (index, supported) in trashable.into_iter().enumerate() {
                if abort() {
                    return Err(FsError::new(
                        FsErrorCode::Cancelled,
                        "Network deletion cancelled.",
                    ));
                }
                // A failed trash request is not retried as a permanent delete.
                if supported {
                    backend.trash(index)?;
                } else {
                    backend.delete(index)?;
                }
                done += 1;
                progress(done, false);
            }
            Ok(())
        })();
        progress(done, true);
        result.map_err(|error: FsError| {
            // Confirmation-required is only safe before any mutation. A mount
            // changing mid-batch must not invite retrying the complete batch.
            let code = if error.code() == FsErrorCode::NetworkConfirmationRequired { FsErrorCode::DeleteFailed } else { error.code() };
            FsError::new(code, format!("{error} {done} of {count} items completed; the current item may be partially affected. Completed network operations cannot be rolled back by Browsey. Refresh before retrying."))
        })
    }

    struct CancelWatch {
        done: Arc<AtomicBool>,
        thread: Option<std::thread::JoinHandle<()>>,
    }
    impl Drop for CancelWatch {
        fn drop(&mut self) {
            self.done.store(true, Ordering::Relaxed);
            if let Some(thread) = self.thread.take() {
                let _ = thread.join();
            }
        }
    }

    pub(super) fn execute(
        app: tauri::AppHandle,
        paths: Vec<String>,
        trash: bool,
        confirmed: bool,
        event: Option<String>,
        cancel_state: super::super::CancelState,
        undo: super::super::UndoState,
    ) -> FsResult<()> {
        if paths.is_empty() {
            return Ok(());
        }
        let guard = event
            .as_ref()
            .map(|id| cancel_state.register(id.clone()))
            .transpose()
            .map_err(|_| {
                FsError::new(
                    FsErrorCode::TaskFailed,
                    "Could not register network cancellation.",
                )
            })?;
        let token = guard.as_ref().map(|guard| guard.token());
        let cancel = gio::Cancellable::new();
        let done = Arc::new(AtomicBool::new(false));
        let watch_done = done.clone();
        let watch_cancel = cancel.clone();
        let watch_app = app.clone();
        let thread = std::thread::spawn(move || {
            while !watch_done.load(Ordering::Relaxed) {
                if crate::runtime_lifecycle::is_shutting_down(&watch_app)
                    || token
                        .as_ref()
                        .is_some_and(|flag| flag.load(Ordering::Relaxed))
                {
                    watch_cancel.cancel();
                    break;
                }
                std::thread::sleep(Duration::from_millis(50));
            }
        });
        let _watch = CancelWatch {
            done,
            thread: Some(thread),
        };
        let mut targets = Vec::with_capacity(paths.len());
        for raw in &paths {
            let path = Path::new(raw);
            if mount_root(path)?.is_some() {
                targets.push(Target::Remote(resolve_remote(path)?));
            } else {
                let path =
                    crate::fs_utils::sanitize_path_nofollow(raw, true).map_err(FsError::from)?;
                crate::fs_utils::check_no_symlink_components(&path).map_err(FsError::from)?;
                targets.push(Target::Local(path));
            }
        }
        if !targets
            .iter()
            .any(|target| matches!(target, Target::Remote(_)))
        {
            return Err(FsError::new(
                FsErrorCode::InvalidPath,
                "No network targets in this request.",
            ));
        }
        let backend = GioBackend {
            targets: &targets,
            cancel: &cancel,
            undo: &undo,
        };
        let mut last_emit = Instant::now();
        let result = run(
            &backend,
            targets.len(),
            trash,
            confirmed,
            || cancel.is_cancelled(),
            |items, finished| {
                if let Some(event) = &event {
                    let now = Instant::now();
                    if !finished && now.duration_since(last_emit) < Duration::from_millis(100) {
                        return;
                    }
                    last_emit = now;
                    crate::runtime_lifecycle::emit_if_running(
                        &app,
                        event,
                        super::super::DeleteProgressPayload {
                            items,
                            total: targets.len() as u64,
                            finished,
                        },
                    );
                }
            },
        );
        if trash {
            crate::runtime_lifecycle::emit_if_running(&app, "trash-changed", ());
        }
        result
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use std::cell::RefCell;
        #[test]
        fn trash_capability_checks_are_read_only_and_reject_invalid_paths() {
            let cancel = gio::Cancellable::new();
            assert!(!trash_supported(&[], &cancel).unwrap());
            assert!(!trash_supported(&["/".into()], &cancel).unwrap());
            assert!(trash_supported(&["relative/file".into()], &cancel).is_err());
            assert!(trash_supported(&["/tmp/../file".into()], &cancel).is_err());
            let root = fixture();
            let file = root.join("document.txt");
            std::fs::write(&file, b"unchanged contents").unwrap();
            let _supported =
                trash_supported(&[file.to_string_lossy().into_owned()], &cancel).unwrap();
            assert_eq!(std::fs::read(&file).unwrap(), b"unchanged contents");
            assert!(trash_supported(
                &[root.join("missing").to_string_lossy().into_owned()],
                &cancel
            )
            .is_err());
            std::fs::remove_dir_all(root).unwrap();
        }
        fn fixture() -> std::path::PathBuf {
            let stamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let path = std::env::temp_dir().join(format!(
                "browsey-native-delete-{}-{stamp}",
                std::process::id()
            ));
            std::fs::create_dir(&path).unwrap();
            path
        }
        struct Fake {
            support: Vec<bool>,
            operations: RefCell<Vec<(usize, bool)>>,
            fail_trash: bool,
        }
        impl Backend for Fake {
            fn can_trash(&self, index: usize) -> FsResult<bool> {
                Ok(self.support[index])
            }
            fn trash(&self, index: usize) -> FsResult<()> {
                if self.fail_trash {
                    return Err(FsError::new(FsErrorCode::PermissionDenied, "Denied"));
                }
                self.operations.borrow_mut().push((index, true));
                Ok(())
            }
            fn delete(&self, index: usize) -> FsResult<()> {
                self.operations.borrow_mut().push((index, false));
                Ok(())
            }
        }
        #[test]
        fn unsupported_trash_and_unconfirmed_delete_do_not_mutate_any_target() {
            let backend = Fake {
                support: vec![true, false],
                operations: RefCell::new(vec![]),
                fail_trash: false,
            };
            assert_eq!(
                run(&backend, 2, true, false, || false, |_, _| {})
                    .unwrap_err()
                    .code(),
                FsErrorCode::NetworkConfirmationRequired
            );
            assert!(run(&backend, 2, false, false, || false, |_, _| {}).is_err());
            assert!(backend.operations.borrow().is_empty());
            run(&backend, 2, true, true, || false, |_, _| {}).unwrap();
            assert_eq!(*backend.operations.borrow(), vec![(0, true), (1, false)]);
        }
        #[test]
        fn trash_failure_is_never_retried_as_delete() {
            let backend = Fake {
                support: vec![true],
                operations: RefCell::new(vec![]),
                fail_trash: true,
            };
            assert!(run(&backend, 1, true, true, || false, |_, _| {}).is_err());
            assert!(backend.operations.borrow().is_empty());
        }
        #[test]
        fn cancellation_reports_partial_results_without_rollback_or_retries() {
            let backend = Fake {
                support: vec![true, true],
                operations: RefCell::new(vec![]),
                fail_trash: false,
            };
            let error = run(
                &backend,
                2,
                true,
                false,
                || !backend.operations.borrow().is_empty(),
                |_, _| {},
            )
            .unwrap_err();
            assert_eq!(error.code(), FsErrorCode::Cancelled);
            assert!(error.to_string().contains("1 of 2 items completed"));
            assert_eq!(*backend.operations.borrow(), vec![(0, true)]);
        }
        #[test]
        fn native_delete_removes_large_sparse_file_and_symlink_without_reading_contents() {
            use std::os::unix::fs::symlink;
            let fixture = fixture();
            let tree = fixture.join("tree");
            std::fs::create_dir(&tree).unwrap();
            let outside = fixture.join("outside");
            std::fs::write(&outside, b"keep").unwrap();
            let outside_dir = fixture.join("outside-dir");
            std::fs::create_dir(&outside_dir).unwrap();
            std::fs::write(outside_dir.join("keep"), b"keep directory").unwrap();
            let large = std::fs::File::create(tree.join("large")).unwrap();
            large.set_len(8 * 1024 * 1024 * 1024).unwrap();
            symlink(&outside, tree.join("link")).unwrap();
            symlink(&outside_dir, tree.join("directory-link")).unwrap();
            delete_tree(&gio::File::for_path(&tree), &gio::Cancellable::new()).unwrap();
            assert!(!tree.exists());
            assert_eq!(std::fs::read(outside).unwrap(), b"keep");
            assert_eq!(
                std::fs::read(outside_dir.join("keep")).unwrap(),
                b"keep directory"
            );
            std::fs::remove_dir_all(fixture).unwrap();
        }
        #[test]
        fn local_paths_and_traversal_never_enter_remote_delete() {
            let fixture = fixture();
            assert!(resolve_remote(&fixture).is_err());
            assert!(mount_root(Path::new("/tmp/../etc")).is_err());
            assert!(mount_root(Path::new("relative")).is_err());
            std::fs::remove_dir(fixture).unwrap();
        }

        #[test]
        fn mixed_batch_keeps_local_undo_but_has_no_remote_backup_or_history() {
            let fixture = fixture();
            let local = fixture.join("local");
            let remote = fixture.join("remote-test-surrogate");
            std::fs::write(&local, b"local undo").unwrap();
            std::fs::write(&remote, b"remote surrogate").unwrap();
            let targets = vec![
                Target::Local(local.clone()),
                Target::Remote(gio::File::for_path(&remote)),
            ];
            let undo = super::super::super::UndoState::default();
            let cancel = gio::Cancellable::new();
            let backend = GioBackend {
                targets: &targets,
                cancel: &cancel,
                undo: &undo,
            };
            let progress = RefCell::new(Vec::new());
            run(
                &backend,
                2,
                false,
                true,
                || false,
                |done, finished| progress.borrow_mut().push((done, finished)),
            )
            .unwrap();
            assert!(!local.exists() && !remote.exists());
            assert_eq!(*progress.borrow(), vec![(1, false), (2, false), (2, true)]);
            undo.undo().unwrap();
            assert_eq!(std::fs::read(&local).unwrap(), b"local undo");
            assert!(!remote.exists());
            assert!(!undo.clone_inner().lock().unwrap().can_undo());
            std::fs::remove_dir_all(fixture).unwrap();
        }

        #[test]
        fn already_cancelled_native_delete_leaves_all_files_intact() {
            let fixture = fixture();
            let path = fixture.join("keep");
            std::fs::write(&path, b"keep").unwrap();
            let cancel = gio::Cancellable::new();
            cancel.cancel();
            assert_eq!(
                delete_tree(&gio::File::for_path(&path), &cancel)
                    .unwrap_err()
                    .code(),
                FsErrorCode::Cancelled
            );
            assert_eq!(std::fs::read(path).unwrap(), b"keep");
            std::fs::remove_dir_all(fixture).unwrap();
        }

        #[test]
        #[ignore = "Read-only mapping check; requires an existing GVFS server mount; never deletes or opens contents"]
        fn existing_gvfs_mount_mapping_read_only() {
            let mounts = gio::VolumeMonitor::get().mounts();
            let mut checked = 0;
            for mount in mounts {
                let root = mount.root();
                if root.is_native() {
                    continue;
                }
                let Some(local) = root.path() else {
                    continue;
                };
                if mount_root(&local).unwrap().is_none() {
                    continue;
                }
                assert!(
                    resolve_remote(&local).is_err(),
                    "Server root must be protected"
                );
                let probe = "browsey-readonly-mapping-probe-no-content-access";
                let resolved = resolve_remote(&local.join(probe)).unwrap();
                assert!(
                    resolved.equal(&root.child(probe)),
                    "Mapped path must retain the exact mounted URI"
                );
                checked += 1;
            }
            assert!(
                checked > 0,
                "No existing GVFS network mount available for this opt-in check"
            );
        }

        #[test]
        #[ignore = "Creates/deletes disposable fixtures only inside BROWSEY_NETWORK_DELETE_TEST_DIR; requires an explicitly approved network directory"]
        fn real_network_delete_acceptance() {
            use std::io::Write;

            let approved = std::path::PathBuf::from(
                std::env::var_os("BROWSEY_NETWORK_DELETE_TEST_DIR")
                    .expect("Set an explicitly approved, existing network test directory"),
            );
            let approved_remote = resolve_remote(&approved).unwrap();
            let cancel = gio::Cancellable::new();
            assert_eq!(
                approved_remote
                    .query_file_type(gio::FileQueryInfoFlags::NOFOLLOW_SYMLINKS, Some(&cancel)),
                gio::FileType::Directory
            );
            let stamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos();
            let owned = approved.join(format!(
                "browsey-delete-acceptance-{}-{stamp}",
                std::process::id()
            ));
            let owned_remote = resolve_remote(&owned).unwrap();
            // make_directory must succeed before installing cleanup: never
            // adopt an existing directory, even in the unlikely event of a collision.
            owned_remote.make_directory(Some(&cancel)).unwrap();
            struct Cleanup(Option<gio::File>);
            impl Drop for Cleanup {
                fn drop(&mut self) {
                    if let Some(file) = &self.0 {
                        if let Err(error) = delete_tree(file, &gio::Cancellable::new()) {
                            eprintln!("Disposable network fixture cleanup failed: {error}");
                        }
                    }
                }
            }
            let mut cleanup = Cleanup(Some(owned_remote.clone()));
            let large = owned.join("64-MiB-disposable.bin");
            let bytes = 64 * 1024 * 1024;
            let mut writer = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&large)
                .unwrap();
            let block = vec![0x5a; 1024 * 1024];
            for _ in 0..64 {
                writer.write_all(&block).unwrap();
            }
            writer.flush().unwrap();
            drop(writer);
            assert_eq!(std::fs::metadata(&large).unwrap().len(), bytes);
            let tree = owned.join("nested");
            std::fs::create_dir(&tree).unwrap();
            std::fs::write(
                tree.join("unicode-\u{e9}.txt"),
                b"disposable nested fixture",
            )
            .unwrap();
            let targets = vec![
                Target::Remote(resolve_remote(&large).unwrap()),
                Target::Remote(resolve_remote(&tree).unwrap()),
            ];
            let undo = super::super::super::UndoState::default();
            let backend = GioBackend {
                targets: &targets,
                cancel: &cancel,
                undo: &undo,
            };
            assert_eq!(
                require_local_backup_path(&large).unwrap_err().code(),
                FsErrorCode::NetworkConfirmationRequired
            );
            assert_eq!(
                run(&backend, 2, false, false, || false, |_, _| {})
                    .unwrap_err()
                    .code(),
                FsErrorCode::NetworkConfirmationRequired
            );
            let trash_supported = backend.can_trash(0).unwrap();
            if !trash_supported {
                assert_eq!(
                    run(&backend, 2, true, false, || false, |_, _| {})
                        .unwrap_err()
                        .code(),
                    FsErrorCode::NetworkConfirmationRequired
                );
            }
            assert_eq!(
                run(&backend, 2, false, true, || true, |_, _| {})
                    .unwrap_err()
                    .code(),
                FsErrorCode::Cancelled
            );
            assert!(large.exists() && tree.exists());
            let progress = RefCell::new(Vec::new());
            let started = Instant::now();
            // Explicit permanent deletion avoids leaving fixtures in a server
            // trash outside the approved test directory.
            run(
                &backend,
                2,
                false,
                true,
                || false,
                |done, finished| progress.borrow_mut().push((done, finished)),
            )
            .unwrap();
            let elapsed = started.elapsed();
            let stale_fuse_metadata = large.exists() || tree.exists();
            for path in [&large, &tree] {
                let error = resolve_remote(path)
                    .unwrap()
                    .query_info(
                        "standard::type",
                        gio::FileQueryInfoFlags::NOFOLLOW_SYMLINKS,
                        Some(&cancel),
                    )
                    .unwrap_err();
                assert!(error.matches(gio::IOErrorEnum::NotFound));
            }
            // GVFS/FUSE may still cache a positive stat after a direct GIO
            // deletion. Browsey's fresh directory listing must nevertheless
            // omit the deleted entries; do not equate stale stat with failure.
            assert_eq!(std::fs::read_dir(&owned).unwrap().count(), 0);
            assert_eq!(*progress.borrow(), vec![(1, false), (2, false), (2, true)]);
            assert!(!undo.clone_inner().lock().unwrap().can_undo());
            delete_tree(&owned_remote, &cancel).unwrap();
            cleanup.0 = None;
            assert!(owned_remote
                .query_info(
                    "standard::type",
                    gio::FileQueryInfoFlags::NOFOLLOW_SYMLINKS,
                    Some(&cancel)
                )
                .unwrap_err()
                .matches(gio::IOErrorEnum::NotFound));
            assert!(approved.is_dir());
            eprintln!(
                "Network acceptance: {bytes} bytes and a nested directory deleted in {elapsed:?}; trash_supported={trash_supported}; stale_fuse_metadata={stale_fuse_metadata}; fresh listing empty; no undo history; owned fixtures cleaned up"
            );
        }
    }
}
