# OneDrive rclone Manual Checklist (Appendix)

Created: 2026-03-02
Role: Provider-specific appendix to
`docs/operations/core-operations/release-checklist.md`

Use this checklist after matrix-based core scenarios pass, to capture OneDrive
provider behavior and real-account anomalies without redefining core semantics.

## Environment

- [x] Linux machine with Browsey build under test
- [x] `rclone` installed and available in `PATH`
- [x] `rclone config` contains a working `onedrive` remote
- [x] Disposable OneDrive test folder (no production data)
- [x] Test data contains:
  - [x] small + large files
  - [x] at least one conflict pair
  - [x] one directory tree with nested entries

## Matrix-Linked Cloud Scenarios

Reference behavior: `docs/operations/core-operations/matrix.md`

- [x] `CO-MTC-001` Local -> cloud copy file
- [x] `CO-MTC-002` Cloud -> local copy file
- [x] `CO-MTC-003` Local -> cloud move file
- [x] `CO-MTC-004` Cloud -> local move file
- [x] `CO-MTC-005` Mixed directory copy/move
- [x] `CO-MTC-006` Mixed conflict preview consistency

## OneDrive-Specific Reliability Checks

- [x] Remote appears in `Network` and opens as `rclone://...`
- [x] Manual refresh after writes shows consistent state
- [x] Reopening same folder does not surface stale/ghost entries
- [x] Errors are user-actionable (not raw provider noise dumps)
- [x] Large-file transfer remains stable with progress and cancellation
- [x] Forced network interruption produces understandable failure state

## Expected Limitations (Linux 1.0 Scope)

Historical Linux 1.0 results below are not acceptance of Unreleased additions.

- [x] Cloud delete uses permanent-delete semantics (no cloud trash integration)
- [x] Advanced rename remains unavailable for cloud entries
- [x] Cloud archive extract/compress remains unavailable
- [x] Open-in-console is blocked for cloud folders

## Unreleased Expansion Acceptance (2026-10-03)

Partial backend acceptance used an explicitly approved empty folder and a new
uniquely named, ownership-marked child. Provider/working-copy/archive production
helpers ran against rclone 1.75.1 on Omarchy 4.0.4. No personal paths or account
identifiers are recorded here. This is not native UI acceptance or a release.

- [x] Download/open working copy, retain edits after source-cache removal
- [x] Explicit upload to a unique new name; original and local edits retained
- [x] Same-size cloud source change detected; existing target refused
- [x] Pre-cancelled upload creates no remote target
- [x] Advanced rename with the shared preflight path
- [x] Password ZIP creation/upload/download/extraction with existing archive engine
- [x] Marker verified before owned-child normal trash cleanup; parent empty afterward
- [x] Backend encrypted ZIP and extracted-tree upload/download: 32 files in eight
  groups, nested empty directory, byte verification, unchanged original archive
  and existing-directory refusal. Real opt-in transfer test passed in 120 seconds.
- [x] Native archive context menu, password checkbox and extraction password
  modal through real IPC/local staging/OneDrive. Private optimized candidate,
  exact returned bytes/empty directories, refreshed output and retained originals;
  passed in 141 seconds. This does not validate an external GTK drag receiver.
- [ ] Open With/native external drag to Nautilus (copy-only, including Shift)
- [ ] Restore owned test data from the provider website recycle bin
- [x] Active real-provider cancellation after positive transfer byte statistics;
  cancelled code, preserved source, remote state inspected without assuming
  rollback, then owned-child normal trash cleanup.
- [x] Active network interruption scoped to one test rclone process via a local
  CONNECT proxy; real bytes precede injection, network error and source retention
  verified. The desktop's network and normal rclone configuration are unchanged.
- [ ] Quota/rate limit on a dedicated test account (fixtures only so far)
- [x] Same-size destination introduced after Browsey preflight: CLI refuses
  replacement, RC progress skips the existing object and checks completed-transfer
  statistics; no-transfer is failure, competing/source bytes retained. Passed
  in 75 seconds; no provider CAS or in-flight race guarantee.
- [ ] Large/deep trees and concurrent target races

Opt-in backend runner: `commands::cloud::workspace::tests::real_onedrive_working_copy_and_archive_acceptance`.
Extracted-tree runner:
`commands::transfer::execute::tests::real_onedrive::real_onedrive_archive_tree_acceptance`.
It is ignored in ordinary CI and requires `BROWSEY_TEST_CLOUD_SCOPE` pointing to
an approved empty non-root folder plus `BROWSEY_TEST_CLOUD_WRITE_APPROVED=yes`.
Failure retains marked test data for inspection; successful cleanup uses trash,
not a global purge. OneDrive Personal hard-delete is not assumed supported.
Both reuse the same empty-parent and ownership-marker guard. A 32-file tree is
functional round-trip evidence, not large/deep-tree performance acceptance.
Additional opt-in runners are `real_onedrive_active_fault_acceptance`,
`real_onedrive_post_preflight_destination_acceptance` and
`real_onedrive_native_archive_acceptance`; the last additionally requires
`BROWSEY_NATIVE_CLOUD_TEST_APPROVED=yes` and an isolated production candidate.
See the [scoped follow-up evidence](../../audits/daily-driver/onedrive-acceptance.md).

## Notes

- Record distro, Browsey commit, `rclone version`, OneDrive account type, and
  observed provider-specific anomalies.
- Link any failure to scenario ID(s) and issue(s) from the core checklist run.

Result: Linux 1.0 provider acceptance passed on the validated Linux target
surface with no release-blocking provider anomalies. See
`docs/operations/linux-release/release-candidate-log.md`.
