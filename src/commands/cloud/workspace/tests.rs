use super::*;

fn fixture() -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "browsey-workspace-test-{}-{stamp}",
        std::process::id()
    ));
    fs::create_dir(&path).unwrap();
    path
}

#[test]
fn working_copies_survive_cache_removal_and_track_same_size_edits() {
    let fixture = fixture();
    let cached = fixture.join("cache.txt");
    fs::write(&cached, b"original").unwrap();
    let base = fixture.join("workspaces");
    let source = CloudPath::parse("rclone://remote/report.txt").unwrap();
    let copy = create_at(&base, &source, &cached, Some(8), Some("before".into())).unwrap();
    fs::remove_file(&cached).unwrap();
    assert_eq!(fs::read(&copy.local_path).unwrap(), b"original");
    assert!(!load_at(&base, &copy.id).unwrap().dirty);
    fs::write(&copy.local_path, b"modified").unwrap();
    assert!(load_at(&base, &copy.id).unwrap().dirty);
    assert_eq!(
        load_at(&base, &copy.id)
            .unwrap()
            .original_modified
            .as_deref(),
        Some("before")
    );
    fs::remove_dir_all(fixture).unwrap();
}

#[test]
fn each_open_preserves_previous_edits_and_original_filename() {
    let fixture = fixture();
    let cached = fixture.join("cache");
    fs::write(&cached, b"original").unwrap();
    let base = fixture.join("workspaces");
    let source = CloudPath::parse("rclone://remote/report.txt").unwrap();
    let first = create_at(&base, &source, &cached, Some(8), None).unwrap();
    fs::write(&first.local_path, b"my edits").unwrap();
    let second = create_at(&base, &source, &cached, Some(8), None).unwrap();
    assert_ne!(first.id, second.id);
    assert!(second.local_path.ends_with("/report.txt"));
    assert_eq!(fs::read(first.local_path).unwrap(), b"my edits");
    fs::remove_dir_all(fixture).unwrap();
}

#[test]
fn reserved_manifest_names_remain_distinct_from_user_contents() {
    let fixture = fixture();
    let cached = fixture.join("cache");
    fs::write(&cached, b"user data").unwrap();
    let base = fixture.join("workspaces");
    for name in ["manifest.json", "manifest.part", "files"] {
        let source = CloudPath::parse(&format!("rclone://remote/{name}")).unwrap();
        let copy = create_at(&base, &source, &cached, None, None).unwrap();
        assert_eq!(fs::read(&copy.local_path).unwrap(), b"user data");
        assert!(!load_at(&base, &copy.id).unwrap().dirty);
    }
    fs::remove_dir_all(fixture).unwrap();
}

#[test]
fn save_as_new_preserves_extension_and_never_targets_original() {
    for name in ["file.txt", "file.tar.gz", "no-extension", ".hidden"] {
        let source = CloudPath::parse(&format!("rclone://remote/folder/{name}")).unwrap();
        let target = save_as_new_path(&source, "1-2-3").unwrap();
        assert_ne!(source, target);
        assert_eq!(source.parent_dir_path(), target.parent_dir_path());
        if name == "file.txt" {
            assert_eq!(target.leaf_name().unwrap(), "file-edited-1-2-3.txt");
        }
    }
}

#[test]
fn manifest_cannot_redirect_upload_to_an_arbitrary_local_file() {
    let fixture = fixture();
    let cached = fixture.join("cache");
    fs::write(&cached, b"secret").unwrap();
    let base = fixture.join("workspaces");
    let source = CloudPath::parse("rclone://remote/report.txt").unwrap();
    let mut copy = create_at(&base, &source, &cached, None, None).unwrap();
    copy.local_path = cached.to_string_lossy().into_owned();
    save_at(&base, &copy).unwrap();
    assert!(load_at(&base, &copy.id).is_err());
    assert!(load_at(&base, "../cache").is_err());
    fs::remove_dir_all(fixture).unwrap();
}

