use super::*;
use crate::performance_fixture::{report, Fixture};
use std::time::{Duration, Instant};

fn run(root: std::path::PathBuf, mut cancelled: impl FnMut() -> bool) -> (usize, bool) {
    run_query(root, "needle", &mut cancelled)
}

fn run_query(
    root: std::path::PathBuf,
    text: &str,
    mut cancelled: impl FnMut() -> bool,
) -> (usize, bool) {
    let query = parse_query(text).unwrap();
    let needle = simple_name_contains_needle_lc(&query);
    let (mut count, mut done) = (0, false);
    scan_search(
        root,
        &query,
        needle.as_deref(),
        &HashSet::new(),
        &mut cancelled,
        |entries, finished, code, error, _| {
            assert!(code.is_none() && error.is_none());
            count += entries.len();
            done |= finished;
        },
    );
    (count, done)
}

#[test]
fn scanner_keeps_name_prefilter_and_cancel_contract() {
    let fixture = Fixture::new("search-contract");
    let root = fixture.entries(10, true);
    assert_eq!(run(root.clone(), || false), (1, true));
    assert_eq!(
        run_query(root.clone(), "name:needle hidden:false", || false),
        (1, true)
    );
    assert_eq!(
        run_query(root.clone(), "hidden:false", || false),
        (20, true)
    );
    assert_eq!(run(root, || true), (0, false));
}

#[test]
#[ignore = "Creates 10k/100k disposable recursive entries plus a controlled delay"]
fn recursive_search_workloads() {
    let fixture = Fixture::new("search");
    for count in [10_000, 100_000] {
        let root = fixture.entries(count, true);
        let mut samples = Vec::new();
        for _ in 0..6 {
            let start = Instant::now();
            assert_eq!(run(root.clone(), || false), (count / 1000, true));
            samples.push(start.elapsed().as_secs_f64() * 1000.0);
        }
        report(
            "recursive-name-search-core",
            &samples[1..],
            serde_json::json!({
                "files": count, "directories": 100, "matches": count / 1000,
                "firstInvocationMs": samples[0], "includesDatabaseOrIpc": false,
            }),
        );
        let mut filtered = Vec::new();
        for _ in 0..5 {
            let start = Instant::now();
            assert_eq!(
                run_query(root.clone(), "hidden:false", || false),
                (count + 100, true)
            );
            filtered.push(start.elapsed().as_secs_f64() * 1000.0);
        }
        report(
            "recursive-metadata-search-core",
            &filtered,
            serde_json::json!({
                "files": count, "directories": 100, "matches": count + 100,
                "query": "hidden:false", "includesDatabaseOrIpc": false,
            }),
        );
        let requested = std::cell::Cell::new(None);
        let checks = std::cell::Cell::new(0);
        let start = Instant::now();
        let result = run(root, || {
            checks.set(checks.get() + 1);
            if checks.get() == 8 {
                requested.set(Some(Instant::now()));
                // Simulate one blocked metadata boundary, not global slow storage.
                std::thread::sleep(Duration::from_millis(20));
                true
            } else {
                false
            }
        });
        assert!(!result.1);
        let latency = requested.get().unwrap().elapsed().as_secs_f64() * 1000.0;
        assert_eq!(
            checks.get(),
            8,
            "no further traversal after cancellation is observed"
        );
        report(
            "search-controlled-slow-cancellation",
            &[latency],
            serde_json::json!({
                "injectedBlockingMs": 20, "cancelChecks": checks.get(),
                "totalMs": start.elapsed().as_secs_f64() * 1000.0,
            }),
        );
    }
}
