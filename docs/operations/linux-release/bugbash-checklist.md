# Linux 1.0 Bugbash Checklist

Created: 2026-03-06
Track: `docs/todo/TODO_PRODUCTION_READY_LINUX.md`
Release-bar reference: `docs/operations/linux-release/release-bar.md`

## Purpose

Provide a Linux 1.0 bugbash checklist that extends the existing
core-operations release checklist instead of duplicating it.

Use this checklist for Linux-focused manual validation of workflows that are
inside the Linux 1.0 production claim but are not fully represented in
`docs/operations/core-operations/release-checklist.md`.

## Base Release Checklist Requirement

Before running this bugbash checklist, run the existing core-operations release
checklist as applicable to the touched scope:

- `docs/operations/core-operations/release-checklist.md`

This Linux 1.0 bugbash checklist is additive. It does not replace the
core-operations checklist.

## Run Metadata

- [ ] Date:
- [ ] Candidate version/build:
- [ ] Commit SHA:
- [ ] Tester:
- [ ] OS + distro/version:
- [ ] Desktop/session:
- [ ] Notes link (logs/screenshots/issues):

## Preconditions

- [ ] Build under test launches successfully
- [ ] Core-operations release checklist has been run or scoped for this candidate
- [ ] Test machine matches a supported Linux target surface from `release-bar.md`

## Linux 1.0 Additional Workflow Rows

| Done | Scenario ID | Action to execute | Result (`PASS`/`FAIL`/`N/A`) | Notes |
|---|---|---|---|---|
| [ ] | `LX-BRW-001` | Launch Browsey and open a normal local directory from the main view. |  |  |
| [ ] | `LX-BRW-002` | Navigate up/down between local directories and confirm selection/focus remains usable. |  |  |
| [ ] | `LX-BRW-003` | Open `Network` and verify non-cloud entries remain usable when cloud is disabled. |  |  |
| [ ] | `LX-CLP-001` | On GNOME Wayland with `xclip` installed, copy/cut/paste local files between two Browsey windows and confirm no shell focus/dock side-effects occur. |  |  |
| [ ] | `LX-CLP-002` | On GNOME Wayland without `xclip`, copy/cut/paste local files within Browsey and confirm internal clipboard flows still work even if cross-window system clipboard interoperability is unavailable. |  |  |
| [ ] | `LX-DEP-001` | Start Browsey without `ffmpeg` available and confirm video thumbnails degrade to file-type icons instead of breaking the view. |  |  |
| [ ] | `LX-DEP-002` | Start Browsey without `rclone` available and confirm normal local browsing remains usable while cloud surfaces show limited/disabled behavior rather than generic app failure. |  |  |
| [ ] | `LX-SRC-001` | Run a normal recursive search and open one result. |  |  |
| [ ] | `LX-SRC-002` | Trigger a search error/invalid query path and confirm user-facing error presentation is understandable. |  |  |
| [ ] | `LX-SRC-003` | Cancel an active search and verify the UI returns to a sane state. |  |  |
| [ ] | `LX-ARN-001` | Run advanced rename on a representative local batch and verify preview/apply behavior. |  |  |
| [ ] | `LX-CMP-001` | Compress representative local selection(s) and verify resulting archive output in the UI. |  |  |
| [ ] | `LX-PRP-001` | Open Properties for a single local file and verify basic metadata/path display. |  |  |
| [ ] | `LX-PRP-002` | Edit supported Linux permissions/ownership fields on a disposable local target and verify apply/refresh behavior. |  |  |
| [ ] | `LX-OPN-001` | Open a local file with its default handler from Browsey. |  |  |
| [ ] | `LX-OPN-002` | Use `Open With` on a local file and verify the chosen app launch path works. |  |  |
| [ ] | `LX-SET-001` | Open Settings, change representative preferences, and confirm they apply without restart unless documented otherwise. |  |  |
| [ ] | `LX-SET-002` | Run `Restore defaults` and confirm representative settings return to documented defaults. |  |  |

## Notes

- If a row fails in a way that maps to an existing core-operations
  release-blocking scenario, classify it using
  `docs/operations/core-operations/release-blocking-policy.md`.
- If a Linux 1.0 bugbash row repeatedly finds production-critical regressions,
  consider promoting that workflow into the formal release operations docs.
