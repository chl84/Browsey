use super::*;
use crate::undo::{run_actions, Direction};
use std::env;
use std::fs;
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::symlink;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime};
#[cfg(target_os = "linux")]
mod measurements;
#[cfg(unix)]
use std::{fs::Permissions, os::unix::fs::PermissionsExt};

fn uniq_path(label: &str) -> PathBuf {
    let ts = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or(Duration::from_secs(0))
        .as_nanos();
    env::temp_dir().join(format!("browsey-cliptest-{label}-{ts}"))
}

fn ensure_undo_dir() -> PathBuf {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| {
        let dir = uniq_path("undo-base");
        let _ = fs::remove_dir_all(&dir);
        env::set_var("BROWSEY_UNDO_DIR", &dir);
        dir
    })
    .clone()
}

fn clear_clipboard() {
    set_clipboard_impl(Vec::new(), "copy".to_string()).expect("clear clipboard");
}

fn clipboard_test_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn lock_clipboard_test() -> std::sync::MutexGuard<'static, ()> {
    clipboard_test_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn write_file(path: &Path, content: &[u8]) {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let mut f = fs::File::create(path).unwrap();
    f.write_all(content).unwrap();
}

#[test]
fn overwrite_interruption_child() {
    use crate::fs_utils::copy_test_hooks::{Phase, Scope};
    use std::io::Read;
    let Some(root) = env::var_os("BROWSEY_TEST_OVERWRITE_INTERRUPTION_ROOT").map(PathBuf::from)
    else {
        return;
    };
    let nested = env::var_os("BROWSEY_TEST_OVERWRITE_NESTED").is_some();
    let moving = env::var_os("BROWSEY_TEST_OVERWRITE_MOVING").is_some();
    let boundary = env::var("BROWSEY_TEST_OVERWRITE_BOUNDARY").unwrap_or_else(|_| "stream".into());
    let relative = if nested {
        "folder/file.bin"
    } else {
        "file.bin"
    };
    let source = root.join("source").join(relative);
    let target = root.join("target").join(relative);
    let paused_target = target.clone();
    let _scope = Scope::new(move |src, dst, phase, bytes| {
        if moving && phase == Phase::Rename {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "force copy fallback",
            ));
        }
        let pause = match boundary.as_str() {
            "prepared" => src == paused_target && phase == Phase::OverwritePrepared,
            "backed-up" => src == paused_target && phase == Phase::OverwriteBackedUp,
            _ => dst == paused_target && phase == Phase::Write && bytes == 0,
        };
        if pause {
            println!("BROWSEY_OVERWRITE_PAUSED");
            std::io::stdout().flush()?;
            std::io::stdin().read_exact(&mut [0_u8])?;
        }
        Ok(())
    });
    let entry = if nested {
        source.parent().unwrap()
    } else {
        &source
    };
    set_clipboard_impl(
        vec![entry.to_string_lossy().into()],
        if moving { "cut" } else { "copy" }.into(),
    )
    .unwrap();
    paste_clipboard_core(
        None,
        root.join("target").to_string_lossy().into(),
        Some("overwrite".into()),
        UndoState::default().clone_inner(),
        CancelState::default(),
        None,
    )
    .unwrap();
}

#[test]
fn killed_overwrite_keeps_original_backup_before_failure_rollback() {
    use std::io::{BufRead, BufReader};
    use std::process::{Command, Stdio};
    for moving in [false, true] {
        for nested in [false, true] {
            for boundary in ["prepared", "backed-up", "stream"] {
                let root = uniq_path("overwrite-killed");
                let relative = if nested {
                    "folder/file.bin"
                } else {
                    "file.bin"
                };
                let source = root.join("source").join(relative);
                let target = root.join("target").join(relative);
                let undo_root = root.join("undo");
                write_file(&source, b"replacement document");
                write_file(&target, b"original document");
                let mut command = Command::new(env::current_exe().unwrap());
                command
                    .args([
                        "--exact",
                        "clipboard::tests::overwrite_interruption_child",
                        "--nocapture",
                    ])
                    .env("BROWSEY_TEST_OVERWRITE_INTERRUPTION_ROOT", &root)
                    .env("BROWSEY_TEST_OVERWRITE_BOUNDARY", boundary)
                    .env("BROWSEY_UNDO_DIR", &undo_root)
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped());
                if nested {
                    command.env("BROWSEY_TEST_OVERWRITE_NESTED", "1");
                }
                if moving {
                    command.env("BROWSEY_TEST_OVERWRITE_MOVING", "1");
                }
                let mut child = command.spawn().unwrap();
                let stdout = child.stdout.take().unwrap();
                let (ready_tx, ready_rx) = std::sync::mpsc::channel();
                let reader = std::thread::spawn(move || {
                    let ready = BufReader::new(stdout)
                        .lines()
                        .map_while(Result::ok)
                        .any(|line| line.contains("BROWSEY_OVERWRITE_PAUSED"));
                    let _ = ready_tx.send(ready);
                });
                let ready = ready_rx
                    .recv_timeout(Duration::from_secs(10))
                    .unwrap_or(false);
                let _ = child.kill();
                let status = child.wait().unwrap();
                reader.join().unwrap();
                assert!(ready, "child must pause before any error rollback");
                assert!(!status.success());
                let cleanup = Command::new(env::current_exe().unwrap())
                    .args(["--exact", "undo::backup::tests::cleanup_child"])
                    .env("BROWSEY_TEST_CLEANUP_ROOT", &undo_root)
                    .output()
                    .unwrap();
                assert!(cleanup.status.success());
                let backups: Vec<_> = fs::read_dir(&undo_root)
                    .unwrap()
                    .filter_map(Result::ok)
                    .filter(|entry| entry.path().is_dir())
                    .flat_map(|entry| fs::read_dir(entry.path()).unwrap().filter_map(Result::ok))
                    .filter(|entry| entry.path().is_dir())
                    .map(|entry| entry.path().join("file.bin"))
                    .filter(|path| path.is_file())
                    .collect();
                let marker_count = fs::read_dir(&undo_root)
                    .unwrap()
                    .filter_map(Result::ok)
                    .filter(|entry| entry.path().is_dir())
                    .flat_map(|entry| fs::read_dir(entry.path()).unwrap().filter_map(Result::ok))
                    .filter(|entry| {
                        entry
                            .file_name()
                            .to_string_lossy()
                            .ends_with(".recovery-required")
                    })
                    .count();
                let source_data = fs::read(&source).unwrap();
                let original_data = backups.first().map(fs::read);
                let target_data = fs::read(&target);
                fs::remove_dir_all(root).unwrap();
                assert_eq!(source_data, b"replacement document");
                assert_eq!(
                    marker_count, 1,
                    "keep protection even before the original moves"
                );
                if boundary == "prepared" {
                    assert!(backups.is_empty(), "original has not moved yet");
                    assert_eq!(target_data.unwrap(), b"original document");
                    continue;
                }
                assert_eq!(
                    backups.len(),
                    1,
                    "original must survive cleanup without rollback"
                );
                assert_eq!(original_data.unwrap().unwrap(), b"original document");
                if boundary == "backed-up" {
                    assert_eq!(
                        target_data.unwrap_err().kind(),
                        std::io::ErrorKind::NotFound
                    );
                } else {
                    assert!(target_data.unwrap().is_empty(), "no new data written yet");
                }
            }
        }
    }
}

#[test]
fn overwrite_protection_failures_leave_the_original_untouched() {
    use crate::fs_utils::copy_test_hooks::{Phase, Scope};
    let _guard = lock_clipboard_test();
    let _ = ensure_undo_dir();
    for failure in ["create", "sync", "replace", "collision"] {
        let root = uniq_path("overwrite-protection-failure");
        let target = root.join("original.bin");
        write_file(&target, b"original document");
        let candidate = std::rc::Rc::new(std::cell::RefCell::new(None::<PathBuf>));
        let observed = candidate.clone();
        let scope = Scope::new(move |_, backup, phase, _| {
            if phase == Phase::RecoveryMarker {
                *observed.borrow_mut() = Some(backup.into());
            }
            if (failure == "create" && phase == Phase::RecoveryMarker)
                || (failure == "sync" && phase == Phase::RecoveryMarkerSync)
            {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::StorageFull,
                    "injected protection failure",
                ));
            }
            if phase == Phase::OverwritePrepared {
                if failure == "collision" {
                    fs::write(backup, b"competing backup")?;
                }
                if failure == "replace" {
                    let marker = overwrite_marker_path(backup);
                    fs::rename(&marker, marker.with_extension("parked"))?;
                    fs::write(marker, b"foreign marker")?;
                }
            }
            Ok(())
        });
        let mut actions = Vec::new();
        let result = backup_existing_target(&target, &mut actions);
        drop(scope);
        let candidate = candidate.borrow().clone().unwrap();
        let original = fs::read(&target).unwrap();
        let competing = fs::read(&candidate);
        let marker = fs::read(overwrite_marker_path(&candidate));
        fs::remove_dir_all(root).unwrap();
        let error = result.unwrap_err();
        assert_eq!(original, b"original document");
        assert!(actions.is_empty());
        assert!(error.to_string().contains(&candidate.display().to_string()));
        if failure == "collision" {
            assert_eq!(competing.unwrap(), b"competing backup");
        } else {
            assert_eq!(competing.unwrap_err().kind(), std::io::ErrorKind::NotFound);
        }
        if failure == "replace" {
            assert_eq!(marker.unwrap(), b"foreign marker");
        }
    }
}

fn overwrite_marker_path(backup: &Path) -> PathBuf {
    let bucket = backup.parent().unwrap();
    let mut name = bucket.file_name().unwrap().to_os_string();
    name.push(".recovery-required");
    bucket.parent().unwrap().join(name)
}

#[test]
fn overwrite_protection_spans_whole_paste_and_replays_with_history() {
    use crate::fs_utils::copy_test_hooks::{Phase, Scope};
    let _guard = lock_clipboard_test();
    let _ = ensure_undo_dir();
    clear_clipboard();
    let root = uniq_path("overwrite-marker-lifecycle");
    let first = root.join("source/first.bin");
    let second = root.join("source/second.bin");
    let dest = root.join("target");
    write_file(&first, b"first new");
    write_file(&second, b"second new");
    write_file(&dest.join("first.bin"), b"first original");
    write_file(&dest.join("second.bin"), b"second original");
    let backups = std::rc::Rc::new(std::cell::RefCell::new(Vec::<PathBuf>::new()));
    let observed = backups.clone();
    let scope = Scope::new(move |_, backup, phase, _| {
        if phase == Phase::OverwriteBackedUp {
            assert!(overwrite_marker_path(backup).is_file());
            observed.borrow_mut().push(backup.into());
        }
        Ok(())
    });
    let checked = backups.clone();
    set_after_paste_item_test_hook(Some(Box::new(move || {
        for backup in checked.borrow().iter() {
            assert!(
                overwrite_marker_path(backup).is_file(),
                "keep protection through the entire batch"
            );
        }
    })));
    set_clipboard_impl(
        vec![
            first.to_string_lossy().into(),
            second.to_string_lossy().into(),
        ],
        "copy".into(),
    )
    .unwrap();
    let undo = UndoState::default();
    let result = paste_clipboard_core(
        None,
        dest.to_string_lossy().into(),
        Some("overwrite".into()),
        undo.clone_inner(),
        CancelState::default(),
        None,
    );
    set_after_paste_item_test_hook(None);
    drop(scope);
    result.unwrap();
    let backups = backups.borrow().clone();
    assert_eq!(backups.len(), 2);
    for backup in &backups {
        assert!(!overwrite_marker_path(backup).exists());
    }
    undo.undo().unwrap();
    assert_eq!(fs::read(dest.join("first.bin")).unwrap(), b"first original");
    assert_eq!(
        fs::read(dest.join("second.bin")).unwrap(),
        b"second original"
    );
    for backup in &backups {
        assert!(!overwrite_marker_path(backup).exists());
    }
    undo.redo().unwrap();
    assert_eq!(fs::read(dest.join("first.bin")).unwrap(), b"first new");
    assert_eq!(fs::read(dest.join("second.bin")).unwrap(), b"second new");
    assert_eq!(fs::read(&backups[0]).unwrap(), b"first original");
    assert_eq!(fs::read(&backups[1]).unwrap(), b"second original");
    for backup in &backups {
        assert!(!overwrite_marker_path(backup).exists());
    }
    clear_clipboard();
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn local_copy_rejects_target_edits_masked_by_later_writes() {
    use crate::fs_utils::copy_test_hooks::{Phase, Scope};
    for moving in [false, true] {
        let root = uniq_path("copy-masked-target-edit");
        let source = root.join("source.bin");
        let target = root.join("target.bin");
        let data = vec![0x41; 32 * 1024];
        write_file(&source, &data);
        let observed = std::rc::Rc::new(std::cell::Cell::new(false));
        let reached = observed.clone();
        let scope = Scope::new(move |_, dst, phase, bytes| {
            if moving && phase == Phase::Rename {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Unsupported,
                    "force fallback",
                ));
            }
            if phase == Phase::Write && bytes == 8192 && !reached.replace(true) {
                fs::OpenOptions::new()
                    .write(true)
                    .open(dst)?
                    .write_all(b"foreign edit")?;
            }
            Ok(())
        });
        let result = if moving {
            move_entry(&source, &target, None, None, None)
        } else {
            copy_entry(&source, &target, None, None, None).map(|_| ())
        };
        drop(scope);
        let source_data = fs::read(&source);
        let target_data = fs::read(&target).unwrap();
        fs::remove_dir_all(root).unwrap();
        assert!(observed.get());
        assert!(
            result.is_err(),
            "must not adopt a foreign edit into a successful receipt"
        );
        assert_eq!(
            source_data.unwrap(),
            data,
            "unsafe output must not justify source removal"
        );
        assert!(target_data.starts_with(b"foreign edit"));
    }
}

