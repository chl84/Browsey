use super::*;
use std::env;
use std::fs;
use std::io::Write;
#[cfg(unix)]
use std::os::unix::fs::symlink;
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use std::sync::atomic::{AtomicBool, Ordering};
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
    assert_eq!(err.code(), ClipboardErrorCode::IoError);
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
    assert_eq!(err.code(), ClipboardErrorCode::IoError);
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
    assert_eq!(err.code(), ClipboardErrorCode::IoError);
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
    let _guard = clipboard_test_lock().lock().unwrap();
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
    let _guard = clipboard_test_lock().lock().unwrap();
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
    let _guard = clipboard_test_lock().lock().unwrap();
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
fn copy_file_best_effort_cancelled_before_transfer_removes_destination() {
    let base = uniq_path("copy-cancelled-file");
    fs::create_dir_all(&base).unwrap();
    let src = base.join("src.bin");
    let dest = base.join("dest.bin");
    write_file(&src, &[7u8; 32 * 1024]);
    let cancel = AtomicBool::new(true);

    let err = copy_file_best_effort(&src, &dest, None, None, Some(&cancel), Some(32 * 1024))
        .unwrap_err();

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
