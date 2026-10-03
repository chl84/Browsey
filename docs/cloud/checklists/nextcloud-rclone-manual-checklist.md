# Nextcloud rclone Manual Checklist (Appendix)

Created: 2026-03-07
Role: Provider-specific appendix to
`docs/operations/core-operations/release-checklist.md`

Use this checklist after matrix-based core scenarios pass, to capture
Nextcloud provider behavior and real-instance anomalies without redefining core
semantics.

## Environment

- [x] Linux machine with Browsey build under test
- [x] `rclone` installed and available in `PATH`
- [x] `rclone config` contains a working `nextcloud` or recognized Nextcloud
      `webdav` remote
- [x] Disposable Nextcloud test folder (no production data)
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

## Nextcloud-Specific Reliability Checks

- [x] Remote appears in `Network` and opens as `rclone://...`
- [x] Manual refresh after writes shows consistent state
- [x] Reopening same folder does not surface stale/ghost entries
- [x] Errors are user-actionable (not raw provider noise dumps)
- [x] Conflict/rename behavior matches Browsey conflict preview assumptions
- [x] Large-file transfer remains stable with progress and cancellation
- [x] Forced network interruption produces understandable failure state

## Expected Limitations (Linux 1.0 Scope)

Historical Linux 1.0 results below are not acceptance of the 1.0.4 additions.

- [x] Cloud delete uses permanent-delete semantics (no cloud trash integration)
- [x] Advanced rename remains unavailable for cloud entries
- [x] Cloud archive extract/compress remains unavailable
- [x] Open-in-console is blocked for cloud folders

## 1.0.4 Expansion Acceptance

Pending: no approved disposable Nextcloud remote/folder was supplied for this
expansion. Automated provider fixtures do not constitute instance acceptance.

- [ ] Marked disposable child only; explicit cleanup verifies ownership
- [ ] Durable copies survive restart/cache clearing; Open With uses the local copy
- [ ] Explicit unique upload, changed source (including unavailable modtime/hash) and target conflict
- [ ] Trash hidden/refused without a supported API; explicit delete never masquerades as trash
- [ ] Password archives, cancellation, retained staging and uploaded extracted trees/empty directories
- [ ] Advanced rename and prepared copy-only external drag
- [ ] Network/quota/rate limits, active cancellation, server version differences and large/deep trees

## Notes

- Record distro, Browsey commit, `rclone version`, Nextcloud version/vendor
  details if relevant, and observed provider-specific anomalies.
- Link any failure to scenario ID(s) and issue(s) from the core checklist run.

Result: Linux 1.0 provider acceptance passed on the validated Linux target
surface with no release-blocking provider anomalies. See
`docs/operations/linux-release/release-candidate-log.md`.
