use super::*;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{Duration, SystemTime};

#[cfg(target_os = "linux")]
mod measurements;

fn uniq_path(label: &str) -> PathBuf {
    let ts = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or(Duration::from_secs(0))
        .as_nanos();
    std::env::temp_dir().join(format!("browsey-undo-test-{label}-{ts}"))
}

fn test_undo_dir() -> PathBuf {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| {
        let dir = uniq_path("undo-base");
        let _ = fs::remove_dir_all(&dir);
        std::env::set_var("BROWSEY_UNDO_DIR", &dir);
        dir
    })
    .clone()
}

fn write_file(path: &Path, content: &[u8]) {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(path)
        .unwrap();
    file.write_all(content).unwrap();
}

#[test]
fn undo_copy_rejects_target_edits_masked_by_later_writes() {
    use crate::fs_utils::copy_test_hooks::{Phase, Scope};
    for moving in [false, true] {
        let root = uniq_path("undo-masked-target-edit");
        let source = root.join("source.bin");
        let target = root.join("target.bin");
        let data = vec![0x42; 32 * 1024];
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
            move_with_fallback(&source, &target)
        } else {
            copy_entry(&source, &target)
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
        assert_eq!(source_data.unwrap(), data);
        assert!(target_data.starts_with(b"foreign edit"));
    }
}

#[test]
fn undo_copy_readback_faults_preserve_both_paths() {
    use crate::fs_utils::copy_test_hooks::{Phase, Scope};
    for moving in [false, true] {
        for fault in ["read", "target", "source", "grow"] {
            let root = uniq_path("undo-readback-fault");
            let source = root.join("source.bin");
            let target = root.join("target.bin");
            let data = vec![0x44; 32 * 1024];
            write_file(&source, &data);
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
                        "target" => fs::OpenOptions::new()
                            .write(true)
                            .open(dst)?
                            .write_all(b"foreign edit")?,
                        "source" => fs::write(&hook_source, b"changed source")?,
                        "grow" => fs::OpenOptions::new()
                            .append(true)
                            .open(dst)?
                            .write_all(&vec![0x45; 128 * 1024])?,
                        _ => unreachable!(),
                    }
                }
                Ok(())
            });
            let result = if moving {
                move_with_fallback(&source, &target)
            } else {
                copy_entry(&source, &target)
            };
            drop(scope);
            let source_data = fs::read(&source).unwrap();
            let target_data = fs::read(&target).unwrap();
            fs::remove_dir_all(root).unwrap();
            assert!(result.is_err());
            assert!(observed.get());
            assert_eq!(
                source_data,
                if fault == "source" {
                    b"changed source".to_vec()
                } else {
                    data.clone()
                }
            );
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
fn undo_copy_refuses_sources_changed_during_streaming_and_keeps_paths() {
    use crate::fs_utils::copy_test_hooks::{Phase, Scope};
    for moving in [false, true] {
        let root = uniq_path("undo-source-stream-change");
        let source = root.join("source.bin");
        let target = root.join("target.bin");
        write_file(&source, &vec![0x41; 32 * 1024]);
        let scope = Scope::new(move |src, _, phase, bytes| {
            if moving && phase == Phase::Rename {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::Unsupported,
                    "force fallback",
                ));
            }
            if phase == Phase::Read && bytes == 8192 {
                fs::write(src, vec![0x42; 32 * 1024])?;
                fs::File::open(src)?.set_times(
                    fs::FileTimes::new()
                        .set_modified(SystemTime::UNIX_EPOCH + Duration::from_secs(100)),
                )?;
            }
            Ok(())
        });
        let result = if moving {
            move_with_fallback(&source, &target)
        } else {
            copy_entry(&source, &target)
        };
        drop(scope);
        let source_data = fs::read(&source).unwrap();
        let target_exists = target.exists();
        fs::remove_dir_all(root).unwrap();
        let error = result.expect_err("changed source must not be reported as a successful copy");
        assert!(error.to_string().contains("Source changed"));
        assert_eq!(source_data, vec![0x42; 32 * 1024]);
        assert!(target_exists);
    }
}

#[test]
fn undo_copy_and_move_refuse_targets_edited_during_finalization() {
    use crate::fs_utils::copy_test_hooks::{Phase, Scope};
    for moving in [true, false] {
        let root = uniq_path("undo-edited-finalizing-target");
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
            if phase == Phase::Synced {
                fs::write(dst, b"other-writer-data")?;
            }
            Ok(())
        });
        let result = if moving {
            move_with_fallback(&source, &target)
        } else {
            copy_entry(&source, &target)
        };
        drop(scope);
        let source_data = fs::read(&source);
        let target_data = fs::read(&target).unwrap();
        fs::remove_dir_all(root).unwrap();
        assert!(
            result.is_err(),
            "edited output cannot justify deleting its source"
        );
        assert_eq!(source_data.unwrap(), data);
        assert_eq!(target_data, b"other-writer-data");
    }
}

#[test]
fn undo_fallback_move_rechecks_outputs_and_preserves_late_source_changes() {
    use crate::fs_utils::copy_test_hooks::{Phase, Scope};
    for change in [
        "target-edit",
        "target-child",
        "source-child",
        "source-edit",
        "source-entry",
        "source-late-child",
    ] {
        let root = uniq_path("undo-fallback-last-gate");
        let source = root.join("source");
        let target = root.join("target");
        write_file(&source.join("a.bin"), b"first-original");
        write_file(&source.join("z.bin"), b"last-original");
        let checked_root = source.clone();
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
                && src == checked_root
                && dst == checked_root.join("a.bin")
            {
                assert!(!checked_root.join("z.bin").exists());
                fs::write(dst, b"late-source-edit")?;
            }
            if change == "source-late-child"
                && phase == Phase::CopyUndoVerified
                && src == checked_root
            {
                fs::write(src.join("new.txt"), b"late-source-child")?;
            }
            Ok(())
        });
        let result = move_with_fallback(&source, &target);
        drop(scope);
        let source_data = fs::read(source.join("a.bin"));
        let target_data = fs::read(target.join("a.bin")).unwrap();
        let new_source_data = fs::read(source.join("new.txt")).ok();
        let new_target_data = fs::read(target.join("new.txt")).ok();
        fs::remove_dir_all(root).unwrap();
        assert!(result.is_err());
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
    }
}

