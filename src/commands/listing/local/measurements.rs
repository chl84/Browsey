use super::*;
use crate::performance_fixture::{report, Fixture};
use std::time::Instant;

#[test]
fn collector_preserves_metadata_sort_and_starred_state() {
    let fixture = Fixture::new("listing-contract");
    let root = fixture.entries(3, false);
    let star = root.join("item-000000-needle.txt");
    let (listing, pending) =
        collect_directory(&root, &HashSet::from([normalize_key_for_db(&star)]), None).unwrap();
    assert!(pending.is_empty());
    assert!(listing.pending_metadata_paths.is_empty());
    assert_eq!(listing.entries.len(), 3);
    assert!(listing.entries[0].starred);
    assert_eq!(listing.entries[0].size, Some(8));
    assert!(listing
        .entries
        .iter()
        .all(|entry| !entry.read_denied && !entry.network));
}

#[test]
fn unknown_camera_file_type_is_not_a_temporary_folder() {
    let entry = stub_entry(
        Path::new("/run/user/1000/gvfs/mtp:host=test/DCIM/Camera/photo.jpg"),
        None,
        false,
    );
    assert_eq!(entry.kind, "file");
    assert!(entry.network);
    assert!(entry.size.is_none());
    assert!(entry.modified.is_none());
}

#[test]
fn metadata_scans_coalesce_per_directory_and_release_after_completion() {
    let fixture = Fixture::new("metadata-refresh-lock");
    let first = fixture.entries(1, false);
    let second = first.join("another-directory");
    let guard = MetadataRefreshGuard::acquire(&first).unwrap();
    assert!(MetadataRefreshGuard::acquire(&first).is_none());
    let independent = MetadataRefreshGuard::acquire(&second).unwrap();
    drop(guard);
    assert!(MetadataRefreshGuard::acquire(&first).is_some());
    assert!(MetadataRefreshGuard::acquire(&second).is_none());
    drop(independent);
    assert!(MetadataRefreshGuard::acquire(&second).is_some());
}

#[test]
fn gvfs_listing_reports_pending_metadata_paths() {
    let fixture = Fixture::new("gvfs-listing-contract");
    let root = fixture.entries(2, false).join("gvfs");
    fs::create_dir(&root).unwrap();
    fs::write(root.join("photo.jpg"), b"test-only").unwrap();
    let (listing, pending) = collect_directory(&root, &HashSet::new(), None).unwrap();
    assert_eq!(listing.entries.len(), 1);
    assert_eq!(pending.len(), 1);
    assert_eq!(
        listing.pending_metadata_paths,
        vec![display_path(&root.join("photo.jpg"))]
    );
    let payload = serde_json::to_value(&listing).unwrap();
    assert_eq!(
        payload["pendingMetadataPaths"][0],
        display_path(&root.join("photo.jpg"))
    );
}

#[test]
#[ignore = "Creates 10k/100k disposable entries; run alone on the selected filesystem"]
fn large_directory_workloads() {
    let fixture = Fixture::new("listing");
    for count in [10_000, 100_000] {
        let root = fixture.entries(count, false);
        let mut samples = Vec::new();
        for _ in 0..6 {
            let start = Instant::now();
            let (listing, pending) = collect_directory(&root, &HashSet::new(), None).unwrap();
            let elapsed = start.elapsed().as_secs_f64() * 1000.0;
            assert_eq!(listing.entries.len(), count);
            assert!(pending.is_empty());
            samples.push(elapsed);
        }
        report(
            "local-listing-core",
            &samples[1..],
            serde_json::json!({
                "entries": count, "firstInvocationMs": samples[0], "logicalFileBytes": count * 8,
                "cacheState": "generated fixtures; OS caches not reset", "includesDatabaseOrIpc": false,
            }),
        );
    }
}

#[test]
fn directory_rejects_unsupported_filename_encoding_without_lossy_aliases() {
    use std::os::unix::ffi::OsStrExt;
    let fixture = Fixture::new("invalid-filename");
    let root = fixture.entries(1, false);
    let raw = root.join(std::ffi::OsStr::from_bytes(b"invalid-\xff"));
    fs::write(&raw, b"invalid-name bytes").unwrap();
    fs::write(root.join("invalid-\u{fffd}"), b"different valid filename").unwrap();
    let error = collect_directory(&root, &HashSet::new(), None)
        .err()
        .expect("No lossy listing");
    assert!(error.to_string().contains("filename is not valid UTF-8"));
    assert_eq!(fs::read(raw).unwrap(), b"invalid-name bytes");
    assert_eq!(
        fs::read(root.join("invalid-\u{fffd}")).unwrap(),
        b"different valid filename"
    );
}
