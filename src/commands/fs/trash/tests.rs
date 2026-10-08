use super::{
    super::error::{FsError, FsErrorCode, FsResult},
    backend::TrashBackend,
    empty_trash_with_ops,
    listing::apply_original_trash_fields,
    move_ops::{move_single_to_trash_with_backend, move_to_trash_many_with_backend},
    purge_trash_items_with_ops, restore_trash_items_with_ops,
    staging::{
        cleanup_stale_trash_staging_at, decode_percent_encoded_unix_path, encode_trash_info_path,
        load_trash_stage_journal_entries_at, store_trash_stage_journal_entries_at,
        TrashStageJournalEntry,
    },
    TrashOps,
};
use crate::{
    entry::build_entry,
    icons::icon_ids::PDF_FILE,
    undo::{Action, UndoState},
};
use ::trash::TrashItem;
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::ffi::OsString;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::os::unix::ffi::OsStringExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

fn uniq_path(label: &str) -> PathBuf {
    let ts = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or(Duration::from_secs(0))
        .as_nanos();
    std::env::temp_dir().join(format!("browsey-fs-test-{label}-{ts}"))
}

fn write_file(path: &Path, bytes: &[u8]) {
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .truncate(true)
        .open(path)
        .expect("open file");
    file.write_all(bytes).expect("write file");
}

#[derive(Default)]
struct FakeTrashBackend {
    items: RefCell<Vec<TrashItem>>,
    list_script: RefCell<VecDeque<FsResult<Vec<TrashItem>>>>,
    delete_calls: RefCell<Vec<PathBuf>>,
    rewrite_calls: RefCell<Vec<(PathBuf, PathBuf)>>,
    fail_delete_call: Cell<Option<usize>>,
    delete_call_count: Cell<usize>,
}

impl FakeTrashBackend {
    fn with_fail_on_delete_call(call: usize) -> Self {
        Self {
            fail_delete_call: Cell::new(Some(call)),
            ..Self::default()
        }
    }

    fn queue_list_response(&self, value: FsResult<Vec<TrashItem>>) {
        self.list_script.borrow_mut().push_back(value);
    }
}

impl TrashBackend for FakeTrashBackend {
    fn list_items(&self) -> FsResult<Vec<TrashItem>> {
        if let Some(next) = self.list_script.borrow_mut().pop_front() {
            return next;
        }
        Ok(self.items.borrow().clone())
    }

    fn delete_path(&self, path: &Path) -> FsResult<()> {
        let next_call = self.delete_call_count.get().saturating_add(1);
        self.delete_call_count.set(next_call);
        self.delete_calls.borrow_mut().push(path.to_path_buf());

        if self.fail_delete_call.get() == Some(next_call) {
            return Err(FsError::new(
                FsErrorCode::TrashFailed,
                "simulated trash delete failure",
            ));
        }

        if let Ok(meta) = fs::symlink_metadata(path) {
            if meta.is_dir() {
                fs::remove_dir_all(path).map_err(|e| {
                    FsError::new(
                        FsErrorCode::TrashFailed,
                        format!("fake delete dir failed: {e}"),
                    )
                })?;
            } else {
                fs::remove_file(path).map_err(|e| {
                    FsError::new(
                        FsErrorCode::TrashFailed,
                        format!("fake delete file failed: {e}"),
                    )
                })?;
            }
        }

        let name = path
            .file_name()
            .map(|n| n.to_os_string())
            .unwrap_or_else(|| OsString::from("item"));
        let original_parent = path
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("/"));
        let id = PathBuf::from(format!("/tmp/fake-trash/info/item-{next_call}.trashinfo"))
            .into_os_string();
        self.items.borrow_mut().push(TrashItem {
            id,
            name,
            original_parent,
            time_deleted: 0,
        });
        Ok(())
    }

    fn rewrite_original_path(&self, item: &TrashItem, original_path: &Path) -> FsResult<()> {
        self.rewrite_calls
            .borrow_mut()
            .push((PathBuf::from(&item.id), original_path.to_path_buf()));
        Ok(())
    }
}