#[test]
fn local_copy_readback_faults_and_cancellation_preserve_both_paths() {
    use crate::fs_utils::copy_test_hooks::{Phase, Scope};
    for moving in [false, true] {
        for fault in ["read", "cancel", "target", "source", "grow"] {
            let root = uniq_path("copy-readback-fault");
            let source = root.join("source.bin");
            let target = root.join("target.bin");
            let data = vec![0x43; 32 * 1024];
            write_file(&source, &data);
            let cancel = std::sync::Arc::new(AtomicBool::new(false));
            let hook_cancel = cancel.clone();
            let hook_source = source.clone();
            let observed = std::rc::Rc::new(std::cell::Cell::new(false));
            let reached = observed.clone();
            let scope = Scope::new(move |_, dst, phase, bytes| {
                if moving && phase == Phase::Rename {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::Unsupported,
                        "force fallback",
                    ));
                }
                if phase == Phase::Readback && bytes == 0 && !reached.replace(true) {
                    match fault {
                        "read" => return Err(std::io::Error::other("injected readback failure")),
                        "cancel" => hook_cancel.store(true, Ordering::Relaxed),
                        "target" => fs::OpenOptions::new()
                            .write(true)
                            .open(dst)?
                            .write_all(b"foreign edit")?,
                        "source" => fs::write(&hook_source, b"changed source")?,
                        "grow" => fs::OpenOptions::new()
                            .append(true)
                            .open(dst)?
                            .write_all(&vec![0x44; 128 * 1024])?,
                        _ => unreachable!(),
                    }
                }
                Ok(())
            });
            let result = if moving {
                move_entry(&source, &target, None, None, Some(&cancel))
            } else {
                copy_entry(&source, &target, None, None, Some(&cancel)).map(|_| ())
            };
            drop(scope);
            let source_data = fs::read(&source).unwrap();
            let target_data = fs::read(&target).unwrap();
            fs::remove_dir_all(root).unwrap();
            let error = result.expect_err("uncertain verification must refuse completion");
            assert!(observed.get());
            assert!(error.to_string().contains("no cleanup attempted"));
            assert_eq!(
                source_data,
                if fault == "source" {
                    b"changed source".to_vec()
                } else {
                    data.clone()
                }
            );
            if fault == "cancel" {
                assert_eq!(error.code(), ClipboardErrorCode::Cancelled);
            }
            if fault == "target" {
                assert!(target_data.starts_with(b"foreign edit"));
            } else if fault == "grow" {
                assert_eq!(target_data.len(), data.len() + 128 * 1024);
            } else {
                assert_eq!(target_data, data);
            }
        }
    }
}

#[test]
#[ignore = "opt-in warm-cache copy cost measurement; run with --release --ignored"]
fn copy_readback_warm_cache_cost() {
    use std::io::Read;
    use std::time::Instant;
    println!("copy-cost optimized profile: {}", !cfg!(debug_assertions));
    let root = uniq_path("copy-readback-cost");
    fs::create_dir(&root).unwrap();
    let source = root.join("source.bin");
    let chunk = vec![0x53; 1024 * 1024];
    let mut input = fs::File::create(&source).unwrap();
    let mut expected = blake3::Hasher::new();
    for _ in 0..64 {
        input.write_all(&chunk).unwrap();
        expected.update(&chunk);
    }
    input.sync_all().unwrap();
    drop(input);
    let expected = expected.finalize();
    for variant in [
        "native-reference",
        "manual-reference",
        "verified-clipboard",
        "verified-undo",
    ] {
        let mut samples = Vec::new();
        for sample in 0..5 {
            let target = root.join(format!("{variant}-{sample}.bin"));
            let started = Instant::now();
            match variant {
                "verified-clipboard" => {
                    assert_eq!(
                        copy_file_best_effort(&source, &target, None, None, None, None).unwrap(),
                        64 * 1024 * 1024
                    );
                }
                "verified-undo" => crate::undo::copy_entry(&source, &target).unwrap(),
                _ => {
                    let mut reader = fs::File::open(&source).unwrap();
                    let mut writer = fs::OpenOptions::new()
                        .write(true)
                        .create_new(true)
                        .open(&target)
                        .unwrap();
                    if variant == "native-reference" {
                        std::io::copy(&mut reader, &mut writer).unwrap();
                    } else {
                        let mut buf = vec![0_u8; 512 * 1024];
                        loop {
                            let n = reader.read(&mut buf).unwrap();
                            if n == 0 {
                                break;
                            }
                            writer.write_all(&buf[..n]).unwrap();
                        }
                    }
                    writer.sync_all().unwrap();
                }
            }
            samples.push(started.elapsed());
            // Validate every result outside the measured interval.
            let mut result = fs::File::open(&target).unwrap();
            let mut hash = blake3::Hasher::new();
            let mut buf = vec![0_u8; 256 * 1024];
            loop {
                let n = result.read(&mut buf).unwrap();
                if n == 0 {
                    break;
                }
                hash.update(&buf[..n]);
            }
            assert_eq!(hash.finalize(), expected);
            drop(result);
            fs::remove_file(target).unwrap();
        }
        samples.sort();
        println!(
            "{variant}: 64 MiB, five warm-cache samples, median={:.2} ms, max={:.2} ms",
            samples[2].as_secs_f64() * 1000.0,
            samples[4].as_secs_f64() * 1000.0
        );
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn failed_streaming_copy_preserves_in_place_target_edits() {
    use crate::fs_utils::copy_test_hooks::{Phase, Scope};
    for moving in [false, true] {
        for failure in ["read", "write", "cancel"] {
            let root = uniq_path("streaming-target-edit");
            let source = root.join("source.bin");
            let target = root.join("target.bin");
            let data = vec![0x58; 64 * 1024];
            write_file(&source, &data);
            let cancel = std::sync::Arc::new(AtomicBool::new(false));
            let hook_cancel = cancel.clone();
            let reached = std::rc::Rc::new(std::cell::Cell::new(false));
            let observed = reached.clone();
            let scope = Scope::new(move |_, dst, phase, bytes| {
                if moving && phase == Phase::Rename {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::Unsupported,
                        "force copy fallback",
                    ));
                }
                let boundary = if failure == "write" {
                    Phase::Write
                } else {
                    Phase::Read
                };
                if phase == boundary && bytes == 8192 && !observed.replace(true) {
                    // Same inode: identity-only cleanup must not erase this edit.
                    fs::OpenOptions::new()
                        .write(true)
                        .open(dst)?
                        .write_all(b"foreign document")?;
                    if failure == "cancel" {
                        hook_cancel.store(true, Ordering::Relaxed);
                        return Ok(());
                    }
                    return Err(std::io::Error::other("injected stream failure"));
                }
                Ok(())
            });
            let result = if moving {
                move_entry(&source, &target, None, None, Some(&cancel))
            } else {
                copy_entry(&source, &target, None, None, Some(&cancel)).map(|_| ())
            };
            drop(scope);
            let retained = fs::read(&target);
            let source_data = fs::read(&source).unwrap();
            fs::remove_dir_all(root).unwrap();
            let error = result.expect_err("stream fault must fail");
            assert!(reached.get());
            assert_eq!(source_data, data);
            assert!(
                retained.unwrap().starts_with(b"foreign document"),
                "{error}"
            );
            if failure == "cancel" {
                assert_eq!(error.code(), ClipboardErrorCode::Cancelled);
            }
            assert!(error.to_string().contains("no cleanup attempted"));
        }
    }
}

#[test]
fn failed_overwrite_preserves_foreign_edits_and_reports_protected_original() {
    use crate::fs_utils::copy_test_hooks::{Phase, Scope};
    let _guard = lock_clipboard_test();
    let _ = ensure_undo_dir();
    for moving in [false, true] {
        let root = uniq_path("overwrite-foreign-edit");
        let source = root.join("source/file.bin");
        let target = root.join("target/file.bin");
        let data = vec![0x46; 64 * 1024];
        write_file(&source, &data);
        write_file(&target, b"original document");
        let backup = std::rc::Rc::new(std::cell::RefCell::new(None::<PathBuf>));
        let observed = backup.clone();
        let scope = Scope::new(move |_, dst, phase, bytes| {
            if phase == Phase::RecoveryMarker {
                *observed.borrow_mut() = Some(dst.to_path_buf());
            }
            if moving && phase == Phase::Rename {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Unsupported,
                    "force copy fallback",
                ));
            }
            if phase == Phase::Write && bytes == 8192 {
                fs::OpenOptions::new()
                    .write(true)
                    .open(dst)?
                    .write_all(b"foreign document")?;
                return Err(std::io::Error::other("injected overwrite failure"));
            }
            Ok(())
        });
        set_clipboard_impl(
            vec![source.to_string_lossy().into()],
            if moving { "cut" } else { "copy" }.into(),
        )
        .unwrap();
        let undo = UndoState::default();
        let result = paste_clipboard_core(
            None,
            target.parent().unwrap().to_string_lossy().into(),
            Some("overwrite".into()),
            undo.clone_inner(),
            CancelState::default(),
            None,
        );
        drop(scope);
        let error = result.unwrap_err();
        let backup = backup.borrow().clone().expect("original must be protected");
        assert_eq!(error.code(), ClipboardErrorCode::RollbackFailed, "{error}");
        assert!(error.to_string().contains(&backup.display().to_string()));
        assert!(error.to_string().contains(&target.display().to_string()));
        assert_eq!(fs::read(&backup).unwrap(), b"original document");
        assert!(fs::read(&target).unwrap().starts_with(b"foreign document"));
        assert_eq!(fs::read(&source).unwrap(), data);
        assert!(current_clipboard().is_some());
        assert!(
            undo.undo().is_err(),
            "failed paste is not an undoable success"
        );
        clear_clipboard();
        fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn local_copy_and_move_faults_preserve_sources_and_retain_uncertain_targets() {
    use crate::fs_utils::copy_test_hooks::{Phase, Scope};
    use std::io::{Error, ErrorKind};

    for moving in [false, true] {
        for (phase, kind) in [
            (Phase::Write, ErrorKind::StorageFull),
            (Phase::Read, ErrorKind::NotFound),
            (Phase::Write, ErrorKind::BrokenPipe),
            (Phase::Sync, ErrorKind::StorageFull),
            (Phase::Sync, ErrorKind::Other),
        ] {
            let root = uniq_path("local-copy-fault");
            let source = root.join("source.bin");
            let target = root.join("target.bin");
            let unrelated = root.join("unrelated.bin");
            let data = vec![0x5a; 64 * 1024];
            write_file(&source, &data);
            write_file(&unrelated, b"unrelated-data");
            let expected_source = source.clone();
            let expected_target = target.clone();
            let reached = std::rc::Rc::new(std::cell::Cell::new(false));
            let observed = reached.clone();
            let scope = Scope::new(move |src, dst, current, bytes| {
                assert_eq!(src, expected_source);
                assert_eq!(dst, expected_target);
                if moving && current == Phase::Rename {
                    return Err(Error::new(ErrorKind::Unsupported, "force copy fallback"));
                }
                if current == phase && bytes > 0 {
                    observed.set(true);
                    return Err(Error::new(kind, "injected local copy fault"));
                }
                Ok(())
            });
            let result = if moving {
                move_entry(&source, &target, None, None, None)
            } else {
                copy_entry(&source, &target, None, None, None).map(|_| ())
            };
            drop(scope);
            let source_data = fs::read(&source).unwrap();
            let unrelated_data = fs::read(&unrelated).unwrap();
            let target_exists = target.exists();
            fs::remove_dir_all(root).unwrap();
            let error = result.expect_err("injected fault must fail the operation");
            assert!(reached.get(), "fault must occur after data was copied");
            assert!(error.to_string().contains("injected local copy fault"));
            assert_eq!(
                error.code(),
                if kind == ErrorKind::NotFound {
                    ClipboardErrorCode::NotFound
                } else {
                    ClipboardErrorCode::IoError
                }
            );
            assert_eq!(source_data, data);
            assert_eq!(unrelated_data, b"unrelated-data");
            assert!(target_exists, "uncertain output must not be deleted");
            assert!(error.to_string().contains("no cleanup attempted"));
        }
    }
}

#[test]
fn local_copy_refuses_sources_changed_during_streaming_and_retains_uncertain_output() {
    use crate::fs_utils::copy_test_hooks::{Phase, Scope};
    for moving in [false, true] {
        for change in ["rewrite", "truncate", "append"] {
            let root = uniq_path("source-stream-change");
            let source = root.join("source.bin");
            let target = root.join("target.bin");
            write_file(&source, &vec![0x41; 32 * 1024]);
            let changed = std::rc::Rc::new(std::cell::Cell::new(false));
            let observed = changed.clone();
            let scope = Scope::new(move |src, _, phase, bytes| {
                if moving && phase == Phase::Rename {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::Unsupported,
                        "force fallback",
                    ));
                }
                if phase == Phase::Read && bytes == 8192 && !observed.replace(true) {
                    match change {
                        "rewrite" => fs::write(src, vec![0x42; 32 * 1024])?,
                        "truncate" => fs::write(src, b"changed")?,
                        _ => fs::OpenOptions::new()
                            .append(true)
                            .open(src)?
                            .write_all(b"appended")?,
                    }
                    let file = fs::File::open(src)?;
                    file.set_times(
                        fs::FileTimes::new()
                            .set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(100)),
                    )?;
                }
                Ok(())
            });
            let result = if moving {
                move_entry(&source, &target, None, None, None)
            } else {
                copy_entry(&source, &target, None, None, None).map(|_| ())
            };
            drop(scope);
            let source_data = fs::read(&source).unwrap();
            let target_exists = target.exists();
            fs::remove_dir_all(root).unwrap();
            let error =
                result.expect_err("a mixed-version copy must not be reported as successful");
            assert!(changed.get());
            assert!(error.to_string().contains("Source changed"));
            assert!(error.to_string().contains("retained"));
            assert!(
                target_exists,
                "uncertain finalized output is kept for inspection"
            );
            assert_eq!(
                source_data,
                match change {
                    "rewrite" => vec![0x42; 32 * 1024],
                    "truncate" => b"changed".to_vec(),
                    _ => [vec![0x41; 32 * 1024], b"appended".to_vec()].concat(),
                }
            );
        }
    }
}

#[test]
fn local_copy_and_move_preserve_targets_edited_during_finalization() {
    use crate::fs_utils::copy_test_hooks::{Phase, Scope};
    for moving in [true, false] {
        for failure in [false, true] {
            let root = uniq_path("edited-finalizing-target");
            let source = root.join("source.bin");
            let target = root.join("target.bin");
            let data = vec![0x41; 32 * 1024];
            write_file(&source, &data);
            let scope = Scope::new(move |_, dst, phase, _| {
                if moving && phase == Phase::Rename {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::Unsupported,
                        "force fallback",
                    ));
                }
                if phase == if failure { Phase::Sync } else { Phase::Synced } {
                    fs::write(dst, b"other-writer-data")?;
                    if failure {
                        return Err(std::io::Error::other("injected finalization failure"));
                    }
                }
                Ok(())
            });
            let result = if moving {
                move_entry(&source, &target, None, None, None)
            } else {
                copy_entry(&source, &target, None, None, None).map(|_| ())
            };
            drop(scope);
            let source_data = fs::read(&source);
            let target_data = fs::read(&target);
            fs::remove_dir_all(root).unwrap();
            assert!(
                result.is_err(),
                "edited output must not be adopted as a successful copy"
            );
            assert_eq!(
                source_data.unwrap(),
                data,
                "a changed target cannot justify deleting its source"
            );
            assert_eq!(
                target_data.unwrap(),
                b"other-writer-data",
                "failure cleanup must not delete another writer's edits"
            );
        }
    }
}

