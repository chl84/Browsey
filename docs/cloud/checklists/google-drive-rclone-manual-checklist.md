# Google Drive rclone Manual Checklist (Appendix)

Created: 2026-03-07
Role: Provider-specific appendix to
`docs/operations/core-operations/release-checklist.md`

Use this checklist after matrix-based core scenarios pass, to capture Google
Drive provider behavior and real-account anomalies without redefining core
semantics.

Reuse shared rclone and OneDrive regression evidence for common workflows.
Prioritize Google-specific names/identity, native documents, shortcuts and
provider failures; do not infer those results from OneDrive acceptance.

## Environment

- [x] Linux machine with Browsey build under test
- [x] `rclone` installed and available in `PATH`
- [x] `rclone config` contains a working `drive` remote
- [x] Disposable Google Drive test folder (no production data)
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

## Google Drive-Specific Reliability Checks

- [x] Remote appears in `Network` and opens as `rclone://...`
- [x] Manual refresh after writes shows consistent state
- [x] Reopening same folder does not surface stale/ghost entries
- [x] Errors are user-actionable (not raw provider noise dumps)
- [x] Identical-name objects are represented and addressed safely in the
  [2026-10-08 candidate run](../../operations/linux-release/runs/2026-10-08-google-drive-object-identity.md).
  Selected-ID listing/download/rename/delete/folder navigation passed;
  ambiguous bulk transfers and overwrites are refused. The installed-build
  [2026-10-07 difference run](../../operations/linux-release/runs/2026-10-07-google-drive-provider-differences.md)
  reproduced two objects collapsing to one row and a download of the other object.
- [x] Large-file transfer remains stable with progress and cancellation
- [x] Forced network interruption produces understandable failure state

## Expected Limitations (Linux 1.0 Scope)

Historical Linux 1.0 results below are not acceptance of the 1.0.4 additions.

- [x] Cloud delete uses permanent-delete semantics (no cloud trash integration)
- [x] Advanced rename remains unavailable for cloud entries
- [x] Cloud archive extract/compress remains unavailable
- [x] Open-in-console is blocked for cloud folders

## 1.0.4 Expansion Acceptance

Partial installed-build acceptance on 2026-10-07 used an explicitly authorized,
unique ownership-marked directory and a private profile with the installed
Browsey 1.0.4 executable. See the
[production run](../../operations/linux-release/runs/2026-10-07-google-drive-production.md)
for exact binary identity, native workflows, independent byte checks, harness
corrections and cleanup. Automated fixtures alone do not constitute real-account
acceptance, and the remaining rows are still open.

The subsequent
[provider-difference run](../../operations/linux-release/runs/2026-10-07-google-drive-provider-differences.md)
passed case-sensitive names, advanced rename, native-document export and
shortcut semantics, but confirmed an identical-name integrity defect in the
installed executable. The subsequent candidate run fixes and validates that
engineering scope; the installed binary has not been replaced.

- [x] Marked disposable directory only; cleanup verifies ownership, never global trash purge
- [x] Durable copies survive restart/cache clearing; Open With uses the local copy
- [x] Explicit unique upload preserves the original and local edits
- [ ] Changed cloud source and existing/concurrently created upload target
- [x] Normal trash, independently verified trashed file, and explicit permanent delete
- [ ] Provider website restore
- [x] Native password ZIP creation/extraction and uploaded extracted tree/empty directories
- [ ] Archive cancellation and retained staging under failure
- [x] Case-distinct upload, case-only rename, exact-name collision refusal and advanced rename
- [x] Native Google document: unknown-size listing, DOCX download and durable Open With copy
- [x] Shortcut server-side copy preserves target identity; trash preserves target; local download returns target bytes
- [x] Identical-name object identity and selected-object download in the candidate
  (see candidate run; bulk/overwrite boundaries remain explicit)
- [ ] Prepared copy-only external drag
- [ ] Network/quota/rate limits, active cancellation and large/deep trees on a dedicated test account

## Notes

- Record distro, Browsey commit, `rclone version`, Google account/workspace
  type, and observed provider-specific anomalies.
- Link any failure to scenario ID(s) and issue(s) from the core checklist run.

Historical result: Linux 1.0 provider acceptance passed on the validated Linux
target surface. See
`docs/operations/linux-release/release-candidate-log.md`.
Current 1.0.4 acceptance is partial. The installed-build identical-name defect
is fixed and validated in the working-tree production candidate, with the
documented bulk/overwrite boundaries; broader provider acceptance remains open.
