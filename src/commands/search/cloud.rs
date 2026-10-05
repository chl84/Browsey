use super::{
    error::{SearchError, SearchErrorCode, SearchResult},
    query::{matches_query, Expr},
    SearchProgress,
};
use crate::{
    commands::{
        cloud::{
            self, path::CloudPath, providers::rclone::RcloneReadOptions, types::CloudEntryKind,
        },
        listing::{fs_entry_from_cloud_entry, ListingFacetBuilder},
    },
    entry::{normalize_key_for_db, FsEntry},
};
use std::{collections::HashSet, path::Path, sync::atomic::AtomicBool, time::Duration};

pub(super) fn run_cloud_search(
    root: CloudPath,
    query: &Expr,
    starred: &HashSet<String>,
    cancel: &AtomicBool,
    mut cancelled: impl FnMut() -> bool,
    send: impl FnMut(SearchProgress),
) -> SearchResult<()> {
    if cancelled() {
        return Ok(());
    }
    cloud::ensure_cloud_enabled().map_err(SearchError::from)?;
    let provider = cloud::configured_rclone_provider()
        .map_err(cloud::CloudCommandError::from)
        .map_err(SearchError::from)?;
    let options = RcloneReadOptions {
        cancel: Some(cancel),
        rc_timeout: Some(Duration::from_secs(10)),
        cli_timeout: Some(Duration::from_secs(20)),
        ..RcloneReadOptions::default()
    };
    if !root.is_root() {
        let entry = cloud::with_cloud_remote_permits(vec![root.remote().to_string()], || {
            provider.stat_path_with_read_options(&root, options)
        })
        .map_err(SearchError::from)?;
        match entry {
            None => {
                return Err(SearchError::new(
                    SearchErrorCode::NotFound,
                    "Search start directory no longer exists",
                ))
            }
            Some(entry) if entry.kind != CloudEntryKind::Dir => {
                return Err(SearchError::new(
                    SearchErrorCode::InvalidPath,
                    "Search start path is not a directory",
                ))
            }
            _ => {}
        }
    }
    let capabilities = cloud::cloud_provider_kind_for_remote(root.remote())
        .map(cloud::types::CloudCapabilities::v1_for_provider)
        .unwrap_or_else(cloud::types::CloudCapabilities::v1_core_rw);
    scan_cloud_search(
        root,
        query,
        starred,
        |directory| {
            // Read live provider metadata for every visited directory. Search must
            // not finish from the interactive listing cache or download file data.
            let entries =
                cloud::with_cloud_remote_permits(vec![directory.remote().to_string()], || {
                    provider.list_dir_with_read_options(directory, options)
                })
                .map_err(SearchError::from)?;
            Ok(entries
                .into_iter()
                .map(|mut entry| {
                    entry.capabilities = capabilities.clone();
                    fs_entry_from_cloud_entry(entry)
                })
                .collect())
        },
        cancelled,
        send,
    )
}

