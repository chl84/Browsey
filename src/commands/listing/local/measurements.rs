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
    assert_eq!(listing.entries.len(), 3);
    assert!(listing.entries[0].starred);
    assert_eq!(listing.entries[0].size, Some(8));
    assert!(listing
        .entries
        .iter()
        .all(|entry| !entry.read_denied && !entry.network));
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