#[test]
fn local_fallback_move_rechecks_outputs_and_preserves_late_source_changes() {
    use crate::fs_utils::copy_test_hooks::{Phase, Scope};
    for change in [
        "target-edit",
        "target-child",
        "source-child",
        "source-edit",
        "source-entry",
        "source-late-child",
    ] {
        let root = uniq_path("fallback-last-gate");
        let source = root.join("source");
        let target = root.join("target");
        write_file(&source.join("a.bin"), b"first-original");
        write_file(&source.join("z.bin"), b"last-original");
        let src_root = source.clone();
        let dst_root = target.clone();
        let scope = Scope::new(move |src, dst, phase, _| {
            if phase == Phase::Rename {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Unsupported,
                    "force fallback",
                ));
            }
            if phase == Phase::BeforeSourceDelete {
                match change {
                    "target-edit" => fs::write(dst.join("a.bin"), b"other-writer-data")?,
                    "target-child" => fs::write(dst.join("new.txt"), b"foreign-child")?,
                    "source-child" => fs::write(src.join("new.txt"), b"late-source-child")?,
                    "source-edit" => fs::write(src.join("a.bin"), b"late-source-edit")?,
                    _ => {}
                }
            }
            if change == "source-entry"
                && phase == Phase::CopyUndoEntry
                && src == src_root
                && dst == src_root.join("a.bin")
            {
                assert!(
                    !src_root.join("z.bin").exists(),
                    "source deletion has already progressed"
                );
                fs::write(dst, b"late-source-edit")?;
                assert_eq!(fs::read(dst_root.join("a.bin"))?, b"first-original");
            }
            if change == "source-late-child" && phase == Phase::CopyUndoVerified && src == src_root
            {
                fs::write(src.join("new.txt"), b"late-source-child")?;
            }
            Ok(())
        });
        let result = move_entry(&source, &target, None, None, None);
        drop(scope);
        let source_data = fs::read(source.join("a.bin"));
        let target_data = fs::read(target.join("a.bin")).unwrap();
        let new_source_data = fs::read(source.join("new.txt")).ok();
        let new_target_data = fs::read(target.join("new.txt")).ok();
        let other_source_data = fs::read(source.join("z.bin")).ok();
        fs::remove_dir_all(root).unwrap();
        let error =
            result.expect_err("a late mutation must not become a successful destructive move");
        assert!(error.to_string().contains("retained"));
        if change == "source-late-child" {
            assert_eq!(
                source_data.unwrap_err().kind(),
                std::io::ErrorKind::NotFound
            );
        } else {
            assert_eq!(
                source_data.unwrap(),
                if ["source-edit", "source-entry"].contains(&change) {
                    b"late-source-edit".as_slice()
                } else {
                    b"first-original".as_slice()
                }
            );
        }
        assert_eq!(
            target_data,
            if change == "target-edit" {
                b"other-writer-data".as_slice()
            } else {
                b"first-original".as_slice()
            }
        );
        if ["source-child", "source-late-child"].contains(&change) {
            assert_eq!(new_source_data.unwrap(), b"late-source-child");
        }
        if change == "target-child" {
            assert_eq!(new_target_data.unwrap(), b"foreign-child");
        }
        if !["source-entry", "source-late-child"].contains(&change) {
            assert_eq!(other_source_data.unwrap(), b"last-original");
        }
    }
}

#[test]
fn local_fallback_move_honors_cancellation_during_source_removal() {
    use crate::fs_utils::copy_test_hooks::{Phase, Scope};
    let root = uniq_path("fallback-removal-cancel");
    let source = root.join("source");
    let target = root.join("target");
    write_file(&source.join("a.bin"), b"first");
    write_file(&source.join("z.bin"), b"last");
    let cancel = std::sync::Arc::new(AtomicBool::new(false));
    let observed = cancel.clone();
    let checked_root = source.clone();
    let scope = Scope::new(move |src, dst, phase, _| {
        if phase == Phase::Rename {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "force fallback",
            ));
        }
        if phase == Phase::CopyUndoEntry && src == checked_root && dst == checked_root.join("a.bin")
        {
            assert!(!checked_root.join("z.bin").exists());
            observed.store(true, Ordering::Relaxed);
        }
        Ok(())
    });
    let result = move_entry(&source, &target, None, None, Some(&cancel));
    drop(scope);
    let remaining = fs::read(source.join("a.bin"));
    let target_a = fs::read(target.join("a.bin")).unwrap();
    let target_z = fs::read(target.join("z.bin")).unwrap();
    fs::remove_dir_all(root).unwrap();
    assert_eq!(result.unwrap_err().code(), ClipboardErrorCode::Cancelled);
    assert!(cancel.load(Ordering::Relaxed));
    assert_eq!(remaining.unwrap(), b"first");
    assert_eq!(target_a, b"first");
    assert_eq!(target_z, b"last");
}

#[test]
fn fallback_move_without_output_receipt_keeps_source_and_completed_output() {
    let root = uniq_path("fallback-opaque-writer");
    let source = root.join("source.bin");
    let target = root.join("target.bin");
    write_file(&source, b"original");
    write_file(&target, b"original");
    let source_tree = crate::fs_utils::TreeSnapshot::capture(&source).unwrap();
    let result = ops::finish_fallback_move(
        &source,
        &target,
        &source_tree,
        &crate::undo::CopyReceipt::default(),
        || Ok(()),
    );
    let source_data = fs::read(&source).unwrap();
    let target_data = fs::read(&target).unwrap();
    fs::remove_dir_all(root).unwrap();
    let error = result.unwrap_err();
    assert!(error.to_string().contains("source not removed"));
    assert!(error.to_string().contains("ownership"));
    assert_eq!(source_data, b"original");
    assert_eq!(target_data, b"original");
}

#[test]
#[cfg(unix)]
fn gvfs_fallback_move_uses_owned_writers_and_preserves_nested_contents() {
    use crate::fs_utils::copy_test_hooks::{Phase, Scope};
    let root = uniq_path("gvfs-owned-move");
    let source = root.join("source");
    let target = root.join("gvfs/provider/target");
    write_file(&source.join("nested/file.txt"), b"generated payload");
    fs::create_dir_all(source.join("empty")).unwrap();
    fs::create_dir_all(target.parent().unwrap()).unwrap();
    let _scope = Scope::new(|_, _, phase, _| match phase {
        Phase::Rename => Err(std::io::Error::from_raw_os_error(libc::EXDEV)),
        Phase::SetPermissions => Err(std::io::Error::from(std::io::ErrorKind::Unsupported)),
        _ => Ok(()),
    });
    move_entry(&source, &target, None, None, None).unwrap();
    assert!(!source.exists());
    assert_eq!(
        fs::read(target.join("nested/file.txt")).unwrap(),
        b"generated payload"
    );
    assert!(target.join("empty").is_dir());
    fs::remove_dir_all(root).unwrap();
}

#[test]
#[cfg(unix)]
fn gvfs_fallback_move_readback_failure_keeps_both_sides_without_retry() {
    use crate::fs_utils::copy_test_hooks::{Phase, Scope};
    let root = uniq_path("gvfs-owned-move-readback");
    let source = root.join("source.txt");
    let target = root.join("gvfs/provider/target.txt");
    write_file(&source, b"generated payload");
    fs::create_dir_all(target.parent().unwrap()).unwrap();
    let _scope = Scope::new(|_, _, phase, _| match phase {
        Phase::Rename => Err(std::io::Error::from_raw_os_error(libc::EXDEV)),
        Phase::Readback => Err(std::io::Error::other("generated readback fault")),
        Phase::BeforeSourceDelete => panic!("source must not be removed after failed verification"),
        _ => Ok(()),
    });
    let error = move_entry(&source, &target, None, None, None).unwrap_err();
    assert!(error.to_string().contains("verification failed"));
    assert_eq!(fs::read(&source).unwrap(), b"generated payload");
    assert_eq!(fs::read(&target).unwrap(), b"generated payload");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn local_move_cancelled_after_sync_retains_both_complete_copies() {
    use crate::fs_utils::copy_test_hooks::{Phase, Scope};
    let root = uniq_path("move-cancel-after-sync");
    let source = root.join("source.bin");
    let target = root.join("target.bin");
    let data = vec![0x37; 32 * 1024];
    write_file(&source, &data);
    let cancel = std::sync::Arc::new(AtomicBool::new(false));
    let hook_cancel = cancel.clone();
    let scope = Scope::new(move |_, _, phase, _| {
        if phase == Phase::Rename {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "force copy fallback",
            ));
        }
        if phase == Phase::Synced {
            hook_cancel.store(true, Ordering::Relaxed);
        }
        Ok(())
    });
    let result = move_entry(&source, &target, None, None, Some(&cancel));
    drop(scope);
    let source_data = fs::read(&source).unwrap();
    let target_data = fs::read(&target).unwrap();
    fs::remove_dir_all(root).unwrap();
    let error = result.unwrap_err();
    assert_eq!(error.code(), ClipboardErrorCode::Cancelled);
    assert!(error.to_string().contains("retained"));
    assert!(error.to_string().contains("no cleanup attempted"));
    assert_eq!(source_data, data);
    assert_eq!(target_data, data);
}

#[test]
fn local_move_cancelled_mid_stream_preserves_source_and_retains_uncertain_partial() {
    use crate::fs_utils::copy_test_hooks::{Phase, Scope};
    let root = uniq_path("move-cancel-mid-stream");
    let source = root.join("source.bin");
    let target = root.join("target.bin");
    let data = vec![0x36; 64 * 1024];
    write_file(&source, &data);
    let cancel = std::sync::Arc::new(AtomicBool::new(false));
    let hook_cancel = cancel.clone();
    let scope = Scope::new(move |_, _, phase, bytes| {
        if phase == Phase::Rename {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "force copy fallback",
            ));
        }
        if phase == Phase::Write && bytes == 8192 {
            hook_cancel.store(true, Ordering::Relaxed);
        }
        Ok(())
    });
    let result = move_entry(&source, &target, None, None, Some(&cancel));
    drop(scope);
    let source_data = fs::read(&source).unwrap();
    let target_exists = target.exists();
    fs::remove_dir_all(root).unwrap();
    assert!(
        cancel.load(Ordering::Relaxed),
        "cancellation must occur mid-stream"
    );
    assert_eq!(result.unwrap_err().code(), ClipboardErrorCode::Cancelled);
    assert_eq!(source_data, data);
    assert!(target_exists);
}

#[test]
fn local_copy_fault_scope_does_not_touch_existing_targets_or_leak_to_next_copy() {
    use crate::fs_utils::copy_test_hooks::{Phase, Scope};
    let root = uniq_path("copy-fault-scope");
    let source = root.join("source.bin");
    let existing = root.join("existing.bin");
    let next = root.join("next.bin");
    write_file(&source, b"source-data");
    write_file(&existing, b"existing-data");
    let scope = Scope::new(|_, _, phase, _| {
        assert_ne!(phase, Phase::Write, "existing target must not be written");
        assert_ne!(phase, Phase::Sync, "existing target must not be finalized");
        Err(std::io::Error::other("scope must not reach I/O"))
    });
    let result = copy_entry(&source, &existing, None, None, None);
    drop(scope);
    assert_eq!(
        result.unwrap_err().code(),
        ClipboardErrorCode::DestinationExists
    );
    copy_entry(&source, &next, None, None, None).unwrap();
    let existing_data = fs::read(&existing).unwrap();
    let next_data = fs::read(&next).unwrap();
    fs::remove_dir_all(root).unwrap();
    assert_eq!(existing_data, b"existing-data");
    assert_eq!(next_data, b"source-data");
}

#[cfg(unix)]
#[test]
fn local_move_with_unlinked_open_source_retains_completed_destination() {
    use crate::fs_utils::copy_test_hooks::{Phase, Scope};
    let root = uniq_path("move-unlinked-source");
    let source = root.join("source.bin");
    let target = root.join("target.bin");
    let data = vec![0x42; 32 * 1024];
    write_file(&source, &data);
    let scope = Scope::new(move |src, _, phase, bytes| {
        if phase == Phase::Rename {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "force copy fallback",
            ));
        }
        if phase == Phase::Read && bytes == 8192 {
            fs::remove_file(src)?;
        }
        Ok(())
    });
    let result = move_entry(&source, &target, None, None, None);
    drop(scope);
    let target_data = fs::read(&target).unwrap();
    let source_exists = source.exists();
    fs::remove_dir_all(root).unwrap();
    assert_eq!(result.unwrap_err().code(), ClipboardErrorCode::NotFound);
    assert!(!source_exists);
    assert_eq!(target_data, data);
}

#[cfg(unix)]
#[test]
fn local_move_with_unlinked_open_target_must_not_delete_source() {
    use crate::fs_utils::copy_test_hooks::{Phase, Scope};
    let root = uniq_path("move-unlinked-target");
    let source = root.join("source.bin");
    let target = root.join("target.bin");
    let data = vec![0x43; 32 * 1024];
    write_file(&source, &data);
    let scope = Scope::new(move |_, dst, phase, bytes| {
        if phase == Phase::Rename {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "force copy fallback",
            ));
        }
        if phase == Phase::Write && bytes == 8192 {
            fs::remove_file(dst)?;
        }
        Ok(())
    });
    let result = move_entry(&source, &target, None, None, None);
    drop(scope);
    let source_data = fs::read(&source);
    let target_exists = target.exists();
    fs::remove_dir_all(root).unwrap();
    assert!(
        result.is_err(),
        "sync of an unlinked inode is not successful delivery"
    );
    assert_eq!(source_data.unwrap(), data);
    assert!(!target_exists);
}

#[cfg(unix)]
#[test]
fn local_move_with_replaced_target_preserves_source_and_competing_file() {
    use crate::fs_utils::copy_test_hooks::{Phase, Scope};
    for sync_failure in [false, true] {
        let root = uniq_path("move-replaced-target");
        let source = root.join("source.bin");
        let target = root.join("target.bin");
        let retained = root.join("retained-copy.bin");
        let data = vec![0x45; 32 * 1024];
        write_file(&source, &data);
        let saved_copy = retained.clone();
        let scope = Scope::new(move |_, dst, phase, _| {
            if phase == Phase::Rename {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Unsupported,
                    "force copy fallback",
                ));
            }
            let replacement_phase = if sync_failure {
                Phase::Sync
            } else {
                Phase::Synced
            };
            if phase == replacement_phase {
                fs::rename(dst, &saved_copy)?;
                fs::write(dst, b"competing-data")?;
                if sync_failure {
                    return Err(std::io::Error::other("injected sync failure"));
                }
            }
            Ok(())
        });
        let result = move_entry(&source, &target, None, None, None);
        drop(scope);
        let source_data = fs::read(&source);
        let competing_data = fs::read(&target);
        let retained_data = fs::read(&retained).unwrap();
        fs::remove_dir_all(root).unwrap();
        assert!(
            result.is_err(),
            "delivery to a replaced path must not succeed"
        );
        assert_eq!(source_data.unwrap(), data);
        assert_eq!(competing_data.unwrap(), b"competing-data");
        assert_eq!(retained_data, data);
    }
}

