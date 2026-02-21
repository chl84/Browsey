use super::{
    get_permissions_batch_impl, get_permissions_impl, refresh_permissions_after_apply,
    set_ownership_batch, set_permissions_batch, AccessUpdate, AggregatedAccessBit,
};
use crate::undo::{Action, UndoState};
use std::fs;
use std::os::unix::fs::symlink;
use std::os::unix::fs::{MetadataExt, PermissionsExt};

fn temp_file(prefix: &str) -> std::path::PathBuf {
    let unique = format!(
        "{}-{}-{}",
        prefix,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    std::env::temp_dir().join(unique)
}

#[test]
fn read_only_toggle_does_not_grant_world_write() {
    let path = temp_file("perm-ro");
    fs::write(&path, b"test").unwrap();
    fs::set_permissions(&path, PermissionsExt::from_mode(0o664)).unwrap();

    set_permissions_batch(
        vec![path.to_string_lossy().to_string()],
        Some(true),
        None,
        None,
        None,
        None,
    )
    .unwrap();
    let after_ro = fs::metadata(&path).unwrap().permissions().mode();
    assert_eq!(after_ro & 0o222, 0o020); // only owner write cleared

    set_permissions_batch(
        vec![path.to_string_lossy().to_string()],
        Some(false),
        None,
        None,
        None,
        None,
    )
    .unwrap();
    let after_restore = fs::metadata(&path).unwrap().permissions().mode();
    assert_eq!(after_restore & 0o222, 0o220); // original writes restored

    let _ = fs::remove_file(&path);
}

#[test]
fn executable_toggle_sets_owner_only() {
    let path = temp_file("perm-exec");
    fs::write(&path, b"test").unwrap();
    fs::set_permissions(&path, PermissionsExt::from_mode(0o654)).unwrap(); // owner no exec, group exec

    set_permissions_batch(
        vec![path.to_string_lossy().to_string()],
        None,
        Some(true),
        None,
        None,
        None,
    )
    .unwrap();
    let after_exec = fs::metadata(&path).unwrap().permissions().mode();
    assert_eq!(after_exec & 0o111, 0o110); // owner + existing group preserved

    set_permissions_batch(
        vec![path.to_string_lossy().to_string()],
        None,
        Some(false),
        None,
        None,
        None,
    )
    .unwrap();
    let after_clear = fs::metadata(&path).unwrap().permissions().mode();
    assert_eq!(after_clear & 0o111, 0o010); // only owner exec cleared; group exec stays

    let _ = fs::remove_file(&path);
}

#[test]
fn owner_group_other_bits_update() {
    let path = temp_file("perm-access");
    fs::write(&path, b"test").unwrap();
    fs::set_permissions(&path, PermissionsExt::from_mode(0o750)).unwrap();

    // Enable other read + owner exec without reintroducing world write.
    set_permissions_batch(
        vec![path.to_string_lossy().to_string()],
        None,
        None,
        None,
        None,
        Some(AccessUpdate {
            read: Some(true),
            write: Some(false),
            exec: Some(false),
        }),
    )
    .unwrap();
    let mode = fs::metadata(&path).unwrap().permissions().mode();
    assert_eq!(mode & 0o004, 0o004);
    assert_eq!(mode & 0o002, 0);
    assert_eq!(mode & 0o001, 0);

    set_permissions_batch(
        vec![path.to_string_lossy().to_string()],
        None,
        None,
        Some(AccessUpdate {
            read: None,
            write: None,
            exec: Some(true),
        }),
        None,
        None,
    )
    .unwrap();
    let mode = fs::metadata(&path).unwrap().permissions().mode();
    assert_eq!(mode & 0o100, 0o100);
}

#[test]
fn set_ownership_requires_owner_or_group() {
    let path = temp_file("owner-empty");
    fs::write(&path, b"test").unwrap();
    let err = match set_ownership_batch(vec![path.to_string_lossy().to_string()], None, None) {
        Ok(_) => panic!("set_ownership_batch should fail without owner/group"),
        Err(err) => err,
    };
    assert!(err
        .to_string()
        .contains("No ownership changes were provided"));
    let _ = fs::remove_file(&path);
}

#[test]
fn set_ownership_rejects_unknown_principals() {
    let path = temp_file("owner-unknown");
    fs::write(&path, b"test").unwrap();
    let err = match set_ownership_batch(
        vec![path.to_string_lossy().to_string()],
        Some("browsey-user-does-not-exist".into()),
        None,
    ) {
        Ok(_) => panic!("set_ownership_batch should fail for unknown user"),
        Err(err) => err,
    };
    assert!(err.to_string().contains("User not found"));

    let err = match set_ownership_batch(
        vec![path.to_string_lossy().to_string()],
        None,
        Some("browsey-group-does-not-exist".into()),
    ) {
        Ok(_) => panic!("set_ownership_batch should fail for unknown group"),
        Err(err) => err,
    };
    assert!(err.to_string().contains("Group not found"));
    let _ = fs::remove_file(&path);
}

#[test]
fn set_ownership_noop_with_current_ids_succeeds() {
    let path = temp_file("owner-noop");
    fs::write(&path, b"test").unwrap();
    let meta = fs::symlink_metadata(&path).unwrap();
    let uid = meta.uid();
    let gid = meta.gid();

    let info = set_ownership_batch(
        vec![path.to_string_lossy().to_string()],
        Some(uid.to_string()),
        Some(gid.to_string()),
    )
    .unwrap();
    assert!(info.ownership_supported);
    let _ = fs::remove_file(&path);
}

#[test]
fn set_permissions_rejects_relative_path() {
    let err = match set_permissions_batch(
        vec!["relative-path.txt".into()],
        Some(true),
        None,
        None,
        None,
        None,
    ) {
        Ok(_) => panic!("set_permissions_batch should fail for relative paths"),
        Err(err) => err,
    };
    assert!(err.to_string().contains("Path must be absolute"));
}

#[test]
fn set_permissions_rejects_symlink_components() {
    let base = std::env::temp_dir().join(format!(
        "perm-symlink-comp-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let real_dir = base.join("real");
    let link_dir = base.join("link");
    let file_path = real_dir.join("target.txt");
    let via_link_path = link_dir.join("target.txt");

    fs::create_dir_all(&real_dir).unwrap();
    fs::write(&file_path, b"test").unwrap();
    symlink(&real_dir, &link_dir).unwrap();

    let err = match set_permissions_batch(
        vec![via_link_path.to_string_lossy().to_string()],
        Some(true),
        None,
        None,
        None,
        None,
    ) {
        Ok(_) => panic!("set_permissions_batch should reject symlink path components"),
        Err(err) => err,
    };
    assert!(err.to_string().contains("Symlinks are not allowed in path"));

    let _ = fs::remove_file(&file_path);
    let _ = fs::remove_file(&link_dir);
    let _ = fs::remove_dir(&real_dir);
    let _ = fs::remove_dir(&base);
}

#[test]
fn get_permissions_rejects_symlink_components() {
    let base = std::env::temp_dir().join(format!(
        "perm-get-symlink-comp-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let real_dir = base.join("real");
    let link_dir = base.join("link");
    let file_path = real_dir.join("target.txt");
    let via_link_path = link_dir.join("target.txt");

    fs::create_dir_all(&real_dir).unwrap();
    fs::write(&file_path, b"test").unwrap();
    symlink(&real_dir, &link_dir).unwrap();

    let err = match get_permissions_impl(via_link_path.to_string_lossy().to_string()) {
        Ok(_) => panic!("get_permissions should reject symlink path components"),
        Err(err) => err,
    };
    assert!(err.to_string().contains("Symlinks are not allowed in path"));

    let _ = fs::remove_file(&file_path);
    let _ = fs::remove_file(&link_dir);
    let _ = fs::remove_dir(&real_dir);
    let _ = fs::remove_dir(&base);
}

#[test]
fn set_permissions_rolls_back_when_later_target_fails_validation() {
    let base = std::env::temp_dir().join(format!(
        "perm-rollback-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let real_dir = base.join("real");
    let link_dir = base.join("link");
    let first_path = base.join("first.txt");
    let file_path = real_dir.join("target.txt");
    let via_link_path = link_dir.join("target.txt");

    fs::create_dir_all(&real_dir).unwrap();
    fs::write(&first_path, b"first").unwrap();
    fs::set_permissions(&first_path, PermissionsExt::from_mode(0o664)).unwrap();
    fs::write(&file_path, b"test").unwrap();
    symlink(&real_dir, &link_dir).unwrap();

    let before_mode = fs::metadata(&first_path).unwrap().permissions().mode() & 0o777;
    let err = match set_permissions_batch(
        vec![
            first_path.to_string_lossy().to_string(),
            via_link_path.to_string_lossy().to_string(),
        ],
        Some(true),
        None,
        None,
        None,
        None,
    ) {
        Ok(_) => panic!("set_permissions_batch should fail when a later target is invalid"),
        Err(err) => err,
    };
    assert!(err.to_string().contains("Symlinks are not allowed in path"));
    let after_mode = fs::metadata(&first_path).unwrap().permissions().mode() & 0o777;
    assert_eq!(after_mode, before_mode);

    let _ = fs::remove_file(&first_path);
    let _ = fs::remove_file(&file_path);
    let _ = fs::remove_file(&link_dir);
    let _ = fs::remove_dir(&real_dir);
    let _ = fs::remove_dir(&base);
}

#[test]
fn set_permissions_noop_returns_actual_state() {
    let path = temp_file("perm-noop-info");
    fs::write(&path, b"test").unwrap();
    fs::set_permissions(&path, PermissionsExt::from_mode(0o640)).unwrap();
    let before_mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;

    let info = set_permissions_batch(
        vec![path.to_string_lossy().to_string()],
        Some(false), // owner write already set, so this is a no-op
        None,
        None,
        None,
        None,
    )
    .unwrap();

    assert!(info.access_supported);
    assert!(info.owner.is_some());
    assert!(info.group.is_some());
    assert!(info.other.is_some());
    assert!(!info.read_only);
    let after_mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
    assert_eq!(after_mode, before_mode);

    let _ = fs::remove_file(&path);
}

#[test]
fn set_ownership_rejects_relative_path() {
    let path = temp_file("owner-rel");
    fs::write(&path, b"test").unwrap();
    let uid = fs::symlink_metadata(&path).unwrap().uid();

    let err = match set_ownership_batch(
        vec!["relative-owner-path".into()],
        Some(uid.to_string()),
        None,
    ) {
        Ok(_) => panic!("set_ownership_batch should fail for relative paths"),
        Err(err) => err,
    };
    assert!(err.to_string().contains("Path must be absolute"));

    let _ = fs::remove_file(&path);
}

#[test]
fn refresh_permissions_after_apply_returns_fallback_when_changed() {
    let missing = temp_file("perm-refresh-missing");
    let info =
        refresh_permissions_after_apply(missing.to_string_lossy().to_string(), true).unwrap();
    assert!(info.access_supported);
}

#[test]
fn refresh_permissions_after_apply_errors_when_no_change() {
    let missing = temp_file("perm-refresh-nochange");
    let err = match refresh_permissions_after_apply(missing.to_string_lossy().to_string(), false) {
        Ok(_) => panic!("refresh should fail when nothing changed and path is invalid"),
        Err(err) => err,
    };
    assert!(err
        .to_string()
        .contains("Path does not exist or unreadable"));
}

#[test]
fn set_permissions_does_not_record_undo_history() {
    let src = temp_file("perm-undo-src");
    let dst = temp_file("perm-undo-dst");
    fs::write(&src, b"undo-test").unwrap();
    let _ = fs::remove_file(&dst);

    let undo = UndoState::default();
    undo.record(Action::Rename {
        from: src.clone(),
        to: dst.clone(),
    })
    .unwrap();
    assert!(!src.exists());
    assert!(dst.exists());

    set_permissions_batch(
        vec![dst.to_string_lossy().to_string()],
        Some(true),
        None,
        None,
        None,
        None,
    )
    .unwrap();

    undo.undo().unwrap();
    assert!(src.exists());
    assert!(!dst.exists());

    let err = undo.undo().unwrap_err();
    assert!(err.to_string().contains("Nothing to undo"));

    let _ = fs::remove_file(&src);
    let _ = fs::remove_file(&dst);
}

#[test]
fn set_ownership_does_not_record_undo_history() {
    let src = temp_file("owner-undo-src");
    let dst = temp_file("owner-undo-dst");
    fs::write(&src, b"undo-test").unwrap();
    let _ = fs::remove_file(&dst);

    let undo = UndoState::default();
    undo.record(Action::Rename {
        from: src.clone(),
        to: dst.clone(),
    })
    .unwrap();
    assert!(!src.exists());
    assert!(dst.exists());

    let meta = fs::symlink_metadata(&dst).unwrap();
    set_ownership_batch(
        vec![dst.to_string_lossy().to_string()],
        Some(meta.uid().to_string()),
        Some(meta.gid().to_string()),
    )
    .unwrap();

    undo.undo().unwrap();
    assert!(src.exists());
    assert!(!dst.exists());

    let err = undo.undo().unwrap_err();
    assert!(err.to_string().contains("Nothing to undo"));

    let _ = fs::remove_file(&src);
    let _ = fs::remove_file(&dst);
}

#[test]
fn get_permissions_batch_aggregates_mixed_values() {
    let path_a = temp_file("perm-batch-a");
    let path_b = temp_file("perm-batch-b");
    fs::write(&path_a, b"a").unwrap();
    fs::write(&path_b, b"b").unwrap();
    fs::set_permissions(&path_a, PermissionsExt::from_mode(0o644)).unwrap();
    fs::set_permissions(&path_b, PermissionsExt::from_mode(0o600)).unwrap();

    let batch = get_permissions_batch_impl(vec![
        path_a.to_string_lossy().to_string(),
        path_b.to_string_lossy().to_string(),
    ])
    .unwrap();

    assert_eq!(batch.per_item.len(), 2);
    assert_eq!(batch.failures, 0);
    assert_eq!(batch.unexpected_failures, 0);
    assert!(batch.aggregate.access_supported);
    assert!(batch.aggregate.executable_supported);
    assert!(batch.aggregate.ownership_supported);
    assert_eq!(
        batch.aggregate.read_only,
        Some(AggregatedAccessBit::Bool(false))
    );
    assert_eq!(
        batch.aggregate.executable,
        Some(AggregatedAccessBit::Bool(false))
    );
    assert!(batch.aggregate.owner_name.is_some());
    assert!(batch.aggregate.group_name.is_some());

    let owner = batch.aggregate.owner.expect("owner aggregate");
    assert_eq!(owner.read, AggregatedAccessBit::Bool(true));
    assert_eq!(owner.write, AggregatedAccessBit::Bool(true));
    assert_eq!(owner.exec, AggregatedAccessBit::Bool(false));

    let group = batch.aggregate.group.expect("group aggregate");
    assert_eq!(group.read, AggregatedAccessBit::Mixed("mixed".into()));
    assert_eq!(group.write, AggregatedAccessBit::Bool(false));
    assert_eq!(group.exec, AggregatedAccessBit::Bool(false));

    let other = batch.aggregate.other.expect("other aggregate");
    assert_eq!(other.read, AggregatedAccessBit::Mixed("mixed".into()));
    assert_eq!(other.write, AggregatedAccessBit::Bool(false));
    assert_eq!(other.exec, AggregatedAccessBit::Bool(false));

    let _ = fs::remove_file(&path_a);
    let _ = fs::remove_file(&path_b);
}

#[test]
fn get_permissions_batch_marks_virtual_uris_as_unsupported_without_failure() {
    let batch = get_permissions_batch_impl(vec!["sftp://user@example.local/home".into()]).unwrap();
    assert_eq!(batch.per_item.len(), 1);
    assert_eq!(batch.failures, 0);
    assert_eq!(batch.unexpected_failures, 0);
    assert!(batch.per_item[0].ok);
    assert!(!batch.per_item[0].permissions.access_supported);
    assert!(!batch.aggregate.access_supported);
    assert!(!batch.aggregate.executable_supported);
    assert!(!batch.aggregate.ownership_supported);
    assert_eq!(batch.aggregate.read_only, None);
    assert_eq!(batch.aggregate.executable, None);
}

#[test]
fn get_permissions_batch_uses_structured_error_codes_for_expected_failures() {
    let missing = temp_file("perm-batch-missing");
    let _ = fs::remove_file(&missing);

    let batch = get_permissions_batch_impl(vec![missing.to_string_lossy().to_string()]).unwrap();
    assert_eq!(batch.per_item.len(), 1);
    assert_eq!(batch.failures, 1);
    assert_eq!(batch.unexpected_failures, 0);
    assert!(!batch.per_item[0].ok);

    let error = batch.per_item[0]
        .error
        .as_ref()
        .expect("missing path should include structured error");
    assert_eq!(error.code, "not_found");
    assert!(error
        .message
        .to_ascii_lowercase()
        .contains("does not exist"));
}

#[test]
fn get_permissions_batch_counts_unexpected_failures_by_error_code() {
    let batch = get_permissions_batch_impl(vec!["relative-path.txt".into()]).unwrap();
    assert_eq!(batch.per_item.len(), 1);
    assert_eq!(batch.failures, 1);
    assert_eq!(batch.unexpected_failures, 1);
    assert!(!batch.per_item[0].ok);

    let error = batch.per_item[0]
        .error
        .as_ref()
        .expect("relative path should include structured error");
    assert_eq!(error.code, "path_not_absolute");
    assert!(error.message.contains("Path must be absolute"));
}