#[derive(Default)]
struct FakeTrashOps {
    items: RefCell<Vec<TrashItem>>,
    restored_ids: RefCell<Vec<OsString>>,
    purged_ids: RefCell<Vec<OsString>>,
    fail_restore: Cell<bool>,
    fail_purge: Cell<bool>,
    purge_calls: Cell<usize>,
    list_error: RefCell<Option<FsError>>,
    restore_error: RefCell<Option<FsError>>,
}

impl TrashOps for FakeTrashOps {
    fn list_items(&self) -> FsResult<Vec<TrashItem>> {
        if let Some(error) = self.list_error.borrow_mut().take() {
            return Err(error);
        }
        Ok(self.items.borrow().clone())
    }

    fn restore_items(&self, items: Vec<TrashItem>) -> FsResult<()> {
        if let Some(error) = self.restore_error.borrow_mut().take() {
            return Err(error);
        }
        if self.fail_restore.get() {
            return Err(FsError::new(
                FsErrorCode::TrashFailed,
                "simulated restore failure",
            ));
        }
        self.restored_ids
            .borrow_mut()
            .extend(items.into_iter().map(|item| item.id));
        Ok(())
    }

    fn purge_items(&self, items: Vec<TrashItem>) -> FsResult<()> {
        self.purge_calls.set(self.purge_calls.get() + 1);
        if self.fail_purge.get() {
            return Err(FsError::new(
                FsErrorCode::TrashFailed,
                "simulated purge failure",
            ));
        }
        self.purged_ids
            .borrow_mut()
            .extend(items.into_iter().map(|item| item.id));
        Ok(())
    }
}

#[test]
fn empty_trash_purges_the_native_catalog_including_non_utf8_ids() {
    let ops = FakeTrashOps::default();
    let ids = vec![
        OsString::from("unreadable-item"),
        OsString::from_vec(vec![0xff]),
    ];
    *ops.items.borrow_mut() = ids
        .iter()
        .map(|id| TrashItem {
            id: id.clone(),
            name: OsString::from("item"),
            original_parent: PathBuf::from("/missing/original"),
            time_deleted: 0,
        })
        .collect();
    let emitted = Cell::new(false);
    empty_trash_with_ops(&ops, || emitted.set(true)).expect("empty trash");
    assert_eq!(*ops.purged_ids.borrow(), ids);
    assert_eq!(ops.purge_calls.get(), 1);
    assert!(emitted.get());
}

#[test]
fn empty_trash_empty_catalog_is_a_noop() {
    let ops = FakeTrashOps::default();
    let emitted = Cell::new(false);
    empty_trash_with_ops(&ops, || emitted.set(true)).expect("already empty");
    assert_eq!(ops.purge_calls.get(), 0);
    assert!(!emitted.get());
}

#[test]
fn empty_trash_does_not_purge_when_listing_fails() {
    let ops = FakeTrashOps::default();
    *ops.list_error.borrow_mut() = Some(FsError::new(FsErrorCode::TrashFailed, "list failed"));
    let emitted = Cell::new(false);
    let error = empty_trash_with_ops(&ops, || emitted.set(true)).expect_err("list failed");
    assert_eq!(error.code(), FsErrorCode::TrashFailed);
    assert_eq!(ops.purge_calls.get(), 0);
    assert!(!emitted.get());
}

#[test]
fn empty_trash_notifies_after_failed_purge_without_retrying() {
    let ops = FakeTrashOps::default();
    ops.items.borrow_mut().push(TrashItem {
        id: OsString::from("id"),
        name: OsString::from("item"),
        original_parent: PathBuf::from("/missing"),
        time_deleted: 0,
    });
    ops.fail_purge.set(true);
    let emitted = Cell::new(false);
    let error = empty_trash_with_ops(&ops, || emitted.set(true)).expect_err("purge failed");
    assert_eq!(error.code(), FsErrorCode::TrashFailed);
    assert_eq!(ops.purge_calls.get(), 1);
    assert!(emitted.get());
}

#[test]
fn encode_trash_info_path_percent_encodes_non_unreserved_bytes() {
    let path = PathBuf::from(OsString::from_vec(vec![
        b'/', b't', b'm', b'p', b'/', b'a', b' ', b'b', b'%', 0xFF,
    ]));
    assert_eq!(encode_trash_info_path(&path), "/tmp/a%20b%25%FF");
}

