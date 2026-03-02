# OneDrive rclone v1 Manual Checklist (Appendix)

Created: 2026-03-02
Role: Provider-specific appendix to
`docs/core-operations-release-checklist.md`

Use this checklist after matrix-based core scenarios pass, to capture OneDrive
provider behavior and real-account anomalies without redefining core semantics.

## Environment

- [ ] Linux machine with Browsey build under test
- [ ] `rclone` installed and available in `PATH`
- [ ] `rclone config` contains a working `onedrive` remote
- [ ] Disposable OneDrive test folder (no production data)
- [ ] Test data contains:
  - small + large files
  - at least one conflict pair
  - one directory tree with nested entries

## Matrix-Linked Cloud Scenarios

Reference behavior: `docs/core-operations-matrix.md`

- [ ] `CO-MTC-001` Local -> cloud copy file
- [ ] `CO-MTC-002` Cloud -> local copy file
- [ ] `CO-MTC-003` Local -> cloud move file
- [ ] `CO-MTC-004` Cloud -> local move file
- [ ] `CO-MTC-005` Mixed directory copy/move
- [ ] `CO-MTC-006` Mixed conflict preview consistency

## OneDrive-Specific Reliability Checks

- [ ] Remote appears in `Network` and opens as `rclone://...`
- [ ] Manual refresh after writes shows consistent state
- [ ] Reopening same folder does not surface stale/ghost entries
- [ ] Errors are user-actionable (not raw provider noise dumps)
- [ ] Large-file transfer remains stable with progress and cancellation
- [ ] Forced network interruption produces understandable failure state

## Expected Limitations (v1 Scope)

- [ ] Cloud delete uses permanent-delete semantics (no cloud trash integration)
- [ ] Advanced rename remains unavailable for cloud entries
- [ ] Cloud archive extract/compress remains unavailable
- [ ] Open-in-console is blocked for cloud folders

## Notes

- Record distro, Browsey commit, `rclone version`, OneDrive account type, and
  observed provider-specific anomalies.
- Link any failure to scenario ID(s) and issue(s) from the core checklist run.
