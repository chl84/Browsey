use super::{
    backend::TrashBackend,
    listing::apply_original_trash_fields,
    move_ops::{move_single_to_trash_with_backend, move_to_trash_many_with_backend},
    staging::{
        cleanup_stale_trash_staging_at, decode_percent_encoded_unix_path, encode_trash_info_path,
        load_trash_stage_journal_entries_at, store_trash_stage_journal_entries_at,
        TrashStageJournalEntry,
    },
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
    list_script: RefCell<VecDeque<Result<Vec<TrashItem>, String>>>,
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

    fn queue_list_response(&self, value: Result<Vec<TrashItem>, String>) {
        self.list_script.borrow_mut().push_back(value);
    }
}

impl TrashBackend for FakeTrashBackend {
    fn list_items(&self) -> Result<Vec<TrashItem>, String> {
        if let Some(next) = self.list_script.borrow_mut().pop_front() {
            return next;
        }
        Ok(self.items.borrow().clone())
    }

    fn delete_path(&self, path: &Path) -> Result<(), String> {
        let next_call = self.delete_call_count.get().saturating_add(1);
        self.delete_call_count.set(next_call);
        self.delete_calls.borrow_mut().push(path.to_path_buf());

        if self.fail_delete_call.get() == Some(next_call) {
            return Err("simulated trash delete failure".into());
        }

        if let Ok(meta) = fs::symlink_metadata(path) {
            if meta.is_dir() {
                fs::remove_dir_all(path).map_err(|e| format!("fake delete dir failed: {e}"))?;
            } else {
                fs::remove_file(path).map_err(|e| format!("fake delete file failed: {e}"))?;
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

    fn rewrite_original_path(&self, item: &TrashItem, original_path: &Path) -> Result<(), String> {
        self.rewrite_calls
            .borrow_mut()
            .push((PathBuf::from(&item.id), original_path.to_path_buf()));
        Ok(())
    }
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
        Action::Delete { path, backup: _ } => assert_eq!(path, src),
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
