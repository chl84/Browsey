use super::*;
use std::env;
use std::fs;
use std::io::Write;
use std::path::Path;
use std::sync::OnceLock;
use std::time::{Duration, SystemTime};

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
