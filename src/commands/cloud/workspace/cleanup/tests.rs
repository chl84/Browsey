use super::*;

struct Fixture {
    dir: PathBuf,
    base: PathBuf,
    copy: CloudWorkingCopy,
}
impl Fixture {
    fn new() -> Self {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir =
            std::env::temp_dir().join(format!("browsey-cleanup-{}-{stamp:x}", std::process::id()));
        fs::create_dir(&dir).unwrap();
        let cached = dir.join("cached");
        fs::write(&cached, b"original").unwrap();
        let base = dir.join("workspaces");
        let mut copy = create_at(
            &base,
            &CloudPath::parse("rclone://owned/file.txt").unwrap(),
            &cached,
            Some(8),
            None,
        )
        .unwrap();
        copy.save_status = CloudSaveStatus::Saved;
        save_at(&base, &copy).unwrap();
        Self { dir, base, copy }
    }
    fn refuse(&self) {
        assert!(remove_at(&self.base, &self.copy.id, |_| panic!(
            "unsafe copy reached the trash"
        ))
        .is_err());
        assert!(Path::new(&self.copy.local_path).exists());
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.dir).unwrap();
    }
}

#[test]
fn saved_cleanup_moves_only_the_selected_copy_and_retains_recoverable_bytes() {
    let f = Fixture::new();
    let other = create_at(
        &f.base,
        &CloudPath::parse("rclone://owned/file.txt").unwrap(),
        &f.dir.join("cached"),
        None,
        None,
    )
    .unwrap();
    let trash = f.dir.join("own-trash");
    remove_at(&f.base, &f.copy.id, |path| {
        fs::rename(path, &trash).map_err(io_error)
    })
    .unwrap();
    assert!(!f.base.join(&f.copy.id).exists());
    assert_eq!(fs::read(trash.join("files/file.txt")).unwrap(), b"original");
    assert!(trash.join("manifest.json").exists());
    assert!(Path::new(&other.local_path).exists());
    assert_eq!(fs::read(f.dir.join("cached")).unwrap(), b"original");
}

#[test]
fn cleanup_rechecks_bytes_after_confirmation_instead_of_trusting_saved_status() {
    let f = Fixture::new();
    assert!(overview_at(&f.base).unwrap().copies[0]
        .cleanup_blocked_reason
        .is_none());
    fs::write(&f.copy.local_path, b"new editor save").unwrap();
    f.refuse();
    assert_eq!(fs::read(&f.copy.local_path).unwrap(), b"new editor save");
}

#[test]
fn an_editor_write_to_a_detached_file_is_caught_before_trash_and_restored() {
    let f = Fixture::new();
    let original = f.base.join(&f.copy.id);
    validate_removable(&original, &f.copy).unwrap();
    let detached = f.dir.join("detached");
    fs::rename(&original, &detached).unwrap();
    fs::write(detached.join("files/file.txt"), b"last editor save").unwrap();
    assert!(finish_removal(&original, &detached, &f.copy, |_| panic!(
        "changed detached file reached trash"
    ))
    .is_err());
    assert_eq!(fs::read(&f.copy.local_path).unwrap(), b"last editor save");
    assert!(!detached.exists());
}

#[test]
fn a_pending_journal_is_kept_even_if_the_status_and_current_file_look_saved() {
    let mut f = Fixture::new();
    f.copy.pending_save = Some(PendingSave {
        relative_path: "save-pending.snapshot".into(),
        hash: f.copy.original_hash.clone(),
        version: crate::commands::cloud::providers::rclone::CloudWriteVersion {
            source_path: f.copy.source_path.clone(),
            provider: crate::commands::cloud::types::CloudProviderKind::Gdrive,
            object_id: "owned-id".into(),
            etag: "etag".into(),
            mime_type: "text/plain".into(),
        },
    });
    save_at(&f.base, &f.copy).unwrap();
    f.refuse();
}

