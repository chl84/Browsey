use super::{set_ownership_batch, set_permissions_batch, AccessUpdate};
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
        None,
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
    let err = match set_ownership_batch(vec![path.to_string_lossy().to_string()], None, None, None)
    {
        Ok(_) => panic!("set_ownership_batch should fail without owner/group"),
        Err(err) => err,
    };
    assert!(err.contains("No ownership changes were provided"));
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
        None,
    ) {
        Ok(_) => panic!("set_ownership_batch should fail for unknown user"),
        Err(err) => err,
    };
    assert!(err.contains("User not found"));

    let err = match set_ownership_batch(
        vec![path.to_string_lossy().to_string()],
        None,
        Some("browsey-group-does-not-exist".into()),
        None,
    ) {
        Ok(_) => panic!("set_ownership_batch should fail for unknown group"),
        Err(err) => err,
    };
    assert!(err.contains("Group not found"));
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
        None,
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
        None,
    ) {
        Ok(_) => panic!("set_permissions_batch should fail for relative paths"),
        Err(err) => err,
    };
    assert!(err.contains("Path must be absolute"));
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
        None,
    ) {
        Ok(_) => panic!("set_permissions_batch should reject symlink path components"),
        Err(err) => err,
    };
    assert!(err.contains("Symlinks are not allowed in path"));

    let _ = fs::remove_file(&file_path);
    let _ = fs::remove_file(&link_dir);
    let _ = fs::remove_dir(&real_dir);
    let _ = fs::remove_dir(&base);
}