#[test]
fn failed_nested_copy_preserves_untracked_files_and_reports_retained_paths() {
    use crate::fs_utils::copy_test_hooks::{Phase, Scope};
    let root = uniq_path("nested-untracked");
    let source = root.join("source");
    let target = root.join("target");
    write_file(&source.join("deep/file.bin"), &vec![0x41; 32 * 1024]);
    let scope = Scope::new(|_, dst, phase, bytes| {
        if phase == Phase::Write && bytes > 0 {
            fs::write(
                dst.parent().unwrap().join("untracked.txt"),
                b"other-process-data",
            )?;
            return Err(std::io::Error::other("injected nested write error"));
        }
        Ok(())
    });
    let result = copy_entry(&source, &target, None, None, None);
    drop(scope);
    let foreign = fs::read(target.join("deep/untracked.txt"));
    let owned_exists = target.join("deep/file.bin").exists();
    let source_data = fs::read(source.join("deep/file.bin")).unwrap();
    fs::remove_dir_all(root).unwrap();
    let error = result.unwrap_err();
    assert_eq!(foreign.unwrap(), b"other-process-data");
    assert!(owned_exists, "failed active output is not safe to delete");
    assert_eq!(source_data, vec![0x41; 32 * 1024]);
    assert!(error.to_string().contains("retained"));
}

#[cfg(unix)]
#[test]
fn failed_nested_copy_preserves_completed_files_edited_by_another_process() {
    use crate::fs_utils::copy_test_hooks::{Phase, Scope};
    let root = uniq_path("nested-edited-output");
    let source = root.join("source");
    let target = root.join("target");
    write_file(&source.join("deep/first.bin"), &vec![0x42; 32 * 1024]);
    write_file(&source.join("deep/second.bin"), &vec![0x43; 32 * 1024]);
    let edited = std::rc::Rc::new(std::cell::RefCell::new(None::<PathBuf>));
    let observed = edited.clone();
    let scope = Scope::new(move |_, dst, phase, _| {
        if phase == Phase::Synced && observed.borrow().is_none() {
            *observed.borrow_mut() = Some(dst.to_path_buf());
        } else if phase == Phase::Write {
            if let Some(first) = observed.borrow().as_ref() {
                fs::write(first, b"edited-after-copy")?;
                return Err(std::io::Error::other("injected next-file error"));
            }
        }
        Ok(())
    });
    let result = copy_entry(&source, &target, None, None, None);
    drop(scope);
    let edited_path = edited.borrow().clone().unwrap();
    let edited_data = fs::read(&edited_path);
    fs::remove_dir_all(root).unwrap();
    assert!(result.is_err());
    assert_eq!(edited_data.unwrap(), b"edited-after-copy");
}

#[cfg(unix)]
#[test]
fn failed_nested_copy_preserves_a_replaced_destination_directory() {
    use crate::fs_utils::copy_test_hooks::{Phase, Scope};
    let root = uniq_path("nested-replaced-directory");
    let source = root.join("source");
    let target = root.join("target");
    let retained = root.join("renamed-output");
    write_file(&source.join("deep/file.bin"), &vec![0x44; 32 * 1024]);
    let saved = retained.clone();
    let destination = target.clone();
    let scope = Scope::new(move |_, _, phase, _| {
        if phase == Phase::Sync {
            fs::rename(&destination, &saved)?;
            fs::create_dir_all(destination.join("deep"))?;
            fs::write(destination.join("deep/foreign.txt"), b"competing-directory")?;
            return Err(std::io::Error::other("injected sync error"));
        }
        Ok(())
    });
    let result = copy_entry(&source, &target, None, None, None);
    drop(scope);
    let foreign = fs::read(target.join("deep/foreign.txt"));
    let retained_data = fs::read(retained.join("deep/file.bin")).unwrap();
    fs::remove_dir_all(root).unwrap();
    assert!(result.is_err());
    assert_eq!(foreign.unwrap(), b"competing-directory");
    assert_eq!(retained_data, vec![0x44; 32 * 1024]);
}

#[test]
fn local_fallback_move_keeps_sources_edited_after_copying() {
    use crate::fs_utils::copy_test_hooks::{Phase, Scope};
    for directory in [false, true] {
        let root = uniq_path("move-edited-source");
        let source = root.join("source");
        let target = root.join("target");
        let source_file = if directory {
            source.join("deep/file.bin")
        } else {
            source.clone()
        };
        let target_file = if directory {
            target.join("deep/file.bin")
        } else {
            target.clone()
        };
        let data = vec![0x45; 32 * 1024];
        write_file(&source_file, &data);
        let scope = Scope::new(|src, _, phase, _| {
            if phase == Phase::Rename {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Unsupported,
                    "force fallback",
                ));
            }
            if phase == Phase::Synced {
                fs::write(src, b"edited-source-content")?;
            }
            Ok(())
        });
        let result = move_entry(&source, &target, None, None, None);
        drop(scope);
        let edited_source = fs::read(&source_file);
        let destination = fs::read(&target_file).unwrap();
        fs::remove_dir_all(root).unwrap();
        assert!(result.is_err(), "changed source must not be deleted");
        assert_eq!(edited_source.unwrap(), b"edited-source-content");
        assert_eq!(destination, data);
    }
}