#[test]
fn problems_unfinished_journals_and_extra_editor_data_are_retained_even_when_clean() {
    let mut f = Fixture::new();
    for status in [
        CloudSaveStatus::Conflict,
        CloudSaveStatus::Error,
        CloudSaveStatus::Pending,
        CloudSaveStatus::Uploading,
    ] {
        f.copy.save_status = status;
        save_at(&f.base, &f.copy).unwrap();
        f.refuse();
    }
    f.copy.save_status = CloudSaveStatus::Saved;
    // A crash-recovery snapshot without a readable journal still belongs to the user.
    save_at(&f.base, &f.copy).unwrap();
    fs::write(
        f.base.join(&f.copy.id).join("save-retained.snapshot"),
        b"unsent edits",
    )
    .unwrap();
    f.refuse();
    fs::remove_file(f.base.join(&f.copy.id).join("save-retained.snapshot")).unwrap();
    fs::write(
        Path::new(&f.copy.local_path).with_extension("bak"),
        b"editor backup",
    )
    .unwrap();
    f.refuse();
}

#[test]
fn an_active_save_is_skipped_without_waiting_for_network_or_deleting_data() {
    let f = Fixture::new();
    sync::with_copy_lock(&f.copy.id, || {
        f.refuse();
        Ok(())
    })
    .unwrap();
}

#[test]
fn trash_failure_rolls_back_and_never_falls_back_to_permanent_deletion() {
    let f = Fixture::new();
    assert!(remove_at(&f.base, &f.copy.id, |_| Err(refused("trash unavailable"))).is_err());
    assert_eq!(fs::read(&f.copy.local_path).unwrap(), b"original");
    assert_eq!(fs::read_dir(&f.base).unwrap().count(), 1);
}

#[test]
fn unexpected_recreated_editor_path_does_not_get_overwritten_during_rollback() {
    let f = Fixture::new();
    let error = remove_at(&f.base, &f.copy.id, |_| {
        fs::create_dir_all(Path::new(&f.copy.local_path).parent().unwrap()).unwrap();
        fs::write(&f.copy.local_path, b"new unsaved data").unwrap();
        Err(refused("trash unavailable"))
    })
    .unwrap_err();
    assert!(error.message().contains("retained working copy is at"));
    assert_eq!(fs::read(&f.copy.local_path).unwrap(), b"new unsaved data");
    assert_eq!(fs::read_dir(&f.base).unwrap().count(), 2);
}

#[test]
fn storage_includes_staging_and_corrupt_sessions_without_selecting_them() {
    let f = Fixture::new();
    let initial = overview_at(&f.base).unwrap();
    let staging = f.base.join("export-owned");
    fs::create_dir(&staging).unwrap();
    fs::write(staging.join("data"), b"staging bytes").unwrap();
    let corrupt = f.base.join("abc-123");
    fs::create_dir(&corrupt).unwrap();
    fs::write(corrupt.join("manifest.json"), b"broken").unwrap();
    let next = overview_at(&f.base).unwrap();
    assert_eq!(next.storage_bytes, initial.storage_bytes + 13 + 6);
    assert_eq!(next.retained_entries, 2);
    assert_eq!(next.copies.len(), 1);
    assert!(!next.incomplete);
    assert!(remove_at(&f.base, "../cached", |_| panic!("invalid id reached trash")).is_err());
    assert!(remove_at(&f.base, "abc-123", |_| panic!(
        "corrupt session reached trash"
    ))
    .is_err());
}

#[cfg(unix)]
#[test]
fn symlinks_and_hard_links_are_kept_and_storage_does_not_follow_external_links() {
    use std::os::unix::fs::symlink;
    let f = Fixture::new();
    let outside = f.dir.join("outside");
    fs::write(&outside, b"external user data").unwrap();
    let initial = overview_at(&f.base).unwrap().storage_bytes;
    symlink(&outside, f.base.join("external-link")).unwrap();
    assert_eq!(overview_at(&f.base).unwrap().storage_bytes, initial);
    let alias = f.dir.join("alias");
    fs::hard_link(&f.copy.local_path, &alias).unwrap();
    f.refuse();
    fs::remove_file(alias).unwrap();
    fs::remove_file(&f.copy.local_path).unwrap();
    symlink(&outside, &f.copy.local_path).unwrap();
    f.refuse();
    assert_eq!(fs::read(outside).unwrap(), b"external user data");
}
