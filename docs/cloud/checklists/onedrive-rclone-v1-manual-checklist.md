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
- [ ] Native full archive orchestration, extracted-folder upload and empty subfolders
- [ ] Open With/native external drag to Nautilus (copy-only, including Shift)
- [ ] Restore owned test data from the provider website recycle bin
- [ ] Active network loss/quota/rate limit on a dedicated test account (fixtures only so far)
- [ ] Active real-provider cancellation, large/deep trees and concurrent target races

Opt-in backend runner: `commands::cloud::workspace::tests::real_onedrive_working_copy_and_archive_acceptance`.
It is ignored in ordinary CI and requires `BROWSEY_TEST_CLOUD_SCOPE` pointing to
an approved empty non-root folder plus `BROWSEY_TEST_CLOUD_WRITE_APPROVED=yes`.
Failure retains marked test data for inspection; successful cleanup uses trash,
not a global purge. OneDrive Personal hard-delete is not assumed supported.

## Notes

- Record distro, Browsey commit, `rclone version`, OneDrive account type, and
  observed provider-specific anomalies.
- Link any failure to scenario ID(s) and issue(s) from the core checklist run.

Result: Linux 1.0 provider acceptance passed on the validated Linux target
surface with no release-blocking provider anomalies. See
`docs/operations/linux-release/release-candidate-log.md`.
