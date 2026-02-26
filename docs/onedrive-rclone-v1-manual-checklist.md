# OneDrive rclone v1 Manual Test Checklist

Use this checklist when validating Browsey's `rclone`-backed OneDrive flow on a real account.

## Environment
- [ ] Linux machine with Browsey build under test
- [ ] `rclone` installed and available in `PATH`
- [ ] `rclone config` contains a working `onedrive` remote
- [ ] Test account has a disposable test folder (no production-only content)

## Basic access and discovery
- [ ] Browsey opens normally with `rclone` installed
- [ ] `Network` view shows the configured OneDrive remote
- [ ] Opening the remote loads a directory listing (`rclone://...`)
- [ ] Listing refresh works (manual refresh / reload current view)

## Core file ops (v1 target scope)
- [ ] Create folder in remote
- [ ] Rename file in remote
- [ ] Rename folder in remote
- [ ] Copy file within same remote
- [ ] Move file within same remote
- [ ] Delete file (permanent delete semantics)
- [ ] Delete non-empty folder (permanent delete semantics)

## Conflict and overwrite flow
- [ ] Paste preview detects same-name file conflict in remote folder
- [ ] Rename-on-conflict creates a suffixed target (for example `-1`)
- [ ] Overwrite path completes when selected
- [ ] Self-paste in same folder auto-renames instead of prompting

## Reliability checks
- [ ] Refresh after rename/mkdir/delete shows expected state
- [ ] Re-opening the same folder shows consistent state after operations
- [ ] No GVFS `/run/user/.../gvfs/...` path is involved in cloud operations
- [ ] Errors (if any) surface user-friendly cloud messages (not raw rclone stderr dumps)

## Large-file / slower operation sanity checks
- [ ] Copy a larger file and confirm operation completes within timeout expectations
- [ ] Confirm timeout error message is understandable if a forced network interruption is tested

## Negative / unsupported flows (expected limitations)
- [ ] Mixed local/cloud paste is rejected with clear message
- [ ] Cloud trash action is blocked / unavailable
- [ ] Advanced rename on cloud entry is blocked / unavailable
- [ ] Open in console for cloud folder is blocked / unavailable

## Notes
- Record distro, Browsey commit, `rclone version`, and any provider-specific anomalies.