fn scan_cloud_search(
    root: CloudPath,
    query: &Expr,
    starred: &HashSet<String>,
    mut list: impl FnMut(&CloudPath) -> SearchResult<Vec<FsEntry>>,
    mut cancelled: impl FnMut() -> bool,
    mut send: impl FnMut(SearchProgress),
) -> SearchResult<()> {
    let mut stack = vec![root];
    let mut seen = HashSet::new();
    let mut facets = ListingFacetBuilder::default();
    while let Some(directory) = stack.pop() {
        if cancelled() {
            return Ok(());
        }
        let entries = list(&directory)?;
        if cancelled() {
            return Ok(());
        }
        // Validate the complete provider response before emitting matches or
        // scheduling descendants. A malformed path cannot escape this root.
        for entry in &entries {
            let expected = directory.child_path(&entry.name).map_err(|error| {
                SearchError::new(SearchErrorCode::InvalidPath, error.to_string())
            })?;
            if entry.path != expected.to_string() {
                return Err(SearchError::new(
                    SearchErrorCode::InvalidPath,
                    "Cloud search received an entry outside its directory",
                ));
            }
        }
        let mut batch = Vec::new();
        for mut entry in entries {
            if cancelled() {
                return Ok(());
            }
            if !seen.insert(entry.path.clone()) {
                continue;
            }
            if entry.kind == "dir" {
                stack.push(directory.child_path(&entry.name).map_err(|error| {
                    SearchError::new(SearchErrorCode::InvalidPath, error.to_string())
                })?);
            }
            entry.starred = starred.contains(&normalize_key_for_db(Path::new(&entry.path)));
            if matches_query(&entry, query) {
                facets.add(&entry);
                batch.push(entry);
                if batch.len() >= 256 {
                    send(SearchProgress {
                        entries: std::mem::take(&mut batch),
                        done: false,
                        error_code: None,
                        error: None,
                        facets: None,
                    });
                }
            }
        }
        if !batch.is_empty() {
            send(SearchProgress {
                entries: batch,
                done: false,
                error_code: None,
                error: None,
                facets: None,
            });
        }
    }
    if !cancelled() {
        send(SearchProgress {
            entries: Vec::new(),
            done: true,
            error_code: None,
            error: None,
            facets: Some(facets.finish()),
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::{
        cloud::types::{CloudCapabilities, CloudEntry},
        search::query::parse_query,
    };
    use std::cell::Cell;

    fn entry(parent: &CloudPath, name: &str, directory: bool) -> FsEntry {
        fs_entry_from_cloud_entry(CloudEntry {
            name: name.to_string(),
            path: parent.child_path(name).unwrap().to_string(),
            kind: if directory {
                CloudEntryKind::Dir
            } else {
                CloudEntryKind::File
            },
            size: Some(32),
            modified: Some("2026-10-05 12:00".to_string()),
            capabilities: CloudCapabilities::v1_core_rw(),
        })
    }

    #[test]
    fn recursive_search_visits_nonmatching_directories_and_retains_cloud_metadata_and_facets() {
        let root = CloudPath::parse("rclone://Generated/owned").unwrap();
        let nested = root.child_path("nested").unwrap();
        let inner = nested.child_path("Alpha-inner.TXT").unwrap();
        let starred = HashSet::from([normalize_key_for_db(Path::new(&inner.to_string()))]);
        let mut visited = Vec::new();
        let mut messages = Vec::new();
        scan_cloud_search(
            root.clone(),
            &parse_query(".TXT").unwrap(),
            &starred,
            |directory| {
                visited.push(directory.clone());
                Ok(if directory == &root {
                    vec![
                        entry(&root, "Alpha.TXT", false),
                        entry(&root, "nested", true),
                    ]
                } else {
                    assert_eq!(directory, &nested);
                    vec![
                        entry(&nested, "Alpha-inner.TXT", false),
                        entry(&nested, "other.md", false),
                    ]
                })
            },
            || false,
            |message| messages.push(message),
        )
        .unwrap();
        assert_eq!(visited, vec![root, nested]);
        let matches: Vec<_> = messages
            .iter()
            .flat_map(|message| &message.entries)
            .collect();
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[1].path, inner.to_string());
        assert!(matches[1].starred && matches[1].network);
        assert_eq!(matches[1].size, Some(32));
        assert!(matches[1].capabilities.as_ref().unwrap().can_copy);
        assert!(messages.last().unwrap().done);
        assert_eq!(
            messages
                .last()
                .unwrap()
                .facets
                .as_ref()
                .unwrap()
                .type_values[0]
                .label,
            "txt"
        );
    }

    #[test]
    fn malformed_provider_response_stops_before_emission_or_outside_reads() {
        let root = CloudPath::parse("rclone://Generated/owned").unwrap();
        for outside in [
            "rclone://Generated/owned-sibling/file",
            "rclone://Other/owned/file",
            "rclone://Generated/owned/../file",
        ] {
            let mut calls = 0;
            let mut messages = Vec::new();
            let error = scan_cloud_search(
                root.clone(),
                &parse_query("alpha").unwrap(),
                &HashSet::new(),
                |directory| {
                    calls += 1;
                    let mut bad = entry(directory, "alpha", true);
                    bad.path = outside.to_string();
                    Ok(vec![entry(directory, "Alpha.TXT", false), bad])
                },
                || false,
                |message| messages.push(message),
            )
            .unwrap_err();
            assert_eq!(error.code_str_value(), "invalid_path");
            assert_eq!(calls, 1);
            assert!(messages.is_empty());
        }
    }

    #[test]
    fn provider_failure_retains_prior_results_and_never_sends_success_completion_or_retries() {
        let root = CloudPath::parse("rclone://Generated/owned").unwrap();
        let mut calls = 0;
        let mut messages = Vec::new();
        let error = scan_cloud_search(
            root.clone(),
            &parse_query("alpha").unwrap(),
            &HashSet::new(),
            |directory| {
                calls += 1;
                if directory == &root {
                    Ok(vec![
                        entry(directory, "Alpha.TXT", false),
                        entry(directory, "nested", true),
                    ])
                } else {
                    Err(SearchError::from(cloud::CloudCommandError::new(
                        cloud::CloudCommandErrorCode::PermissionDenied,
                        "generated denial",
                    )))
                }
            },
            || false,
            |message| messages.push(message),
        )
        .unwrap_err();
        assert_eq!(error.code_str_value(), "permission_denied");
        assert_eq!(calls, 2);
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].entries.len(), 1);
        assert!(!messages[0].done);
    }

    #[test]
    fn cancellation_before_and_during_listing_stops_without_emitting_stale_results() {
        for initial in [true, false] {
            let root = CloudPath::parse("rclone://Generated/owned").unwrap();
            let cancelled = Cell::new(initial);
            let mut calls = 0;
            scan_cloud_search(
                root,
                &parse_query("alpha").unwrap(),
                &HashSet::new(),
                |directory| {
                    calls += 1;
                    cancelled.set(true);
                    Ok(vec![entry(directory, "Alpha.TXT", false)])
                },
                || cancelled.get(),
                |_| panic!("Cancelled searches must not emit results/completion"),
            )
            .unwrap();
            assert_eq!(calls, usize::from(!initial));
        }
    }

    #[test]
    fn duplicate_entries_are_emitted_once_in_bounded_chunks_and_empty_search_finishes() {
        let root = CloudPath::parse("rclone://Generated/owned").unwrap();
        let mut messages = Vec::new();
        scan_cloud_search(
            root.clone(),
            &parse_query("alpha").unwrap(),
            &HashSet::new(),
            |directory| {
                let mut entries: Vec<_> = (0..300)
                    .map(|index| entry(directory, &format!("alpha-{index}.txt"), false))
                    .collect();
                entries.push(entries[0].clone());
                Ok(entries)
            },
            || false,
            |message| messages.push(message),
        )
        .unwrap();
        assert_eq!(
            messages
                .iter()
                .map(|message| message.entries.len())
                .collect::<Vec<_>>(),
            vec![256, 44, 0]
        );
        assert!(messages.last().unwrap().done);
        messages.clear();
        scan_cloud_search(
            root,
            &parse_query("alpha").unwrap(),
            &HashSet::new(),
            |_| Ok(Vec::new()),
            || false,
            |message| messages.push(message),
        )
        .unwrap();
        assert_eq!(messages.len(), 1);
        assert!(messages[0].done && messages[0].entries.is_empty());
    }
}