#[cfg(unix)]
#[test]
fn workspace_files_are_private_and_symlinks_are_rejected() {
    use std::os::unix::fs::{symlink, PermissionsExt};
    let fixture = fixture();
    let cached = fixture.join("cache");
    fs::write(&cached, b"secret").unwrap();
    let base = fixture.join("workspaces");
    let source = CloudPath::parse("rclone://remote/report.txt").unwrap();
    let copy = create_at(&base, &source, &cached, None, None).unwrap();
    assert_eq!(
        fs::metadata(&copy.local_path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert_eq!(
        fs::metadata(base.join(&copy.id))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o700
    );
    fs::remove_file(&copy.local_path).unwrap();
    symlink(&cached, &copy.local_path).unwrap();
    assert!(load_at(&base, &copy.id).is_err());
    fs::remove_dir_all(fixture).unwrap();
}

/// Opt-in acceptance uses the production provider and working-copy/archive
/// engines. It never touches entries outside a newly created owned child.
/// Run only with an explicitly approved EMPTY folder and --ignored --exact.
#[cfg(unix)]
#[test]
#[ignore = "requires an explicitly approved disposable real OneDrive folder"]
fn real_onedrive_working_copy_and_archive_acceptance() -> Result<(), Box<dyn std::error::Error>> {
    use crate::commands::{
        cloud::{
            provider::CloudProvider, providers::rclone::RcloneCloudProvider, rclone_cli::RcloneCli,
        },
        rename::RenameEntryRequest,
    };
    let raw = std::env::var("BROWSEY_TEST_CLOUD_SCOPE")?;
    let parent = CloudPath::parse(&raw)?;
    if parent.is_root() || std::env::var("BROWSEY_TEST_CLOUD_WRITE_APPROVED")?.as_str() != "yes" {
        return Err("Explicit approval and a non-root disposable folder are required".into());
    }
    let provider = RcloneCloudProvider::new(RcloneCli::new("rclone"));
    let remotes = provider.list_remotes()?;
    if !remotes.iter().any(|remote| {
        remote.id == parent.remote()
            && remote.provider == crate::commands::cloud::types::CloudProviderKind::Onedrive
    }) {
        return Err("The approved remote must be a supported OneDrive remote".into());
    }
    if !provider.stat_path(&parent)?.is_some_and(|entry| {
        matches!(
            entry.kind,
            crate::commands::cloud::types::CloudEntryKind::Dir
        )
    }) || !provider.list_dir(&parent)?.is_empty()
    {
        return Err("The approved test folder must exist and be empty; no writes performed".into());
    }
    let local = fixture();
    let name = local.file_name().unwrap().to_str().unwrap();
    let child = parent.child_path(name)?;
    if provider.stat_path(&child)?.is_some() {
        return Err("Refusing to reuse an existing test child".into());
    }
    provider.mkdir(&child, None)?;
    // Printed only to the local test runner for recovery if a later check fails.
    eprintln!(
        "Owned disposable acceptance child: {child}; local data: {}",
        local.display()
    );
    let marker = local.join("owner.txt");
    fs::write(&marker, name)?;
    let marker_cloud = child.child_path("browsey-test-owner.txt")?;
    provider.upload_new_file(&marker, &marker_cloud, None)?;
    let original = child.child_path("report.txt")?;
    let input = local.join("report.txt");
    fs::write(&input, "original")?;
    provider.upload_new_file(&input, &original, None)?;
    let entry = provider
        .stat_path(&original)?
        .ok_or("uploaded file missing")?;
    let downloaded = local.join("downloaded.txt");
    provider.download_file(&original, &downloaded, None)?;
    assert_eq!(fs::read(&downloaded)?, b"original");
    let base = local.join("working-copies");
    let copy = create_at(&base, &original, &downloaded, entry.size, entry.modified)?;
    fs::write(&copy.local_path, "my edits")?;
    fs::remove_file(&downloaded)?;
    assert!(load_at(&base, &copy.id)?.dirty);
    let uploaded = upload_at(&base, &copy.id, &provider, None)?;
    assert!(!uploaded.source_changed);
    let new_path = CloudPath::parse(&uploaded.path)?;
    provider.download_file(&new_path, &downloaded, None)?;
    assert_eq!(fs::read(&downloaded)?, b"my edits");
    // Simulate another editor on the cloud source, with an equal-length change.
    provider.trash_entry(&original, None)?;
    fs::write(&input, "new data")?;
    provider.upload_new_file(&input, &original, None)?;
    let uploaded_again = upload_at(&base, &copy.id, &provider, None)?;
    assert!(uploaded_again.source_changed);
    assert_ne!(uploaded.path, uploaded_again.path);
    assert_eq!(
        provider
            .upload_new_file(Path::new(&copy.local_path), &original, None)
            .unwrap_err()
            .code(),
        CloudCommandErrorCode::DestinationExists
    );
    provider.download_file(&original, &downloaded, None)?;
    assert_eq!(fs::read(&downloaded)?, b"new data");
    eprintln!("PASS: persistent edits, explicit new upload, same-size source change, no clobber");

    let cancel = std::sync::atomic::AtomicBool::new(true);
    let cancelled = child.child_path("cancelled.txt")?;
    assert_eq!(
        provider
            .upload_new_file(&input, &cancelled, Some(&cancel))
            .unwrap_err()
            .code(),
        CloudCommandErrorCode::Cancelled
    );
    assert!(provider.stat_path(&cancelled)?.is_none());
    let renamed = child.child_path("renamed.txt")?;
    let outcome = super::super::batch_rename::rename_batch(
        &provider,
        vec![RenameEntryRequest {
            path: uploaded_again.path,
            new_name: "renamed.txt".into(),
        }],
    )?;
    assert!(outcome.error.is_none());
    assert_eq!(outcome.renamed.len(), 1);
    assert!(provider.stat_path(&renamed)?.is_some());
    eprintln!("PASS: pre-cancellation and preflighted advanced rename");

    let archive = crate::commands::compress::compress_staged(
        None,
        vec![copy.local_path.clone()],
        "protected.zip".into(),
        None,
        None,
        Some("test-only-password"),
        None,
    )
    .map_err(|error| error.message)?;
    let archive_cloud = child.child_path("protected.zip")?;
    provider.upload_new_file(Path::new(&archive), &archive_cloud, None)?;
    let fetched = local.join("fetched.zip");
    provider.download_file(&archive_cloud, &fetched, None)?;
    let password_error = crate::commands::decompress::extract_staged(
        None,
        fetched.to_string_lossy().into_owned(),
        None,
        None,
        None,
    )
    .err()
    .ok_or("password-free extraction unexpectedly succeeded")?;
    assert_eq!(password_error.code, "archive_password_required");
    let extracted = crate::commands::decompress::extract_staged(
        None,
        fetched.to_string_lossy().into_owned(),
        None,
        Some("test-only-password"),
        None,
    )
    .map_err(|error| error.message)?;
    assert_eq!(
        fs::read(Path::new(&extracted.destination).join("report.txt"))?,
        b"my edits"
    );
    assert!(provider.stat_path(&archive_cloud)?.is_some());
    eprintln!("PASS: password-protected ZIP through existing archive engine and real cloud upload/download");

    // Trash, not hard delete. Verify our marker before removing only our child.
    provider.download_file(&marker_cloud, &downloaded, None)?;
    assert_eq!(fs::read_to_string(&downloaded)?, name);
    provider.trash_entry(&child, None)?;
    assert!(provider.stat_path(&child)?.is_none());
    assert!(provider.list_dir(&parent)?.is_empty());
    eprintln!(
        "PASS: owned child moved to provider trash; test parent empty. Web restore not verified."
    );
    fs::remove_dir_all(local)?;
    Ok(())
}
