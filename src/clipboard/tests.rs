use super::*;
use std::env;
use std::fs;
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::symlink;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, SystemTime};
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
fn copy_file_best_effort_cancelled_before_transfer_removes_destination() {
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
    assert!(!dest.exists(), "destination should be cleaned up on cancel");

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