#[test]
fn undo_copy_and_move_faults_keep_source_and_report_partial_output() {
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
            let root = uniq_path("undo-copy-fault");
            let source = root.join("source.bin");
            let target = root.join("target.bin");
            let unrelated = root.join("unrelated.bin");
            let data = vec![0x5a; 64 * 1024];
            write_file(&source, &data);
            write_file(&unrelated, b"unrelated-data");
            let reached = std::rc::Rc::new(std::cell::Cell::new(false));
            let observed = reached.clone();
            let scope = Scope::new(move |_, _, current, bytes| {
                if moving && current == Phase::Rename {
                    return Err(Error::new(ErrorKind::Unsupported, "force copy fallback"));
                }
                if current == phase && bytes > 0 {
                    observed.set(true);
                    return Err(Error::new(kind, "injected undo copy fault"));
                }
                Ok(())
            });
            let result = if moving {
                move_with_fallback(&source, &target)
            } else {
                copy_entry(&source, &target)
            };
            drop(scope);
            let source_data = fs::read(&source).unwrap();
            let partial = fs::read(&target).unwrap();
            let unrelated_data = fs::read(&unrelated).unwrap();
            fs::remove_dir_all(root).unwrap();
            let error = result.expect_err("fault must prevent source deletion");
            assert!(reached.get());
            assert!(error.to_string().contains("injected undo copy fault"));
            assert!(error.to_string().contains("source retained"));
            assert!(error.to_string().contains("destination may remain"));
            assert_eq!(
                error.code(),
                if kind == ErrorKind::NotFound {
                    UndoErrorCode::NotFound
                } else {
                    UndoErrorCode::IoError
                }
            );
            assert_eq!(source_data, data);
            assert_eq!(unrelated_data, b"unrelated-data");
            if phase == Phase::Sync {
                assert_eq!(partial, data);
            } else {
                assert!(!partial.is_empty() && partial.len() < data.len());
                assert_eq!(partial, data[..partial.len()]);
            }
        }
    }
}