#[test]
fn explicit_paste_ignores_clipboard_changes_between_preview_and_execution() {
    let _lock = lock_clipboard_test();
    ensure_undo_dir();
    for mode in ["copy", "cut"] {
        let root = uniq_path("explicit-paste");
        let source = root.join("src/report.txt");
        let unrelated = root.join("other/unrelated.txt");
        let dest = root.join("dest");
        write_file(&source, b"original-source");
        write_file(&unrelated, b"unrelated-source");
        write_file(&dest.join("report.txt"), b"old-destination");
        let input = || {
            Some(ClipboardInput {
                paths: vec![source.to_string_lossy().into_owned()],
                mode: mode.into(),
            })
        };
        set_clipboard_impl(vec![unrelated.to_string_lossy().into_owned()], "cut".into()).unwrap();
        let preview = preview_entries(dest.to_string_lossy().into_owned(), input()).unwrap();
        assert_eq!(preview.len(), 1);
        assert_eq!(preview[0].src, source.to_string_lossy());
        // A different operation replaces the shared clipboard after conflict inspection.
        set_clipboard_impl(
            vec![unrelated.to_string_lossy().into_owned()],
            "copy".into(),
        )
        .unwrap();
        let before = current_clipboard().unwrap();
        let undo = UndoState::default();
        paste_entries_core(
            None,
            dest.to_string_lossy().into_owned(),
            Some("overwrite".into()),
            undo.clone_inner(),
            CancelState::default(),
            None,
            input(),
        )
        .unwrap();
        assert_eq!(
            fs::read(dest.join("report.txt")).unwrap(),
            b"original-source"
        );
        assert_eq!(source.exists(), mode == "copy");
        assert_eq!(fs::read(&unrelated).unwrap(), b"unrelated-source");
        assert!(!dest.join("unrelated.txt").exists());
        assert!(current_clipboard().as_ref() == Some(&before));
        clear_clipboard();
        fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn explicit_paste_revalidates_sources_and_never_falls_back_to_shared_clipboard() {
    let _lock = lock_clipboard_test();
    ensure_undo_dir();
    let root = uniq_path("explicit-invalid");
    let source = root.join("source.txt");
    let dest = root.join("dest");
    write_file(&source, b"keep");
    fs::create_dir_all(&dest).unwrap();
    set_clipboard_impl(vec![source.to_string_lossy().into_owned()], "cut".into()).unwrap();
    let before = current_clipboard().unwrap();
    for input in [
        ClipboardInput {
            paths: vec![],
            mode: "copy".into(),
        },
        ClipboardInput {
            paths: vec![source.to_string_lossy().into_owned()],
            mode: "invalid".into(),
        },
        ClipboardInput {
            paths: vec![root.join("missing").to_string_lossy().into_owned()],
            mode: "copy".into(),
        },
        ClipboardInput {
            paths: vec!["rclone://remote/file".into()],
            mode: "copy".into(),
        },
    ] {
        let undo = UndoState::default();
        assert!(paste_entries_core(
            None,
            dest.to_string_lossy().into_owned(),
            None,
            undo.clone_inner(),
            CancelState::default(),
            None,
            Some(input)
        )
        .is_err());
        assert_eq!(fs::read(&source).unwrap(), b"keep");
        assert_eq!(fs::read_dir(&dest).unwrap().count(), 0);
        assert!(current_clipboard().as_ref() == Some(&before));
    }
    #[cfg(unix)]
    {
        let link = root.join("link");
        symlink(&source, &link).unwrap();
        assert!(preview_entries(
            dest.to_string_lossy().into_owned(),
            Some(ClipboardInput {
                paths: vec![link.to_string_lossy().into_owned()],
                mode: "copy".into(),
            })
        )
        .is_err());
    }
    clear_clipboard();
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn merge_copy_can_undo_without_touching_existing() {
    let _ = ensure_undo_dir();
    let base = uniq_path("merge-copy");
    let dest = base.join("dest");
    fs::create_dir_all(&dest).unwrap();
    write_file(&dest.join("old.txt"), b"old");

    let src = dest.join("child");
    fs::create_dir_all(&src).unwrap();
    write_file(&src.join("a.txt"), b"a");

    let mut actions = Vec::new();
    merge_dir(
        &src,
        &dest,
        ClipboardMode::Copy,
        &mut actions,
        None,
        None,
        None,
    )
    .unwrap();

    assert!(dest.join("old.txt").exists());
    assert!(dest.join("a.txt").exists());
    assert!(src.join("a.txt").exists());

    run_actions(&mut actions, Direction::Backward).unwrap();

    assert!(dest.join("old.txt").exists());
    assert!(!dest.join("a.txt").exists());
    assert!(src.join("a.txt").exists());

    let _ = fs::remove_dir_all(&base);
}

#[test]
fn merge_copy_undo_preserves_edited_output_and_overwrite_backup() {
    let _guard = lock_clipboard_test();
    let _ = ensure_undo_dir();
    for directory in [false, true] {
        let root = uniq_path("merge-copy-edited-output");
        let source = root.join("source");
        let destination = root.join("destination");
        let child = if directory {
            "item/nested/file.txt"
        } else {
            "item"
        };
        write_file(&source.join(child), b"original");
        fs::create_dir_all(&destination).unwrap();
        // A pre-existing file replaced by a directory exercises the other
        // merge branch as well as preservation of a Delete action's backup.
        write_file(&destination.join("item"), b"preexisting");
        let mut actions = Vec::new();
        merge_dir(
            &source,
            &destination,
            ClipboardMode::Copy,
            &mut actions,
            None,
            None,
            None,
        )
        .unwrap();
        let backup = match &actions[0] {
            Action::Delete { backup, .. } => backup.clone(),
            _ => panic!("overwrite must retain a backup"),
        };
        write_file(&destination.join(child), b"edited");
        let error = run_actions(&mut actions, Direction::Backward).unwrap_err();
        assert!(error.to_string().contains("retained"));
        assert_eq!(fs::read(destination.join(child)).unwrap(), b"edited");
        assert_eq!(fs::read(source.join(child)).unwrap(), b"original");
        assert_eq!(fs::read(&backup).unwrap(), b"preexisting");
        fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn paste_copy_undo_redo_records_current_output_versions() {
    let _guard = lock_clipboard_test();
    let _ = ensure_undo_dir();
    for directory in [false, true] {
        clear_clipboard();
        let root = uniq_path("paste-copy-receipt-roundtrip");
        let source = root.join("source/item");
        let destination = root.join("destination");
        let original = if directory {
            source.join("nested/file.txt")
        } else {
            source.clone()
        };
        write_file(&original, b"original");
        fs::create_dir_all(&destination).unwrap();
        set_clipboard_impl(vec![source.to_string_lossy().into()], "copy".into()).unwrap();
        let undo = UndoState::default();
        paste_clipboard_core(
            None,
            destination.to_string_lossy().into(),
            None,
            undo.clone_inner(),
            CancelState::default(),
            None,
        )
        .unwrap();
        undo.undo().unwrap();
        assert!(!destination.join("item").exists());
        // Redo must preserve the copied bytes, while renewing ownership of the
        // newly restored output instead of reusing the old writer's identity.
        write_file(&original, b"new-source");
        undo.redo().unwrap();
        let output = if directory {
            destination.join("item/nested/file.txt")
        } else {
            destination.join("item")
        };
        assert_eq!(fs::read(&output).unwrap(), b"original");
        undo.undo().unwrap();
        undo.redo().unwrap();
        write_file(&output, b"edited-output");
        assert!(undo.undo().is_err());
        assert_eq!(fs::read(output).unwrap(), b"edited-output");
        fs::remove_dir_all(root).unwrap();
    }
    clear_clipboard();
}

#[test]
fn merge_cut_undo_restores_source_and_target() {
    let _ = ensure_undo_dir();
    let base = uniq_path("merge-cut");
    let dest = base.join("dest");
    fs::create_dir_all(&dest).unwrap();
    write_file(&dest.join("old.txt"), b"old");

    let src = dest.join("child");
    fs::create_dir_all(&src).unwrap();
    write_file(&src.join("a.txt"), b"a");

    let mut actions = Vec::new();
    merge_dir(
        &src,
        &dest,
        ClipboardMode::Cut,
        &mut actions,
        None,
        None,
        None,
    )
    .unwrap();

    assert!(dest.join("old.txt").exists());
    assert!(dest.join("a.txt").exists());
    assert!(!src.exists());

    run_actions(&mut actions, Direction::Backward).unwrap();

    assert!(src.join("a.txt").exists());
    assert!(dest.join("old.txt").exists());
    assert!(!dest.join("a.txt").exists());

    let _ = fs::remove_dir_all(&base);
}

#[test]
fn copy_file_best_effort_does_not_overwrite_existing_target() {
    let base = uniq_path("copy-no-overwrite");
    fs::create_dir_all(&base).unwrap();
    let src = base.join("src.txt");
    let dest = base.join("dest.txt");
    write_file(&src, b"new-content");
    write_file(&dest, b"old-content");

    let err = copy_file_best_effort(&src, &dest, None, None, None, None).unwrap_err();
    assert!(is_destination_exists_error(&err), "unexpected error: {err}");
    assert_eq!(
        fs::read(&dest).unwrap(),
        b"old-content",
        "existing destination should remain unchanged"
    );

    let _ = fs::remove_dir_all(&base);
}

#[test]
fn move_entry_does_not_overwrite_existing_target() {
    let base = uniq_path("move-no-overwrite");
    fs::create_dir_all(&base).unwrap();
    let src = base.join("src.txt");
    let dest = base.join("dest.txt");
    write_file(&src, b"new-content");
    write_file(&dest, b"old-content");

    let err = move_entry(&src, &dest, None, None, None).unwrap_err();
    assert!(is_destination_exists_error(&err), "unexpected error: {err}");
    assert_eq!(
        fs::read(&dest).unwrap(),
        b"old-content",
        "existing destination should remain unchanged"
    );
    assert_eq!(
        fs::read(&src).unwrap(),
        b"new-content",
        "source should remain unchanged when move is blocked"
    );

    let _ = fs::remove_dir_all(&base);
}

#[test]
fn rename_candidate_is_deterministic_without_exists_probe() {
    let base = uniq_path("candidate").join("report.pdf");
    assert_eq!(rename_candidate(&base, 0), base);
    assert_eq!(
        rename_candidate(&base, 1),
        base.parent().unwrap().join("report-1.pdf")
    );
    assert_eq!(
        rename_candidate(&base, 2),
        base.parent().unwrap().join("report-2.pdf")
    );
}

#[test]
fn move_entry_preserves_a_destination_created_after_the_conflict_check() {
    let base = uniq_path("move-late-conflict");
    let src = base.join("src.txt");
    let dest = base.join("dest.txt");
    write_file(&src, b"source-data");
    let late_dest = dest.clone();
    ops::set_before_move_rename_test_hook(Some(Box::new(move || {
        fs::write(late_dest, b"other-process-data").unwrap();
    })));
    let result = move_entry(&src, &dest, None, None, None);
    ops::set_before_move_rename_test_hook(None);
    let source = fs::read(&src).ok();
    let destination = fs::read(&dest).unwrap();
    fs::remove_dir_all(base).unwrap();
    assert!(result.is_err(), "late conflict must not be overwritten");
    assert_eq!(
        result.unwrap_err().code(),
        ClipboardErrorCode::DestinationExists
    );
    assert_eq!(source.as_deref(), Some(b"source-data".as_slice()));
    assert_eq!(destination, b"other-process-data");
}

#[test]
fn move_entry_cancelled_before_rename_keeps_source() {
    let base = uniq_path("move-cancelled-before-rename");
    let src = base.join("src.txt");
    let dest = base.join("dest.txt");
    write_file(&src, b"source-data");
    let cancel = AtomicBool::new(true);
    let result = move_entry(&src, &dest, None, None, Some(&cancel));
    let source = fs::read(&src).ok();
    let destination_exists = dest.exists();
    fs::remove_dir_all(base).unwrap();
    assert!(result.is_err(), "cancelled move must not start");
    assert_eq!(result.unwrap_err().code(), ClipboardErrorCode::Cancelled);
    assert_eq!(source.as_deref(), Some(b"source-data".as_slice()));
    assert!(!destination_exists);
}

#[test]
fn move_entry_revalidates_source_identity_before_rename() {
    let root = uniq_path("move-replaced-source");
    let source = root.join("source.txt");
    let destination = root.join("destination.txt");
    let saved_original = root.join("saved-original.txt");
    write_file(&source, b"original-data");
    let hook_source = source.clone();
    let hook_saved = saved_original.clone();
    ops::set_before_move_rename_test_hook(Some(Box::new(move || {
        fs::rename(&hook_source, &hook_saved).unwrap();
        fs::write(hook_source, b"new-source-data").unwrap();
    })));
    let result = move_entry(&source, &destination, None, None, None);
    ops::set_before_move_rename_test_hook(None);
    let source_data = fs::read(&source).ok();
    let original_data = fs::read(&saved_original).unwrap();
    let destination_exists = destination.exists();
    fs::remove_dir_all(root).unwrap();
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("Path changed during operation"));
    assert_eq!(source_data.as_deref(), Some(b"new-source-data".as_slice()));
    assert_eq!(original_data, b"original-data");
    assert!(!destination_exists);
}

#[test]
#[cfg(unix)]
fn copy_rejects_special_inputs_without_creating_a_target() {
    use std::os::unix::{ffi::OsStrExt, net::UnixListener};
    let root = uniq_path("copy-special-inputs");
    fs::create_dir(&root).unwrap();
    let fifo = root.join("pipe");
    let cpath = std::ffi::CString::new(fifo.as_os_str().as_bytes()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(cpath.as_ptr(), 0o600) }, 0);
    let socket = root.join("socket");
    let listener = UnixListener::bind(&socket).unwrap();
    for source in [&fifo, &socket] {
        let target = root.join("destination");
        assert!(copy_file_best_effort(source, &target, None, None, None, None).is_err());
        assert!(!target.exists());
        assert!(fs::symlink_metadata(source).is_ok());
    }
    drop(listener);
    fs::remove_dir_all(root).unwrap();
}

#[test]
#[cfg(unix)]
fn direct_file_copy_does_not_follow_a_replacement_symlink() {
    let root = uniq_path("copy-direct-symlink");
    let original = root.join("original.txt");
    let link = root.join("replacement.txt");
    let target = root.join("destination.txt");
    write_file(&original, b"do-not-read-through-link");
    symlink(&original, &link).unwrap();
    assert!(copy_file_best_effort(&link, &target, None, None, None, None).is_err());
    assert!(!target.exists());
    assert_eq!(fs::read(&original).unwrap(), b"do-not-read-through-link");
    fs::remove_dir_all(root).unwrap();
}

#[test]
#[cfg(unix)]
fn directory_copy_preserves_unicode_and_non_utf8_child_names() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;
    let root = uniq_path("copy-native-child-names");
    let source = root.join("source");
    let target = root.join("target");
    let names = [
        OsString::from("kamera æøå\n bilde.txt"),
        OsString::from_vec(vec![0xff, b'x', b'.', b't', b'x', b't']),
    ];
    for name in &names {
        write_file(&source.join(name), b"unchanged-data");
    }
    copy_entry(&source, &target, None, None, None).unwrap();
    for name in &names {
        assert_eq!(fs::read(target.join(name)).unwrap(), b"unchanged-data");
        assert_eq!(fs::read(source.join(name)).unwrap(), b"unchanged-data");
    }
    assert_eq!(fs::read_dir(target).unwrap().count(), names.len());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn resolve_drop_mode_prefers_copy_modifier() {
    let base = uniq_path("drop-mode-copy");
    let src_dir = base.join("src");
    let dest_dir = base.join("dest");
    fs::create_dir_all(&src_dir).unwrap();
    fs::create_dir_all(&dest_dir).unwrap();
    let src_file = src_dir.join("a.txt");
    write_file(&src_file, b"a");

    let mode = resolve_drop_clipboard_mode_impl(
        vec![src_file.to_string_lossy().to_string()],
        dest_dir.to_string_lossy().to_string(),
        true,
    )
    .unwrap();

    assert_eq!(mode, ClipboardMode::Copy);
    let _ = fs::remove_dir_all(&base);
}

#[test]
fn resolve_drop_mode_defaults_to_cut_on_same_filesystem() {
    let base = uniq_path("drop-mode-cut");
    let src_dir = base.join("src");
    let dest_dir = base.join("dest");
    fs::create_dir_all(&src_dir).unwrap();
    fs::create_dir_all(&dest_dir).unwrap();
    let src_file = src_dir.join("a.txt");
    write_file(&src_file, b"a");

    let mode = resolve_drop_clipboard_mode_impl(
        vec![src_file.to_string_lossy().to_string()],
        dest_dir.to_string_lossy().to_string(),
        false,
    )
    .unwrap();

    assert_eq!(mode, ClipboardMode::Cut);
    let _ = fs::remove_dir_all(&base);
}

#[test]
fn copy_file_best_effort_fails_when_source_is_missing() {
    let base = uniq_path("copy-missing-source");
    fs::create_dir_all(&base).unwrap();
    let src = base.join("missing.txt");
    let dest = base.join("dest.txt");

    let err = copy_file_best_effort(&src, &dest, None, None, None, None).unwrap_err();
    assert_eq!(err.code(), ClipboardErrorCode::NotFound);
    assert!(
        !dest.exists(),
        "destination should not be created on failure"
    );

    let _ = fs::remove_dir_all(&base);
}

#[test]
fn move_entry_fails_when_source_is_missing() {
    let base = uniq_path("move-missing-source");
    fs::create_dir_all(&base).unwrap();
    let src = base.join("missing.txt");
    let dest = base.join("dest.txt");

    let err = move_entry(&src, &dest, None, None, None).unwrap_err();
    assert_eq!(err.code(), ClipboardErrorCode::NotFound);
    assert!(
        !dest.exists(),
        "destination should not be created on failure"
    );

    let _ = fs::remove_dir_all(&base);
}

#[test]
fn move_entry_keeps_source_when_destination_parent_disappears() {
    let base = uniq_path("move-missing-dest-parent");
    fs::create_dir_all(&base).unwrap();
    let src = base.join("src.txt");
    write_file(&src, b"data");
    let dest = base.join("missing").join("dest.txt");

    let err = move_entry(&src, &dest, None, None, None).unwrap_err();
    assert_eq!(err.code(), ClipboardErrorCode::NotFound);
    assert!(src.exists(), "source should remain when move fails");
    assert!(!dest.exists(), "destination should not be created");

    let _ = fs::remove_dir_all(&base);
}

#[cfg(unix)]
#[test]
fn copy_file_best_effort_fails_when_destination_dir_is_read_only() {
    let base = uniq_path("copy-read-only-dir");
    fs::create_dir_all(&base).unwrap();
    let src = base.join("src.txt");
    write_file(&src, b"data");

    let dest_dir = base.join("dest");
    fs::create_dir_all(&dest_dir).unwrap();
    fs::set_permissions(&dest_dir, Permissions::from_mode(0o555)).unwrap();
    let dest = dest_dir.join("out.txt");

    let err = copy_file_best_effort(&src, &dest, None, None, None, None).unwrap_err();
    assert_eq!(err.code(), ClipboardErrorCode::IoError);
    assert!(src.exists(), "source should remain");
    assert!(!dest.exists(), "destination should not be created");

    fs::set_permissions(&dest_dir, Permissions::from_mode(0o755)).unwrap();
    let _ = fs::remove_dir_all(&base);
}

#[cfg(unix)]
#[test]
fn move_entry_fails_when_destination_dir_is_read_only_and_keeps_source() {
    let base = uniq_path("move-read-only-dir");
    fs::create_dir_all(&base).unwrap();
    let src = base.join("src.txt");
    write_file(&src, b"data");

    let dest_dir = base.join("dest");
    fs::create_dir_all(&dest_dir).unwrap();
    fs::set_permissions(&dest_dir, Permissions::from_mode(0o555)).unwrap();
    let dest = dest_dir.join("out.txt");

    let err = move_entry(&src, &dest, None, None, None).unwrap_err();
    assert_eq!(err.code(), ClipboardErrorCode::IoError);
    assert!(src.exists(), "source should remain on permission failure");
    assert!(!dest.exists(), "destination should not be created");

    fs::set_permissions(&dest_dir, Permissions::from_mode(0o755)).unwrap();
    let _ = fs::remove_dir_all(&base);
}

#[cfg(unix)]
#[test]
fn copy_entry_rejects_symlink_source_no_follow() {
    let base = uniq_path("copy-symlink-no-follow");
    fs::create_dir_all(&base).unwrap();
    let real_src = base.join("real.txt");
    write_file(&real_src, b"data");
    let link_src = base.join("link.txt");
    symlink(&real_src, &link_src).unwrap();
    let dest = base.join("dest.txt");

    let err = copy_entry(&link_src, &dest, None, None, None).unwrap_err();
    assert_eq!(err.code(), ClipboardErrorCode::SymlinkUnsupported);
    assert!(!dest.exists(), "destination should not be created");
    assert!(real_src.exists(), "real source should remain unchanged");

    let _ = fs::remove_dir_all(&base);
}

#[test]
fn paste_clipboard_preview_reports_existing_file_conflict() {
    let _guard = lock_clipboard_test();
    clear_clipboard();
    let base = uniq_path("preview-file-conflict");
    let src_dir = base.join("src");
    let dest_dir = base.join("dest");
    fs::create_dir_all(&src_dir).unwrap();
    fs::create_dir_all(&dest_dir).unwrap();

    let src = src_dir.join("report.txt");
    let dest = dest_dir.join("report.txt");
    write_file(&src, b"new");
    write_file(&dest, b"old");

    set_clipboard_impl(vec![src.to_string_lossy().to_string()], "copy".to_string()).unwrap();
    let preview = paste_clipboard_preview_impl(dest_dir.to_string_lossy().to_string()).unwrap();

    assert_eq!(preview.len(), 1);
    assert_eq!(preview[0].src, src.to_string_lossy());
    assert_eq!(preview[0].target, dest.to_string_lossy());
    assert!(preview[0].exists);
    assert!(!preview[0].is_dir);

    clear_clipboard();
    let _ = fs::remove_dir_all(&base);
}

#[test]
fn paste_clipboard_preview_reports_existing_directory_conflict() {
    let _guard = lock_clipboard_test();
    clear_clipboard();
    let base = uniq_path("preview-dir-conflict");
    let src_dir = base.join("src");
    let dest_dir = base.join("dest");
    fs::create_dir_all(&src_dir).unwrap();
    fs::create_dir_all(&dest_dir).unwrap();

    let src = src_dir.join("photos");
    let dest = dest_dir.join("photos");
    fs::create_dir_all(&src).unwrap();
    fs::create_dir_all(&dest).unwrap();
    write_file(&src.join("a.jpg"), b"a");
    write_file(&dest.join("existing.jpg"), b"old");

    set_clipboard_impl(vec![src.to_string_lossy().to_string()], "copy".to_string()).unwrap();
    let preview = paste_clipboard_preview_impl(dest_dir.to_string_lossy().to_string()).unwrap();

    assert_eq!(preview.len(), 1);
    assert_eq!(preview[0].src, src.to_string_lossy());
    assert_eq!(preview[0].target, dest.to_string_lossy());
    assert!(preview[0].exists);
    assert!(preview[0].is_dir);

    clear_clipboard();
    let _ = fs::remove_dir_all(&base);
}

#[test]
fn paste_clipboard_preview_filters_non_conflicting_entries() {
    let _guard = lock_clipboard_test();
    clear_clipboard();
    let base = uniq_path("preview-filters-non-conflicts");
    let src_dir = base.join("src");
    let dest_dir = base.join("dest");
    fs::create_dir_all(&src_dir).unwrap();
    fs::create_dir_all(&dest_dir).unwrap();

    let conflict = src_dir.join("report.txt");
    let unique = src_dir.join("notes.txt");
    write_file(&conflict, b"new");
    write_file(&unique, b"unique");
    write_file(&dest_dir.join("report.txt"), b"old");

    set_clipboard_impl(
        vec![
            conflict.to_string_lossy().to_string(),
            unique.to_string_lossy().to_string(),
        ],
        "copy".to_string(),
    )
    .unwrap();
    let preview = paste_clipboard_preview_impl(dest_dir.to_string_lossy().to_string()).unwrap();

    assert_eq!(preview.len(), 1);
    assert_eq!(preview[0].src, conflict.to_string_lossy());
    assert_eq!(
        preview[0].target,
        dest_dir.join("report.txt").to_string_lossy()
    );

    clear_clipboard();
    let _ = fs::remove_dir_all(&base);
}

#[test]
fn paste_clipboard_preview_matches_rename_execution_for_file_and_directory_conflicts() {
    let _guard = lock_clipboard_test();
    let _ = ensure_undo_dir();
    clear_clipboard();

    let base = uniq_path("preview-execute-rename-align");
    let src_dir = base.join("src");
    let dest_dir = base.join("dest");
    fs::create_dir_all(&src_dir).unwrap();
    fs::create_dir_all(&dest_dir).unwrap();

    let src_file = src_dir.join("report.txt");
    let src_folder = src_dir.join("photos");
    let dest_file = dest_dir.join("report.txt");
    let dest_folder = dest_dir.join("photos");
    write_file(&src_file, b"new-report");
    fs::create_dir_all(&src_folder).unwrap();
    write_file(&src_folder.join("a.jpg"), b"new-photo");
    write_file(&dest_file, b"old-report");
    fs::create_dir_all(&dest_folder).unwrap();
    write_file(&dest_folder.join("existing.jpg"), b"old-photo");

    set_clipboard_impl(
        vec![
            src_file.to_string_lossy().to_string(),
            src_folder.to_string_lossy().to_string(),
        ],
        "copy".to_string(),
    )
    .unwrap();

    let preview = paste_clipboard_preview_impl(dest_dir.to_string_lossy().to_string()).unwrap();
    assert_eq!(preview.len(), 2);
    assert!(preview.iter().any(|item| {
        item.src == src_file.to_string_lossy()
            && item.target == dest_file.to_string_lossy()
            && item.exists
            && !item.is_dir
    }));
    assert!(preview.iter().any(|item| {
        item.src == src_folder.to_string_lossy()
            && item.target == dest_folder.to_string_lossy()
            && item.exists
            && item.is_dir
    }));

    let undo = UndoState::default();
    let created = paste_clipboard_core(
        None,
        dest_dir.to_string_lossy().to_string(),
        Some("rename".to_string()),
        undo.clone_inner(),
        CancelState::default(),
        None,
    )
    .unwrap();

    let renamed_file = dest_dir.join("report-1.txt");
    let renamed_folder = dest_dir.join("photos-1");
    assert_eq!(
        created,
        vec![
            renamed_file.to_string_lossy().to_string(),
            renamed_folder.to_string_lossy().to_string(),
        ]
    );
    assert_eq!(fs::read(&dest_file).unwrap(), b"old-report");
    assert_eq!(fs::read(&renamed_file).unwrap(), b"new-report");
    assert!(dest_folder.join("existing.jpg").exists());
    assert_eq!(
        fs::read(renamed_folder.join("a.jpg")).unwrap(),
        b"new-photo"
    );
    assert!(src_file.exists(), "copy rename should keep the file source");
    assert!(
        src_folder.exists(),
        "copy rename should keep the dir source"
    );

    clear_clipboard();
    let _ = fs::remove_dir_all(&base);
}

#[test]
fn copy_file_best_effort_cancelled_before_transfer_creates_no_destination() {
    let base = uniq_path("copy-cancelled-file");
    fs::create_dir_all(&base).unwrap();
    let src = base.join("src.bin");
    let dest = base.join("dest.bin");
    write_file(&src, &[7u8; 32 * 1024]);
    let cancel = AtomicBool::new(true);

    let err =
        copy_file_best_effort(&src, &dest, None, None, Some(&cancel), Some(32 * 1024)).unwrap_err();

    assert_eq!(err.code(), ClipboardErrorCode::Cancelled);
    assert!(src.exists(), "source should remain on cancel");
    assert!(
        !dest.exists(),
        "pre-cancelled copy must not create a destination"
    );

    let _ = fs::remove_dir_all(&base);
}

#[test]
fn copy_entry_directory_cancelled_cleans_up_created_destination_dir() {
    let base = uniq_path("copy-cancelled-dir");
    let src = base.join("src");
    let dest = base.join("dest");
    fs::create_dir_all(&src).unwrap();
    write_file(&src.join("a.txt"), b"a");
    let cancel = AtomicBool::new(true);

    let err = copy_entry(&src, &dest, None, None, Some(&cancel)).unwrap_err();

    assert_eq!(err.code(), ClipboardErrorCode::Cancelled);
    assert!(src.exists(), "source directory should remain on cancel");
    assert!(
        !dest.exists(),
        "destination directory should be cleaned up on cancel"
    );

    cancel.store(false, Ordering::Relaxed);
    let _ = fs::remove_dir_all(&base);
}

#[test]
fn paste_clipboard_copy_rolls_back_successful_items_when_later_source_fails() {
    let _guard = lock_clipboard_test();
    let _ = ensure_undo_dir();
    clear_clipboard();

    let base = uniq_path("paste-copy-rollback");
    let src_dir = base.join("src");
    let dest_dir = base.join("dest");
    fs::create_dir_all(&src_dir).unwrap();
    fs::create_dir_all(&dest_dir).unwrap();

    let first = src_dir.join("first.txt");
    let second = src_dir.join("second.txt");
    write_file(&first, b"first");
    write_file(&second, b"second");

    set_clipboard_impl(
        vec![
            first.to_string_lossy().to_string(),
            second.to_string_lossy().to_string(),
        ],
        "copy".to_string(),
    )
    .unwrap();
    fs::remove_file(&second).unwrap();

    let undo = UndoState::default();
    let err = paste_clipboard_core(
        None,
        dest_dir.to_string_lossy().to_string(),
        None,
        undo.clone_inner(),
        CancelState::default(),
        None,
    )
    .unwrap_err();

    assert_eq!(err.code(), ClipboardErrorCode::NotFound);
    assert!(
        err.to_string().contains("Failed to read metadata"),
        "unexpected error: {err}"
    );
    assert!(
        first.exists(),
        "source should remain after failed copy rollback"
    );
    assert!(
        !dest_dir.join("first.txt").exists(),
        "destination copy should be rolled back when a later item fails"
    );
    assert!(
        undo.undo().is_err(),
        "failed paste should not leave an applied undo action behind"
    );

    clear_clipboard();
    let _ = fs::remove_dir_all(&base);
}

#[test]
fn paste_clipboard_cut_rolls_back_successful_items_when_later_source_fails() {
    let _guard = lock_clipboard_test();
    let _ = ensure_undo_dir();
    clear_clipboard();

    let base = uniq_path("paste-cut-rollback");
    let src_dir = base.join("src");
    let dest_dir = base.join("dest");
    fs::create_dir_all(&src_dir).unwrap();
    fs::create_dir_all(&dest_dir).unwrap();

    let first = src_dir.join("first.txt");
    let second = src_dir.join("second.txt");
    write_file(&first, b"first");
    write_file(&second, b"second");

    set_clipboard_impl(
        vec![
            first.to_string_lossy().to_string(),
            second.to_string_lossy().to_string(),
        ],
        "cut".to_string(),
    )
    .unwrap();
    fs::remove_file(&second).unwrap();

    let undo = UndoState::default();
    let err = paste_clipboard_core(
        None,
        dest_dir.to_string_lossy().to_string(),
        None,
        undo.clone_inner(),
        CancelState::default(),
        None,
    )
    .unwrap_err();

    assert_eq!(err.code(), ClipboardErrorCode::NotFound);
    assert!(
        err.to_string().contains("Failed to read metadata"),
        "unexpected error: {err}"
    );
    assert!(
        first.exists(),
        "source should be restored after failed cut rollback"
    );
    assert!(
        !dest_dir.join("first.txt").exists(),
        "moved destination should be rolled back when a later item fails"
    );
    assert!(
        current_clipboard().is_some(),
        "failed cut should keep clipboard contents for retry"
    );
    assert!(
        undo.undo().is_err(),
        "failed paste should not leave an applied undo action behind"
    );

    clear_clipboard();
    let _ = fs::remove_dir_all(&base);
}

#[test]
fn paste_clipboard_copy_cancelled_after_first_item_rolls_back_created_targets() {
    let _guard = lock_clipboard_test();
    let _ = ensure_undo_dir();
    clear_clipboard();

    let base = uniq_path("paste-copy-cancel-mid-batch");
    let src_dir = base.join("src");
    let dest_dir = base.join("dest");
    fs::create_dir_all(&src_dir).unwrap();
    fs::create_dir_all(&dest_dir).unwrap();

    let first = src_dir.join("first.txt");
    let second = src_dir.join("second.txt");
    write_file(&first, &[1u8; 16 * 1024]);
    write_file(&second, &[2u8; 16 * 1024]);

    set_clipboard_impl(
        vec![
            first.to_string_lossy().to_string(),
            second.to_string_lossy().to_string(),
        ],
        "copy".to_string(),
    )
    .unwrap();

    let cancel_state = CancelState::default();
    let cancel_state_bg = cancel_state.clone();
    let mut copied_once = false;
    set_after_paste_item_test_hook(Some(Box::new(move || {
        if !copied_once {
            copied_once = true;
            let _ = cancel_state_bg.cancel("paste-copy-cancel");
        }
    })));
    let undo = UndoState::default();
    let err = paste_clipboard_core(
        None,
        dest_dir.to_string_lossy().to_string(),
        None,
        undo.clone_inner(),
        cancel_state,
        Some("paste-copy-cancel".to_string()),
    )
    .unwrap_err();
    set_after_paste_item_test_hook(None);

    assert_eq!(err.code(), ClipboardErrorCode::Cancelled);
    assert!(first.exists(), "source should remain after cancelled copy");
    assert!(second.exists(), "second source should remain untouched");
    assert!(
        !dest_dir.join("first.txt").exists(),
        "first copied target should be rolled back after mid-batch cancellation"
    );
    assert!(
        !dest_dir.join("second.txt").exists(),
        "later targets should not be created after cancellation"
    );
    assert!(
        undo.undo().is_err(),
        "cancelled paste should not leave an applied undo action behind"
    );

    clear_clipboard();
    let _ = fs::remove_dir_all(&base);
}

#[test]
fn paste_copy_rollback_preserves_changed_completed_outputs() {
    let _guard = lock_clipboard_test();
    let _ = ensure_undo_dir();
    for change in ["edit-file", "replace-file", "foreign-child", "edit-child"] {
        clear_clipboard();
        let root = uniq_path("paste-copy-changed-rollback");
        let source = root.join("source/first");
        let second = root.join("source/second.txt");
        let destination = root.join("destination");
        fs::create_dir_all(&destination).unwrap();
        let is_dir = change.ends_with("child");
        let original = if is_dir {
            source.join("nested/file.txt")
        } else {
            source.clone()
        };
        write_file(&original, b"original");
        write_file(&second, b"second");
        set_clipboard_impl(
            vec![
                source.to_string_lossy().into(),
                second.to_string_lossy().into(),
            ],
            "copy".into(),
        )
        .unwrap();
        let target = destination.join("first");
        let changed_target = target.clone();
        let parked = destination.join("saved-copy");
        let cancel = CancelState::default();
        let cancellation = cancel.clone();
        set_after_paste_item_test_hook(Some(Box::new(move || {
            match change {
                "edit-file" => write_file(&changed_target, b"edited"),
                "replace-file" => {
                    fs::rename(&changed_target, &parked).unwrap();
                    write_file(&changed_target, b"foreign");
                }
                "foreign-child" => write_file(&changed_target.join("foreign.txt"), b"foreign"),
                _ => write_file(&changed_target.join("nested/file.txt"), b"edited"),
            }
            cancellation.cancel("changed-copy").unwrap();
        })));
        let undo = UndoState::default();
        let result = paste_clipboard_core(
            None,
            destination.to_string_lossy().into(),
            None,
            undo.clone_inner(),
            cancel,
            Some("changed-copy".into()),
        );
        set_after_paste_item_test_hook(None);
        let error = result.unwrap_err();
        assert_eq!(error.code(), ClipboardErrorCode::RollbackFailed);
        assert!(error.to_string().contains("cancelled") || error.to_string().contains("Cancelled"));
        assert!(error.to_string().contains("retained"));
        let preserved = match change {
            "foreign-child" => target.join("foreign.txt"),
            "edit-child" => target.join("nested/file.txt"),
            _ => target,
        };
        assert_eq!(
            fs::read(preserved).unwrap(),
            if change.starts_with("edit") {
                b"edited".as_slice()
            } else {
                b"foreign".as_slice()
            }
        );
        assert!(source.exists());
        assert_eq!(fs::read(second).unwrap(), b"second");
        assert!(!destination.join("second.txt").exists());
        assert!(
            undo.undo().is_err(),
            "failed paste is not recorded as completed"
        );
        assert!(!current_clipboard().unwrap().entries.is_empty());
        fs::remove_dir_all(root).unwrap();
    }
    clear_clipboard();
}

#[test]
fn paste_clipboard_cut_cancelled_after_first_item_restores_moved_source() {
    let _guard = lock_clipboard_test();
    let _ = ensure_undo_dir();
    clear_clipboard();

    let base = uniq_path("paste-cut-cancel-mid-batch");
    let src_dir = base.join("src");
    let dest_dir = base.join("dest");
    fs::create_dir_all(&src_dir).unwrap();
    fs::create_dir_all(&dest_dir).unwrap();

    let first = src_dir.join("first.txt");
    let second = src_dir.join("second.txt");
    write_file(&first, &[1u8; 16 * 1024]);
    write_file(&second, &[2u8; 16 * 1024]);

    set_clipboard_impl(
        vec![
            first.to_string_lossy().to_string(),
            second.to_string_lossy().to_string(),
        ],
        "cut".to_string(),
    )
    .unwrap();

    let cancel_state = CancelState::default();
    let cancel_state_bg = cancel_state.clone();
    let mut moved_once = false;
    set_after_paste_item_test_hook(Some(Box::new(move || {
        if !moved_once {
            moved_once = true;
            let _ = cancel_state_bg.cancel("paste-cut-cancel");
        }
    })));
    let undo = UndoState::default();
    let err = paste_clipboard_core(
        None,
        dest_dir.to_string_lossy().to_string(),
        None,
        undo.clone_inner(),
        cancel_state,
        Some("paste-cut-cancel".to_string()),
    )
    .unwrap_err();
    set_after_paste_item_test_hook(None);

    assert_eq!(err.code(), ClipboardErrorCode::Cancelled);
    assert!(
        first.exists(),
        "first source should be restored after cancelled cut"
    );
    assert!(second.exists(), "second source should remain untouched");
    assert!(
        !dest_dir.join("first.txt").exists(),
        "first moved target should be rolled back after mid-batch cancellation"
    );
    assert!(
        !dest_dir.join("second.txt").exists(),
        "later targets should not be created after cancellation"
    );
    assert!(
        current_clipboard().is_some(),
        "cancelled cut should keep clipboard contents for retry"
    );
    assert!(
        undo.undo().is_err(),
        "cancelled paste should not leave an applied undo action behind"
    );

    clear_clipboard();
    let _ = fs::remove_dir_all(&base);
}

#[test]
fn paste_clipboard_directory_copy_cancelled_after_first_item_rolls_back_created_targets() {
    let _guard = lock_clipboard_test();
    let _ = ensure_undo_dir();
    clear_clipboard();

    let base = uniq_path("paste-dir-copy-cancel-mid-batch");
    let src_dir = base.join("src");
    let dest_dir = base.join("dest");
    fs::create_dir_all(&src_dir).unwrap();
    fs::create_dir_all(&dest_dir).unwrap();

    let first = src_dir.join("first");
    let second = src_dir.join("second");
    fs::create_dir_all(first.join("nested")).unwrap();
    fs::create_dir_all(second.join("nested")).unwrap();
    write_file(&first.join("nested/a.txt"), b"alpha");
    write_file(&second.join("nested/b.txt"), b"beta");

    set_clipboard_impl(
        vec![
            first.to_string_lossy().to_string(),
            second.to_string_lossy().to_string(),
        ],
        "copy".to_string(),
    )
    .unwrap();

    let cancel_state = CancelState::default();
    let cancel_state_bg = cancel_state.clone();
    let mut copied_once = false;
    set_after_paste_item_test_hook(Some(Box::new(move || {
        if !copied_once {
            copied_once = true;
            let _ = cancel_state_bg.cancel("paste-dir-copy-cancel");
        }
    })));
    let undo = UndoState::default();
    let err = paste_clipboard_core(
        None,
        dest_dir.to_string_lossy().to_string(),
        None,
        undo.clone_inner(),
        cancel_state,
        Some("paste-dir-copy-cancel".to_string()),
    )
    .unwrap_err();
    set_after_paste_item_test_hook(None);

    assert_eq!(err.code(), ClipboardErrorCode::Cancelled);
    assert!(
        first.exists(),
        "first source directory should remain after cancelled copy"
    );
    assert!(
        second.exists(),
        "second source directory should remain untouched"
    );
    assert!(
        !dest_dir.join("first").exists(),
        "first copied directory should be rolled back after mid-batch cancellation"
    );
    assert!(
        !dest_dir.join("second").exists(),
        "later directory targets should not be created after cancellation"
    );
    assert!(
        undo.undo().is_err(),
        "cancelled directory paste should not leave an applied undo action behind"
    );

    clear_clipboard();
    let _ = fs::remove_dir_all(&base);
}

#[test]
fn paste_clipboard_directory_cut_cancelled_after_first_item_restores_moved_source() {
    let _guard = lock_clipboard_test();
    let _ = ensure_undo_dir();
    clear_clipboard();

    let base = uniq_path("paste-dir-cut-cancel-mid-batch");
    let src_dir = base.join("src");
    let dest_dir = base.join("dest");
    fs::create_dir_all(&src_dir).unwrap();
    fs::create_dir_all(&dest_dir).unwrap();

    let first = src_dir.join("first");
    let second = src_dir.join("second");
    fs::create_dir_all(first.join("nested")).unwrap();
    fs::create_dir_all(second.join("nested")).unwrap();
    write_file(&first.join("nested/a.txt"), b"alpha");
    write_file(&second.join("nested/b.txt"), b"beta");

    set_clipboard_impl(
        vec![
            first.to_string_lossy().to_string(),
            second.to_string_lossy().to_string(),
        ],
        "cut".to_string(),
    )
    .unwrap();

    let cancel_state = CancelState::default();
    let cancel_state_bg = cancel_state.clone();
    let mut moved_once = false;
    set_after_paste_item_test_hook(Some(Box::new(move || {
        if !moved_once {
            moved_once = true;
            let _ = cancel_state_bg.cancel("paste-dir-cut-cancel");
        }
    })));
    let undo = UndoState::default();
    let err = paste_clipboard_core(
        None,
        dest_dir.to_string_lossy().to_string(),
        None,
        undo.clone_inner(),
        cancel_state,
        Some("paste-dir-cut-cancel".to_string()),
    )
    .unwrap_err();
    set_after_paste_item_test_hook(None);

    assert_eq!(err.code(), ClipboardErrorCode::Cancelled);
    assert!(
        first.exists(),
        "first source directory should be restored after cancelled cut"
    );
    assert!(
        second.exists(),
        "second source directory should remain untouched"
    );
    assert!(
        !dest_dir.join("first").exists(),
        "first moved directory should be rolled back after mid-batch cancellation"
    );
    assert!(
        !dest_dir.join("second").exists(),
        "later directory targets should not be created after cancellation"
    );
    assert!(
        current_clipboard().is_some(),
        "cancelled directory cut should keep clipboard contents for retry"
    );
    assert!(
        undo.undo().is_err(),
        "cancelled directory cut should not leave an applied undo action behind"
    );

    clear_clipboard();
    let _ = fs::remove_dir_all(&base);
}

#[test]
fn paste_clipboard_directory_copy_rolls_back_successful_items_when_later_source_fails() {
    let _guard = lock_clipboard_test();
    let _ = ensure_undo_dir();
    clear_clipboard();

    let base = uniq_path("paste-dir-copy-later-source-fails");
    let src_dir = base.join("src");
    let dest_dir = base.join("dest");
    fs::create_dir_all(&src_dir).unwrap();
    fs::create_dir_all(&dest_dir).unwrap();

    let first = src_dir.join("first");
    let second = src_dir.join("second");
    fs::create_dir_all(first.join("nested")).unwrap();
    fs::create_dir_all(second.join("nested")).unwrap();
    write_file(&first.join("nested/a.txt"), b"alpha");
    write_file(&second.join("nested/b.txt"), b"beta");

    set_clipboard_impl(
        vec![
            first.to_string_lossy().to_string(),
            second.to_string_lossy().to_string(),
        ],
        "copy".to_string(),
    )
    .unwrap();

    let second_for_hook = second.clone();
    let mut removed_once = false;
    set_after_paste_item_test_hook(Some(Box::new(move || {
        if !removed_once {
            removed_once = true;
            let _ = fs::remove_dir_all(&second_for_hook);
        }
    })));
    let undo = UndoState::default();
    let err = paste_clipboard_core(
        None,
        dest_dir.to_string_lossy().to_string(),
        None,
        undo.clone_inner(),
        CancelState::default(),
        None,
    )
    .unwrap_err();
    set_after_paste_item_test_hook(None);

    assert_eq!(err.code(), ClipboardErrorCode::NotFound);
    assert!(
        first.exists(),
        "first source directory should remain after failed copy rollback"
    );
    assert!(
        !second.exists(),
        "injected missing source directory should remain missing"
    );
    assert!(
        !dest_dir.join("first").exists(),
        "first copied directory should be rolled back when a later source fails"
    );
    assert!(
        !dest_dir.join("second").exists(),
        "later directory target should not remain after rollback"
    );
    assert!(
        undo.undo().is_err(),
        "failed directory copy should not leave an applied undo action behind"
    );

    clear_clipboard();
    let _ = fs::remove_dir_all(&base);
}

#[test]
fn paste_clipboard_directory_cut_rolls_back_successful_items_when_later_source_fails() {
    let _guard = lock_clipboard_test();
    let _ = ensure_undo_dir();
    clear_clipboard();

    let base = uniq_path("paste-dir-cut-later-source-fails");
    let src_dir = base.join("src");
    let dest_dir = base.join("dest");
    fs::create_dir_all(&src_dir).unwrap();
    fs::create_dir_all(&dest_dir).unwrap();

    let first = src_dir.join("first");
    let second = src_dir.join("second");
    fs::create_dir_all(first.join("nested")).unwrap();
    fs::create_dir_all(second.join("nested")).unwrap();
    write_file(&first.join("nested/a.txt"), b"alpha");
    write_file(&second.join("nested/b.txt"), b"beta");

    set_clipboard_impl(
        vec![
            first.to_string_lossy().to_string(),
            second.to_string_lossy().to_string(),
        ],
        "cut".to_string(),
    )
    .unwrap();

    let second_for_hook = second.clone();
    let mut removed_once = false;
    set_after_paste_item_test_hook(Some(Box::new(move || {
        if !removed_once {
            removed_once = true;
            let _ = fs::remove_dir_all(&second_for_hook);
        }
    })));
    let undo = UndoState::default();
    let err = paste_clipboard_core(
        None,
        dest_dir.to_string_lossy().to_string(),
        None,
        undo.clone_inner(),
        CancelState::default(),
        None,
    )
    .unwrap_err();
    set_after_paste_item_test_hook(None);

    assert_eq!(err.code(), ClipboardErrorCode::NotFound);
    assert!(
        first.exists(),
        "first source directory should be restored after failed cut rollback"
    );
    assert!(
        !second.exists(),
        "injected missing source directory should remain missing"
    );
    assert!(
        !dest_dir.join("first").exists(),
        "first moved directory should be rolled back when a later source fails"
    );
    assert!(
        !dest_dir.join("second").exists(),
        "later directory target should not remain after rollback"
    );
    assert!(
        current_clipboard().is_some(),
        "failed directory cut should keep clipboard contents for retry"
    );
    assert!(
        undo.undo().is_err(),
        "failed directory cut should not leave an applied undo action behind"
    );

    clear_clipboard();
    let _ = fs::remove_dir_all(&base);
}

#[test]
fn paste_clipboard_overwrite_directory_copy_cancelled_after_first_merged_item_rolls_back() {
    let _guard = lock_clipboard_test();
    let _ = ensure_undo_dir();
    clear_clipboard();

    let base = uniq_path("paste-dir-copy-cancel-mid-merge");
    let src_dir = base.join("src");
    let dest_dir = base.join("dest");
    let src_tree = src_dir.join("photos");
    let dest_tree = dest_dir.join("photos");
    fs::create_dir_all(&src_tree).unwrap();
    fs::create_dir_all(&dest_tree).unwrap();

    let first = src_tree.join("a.txt");
    let second = src_tree.join("b.txt");
    write_file(&first, b"a");
    write_file(&second, b"b");
    write_file(&dest_tree.join("existing.txt"), b"keep");

    set_clipboard_impl(
        vec![src_tree.to_string_lossy().to_string()],
        "copy".to_string(),
    )
    .unwrap();

    let cancel_state = CancelState::default();
    let cancel_state_bg = cancel_state.clone();
    let mut merged_once = false;
    set_after_merge_item_test_hook(Some(Box::new(move |_| {
        if !merged_once {
            merged_once = true;
            let _ = cancel_state_bg.cancel("paste-dir-copy-cancel");
        }
    })));
    let undo = UndoState::default();
    let err = paste_clipboard_core(
        None,
        dest_dir.to_string_lossy().to_string(),
        Some("overwrite".to_string()),
        undo.clone_inner(),
        cancel_state,
        Some("paste-dir-copy-cancel".to_string()),
    )
    .unwrap_err();
    set_after_merge_item_test_hook(None);

    assert_eq!(err.code(), ClipboardErrorCode::Cancelled);
    assert!(
        first.exists(),
        "source content should remain after cancelled merge-copy"
    );
    assert!(
        second.exists(),
        "later source content should remain untouched"
    );
    assert!(
        !dest_tree.join("a.txt").exists(),
        "created merged target should be rolled back after cancellation"
    );
    assert!(
        !dest_tree.join("b.txt").exists(),
        "later merged targets should not be created after cancellation"
    );
    assert_eq!(
        fs::read(dest_tree.join("existing.txt")).unwrap(),
        b"keep",
        "pre-existing destination content should remain unchanged"
    );
    assert!(
        undo.undo().is_err(),
        "cancelled merge-copy should not leave an applied undo action behind"
    );

    clear_clipboard();
    let _ = fs::remove_dir_all(&base);
}

#[test]
fn paste_clipboard_overwrite_directory_cut_cancelled_after_first_merged_item_rolls_back() {
    let _guard = lock_clipboard_test();
    let _ = ensure_undo_dir();
    clear_clipboard();

    let base = uniq_path("paste-dir-cut-cancel-mid-merge");
    let src_dir = base.join("src");
    let dest_dir = base.join("dest");
    let src_tree = src_dir.join("photos");
    let dest_tree = dest_dir.join("photos");
    fs::create_dir_all(&src_tree).unwrap();
    fs::create_dir_all(&dest_tree).unwrap();

    let first = src_tree.join("a.txt");
    let second = src_tree.join("b.txt");
    write_file(&first, b"a");
    write_file(&second, b"b");
    write_file(&dest_tree.join("existing.txt"), b"keep");

    set_clipboard_impl(
        vec![src_tree.to_string_lossy().to_string()],
        "cut".to_string(),
    )
    .unwrap();

    let cancel_state = CancelState::default();
    let cancel_state_bg = cancel_state.clone();
    let mut merged_once = false;
    set_after_merge_item_test_hook(Some(Box::new(move |_| {
        if !merged_once {
            merged_once = true;
            let _ = cancel_state_bg.cancel("paste-dir-cut-cancel");
        }
    })));
    let undo = UndoState::default();
    let err = paste_clipboard_core(
        None,
        dest_dir.to_string_lossy().to_string(),
        Some("overwrite".to_string()),
        undo.clone_inner(),
        cancel_state,
        Some("paste-dir-cut-cancel".to_string()),
    )
    .unwrap_err();
    set_after_merge_item_test_hook(None);

    assert_eq!(err.code(), ClipboardErrorCode::Cancelled);
    assert!(
        first.exists(),
        "first source should be restored after cancelled merge-cut"
    );
    assert!(second.exists(), "later source should remain untouched");
    assert!(
        !dest_tree.join("a.txt").exists(),
        "created merged target should be rolled back after cancellation"
    );
    assert!(
        !dest_tree.join("b.txt").exists(),
        "later merged targets should not be created after cancellation"
    );
    assert_eq!(
        fs::read(dest_tree.join("existing.txt")).unwrap(),
        b"keep",
        "pre-existing destination content should remain unchanged"
    );
    assert!(
        current_clipboard().is_some(),
        "cancelled merge-cut should keep clipboard contents for retry"
    );
    assert!(
        undo.undo().is_err(),
        "cancelled merge-cut should not leave an applied undo action behind"
    );

    clear_clipboard();
    let _ = fs::remove_dir_all(&base);
}

#[test]
fn paste_clipboard_overwrite_directory_copy_rolls_back_when_later_merged_source_fails() {
    let _guard = lock_clipboard_test();
    let _ = ensure_undo_dir();
    clear_clipboard();

    let base = uniq_path("paste-dir-copy-later-source-fails");
    let src_dir = base.join("src");
    let dest_dir = base.join("dest");
    let src_tree = src_dir.join("photos");
    let dest_tree = dest_dir.join("photos");
    fs::create_dir_all(&src_tree).unwrap();
    fs::create_dir_all(&dest_tree).unwrap();

    let first = src_tree.join("a.txt");
    let second = src_tree.join("b.txt");
    write_file(&first, b"a");
    write_file(&second, b"b");
    write_file(&dest_tree.join("existing.txt"), b"keep");

    set_clipboard_impl(
        vec![src_tree.to_string_lossy().to_string()],
        "copy".to_string(),
    )
    .unwrap();

    let first_for_hook = first.clone();
    let second_for_hook = second.clone();
    let removed_path = std::sync::Arc::new(std::sync::Mutex::new(None::<PathBuf>));
    let removed_path_for_hook = removed_path.clone();
    let mut removed_once = false;
    set_after_merge_item_test_hook(Some(Box::new(move |processed| {
        if !removed_once {
            removed_once = true;
            let to_remove = if processed.file_name() == first_for_hook.file_name() {
                second_for_hook.clone()
            } else {
                first_for_hook.clone()
            };
            *removed_path_for_hook.lock().unwrap() = Some(to_remove.clone());
            let _ = fs::remove_file(&to_remove);
        }
    })));
    let undo = UndoState::default();
    let err = paste_clipboard_core(
        None,
        dest_dir.to_string_lossy().to_string(),
        Some("overwrite".to_string()),
        undo.clone_inner(),
        CancelState::default(),
        None,
    )
    .unwrap_err();
    set_after_merge_item_test_hook(None);

    assert_eq!(err.code(), ClipboardErrorCode::NotFound);
    let removed = removed_path.lock().unwrap().clone().expect("removed path");
    let surviving = if removed == first {
        second.clone()
    } else {
        first.clone()
    };
    assert!(
        !removed.exists(),
        "injected missing source should remain missing"
    );
    assert!(
        surviving.exists(),
        "surviving source should remain after failed merge-copy rollback"
    );
    assert!(
        !dest_tree.join("a.txt").exists(),
        "created merged target should be rolled back when a later source fails"
    );
    assert!(
        !dest_tree.join("b.txt").exists(),
        "later merged target should not remain after rollback"
    );
    assert_eq!(
        fs::read(dest_tree.join("existing.txt")).unwrap(),
        b"keep",
        "pre-existing destination content should remain unchanged"
    );
    assert!(
        undo.undo().is_err(),
        "failed merge-copy should not leave an applied undo action behind"
    );

    clear_clipboard();
    let _ = fs::remove_dir_all(&base);
}

#[test]
fn paste_clipboard_overwrite_directory_cut_rolls_back_when_later_merged_source_fails() {
    let _guard = lock_clipboard_test();
    let _ = ensure_undo_dir();
    clear_clipboard();

    let base = uniq_path("paste-dir-cut-later-source-fails");
    let src_dir = base.join("src");
    let dest_dir = base.join("dest");
    let src_tree = src_dir.join("photos");
    let dest_tree = dest_dir.join("photos");
    fs::create_dir_all(&src_tree).unwrap();
    fs::create_dir_all(&dest_tree).unwrap();

    let first = src_tree.join("a.txt");
    let second = src_tree.join("b.txt");
    write_file(&first, b"a");
    write_file(&second, b"b");
    write_file(&dest_tree.join("existing.txt"), b"keep");

    set_clipboard_impl(
        vec![src_tree.to_string_lossy().to_string()],
        "cut".to_string(),
    )
    .unwrap();

    let first_for_hook = first.clone();
    let second_for_hook = second.clone();
    let removed_path = std::sync::Arc::new(std::sync::Mutex::new(None::<PathBuf>));
    let removed_path_for_hook = removed_path.clone();
    let mut removed_once = false;
    set_after_merge_item_test_hook(Some(Box::new(move |processed| {
        if !removed_once {
            removed_once = true;
            let to_remove = if processed.file_name() == first_for_hook.file_name() {
                second_for_hook.clone()
            } else {
                first_for_hook.clone()
            };
            *removed_path_for_hook.lock().unwrap() = Some(to_remove.clone());
            let _ = fs::remove_file(&to_remove);
        }
    })));
    let undo = UndoState::default();
    let err = paste_clipboard_core(
        None,
        dest_dir.to_string_lossy().to_string(),
        Some("overwrite".to_string()),
        undo.clone_inner(),
        CancelState::default(),
        None,
    )
    .unwrap_err();
    set_after_merge_item_test_hook(None);

    assert_eq!(err.code(), ClipboardErrorCode::NotFound);
    let removed = removed_path.lock().unwrap().clone().expect("removed path");
    let surviving = if removed == first {
        second.clone()
    } else {
        first.clone()
    };
    assert!(
        !removed.exists(),
        "injected missing source should remain missing"
    );
    assert!(
        surviving.exists(),
        "surviving source should be restored after failed merge-cut rollback"
    );
    assert!(
        !dest_tree.join("a.txt").exists(),
        "created merged target should be rolled back when a later source fails"
    );
    assert!(
        !dest_tree.join("b.txt").exists(),
        "later merged target should not remain after rollback"
    );
    assert_eq!(
        fs::read(dest_tree.join("existing.txt")).unwrap(),
        b"keep",
        "pre-existing destination content should remain unchanged"
    );
    assert!(
        current_clipboard().is_some(),
        "failed merge-cut should keep clipboard contents for retry"
    );
    assert!(
        undo.undo().is_err(),
        "failed merge-cut should not leave an applied undo action behind"
    );

    clear_clipboard();
    let _ = fs::remove_dir_all(&base);
}

#[test]
#[cfg(unix)]
fn copy_preserves_private_and_executable_permissions() {
    let base = uniq_path("copy-permissions");
    for mode in [0o600, 0o755] {
        let src = base.join(format!("src-{mode}"));
        let dst = base.join(format!("dst-{mode}"));
        write_file(&src, b"payload");
        fs::set_permissions(&src, Permissions::from_mode(mode)).unwrap();
        copy_entry(&src, &dst, None, None, None).unwrap();
        assert_eq!(
            fs::metadata(&dst).unwrap().permissions().mode() & 0o777,
            mode
        );
    }
    let src = base.join("private-dir");
    let dst = base.join("copied-dir");
    write_file(&src.join("secret"), b"secret");
    fs::set_permissions(&src, Permissions::from_mode(0o700)).unwrap();
    fs::set_permissions(src.join("secret"), Permissions::from_mode(0o600)).unwrap();
    copy_entry(&src, &dst, None, None, None).unwrap();
    assert_eq!(
        fs::metadata(&dst).unwrap().permissions().mode() & 0o777,
        0o700
    );
    assert_eq!(
        fs::metadata(dst.join("secret"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o600
    );
    fs::remove_dir_all(base).unwrap();
}

#[test]
fn copy_keeps_bytes_when_destination_permissions_are_unsupported() {
    use crate::fs_utils::copy_test_hooks::{Phase, Scope};
    let base = uniq_path("copy-unsupported-permissions");
    let src = base.join("source");
    let dst = base.join("target");
    write_file(&src.join("nested/payload.txt"), b"generated payload");
    let _scope = Scope::new(|_, _, phase, _| {
        if phase == Phase::SetPermissions {
            Err(std::io::Error::from(std::io::ErrorKind::Unsupported))
        } else {
            Ok(())
        }
    });
    copy_entry(&src, &dst, None, None, None).unwrap();
    assert_eq!(
        fs::read(dst.join("nested/payload.txt")).unwrap(),
        b"generated payload"
    );
    assert_eq!(
        fs::read(src.join("nested/payload.txt")).unwrap(),
        b"generated payload"
    );
    fs::remove_dir_all(base).unwrap();
}

#[test]
fn copy_permission_denial_remains_an_error_and_preserves_source() {
    use crate::fs_utils::copy_test_hooks::{Phase, Scope};
    let base = uniq_path("copy-denied-permissions");
    let src = base.join("source");
    let dst = base.join("target");
    write_file(&src.join("payload.txt"), b"generated payload");
    let _scope = Scope::new(|_, target, phase, _| {
        if phase == Phase::SetPermissions && target.is_dir() {
            Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied))
        } else {
            Ok(())
        }
    });
    let error = copy_entry(&src, &dst, None, None, None).unwrap_err();
    assert!(error.to_string().contains("Set directory permissions"));
    assert_eq!(
        fs::read(src.join("payload.txt")).unwrap(),
        b"generated payload"
    );
    assert!(
        !dst.exists(),
        "owned copied output should be cleaned on a real failure"
    );
    fs::remove_dir_all(base).unwrap();
}

#[test]
fn merge_move_rolls_back_without_deleting_a_new_source_file() {
    let _guard = lock_clipboard_test();
    let _ = ensure_undo_dir();
    let base = uniq_path("merge-new-source-file");
    let src = base.join("src/folder");
    let dst = base.join("dst/folder");
    write_file(&src.join("initial.txt"), b"initial");
    write_file(&dst.join("existing.txt"), b"existing");
    let late = src.join("late.txt");
    let hook_late = late.clone();
    set_after_merge_item_test_hook(Some(Box::new(move |_| {
        fs::write(&hook_late, b"newly saved document").unwrap();
    })));
    set_clipboard_impl(vec![src.to_string_lossy().to_string()], "cut".into()).unwrap();
    let result = paste_clipboard_core(
        None,
        dst.parent().unwrap().to_string_lossy().to_string(),
        Some("overwrite".into()),
        UndoState::default().clone_inner(),
        CancelState::default(),
        None,
    );
    set_after_merge_item_test_hook(None);
    clear_clipboard();
    assert!(result.is_err(), "a nonempty source must abort the move");
    assert_eq!(fs::read(late).unwrap(), b"newly saved document");
    assert_eq!(fs::read(src.join("initial.txt")).unwrap(), b"initial");
    assert_eq!(fs::read(dst.join("existing.txt")).unwrap(), b"existing");
    assert!(!dst.join("initial.txt").exists());
    fs::remove_dir_all(base).unwrap();
}

#[test]
#[cfg(unix)]
fn failed_overwrite_retains_original_backup_after_write_error() {
    // File-size limits and signal dispositions affect the entire process. Keep
    // fault injection in its own test subprocess so parallel tests stay isolated.
    if std::env::var_os("BROWSEY_TEST_WRITE_ERROR_CHILD").is_none() {
        let output = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "clipboard::tests::failed_overwrite_retains_original_backup_after_write_error",
                "--nocapture",
            ])
            .env("BROWSEY_TEST_WRITE_ERROR_CHILD", "1")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        return;
    }
    let _ = ensure_undo_dir();
    let base = uniq_path("overwrite-write-error");
    let src = base.join("src/file.txt");
    let dst = base.join("dst/file.txt");
    write_file(&src, b"replacement content");
    write_file(&dst, b"original content");
    set_clipboard_impl(vec![src.to_string_lossy().to_string()], "copy".into()).unwrap();
    let mut original_limit: libc::rlimit = unsafe { std::mem::zeroed() };
    assert_eq!(
        unsafe { libc::getrlimit(libc::RLIMIT_FSIZE, &mut original_limit) },
        0
    );
    let limit = libc::rlimit {
        rlim_cur: 4,
        rlim_max: original_limit.rlim_max,
    };
    let old_signal = unsafe { libc::signal(libc::SIGXFSZ, libc::SIG_IGN) };
    let scope = crate::fs_utils::copy_test_hooks::Scope::new(move |_, _, phase, _| {
        if phase == crate::fs_utils::copy_test_hooks::Phase::OverwriteBackedUp
            && unsafe { libc::setrlimit(libc::RLIMIT_FSIZE, &limit) } != 0
        {
            return Err(std::io::Error::last_os_error());
        }
        Ok(())
    });
    let result = paste_clipboard_core(
        None,
        dst.parent().unwrap().to_string_lossy().to_string(),
        Some("overwrite".into()),
        UndoState::default().clone_inner(),
        CancelState::default(),
        None,
    );
    drop(scope);
    unsafe {
        libc::setrlimit(libc::RLIMIT_FSIZE, &original_limit);
        libc::signal(libc::SIGXFSZ, old_signal);
    }
    let error = result.expect_err("write limit must fail the copy");
    assert_eq!(error.code(), ClipboardErrorCode::RollbackFailed, "{error}");
    assert_eq!(fs::read(&dst).unwrap(), b"repl");
    let backups: Vec<_> = fs::read_dir(ensure_undo_dir())
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| entry.path().is_dir())
        .flat_map(|entry| fs::read_dir(entry.path()).unwrap().filter_map(Result::ok))
        .filter(|entry| entry.path().is_dir())
        .map(|entry| entry.path().join("file.txt"))
        .filter(|path| path.is_file())
        .collect();
    assert_eq!(backups.len(), 1);
    assert_eq!(fs::read(&backups[0]).unwrap(), b"original content");
    assert!(error
        .to_string()
        .contains(&backups[0].display().to_string()));
    assert_eq!(fs::read(&src).unwrap(), b"replacement content");
    clear_clipboard();
    fs::remove_dir_all(base).unwrap();
}

#[test]
fn startup_cleanup_preserves_existing_undo() {
    let _guard = lock_clipboard_test();
    let _ = ensure_undo_dir();
    let base = uniq_path("startup-undo");
    let src = base.join("src/file.txt");
    let dst = base.join("dst/file.txt");
    write_file(&src, b"replacement content");
    write_file(&dst, b"original content");
    set_clipboard_impl(vec![src.to_string_lossy().to_string()], "copy".into()).unwrap();
    let undo = UndoState::default();
    paste_clipboard_core(
        None,
        dst.parent().unwrap().to_string_lossy().to_string(),
        Some("overwrite".into()),
        undo.clone_inner(),
        CancelState::default(),
        None,
    )
    .unwrap();
    crate::undo::cleanup_stale_backups(None);
    undo.undo().unwrap();
    assert_eq!(fs::read(dst).unwrap(), b"original content");
    clear_clipboard();
    fs::remove_dir_all(base).unwrap();
}