#[test]
fn decode_percent_encoded_unix_path_roundtrips_non_utf8() {
    let original = PathBuf::from(OsString::from_vec(vec![
        b'/', b't', b'm', b'p', b'/', b'x', 0xFF, b' ', b'y',
    ]));
    let encoded = encode_trash_info_path(&original);
    let decoded = decode_percent_encoded_unix_path(&encoded).expect("decode should succeed");
    assert_eq!(decoded, original);
}

#[test]
fn move_single_to_trash_uses_backend_and_rewrites_original_path() {
    let dir = uniq_path("single-trash-success");
    let _ = fs::create_dir_all(&dir);
    let src = dir.join("file.txt");
    write_file(&src, b"hello");

    let backend = FakeTrashBackend::default();
    let action =
        move_single_to_trash_with_backend(&src.to_string_lossy(), &backend).expect("success");

    match action {
        Action::Move { from, to: _ } => assert_eq!(from, src),
        other => panic!("expected move action, got {other:?}"),
    }
    assert_eq!(
        backend.delete_calls.borrow().len(),
        1,
        "one delete expected"
    );
    assert_eq!(
        backend.rewrite_calls.borrow().len(),
        1,
        "one rewrite expected"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn move_single_to_trash_falls_back_to_delete_when_item_not_detected() {
    let dir = uniq_path("single-trash-delete-fallback");
    let _ = fs::create_dir_all(&dir);
    let src = dir.join("file.txt");
    write_file(&src, b"hello");

    let backend = FakeTrashBackend::default();
    backend.queue_list_response(Ok(Vec::new()));
    backend.queue_list_response(Ok(Vec::new()));

    let action =
        move_single_to_trash_with_backend(&src.to_string_lossy(), &backend).expect("success");
    match action {
        Action::Delete { path, .. } => assert_eq!(path, src),
        other => panic!("expected delete action, got {other:?}"),
    }
    assert_eq!(
        backend.rewrite_calls.borrow().len(),
        0,
        "no rewrite expected"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn trash_batch_preserves_file_and_folder_contents_through_undo_redo() {
    let dir = uniq_path("trash-batch-undo-redo");
    let file = dir.join("document.txt");
    let folder = dir.join("folder");
    write_file(&file, b"document contents");
    write_file(&folder.join("nested.txt"), b"nested contents");
    let backend = FakeTrashBackend::default();
    // Simulate a platform where trash IDs cannot be discovered. The existing
    // undo fallback must preserve both items, including the directory tree.
    backend.queue_list_response(Ok(Vec::new()));
    backend.queue_list_response(Ok(Vec::new()));
    let undo = UndoState::default();
    move_to_trash_many_with_backend(
        vec![
            file.to_string_lossy().into_owned(),
            folder.to_string_lossy().into_owned(),
        ],
        undo.clone(),
        None,
        &backend,
        |_| false,
        |_, _, _| {},
        || {},
    )
    .unwrap();
    assert!(!file.exists() && !folder.exists());
    undo.undo().unwrap();
    assert_eq!(fs::read(&file).unwrap(), b"document contents");
    assert_eq!(
        fs::read(folder.join("nested.txt")).unwrap(),
        b"nested contents"
    );
    undo.redo().unwrap();
    assert!(!file.exists() && !folder.exists());
    undo.undo().unwrap();
    assert_eq!(fs::read(&file).unwrap(), b"document contents");
    assert_eq!(
        fs::read(folder.join("nested.txt")).unwrap(),
        b"nested contents"
    );
    fs::remove_dir_all(dir).unwrap();
}

#[test]
fn move_to_trash_many_rolls_back_previous_on_later_failure() {
    let dir = uniq_path("many-trash-rollback");
    let _ = fs::create_dir_all(&dir);
    let src1 = dir.join("a.txt");
    let src2 = dir.join("b.txt");
    write_file(&src1, b"a");
    write_file(&src2, b"b");

    let backend = FakeTrashBackend::with_fail_on_delete_call(2);
    let undo = UndoState::default();
    let result = move_to_trash_many_with_backend(
        vec![
            src1.to_string_lossy().into_owned(),
            src2.to_string_lossy().into_owned(),
        ],
        undo,
        None,
        &backend,
        |_| false,
        |_done, _total, _finished| {},
        || {},
    );

    assert!(result.is_err(), "second delete should fail");
    assert!(
        src1.exists(),
        "first file should be restored after rollback"
    );
    assert!(
        src2.exists(),
        "second file should be rolled back by staging logic"
    );

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn cleanup_stale_trash_staging_recovers_staged_item_and_clears_journal() {
    let dir = uniq_path("cleanup-staged-trash");
    let journal = dir.join("journal.tsv");
    let _ = fs::create_dir_all(&dir);
    let staged = dir.join("browsey-trash-stage-test");
    let original = dir.join("original.txt");
    write_file(&staged, b"staged");

    let entries = vec![TrashStageJournalEntry {
        staged: staged.clone(),
        original: original.clone(),
    }];
    store_trash_stage_journal_entries_at(&journal, &entries).expect("store journal");

    cleanup_stale_trash_staging_at(&journal);

    assert!(!staged.exists(), "staged path should be gone after cleanup");
    assert!(
        original.exists(),
        "original path should be restored after cleanup"
    );
    let remaining = load_trash_stage_journal_entries_at(&journal).expect("load journal");
    assert!(remaining.is_empty(), "journal should be emptied");

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn trash_entry_icon_uses_original_path_extension() {
    let dir = uniq_path("trash-original-icon");
    let _ = fs::create_dir_all(&dir);

    let staged = dir.join("browsey-trash-stage-test");
    write_file(&staged, b"dummy");
    let meta = fs::symlink_metadata(&staged).expect("staged metadata");
    let is_link = meta.file_type().is_symlink();
    let mut entry = build_entry(&staged, &meta, is_link, false);

    let item = TrashItem {
        id: OsString::from("/tmp/fake-trash/info/entry.trashinfo"),
        name: OsString::from("report.pdf"),
        original_parent: dir.clone(),
        time_deleted: 0,
    };
    let original_path = item.original_path();
    apply_original_trash_fields(&mut entry, &original_path, &item, &meta, is_link);

    assert_eq!(entry.ext.as_deref(), Some("pdf"));
    assert_eq!(entry.icon_id, PDF_FILE);

    let _ = fs::remove_dir_all(&dir);
}

#[test]
fn restore_with_ops_restores_selected_ids_and_emits_change() {
    let ops = FakeTrashOps::default();
    ops.items.borrow_mut().push(TrashItem {
        id: OsString::from("id-a"),
        name: OsString::from("a.txt"),
        original_parent: PathBuf::from("/tmp"),
        time_deleted: 0,
    });
    ops.items.borrow_mut().push(TrashItem {
        id: OsString::from("id-b"),
        name: OsString::from("b.txt"),
        original_parent: PathBuf::from("/tmp"),
        time_deleted: 0,
    });
    let emitted = Cell::new(false);

    restore_trash_items_with_ops(vec!["id-b".into()], &ops, || emitted.set(true))
        .expect("restore should succeed");

    assert_eq!(ops.restored_ids.borrow().len(), 1);
    assert_eq!(ops.restored_ids.borrow()[0], OsString::from("id-b"));
    assert!(emitted.get(), "successful restore should emit change event");
}

#[test]
fn restore_with_ops_rejects_empty_selection_after_filtering() {
    let ops = FakeTrashOps::default();
    let emitted = Cell::new(false);

    let err = restore_trash_items_with_ops(vec!["missing".into()], &ops, || emitted.set(true))
        .expect_err("restore should fail when no ids match");

    assert!(err.to_string().contains("Nothing to restore"));
    assert!(
        ops.restored_ids.borrow().is_empty(),
        "no restore should be attempted"
    );
    assert!(
        !emitted.get(),
        "failed restore should not emit change event"
    );
}

#[test]
fn restore_with_ops_conflict_failure_does_not_emit_change() {
    let ops = FakeTrashOps::default();
    ops.items.borrow_mut().push(TrashItem {
        id: OsString::from("id-conflict"),
        name: OsString::from("conflict.txt"),
        original_parent: PathBuf::from("/tmp"),
        time_deleted: 0,
    });
    ops.restore_error.borrow_mut().replace(FsError::new(
        FsErrorCode::TargetExists,
        "Destination already exists: /tmp/conflict.txt",
    ));
    let emitted = Cell::new(false);

    let err = restore_trash_items_with_ops(vec!["id-conflict".into()], &ops, || emitted.set(true))
        .expect_err("restore should fail when destination already exists");

    assert_eq!(err.code(), FsErrorCode::TargetExists);
    assert!(
        err.to_string().contains("Destination already exists"),
        "unexpected error: {err}"
    );
    assert!(
        ops.restored_ids.borrow().is_empty(),
        "restore backend should not report successful ids"
    );
    assert!(
        !emitted.get(),
        "restore conflict should not emit change event"
    );
}

#[test]
fn purge_with_ops_purges_selected_ids_and_emits_change() {
    let ops = FakeTrashOps::default();
    ops.items.borrow_mut().push(TrashItem {
        id: OsString::from("id-x"),
        name: OsString::from("x.txt"),
        original_parent: PathBuf::from("/tmp"),
        time_deleted: 0,
    });
    let emitted = Cell::new(false);

    purge_trash_items_with_ops(vec!["id-x".into()], &ops, || emitted.set(true))
        .expect("purge should succeed");

    assert_eq!(ops.purged_ids.borrow().len(), 1);
    assert_eq!(ops.purged_ids.borrow()[0], OsString::from("id-x"));
    assert!(emitted.get(), "successful purge should emit change event");
}

#[test]
fn purge_with_ops_failure_does_not_emit_change() {
    let ops = FakeTrashOps::default();
    ops.fail_purge.set(true);
    ops.items.borrow_mut().push(TrashItem {
        id: OsString::from("id-y"),
        name: OsString::from("y.txt"),
        original_parent: PathBuf::from("/tmp"),
        time_deleted: 0,
    });
    let emitted = Cell::new(false);

    let err = purge_trash_items_with_ops(vec!["id-y".into()], &ops, || emitted.set(true))
        .expect_err("purge should fail when backend fails");

    assert!(
        err.to_string().contains("simulated purge failure"),
        "unexpected error: {err}"
    );
    assert!(
        ops.purged_ids.borrow().is_empty(),
        "no ids should be recorded"
    );
    assert!(!emitted.get(), "failed purge should not emit change event");
}

#[test]
fn restore_with_ops_list_failure_does_not_emit_change() {
    let ops = FakeTrashOps::default();
    ops.list_error.borrow_mut().replace(FsError::new(
        FsErrorCode::TrashFailed,
        "simulated trash listing failure",
    ));
    let emitted = Cell::new(false);

    let err = restore_trash_items_with_ops(vec!["id-z".into()], &ops, || emitted.set(true))
        .expect_err("restore should fail when trash listing fails");

    assert_eq!(err.code(), FsErrorCode::TrashFailed);
    assert!(
        err.to_string().contains("simulated trash listing failure"),
        "unexpected error: {err}"
    );
    assert!(
        ops.restored_ids.borrow().is_empty(),
        "restore should not be attempted when list fails"
    );
    assert!(!emitted.get(), "list failure should not emit change event");
}

#[test]
fn purge_with_ops_list_failure_does_not_emit_change() {
    let ops = FakeTrashOps::default();
    ops.list_error.borrow_mut().replace(FsError::new(
        FsErrorCode::TrashFailed,
        "simulated trash listing failure",
    ));
    let emitted = Cell::new(false);

    let err = purge_trash_items_with_ops(vec!["id-z".into()], &ops, || emitted.set(true))
        .expect_err("purge should fail when trash listing fails");

    assert_eq!(err.code(), FsErrorCode::TrashFailed);
    assert!(
        err.to_string().contains("simulated trash listing failure"),
        "unexpected error: {err}"
    );
    assert!(
        ops.purged_ids.borrow().is_empty(),
        "purge should not be attempted when list fails"
    );
    assert!(!emitted.get(), "list failure should not emit change event");
}