#[cfg(unix)]
#[test]
fn undo_move_with_unlinked_open_target_must_not_delete_source() {
    use crate::fs_utils::copy_test_hooks::{Phase, Scope};
    let root = uniq_path("undo-move-unlinked-target");
    let source = root.join("source.bin");
    let target = root.join("target.bin");
    let data = vec![0x44; 32 * 1024];
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
    let result = move_with_fallback(&source, &target);
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
fn undo_move_with_unlinked_open_source_retains_completed_destination() {
    use crate::fs_utils::copy_test_hooks::{Phase, Scope};
    let root = uniq_path("undo-move-unlinked-source");
    let source = root.join("source.bin");
    let target = root.join("target.bin");
    let data = vec![0x46; 32 * 1024];
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
    let result = move_with_fallback(&source, &target);
    drop(scope);
    let target_data = fs::read(&target).unwrap();
    let source_exists = source.exists();
    fs::remove_dir_all(root).unwrap();
    assert_eq!(result.unwrap_err().code(), UndoErrorCode::NotFound);
    assert!(!source_exists);
    assert_eq!(target_data, data);
}

#[cfg(unix)]
#[test]
fn undo_move_with_replaced_target_preserves_source_and_competing_file() {
    use crate::fs_utils::copy_test_hooks::{Phase, Scope};
    let root = uniq_path("undo-move-replaced-target");
    let source = root.join("source.bin");
    let target = root.join("target.bin");
    let retained = root.join("retained-copy.bin");
    let data = vec![0x47; 32 * 1024];
    write_file(&source, &data);
    let saved_copy = retained.clone();
    let scope = Scope::new(move |_, dst, phase, _| {
        if phase == Phase::Rename {
            return Err(std::io::Error::new(
                std::io::ErrorKind::Unsupported,
                "force copy fallback",
            ));
        }
        if phase == Phase::Synced {
            fs::rename(dst, &saved_copy)?;
            fs::write(dst, b"competing-data")?;
        }
        Ok(())
    });
    let result = move_with_fallback(&source, &target);
    drop(scope);
    let source_data = fs::read(&source).unwrap();
    let target_data = fs::read(&target).unwrap();
    let retained_data = fs::read(&retained).unwrap();
    fs::remove_dir_all(root).unwrap();
    assert_eq!(result.unwrap_err().code(), UndoErrorCode::IoError);
    assert_eq!(source_data, data);
    assert_eq!(target_data, b"competing-data");
    assert_eq!(retained_data, data);
}

#[test]
fn undo_fallback_move_keeps_sources_edited_after_copying() {
    use crate::fs_utils::copy_test_hooks::{Phase, Scope};
    for directory in [false, true] {
        let root = uniq_path("undo-move-edited-source");
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
        let data = vec![0x46; 32 * 1024];
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
        let result = move_with_fallback(&source, &target);
        drop(scope);
        let source_data = fs::read(&source_file).unwrap();
        let target_data = fs::read(&target_file).unwrap();
        fs::remove_dir_all(root).unwrap();
        assert!(result.is_err());
        assert_eq!(source_data, b"edited-source-content");
        assert_eq!(target_data, data);
    }
}

#[test]
fn rename_and_undo_redo() {
    let dir = uniq_path("rename");
    let _ = fs::create_dir_all(&dir);
    let from = dir.join("a.txt");
    let to = dir.join("b.txt");
    write_file(&from, b"hello");

    let mut mgr = UndoManager::new();
    mgr.apply(Action::Rename {
        from: from.clone(),
        to: to.clone(),
    })
    .unwrap();
    assert!(!from.exists());
    assert!(to.exists());

    mgr.undo().unwrap();
    assert!(from.exists());
    assert!(!to.exists());

    mgr.redo().unwrap();
    assert!(!from.exists());
    assert!(to.exists());

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn delete_and_restore() {
    let _ = test_undo_dir();
    let dir = uniq_path("delete");
    let _ = fs::create_dir_all(&dir);
    let path = dir.join("file.txt");
    write_file(&path, b"bye");
    let backup = temp_backup_path(&path).unwrap();

    let mut mgr = UndoManager::new();
    mgr.apply(Action::Delete {
        path: path.clone(),
        backup: backup.clone(),
        protection: None,
    })
    .unwrap();
    assert!(!path.exists());
    assert!(backup.exists());

    mgr.undo().unwrap();
    assert!(path.exists());
    assert!(!backup.exists());

    let _ = fs::remove_dir_all(&dir);
    let _ = fs::remove_dir_all(backup.parent().unwrap_or_else(|| Path::new(".")));
}

#[test]
fn create_folder_and_undo() {
    let path = uniq_path("mkdir");
    let mut mgr = UndoManager::new();
    mgr.apply(Action::CreateFolder { path: path.clone() })
        .unwrap();
    assert!(path.is_dir());

    mgr.undo().unwrap();
    assert!(!path.exists());

    let _ = fs::remove_dir_all(&path);
}

#[test]
fn create_file_action_undo_redo() {
    let path = uniq_path("create-file").join("file.txt");
    write_file(&path, b"hello");
    assert!(path.exists());

    let backup = temp_backup_path(&path).unwrap();
    let mut mgr = UndoManager::new();
    mgr.record_applied(Action::Create {
        path: path.clone(),
        backup: backup.clone(),
    });

    mgr.undo().unwrap();
    assert!(!path.exists());
    assert!(backup.exists());

    mgr.redo().unwrap();
    assert!(path.exists());
    assert!(!backup.exists());

    let _ = fs::remove_dir_all(path.parent().unwrap_or_else(|| Path::new(".")));
}

#[test]
fn create_dir_action_undo_redo() {
    let dir = uniq_path("create-dir");
    fs::create_dir_all(&dir).unwrap();
    let backup = temp_backup_path(&dir).unwrap();

    let mut mgr = UndoManager::new();
    mgr.record_applied(Action::Create {
        path: dir.clone(),
        backup: backup.clone(),
    });

    mgr.undo().unwrap();
    assert!(!dir.exists());
    assert!(backup.exists());

    mgr.redo().unwrap();
    assert!(dir.exists());
    assert!(!backup.exists());

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn batch_apply_and_undo_redo() {
    let dir = uniq_path("batch");
    let _ = fs::create_dir_all(&dir);
    let source = dir.join("a.txt");
    let subdir = dir.join("nested");
    let moved = subdir.join("a.txt");
    let copied = dir.join("b.txt");
    write_file(&source, b"hello");

    let mut mgr = UndoManager::new();
    mgr.apply(Action::Batch(vec![
        Action::CreateFolder {
            path: subdir.clone(),
        },
        Action::Move {
            from: source.clone(),
            to: moved.clone(),
        },
        Action::Copy {
            from: moved.clone(),
            to: copied.clone(),
            receipt: CopyReceipt::default(),
        },
    ]))
    .unwrap();

    assert!(!source.exists());
    assert!(moved.exists());
    assert!(copied.exists());
    assert!(subdir.exists());

    mgr.undo().unwrap();
    assert!(source.exists());
    assert!(!moved.exists());
    assert!(!copied.exists());
    assert!(!subdir.exists());

    mgr.redo().unwrap();
    assert!(!source.exists());
    assert!(moved.exists());
    assert!(copied.exists());

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn undo_copy_preserves_edited_and_replaced_targets() {
    for change in ["edit", "replace"] {
        let root = uniq_path("copy-undo-target-change");
        let source = root.join("source.txt");
        let target = root.join("target.txt");
        write_file(&source, b"original");
        let mut manager = UndoManager::new();
        manager
            .apply(Action::Copy {
                from: source.clone(),
                to: target.clone(),
                receipt: CopyReceipt::default(),
            })
            .unwrap();
        if change == "replace" {
            fs::rename(&target, root.join("saved-copy.txt")).unwrap();
        }
        write_file(&target, b"foreign-changes");
        let result = manager.undo();
        assert!(result.is_err(), "must refuse to delete {change}d target");
        assert_eq!(fs::read(&target).unwrap(), b"foreign-changes");
        assert_eq!(fs::read(&source).unwrap(), b"original");
        assert!(manager.can_undo(), "failed undo must retain history");
        assert!(!manager.can_redo());
        fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn undo_directory_copy_preserves_changed_children_and_foreign_files() {
    for change in ["edit", "add", "remove", "rename"] {
        let root = uniq_path("copy-undo-tree-change");
        let source = root.join("source");
        let target = root.join("target");
        write_file(&source.join("nested/file.txt"), b"original");
        let mut manager = UndoManager::new();
        manager
            .apply(Action::Copy {
                from: source.clone(),
                to: target.clone(),
                receipt: CopyReceipt::default(),
            })
            .unwrap();
        let child = target.join("nested/file.txt");
        match change {
            "edit" => write_file(&child, b"edited"),
            "add" => write_file(&target.join("foreign.txt"), b"foreign"),
            "remove" => fs::remove_file(&child).unwrap(),
            _ => fs::rename(&child, target.join("renamed.txt")).unwrap(),
        }
        let before = crate::fs_utils::TreeSnapshot::capture(&target).unwrap();
        let result = manager.undo();
        assert!(result.is_err(), "must refuse changed tree: {change}");
        before.verify(&target).unwrap();
        assert_eq!(
            fs::read(source.join("nested/file.txt")).unwrap(),
            b"original"
        );
        assert!(manager.can_undo());
        fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn undo_copy_rechecks_files_and_never_recursively_removes_late_foreign_children() {
    use crate::fs_utils::copy_test_hooks::{Phase, Scope};
    for change in ["edit", "add", "replace-directory"] {
        let root = uniq_path("copy-undo-after-check");
        let source = root.join("source");
        let target = root.join("target");
        write_file(&source.join("nested/file.txt"), b"original");
        let mut manager = UndoManager::new();
        manager
            .apply(Action::Copy {
                from: source,
                to: target.clone(),
                receipt: CopyReceipt::default(),
            })
            .unwrap();
        let changed_target = target.clone();
        let parked = root.join("parked");
        let reached = std::rc::Rc::new(std::cell::Cell::new(false));
        let observed = reached.clone();
        let scope = Scope::new(move |_, _, phase, _| {
            if phase == Phase::CopyUndoVerified {
                observed.set(true);
                match change {
                    "edit" => write_file(&changed_target.join("nested/file.txt"), b"edited"),
                    "add" => write_file(&changed_target.join("nested/foreign.txt"), b"foreign"),
                    _ => {
                        fs::rename(&changed_target, &parked).unwrap();
                        write_file(&changed_target.join("nested/file.txt"), b"foreign");
                    }
                }
            }
            Ok(())
        });
        let error = manager.undo().unwrap_err();
        drop(scope);
        assert!(reached.get());
        assert!(error.to_string().contains("retained"));
        let preserved = match change {
            "add" => target.join("nested/foreign.txt"),
            _ => target.join("nested/file.txt"),
        };
        assert_eq!(
            fs::read(preserved).unwrap(),
            if change == "edit" {
                b"edited".as_slice()
            } else {
                b"foreign".as_slice()
            }
        );
        fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn undo_copy_without_owned_writer_evidence_retains_target() {
    let root = uniq_path("copy-undo-unverified");
    let target = root.join("target.txt");
    write_file(&target, b"unverified");
    let mut manager = UndoManager::new();
    manager.record_applied(Action::Copy {
        from: root.join("source.txt"),
        to: target.clone(),
        receipt: CopyReceipt::default(),
    });
    let error = manager.undo().unwrap_err();
    assert!(error.to_string().contains("ownership"));
    assert_eq!(fs::read(target).unwrap(), b"unverified");
    assert!(manager.can_undo());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn undo_copy_batch_checks_all_targets_before_removing_any() {
    let root = uniq_path("copy-undo-batch-preflight");
    let first = root.join("first.txt");
    let second = root.join("second.txt");
    let first_copy = root.join("first-copy.txt");
    let second_copy = root.join("second-copy.txt");
    write_file(&first, b"first");
    write_file(&second, b"second");
    let mut manager = UndoManager::new();
    manager
        .apply(Action::Batch(vec![
            Action::Copy {
                from: first,
                to: first_copy.clone(),
                receipt: CopyReceipt::default(),
            },
            Action::Copy {
                from: second.clone(),
                to: second_copy.clone(),
                receipt: CopyReceipt::default(),
            },
        ]))
        .unwrap();
    // Without preflight undo deletes the second copy, discovers the edited
    // first copy, then cannot compensate by re-reading the now missing source.
    fs::remove_file(second).unwrap();
    write_file(&first_copy, b"edited");
    let error = manager.undo().unwrap_err();
    assert!(error.to_string().contains("preflight"));
    assert_eq!(fs::read(first_copy).unwrap(), b"edited");
    assert_eq!(fs::read(second_copy).unwrap(), b"second");
    assert!(manager.can_undo());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn batch_rolls_back_on_failure() {
    let dir = uniq_path("batch-fail");
    let _ = fs::create_dir_all(&dir);
    let source = dir.join("source.txt");
    let existing = dir.join("existing.txt");
    let new_dir = dir.join("new-dir");
    write_file(&source, b"hello");
    write_file(&existing, b"keep");

    let mut mgr = UndoManager::new();
    let err = mgr
        .apply(Action::Batch(vec![
            Action::CreateFolder {
                path: new_dir.clone(),
            },
            Action::Copy {
                from: source.clone(),
                to: existing.clone(),
                receipt: CopyReceipt::default(),
            },
        ]))
        .unwrap_err();
    assert!(err.to_string().contains("Batch action 2 failed"));
    assert!(source.exists());
    assert!(existing.exists());
    assert!(!new_dir.exists());
    assert!(!mgr.can_undo());
    assert!(!mgr.can_redo());

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn copy_redo_restores_copied_bytes_without_the_original_source() {
    let _ = test_undo_dir();
    for directory in [false, true] {
        for missing in [false, true] {
            let root = uniq_path("copy-redo-preserved-bytes");
            let source = root.join("source");
            let target = root.join("target");
            let input = if directory {
                source.join("nested/file.txt")
            } else {
                source.clone()
            };
            let output = if directory {
                target.join("nested/file.txt")
            } else {
                target.clone()
            };
            write_file(&input, b"copied-bytes");
            let mut manager = UndoManager::new();
            manager
                .apply(Action::Copy {
                    from: source.clone(),
                    to: target.clone(),
                    receipt: CopyReceipt::default(),
                })
                .unwrap();
            manager.undo().unwrap();
            if missing {
                if directory {
                    fs::remove_dir_all(&source).unwrap();
                } else {
                    fs::remove_file(&source).unwrap();
                }
            } else {
                write_file(&input, b"changed-source");
            }
            manager.redo().unwrap();
            assert_eq!(fs::read(&output).unwrap(), b"copied-bytes");
            if !missing {
                assert_eq!(fs::read(&input).unwrap(), b"changed-source");
            }
            manager.undo().unwrap();
            manager.redo().unwrap();
            assert_eq!(fs::read(&output).unwrap(), b"copied-bytes");
            fs::remove_dir_all(root).unwrap();
        }
    }
}

#[test]
fn failed_mixed_batch_undo_restores_copy_without_rereading_original() {
    let _ = test_undo_dir();
    for missing in [false, true] {
        let root = uniq_path("mixed-copy-undo-compensation");
        let moved_from = root.join("moved-from.txt");
        let moved_to = root.join("moved-to.txt");
        let copied_from = root.join("copied-from.txt");
        let copied_to = root.join("copied-to.txt");
        write_file(&moved_from, b"moved");
        write_file(&copied_from, b"copied");
        let mut manager = UndoManager::new();
        manager
            .apply(Action::Batch(vec![
                Action::Move {
                    from: moved_from.clone(),
                    to: moved_to.clone(),
                },
                Action::Copy {
                    from: copied_from.clone(),
                    to: copied_to.clone(),
                    receipt: CopyReceipt::default(),
                },
            ]))
            .unwrap();
        write_file(&moved_from, b"foreign");
        if missing {
            fs::remove_file(&copied_from).unwrap();
        } else {
            write_file(&copied_from, b"changed-source");
        }
        let error = manager.undo().unwrap_err();
        assert!(
            !error.to_string().contains("additional rollback issues"),
            "{error}"
        );
        assert_eq!(fs::read(&copied_to).unwrap(), b"copied");
        assert_eq!(fs::read(&moved_to).unwrap(), b"moved");
        assert_eq!(fs::read(&moved_from).unwrap(), b"foreign");
        assert!(manager.can_undo());
        fs::remove_file(&moved_from).unwrap();
        manager.undo().unwrap();
        assert!(!copied_to.exists());
        manager.redo().unwrap();
        assert_eq!(fs::read(copied_to).unwrap(), b"copied");
        fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn copy_restore_conflicts_and_writeback_faults_keep_recovery_bytes() {
    use crate::fs_utils::copy_test_hooks::{Phase, Scope};
    let _ = test_undo_dir();
    for fault in ["conflict", "write", "sync"] {
        let root = uniq_path("copy-restore-fault");
        let source = root.join("source.bin");
        let target = root.join("target.bin");
        let data = vec![0x5a; 64 * 1024];
        write_file(&source, &data);
        let mut action = Action::Copy {
            from: source.clone(),
            to: target.clone(),
            receipt: CopyReceipt::default(),
        };
        super::engine::execute_action(&mut action, Direction::Forward).unwrap();
        super::engine::execute_action(&mut action, Direction::Backward).unwrap();
        let backup = match &action {
            Action::Copy { receipt, .. } => receipt.recovery_path().unwrap().to_path_buf(),
            _ => unreachable!(),
        };
        fs::remove_file(source).unwrap();
        let scope = if fault == "conflict" {
            write_file(&target, b"foreign");
            None
        } else {
            Some(Scope::new(move |_, _, phase, bytes| {
                if (fault == "write" && phase == Phase::Write && bytes > 0)
                    || (fault == "sync" && phase == Phase::Sync)
                {
                    Err(std::io::Error::new(
                        std::io::ErrorKind::StorageFull,
                        "injected restore fault",
                    ))
                } else {
                    Ok(())
                }
            }))
        };
        let error = super::engine::execute_action(&mut action, Direction::Forward).unwrap_err();
        drop(scope);
        assert!(error.to_string().contains(&backup.display().to_string()));
        assert_eq!(fs::read(&backup).unwrap(), data);
        if fault == "conflict" {
            assert_eq!(fs::read(&target).unwrap(), b"foreign");
        }
        fs::remove_file(&target).unwrap();
        super::engine::execute_action(&mut action, Direction::Forward).unwrap();
        super::engine::finalize_action(&mut action);
        assert_eq!(fs::read(&target).unwrap(), data);
        fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn copy_undo_backup_faults_never_remove_the_target() {
    use crate::fs_utils::copy_test_hooks::{Phase, Scope};
    let _ = test_undo_dir();
    for phase in [Phase::Write, Phase::Sync, Phase::RecoveryMarker] {
        let root = uniq_path("copy-undo-backup-fault");
        let source = root.join("source.bin");
        let target = root.join("target.bin");
        let data = vec![0x5a; 64 * 1024];
        write_file(&source, &data);
        let mut manager = UndoManager::new();
        manager
            .apply(Action::Copy {
                from: source.clone(),
                to: target.clone(),
                receipt: CopyReceipt::default(),
            })
            .unwrap();
        let reached = std::rc::Rc::new(std::cell::Cell::new(false));
        let observed = reached.clone();
        let scope = Scope::new(move |_, _, current, bytes| {
            if current == phase && (current != Phase::Write || bytes > 0) {
                observed.set(true);
                Err(std::io::Error::new(
                    std::io::ErrorKind::StorageFull,
                    "injected backup fault",
                ))
            } else {
                Ok(())
            }
        });
        let error = manager.undo().unwrap_err();
        drop(scope);
        assert!(reached.get());
        assert!(error.to_string().contains("retained"));
        assert_eq!(fs::read(&source).unwrap(), data);
        assert_eq!(fs::read(&target).unwrap(), data);
        assert!(manager.can_undo());
        assert!(!manager.can_redo());
        manager.undo().unwrap();
        manager.redo().unwrap();
        assert_eq!(fs::read(&target).unwrap(), data);
        fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn partial_copy_undo_retains_a_complete_protected_backup() {
    use crate::fs_utils::copy_test_hooks::{Phase, Scope};
    let _ = test_undo_dir();
    let root = uniq_path("copy-undo-partial-removal");
    let source = root.join("source");
    let target = root.join("target");
    write_file(&source.join("a.txt"), b"alpha");
    write_file(&source.join("z.txt"), b"zulu");
    let mut action = Action::Copy {
        from: source.clone(),
        to: target.clone(),
        receipt: CopyReceipt::default(),
    };
    super::engine::execute_action(&mut action, Direction::Forward).unwrap();
    fs::remove_dir_all(source).unwrap();
    let checked = target.join("a.txt");
    let scope = Scope::new(move |_, path, phase, _| {
        if phase == Phase::CopyUndoEntry && path == checked {
            Err(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                "injected partial undo fault",
            ))
        } else {
            Ok(())
        }
    });
    let error = super::engine::execute_action(&mut action, Direction::Backward).unwrap_err();
    drop(scope);
    assert!(
        !target.join("z.txt").exists(),
        "first removal must have succeeded"
    );
    assert_eq!(fs::read(target.join("a.txt")).unwrap(), b"alpha");
    let backup = match &action {
        Action::Copy { receipt, .. } => receipt.recovery_path().unwrap(),
        _ => unreachable!(),
    };
    assert_eq!(fs::read(backup.join("a.txt")).unwrap(), b"alpha");
    assert_eq!(fs::read(backup.join("z.txt")).unwrap(), b"zulu");
    assert!(error.to_string().contains(&backup.display().to_string()));
    assert!(fs::read_dir(backup.parent().unwrap().parent().unwrap())
        .unwrap()
        .any(|entry| entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .ends_with(".recovery-required")));
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn changed_recovery_copy_is_not_used_for_redo() {
    let _ = test_undo_dir();
    let root = uniq_path("copy-redo-changed-backup");
    let source = root.join("source.txt");
    let target = root.join("target.txt");
    write_file(&source, b"original");
    let mut action = Action::Copy {
        from: source,
        to: target.clone(),
        receipt: CopyReceipt::default(),
    };
    super::engine::execute_action(&mut action, Direction::Forward).unwrap();
    super::engine::execute_action(&mut action, Direction::Backward).unwrap();
    let backup = match &action {
        Action::Copy { receipt, .. } => receipt.recovery_path().unwrap().to_path_buf(),
        _ => unreachable!(),
    };
    write_file(&backup, b"tampered");
    let error = super::engine::execute_action(&mut action, Direction::Forward).unwrap_err();
    assert!(error.to_string().contains("could not be verified"));
    assert!(!target.exists());
    assert_eq!(fs::read(backup).unwrap(), b"tampered");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn copy_undo_rejects_target_edits_during_backup_without_caching_mixed_bytes() {
    use crate::fs_utils::copy_test_hooks::{Phase, Scope};
    let _ = test_undo_dir();
    let root = uniq_path("copy-undo-target-edit-during-backup");
    let source = root.join("source.txt");
    let target = root.join("target.txt");
    write_file(&source, b"original");
    let mut action = Action::Copy {
        from: source,
        to: target.clone(),
        receipt: CopyReceipt::default(),
    };
    super::engine::execute_action(&mut action, Direction::Forward).unwrap();
    let changed_target = target.clone();
    let scope = Scope::new(move |src, _, phase, _| {
        if phase == Phase::Synced && src == changed_target {
            write_file(&changed_target, b"edited-target");
        }
        Ok(())
    });
    let error = super::engine::execute_action(&mut action, Direction::Backward).unwrap_err();
    drop(scope);
    assert_eq!(fs::read(&target).unwrap(), b"edited-target");
    assert!(error.to_string().contains("target not removed"));
    if let Action::Copy { receipt, .. } = action {
        assert!(
            receipt.backup.is_none(),
            "do not reuse a backup after source verification failed"
        );
    }
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn failed_copy_compensation_keeps_bytes_and_allows_safe_history_retry() {
    use crate::fs_utils::copy_test_hooks::{Phase, Scope};
    let _ = test_undo_dir();
    let root = uniq_path("copy-compensation-write-fault");
    let moved_from = root.join("from.txt");
    let moved_to = root.join("to.txt");
    let source = root.join("source.bin");
    let target = root.join("target.bin");
    let data = vec![0x5a; 64 * 1024];
    write_file(&source, &data);
    write_file(&moved_from, b"moved");
    let mut manager = UndoManager::new();
    manager
        .apply(Action::Batch(vec![
            Action::Move {
                from: moved_from.clone(),
                to: moved_to,
            },
            Action::Copy {
                from: source.clone(),
                to: target.clone(),
                receipt: CopyReceipt::default(),
            },
        ]))
        .unwrap();
    fs::remove_file(source).unwrap();
    write_file(&moved_from, b"foreign");
    let restore_target = target.clone();
    let scope = Scope::new(move |_, dst, phase, bytes| {
        if dst == restore_target && phase == Phase::Write && bytes > 0 {
            Err(std::io::Error::new(
                std::io::ErrorKind::StorageFull,
                "injected compensation fault",
            ))
        } else {
            Ok(())
        }
    });
    let error = manager.undo().unwrap_err();
    drop(scope);
    assert!(error.to_string().contains("additional rollback issues"));
    assert!(error.to_string().contains("Recovery copies retained at"));
    assert_eq!(fs::read(&moved_from).unwrap(), b"foreign");
    assert!(
        target.exists(),
        "partial restored file is reported, not discarded"
    );
    // User-inspected, explicit fixture cleanup models manual recovery, not an
    // automatic retry or overwrite of an uncertain destination.
    fs::remove_file(&target).unwrap();
    fs::remove_file(&moved_from).unwrap();
    manager.undo().unwrap();
    manager.redo().unwrap();
    assert_eq!(fs::read(target).unwrap(), data);
    fs::remove_dir_all(root).unwrap();
}

#[cfg(unix)]
#[test]
fn copy_recovery_preserves_non_utf8_file_names() {
    use std::os::unix::ffi::OsStringExt;
    let _ = test_undo_dir();
    let root = uniq_path("copy-recovery-native-name");
    let source = root.join("source");
    let name = std::ffi::OsString::from_vec(b"file-\xff.bin".to_vec());
    let target = root.join(&name);
    write_file(&source, b"native-name");
    let mut action = Action::Copy {
        from: source.clone(),
        to: target.clone(),
        receipt: CopyReceipt::default(),
    };
    super::engine::execute_action(&mut action, Direction::Forward).unwrap();
    super::engine::execute_action(&mut action, Direction::Backward).unwrap();
    let backup = match &action {
        Action::Copy { receipt, .. } => receipt.recovery_path().unwrap(),
        _ => unreachable!(),
    };
    assert_eq!(backup.file_name().unwrap(), name);
    fs::remove_file(source).unwrap();
    super::engine::execute_action(&mut action, Direction::Forward).unwrap();
    super::engine::finalize_action(&mut action);
    assert_eq!(fs::read(target).unwrap(), b"native-name");
    fs::remove_dir_all(root).unwrap();
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "Requires writable disposable /dev/shm and a distinct backup filesystem"]
fn copy_recovery_roundtrip_across_disposable_filesystems() {
    use std::os::unix::fs::MetadataExt;
    let _ = test_undo_dir();
    for directory in [false, true] {
        let root = Path::new("/dev/shm")
            .join(uniq_path("copy-recovery-cross-volume").file_name().unwrap());
        fs::create_dir(&root).unwrap();
        let source = root.join("source");
        let target = root.join("target");
        let input = if directory {
            source.join("nested/file.bin")
        } else {
            source.clone()
        };
        let output = if directory {
            target.join("nested/file.bin")
        } else {
            target.clone()
        };
        write_file(&input, b"cross-volume");
        let mut action = Action::Copy {
            from: source.clone(),
            to: target,
            receipt: CopyReceipt::default(),
        };
        super::engine::execute_action(&mut action, Direction::Forward).unwrap();
        super::engine::execute_action(&mut action, Direction::Backward).unwrap();
        let backup = match &action {
            Action::Copy { receipt, .. } => receipt.recovery_path().unwrap(),
            _ => unreachable!(),
        };
        assert_ne!(
            fs::metadata(backup).unwrap().dev(),
            fs::metadata(&root).unwrap().dev()
        );
        if directory {
            fs::remove_dir_all(source).unwrap();
        } else {
            fs::remove_file(source).unwrap();
        }
        super::engine::execute_action(&mut action, Direction::Forward).unwrap();
        super::engine::finalize_action(&mut action);
        assert_eq!(fs::read(output).unwrap(), b"cross-volume");
        fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn history_worker_keeps_typed_errors_and_copy_recovery() {
    tauri::async_runtime::block_on(async {
        let _ = test_undo_dir();
        let state = UndoState::default();
        assert_eq!(
            super::execute_history_operation(state.clone(), Direction::Backward)
                .await
                .unwrap_err()
                .code(),
            UndoErrorCode::UndoUnavailable
        );
        let root = uniq_path("copy-history-worker");
        let source = root.join("source.txt");
        let target = root.join("target.txt");
        write_file(&source, b"original");
        state
            .record(Action::Copy {
                from: source.clone(),
                to: target.clone(),
                receipt: CopyReceipt::default(),
            })
            .unwrap();
        super::execute_history_operation(state.clone(), Direction::Backward)
            .await
            .unwrap();
        fs::remove_file(source).unwrap();
        super::execute_history_operation(state, Direction::Forward)
            .await
            .unwrap();
        assert_eq!(fs::read(target).unwrap(), b"original");
        fs::remove_dir_all(root).unwrap();
    });
}

#[test]
fn move_with_fallback_refuses_existing_destination() {
    let dir = uniq_path("move-no-overwrite");
    let _ = fs::create_dir_all(&dir);
    let source = dir.join("source.txt");
    let dest = dir.join("dest.txt");
    write_file(&source, b"source-data");
    write_file(&dest, b"dest-data");

    let err = move_with_fallback(&source, &dest).expect_err("existing destination should fail");
    let err_msg = err.to_string();
    assert!(
        err_msg.contains("File exists")
            || err_msg.contains("already exists")
            || err_msg.contains("rename"),
        "unexpected error: {err}"
    );
    assert!(source.exists(), "source should remain when move fails");
    assert_eq!(fs::read(&dest).unwrap_or_default(), b"dest-data");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn copy_delete_fallback_moves_file_without_overwrite() {
    let dir = uniq_path("move-copy-delete");
    let _ = fs::create_dir_all(&dir);
    let source = dir.join("source.txt");
    let dest = dir.join("dest.txt");
    write_file(&source, b"source-data");
    let src_snapshot = snapshot_existing_path(&source).expect("snapshot");

    move_by_copy_delete_noreplace(&source, &dest, &src_snapshot).expect("fallback move");
    assert!(
        !source.exists(),
        "source should be deleted after fallback move"
    );
    assert_eq!(fs::read(&dest).unwrap_or_default(), b"source-data");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn copy_delete_fallback_refuses_existing_destination() {
    let dir = uniq_path("move-copy-delete-exists");
    let _ = fs::create_dir_all(&dir);
    let source = dir.join("source.txt");
    let dest = dir.join("dest.txt");
    write_file(&source, b"source-data");
    write_file(&dest, b"dest-data");
    let src_snapshot = snapshot_existing_path(&source).expect("snapshot");

    let err = move_by_copy_delete_noreplace(&source, &dest, &src_snapshot)
        .expect_err("fallback move should fail when destination exists");
    assert!(is_destination_exists_error(&err), "unexpected error: {err}");
    assert!(
        source.exists(),
        "source should remain when destination exists"
    );
    assert_eq!(fs::read(&dest).unwrap_or_default(), b"dest-data");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn fallback_copy_reports_final_sync_failure_and_keeps_source() {
    let root = uniq_path("copy-sync-failure");
    let source = root.join("source.txt");
    let destination = root.join("destination.txt");
    write_file(&source, b"original-data");
    let error = super::path_ops::copy_file_noreplace_with_sync(&source, &destination, |file| {
        assert_eq!(
            file.metadata().unwrap().len(),
            13,
            "all writes precede sync"
        );
        Err(std::io::Error::other("injected delayed writeback failure"))
    })
    .unwrap_err();
    assert_eq!(error.code(), UndoErrorCode::IoError);
    assert!(error
        .to_string()
        .contains("injected delayed writeback failure"));
    assert_eq!(fs::read(&source).unwrap(), b"original-data");
    assert_eq!(fs::read(&destination).unwrap(), b"original-data");
    fs::remove_dir_all(root).unwrap();
}

#[test]
#[cfg(unix)]
fn fallback_move_rejects_fifo_without_deleting_source_or_creating_target() {
    use std::os::unix::ffi::OsStrExt;
    let root = uniq_path("fallback-fifo");
    fs::create_dir(&root).unwrap();
    let source = root.join("pipe");
    let destination = root.join("destination");
    let cpath = std::ffi::CString::new(source.as_os_str().as_bytes()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(cpath.as_ptr(), 0o600) }, 0);
    let snapshot = snapshot_existing_path(&source).unwrap();
    assert!(move_by_copy_delete_noreplace(&source, &destination, &snapshot).is_err());
    assert!(fs::symlink_metadata(&source).is_ok());
    assert!(!destination.exists());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn fallback_copy_checks_sync_only_after_exclusive_destination_creation() {
    let root = uniq_path("copy-sync-existing");
    let source = root.join("source.txt");
    let destination = root.join("destination.txt");
    write_file(&source, b"original-data");
    write_file(&destination, b"other-process-data");
    let error = super::path_ops::copy_file_noreplace_with_sync(&source, &destination, |_| {
        panic!("an existing destination must never be opened for finalization");
    })
    .unwrap_err();
    assert_eq!(error.code(), UndoErrorCode::TargetExists);
    assert_eq!(fs::read(&source).unwrap(), b"original-data");
    assert_eq!(fs::read(&destination).unwrap(), b"other-process-data");
    fs::remove_dir_all(root).unwrap();
}

#[test]
#[cfg(unix)]
fn copy_delete_fallback_keeps_complete_destination_after_partial_source_deletion() {
    use std::os::unix::fs::PermissionsExt;
    if unsafe { libc::geteuid() } == 0 {
        return;
    }
    let root = uniq_path("fallback-partial-delete");
    let parent = root.join("source-parent");
    let source = parent.join("tree");
    let dest = root.join("destination");
    write_file(&source.join("document.txt"), b"irreplaceable-data");
    let snapshot = snapshot_existing_path(&source).unwrap();
    // Child removal is allowed, but removing the source root from its parent
    // fails only after the directory's contents have already been deleted.
    fs::set_permissions(&parent, fs::Permissions::from_mode(0o500)).unwrap();
    let result = move_by_copy_delete_noreplace(&source, &dest, &snapshot);
    fs::set_permissions(&parent, fs::Permissions::from_mode(0o700)).unwrap();
    let original_exists = source.join("document.txt").exists();
    let copied = fs::read(dest.join("document.txt")).ok();
    fs::remove_dir_all(root).unwrap();
    assert!(result.is_err());
    assert!(
        !original_exists,
        "source deletion must actually have progressed"
    );
    assert_eq!(
        copied.as_deref(),
        Some(b"irreplaceable-data".as_slice()),
        "failed source deletion must never destroy the only complete copy"
    );
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("destination retained"));
}

#[test]
fn delete_entry_path_removes_non_empty_directory() {
    let dir = uniq_path("delete-dir-recursive");
    let nested = dir.join("nested");
    let deep_file = nested.join("child.txt");
    let _ = fs::create_dir_all(&nested);
    write_file(&deep_file, b"child");

    delete_entry_path(&dir).expect("recursive delete should succeed");
    assert!(!dir.exists(), "directory should be removed recursively");
}

#[test]
fn undo_failure_restores_stack() {
    let _ = test_undo_dir();
    let dir = uniq_path("undo-fail");
    let _ = fs::create_dir_all(&dir);
    let path = dir.join("file.txt");
    write_file(&path, b"bye");
    let backup = temp_backup_path(&path).unwrap();

    let mut mgr = UndoManager::new();
    mgr.apply(Action::Delete {
        path: path.clone(),
        backup: backup.clone(),
        protection: None,
    })
    .unwrap();
    assert!(!path.exists());
    assert!(backup.exists());

    let _ = fs::remove_file(&backup);
    let err = mgr.undo().unwrap_err();
    let err_msg = err.to_string();
    assert!(
        err_msg.contains("Backup")
            || err_msg.contains("rename")
            || err_msg.contains("metadata")
            || err_msg.contains("does not exist")
    );
    assert!(mgr.can_undo());
    assert!(!mgr.can_redo());

    let _ = fs::remove_dir_all(&dir);
    let _ = fs::remove_dir_all(backup.parent().unwrap_or_else(|| Path::new(".")));
}

#[test]
fn path_snapshot_accepts_unchanged_path() {
    let dir = uniq_path("snapshot-unchanged");
    let _ = fs::create_dir_all(&dir);
    let path = dir.join("file.txt");
    write_file(&path, b"one");

    let snapshot = snapshot_existing_path(&path).expect("snapshot should succeed");
    assert!(
        assert_path_snapshot(&path, &snapshot).is_ok(),
        "unchanged path should pass snapshot check"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn history_keeps_only_the_last_fifty_actions() {
    let root = uniq_path("history-cap");
    fs::create_dir(&root).unwrap();
    let mut manager = UndoManager::new();
    for index in 0..51 {
        manager
            .apply(Action::CreateFolder {
                path: root.join(index.to_string()),
            })
            .unwrap();
    }
    for _ in 0..50 {
        manager.undo().unwrap();
    }
    assert!(!manager.can_undo());
    assert!(manager.can_redo());
    assert!(
        root.join("0").is_dir(),
        "oldest action is outside retained history"
    );
    assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn new_action_clears_redo_and_a_new_manager_has_no_history() {
    let root = uniq_path("history-lifetime");
    fs::create_dir(&root).unwrap();
    let mut manager = UndoManager::new();
    manager
        .apply(Action::CreateFolder {
            path: root.join("first"),
        })
        .unwrap();
    manager.undo().unwrap();
    assert!(manager.can_redo());
    manager
        .apply(Action::CreateFolder {
            path: root.join("second"),
        })
        .unwrap();
    assert!(!manager.can_redo());
    assert_eq!(
        manager.redo().unwrap_err().code(),
        UndoErrorCode::RedoUnavailable
    );
    let mut fresh = UndoManager::new();
    assert!(!fresh.can_undo());
    assert_eq!(
        fresh.undo().unwrap_err().code(),
        UndoErrorCode::UndoUnavailable
    );
    assert!(root.join("second").exists());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn redo_failure_keeps_history_and_preserves_conflicting_destination() {
    let root = uniq_path("redo-conflict");
    let source = root.join("source.txt");
    let destination = root.join("destination.txt");
    write_file(&source, b"original");
    let mut manager = UndoManager::new();
    manager
        .apply(Action::Move {
            from: source.clone(),
            to: destination.clone(),
        })
        .unwrap();
    manager.undo().unwrap();
    write_file(&destination, b"other-process-data");
    assert_eq!(
        manager.redo().unwrap_err().code(),
        UndoErrorCode::TargetExists
    );
    assert!(manager.can_redo());
    assert!(!manager.can_undo());
    assert_eq!(fs::read(&source).unwrap(), b"original");
    assert_eq!(fs::read(&destination).unwrap(), b"other-process-data");
    fs::remove_file(&destination).unwrap();
    manager.redo().unwrap();
    assert!(manager.can_undo());
    assert!(!source.exists());
    assert_eq!(fs::read(&destination).unwrap(), b"original");
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn path_snapshot_detects_replaced_path() {
    let dir = uniq_path("snapshot-replaced");
    let _ = fs::create_dir_all(&dir);
    let path = dir.join("file.txt");
    write_file(&path, b"first");

    let snapshot = snapshot_existing_path(&path).expect("snapshot should succeed");
    let _ = fs::remove_file(&path);
    // Replace with a different path kind to avoid inode-reuse flakiness on CI filesystems.
    let _ = fs::create_dir(&path);

    let err = assert_path_snapshot(&path, &snapshot).expect_err("snapshot mismatch expected");
    assert!(err.to_string().contains("Path changed during operation"));

    let _ = fs::remove_dir_all(&dir);
}
