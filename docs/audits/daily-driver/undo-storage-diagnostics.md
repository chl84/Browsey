# Undo Storage Diagnostics

Date: 2026-10-03
Baseline: `c8949ca`
Track: [Daily-driver completeness plan](../../todo/TODO_DAILY_DRIVER_COMPLETENESS.md)
Status: Backend fixtures and mocked Settings behavior verified; installed/manual
recovery and retention policy remain open.

## Read-only Inspection

`inspect_undo_storage` measures the existing `browsey/undo-sessions` directory
on a blocking worker. It does not create storage, read file contents, acquire
or release session locks, clear recovery markers, restore files, or purge data.
Counts include regular-file logical lengths within session directories,
including regular marker files. Sibling locks and unknown/legacy directories
are excluded. Hard-linked paths count separately; sparse/compressed files and
metadata overhead mean this is not allocated disk usage or available space.

Marker counts include running operations as well as retained failed sessions;
the presence of a marker is not evidence that work has finished or failed.
Suspicious marker types are counted but not followed. Ordinary symlinks and
special files are not traversed. Invalid UTF-8 storage roots are explicitly
rejected instead of returning a misleading lossy path; backup basenames can
still contain native Unix bytes.

A shared 10,000-entry budget and 250 ms cooperative deadline stop additional
metadata traversal. Errors, unsafe types, overflow and budget exhaustion flag
the measurement as incomplete. A single filesystem call can still block longer
on slow/unresponsive storage; this is not an I/O timeout. Measurements are
diagnostic snapshots, not atomic ownership/security receipts or complete totals
when filesystem contents change during inspection.

## Verified Backend Increment

- All six new storage fixture tests passed: ordinary/marked inventory and
  unchanged file/marker/lock contents; absent storage without directory creation;
  entry/time limits; suspicious directory markers; symlink exclusion; native
  basenames and explicit refusal of lossy storage root paths.
- Offline/locked all-target/all-feature Clippy passed with warnings denied.
- No real device or personal backup data was modified. Fixtures are unique
  temporary directories; their teardown removes only their own created data.

## Remaining Work

- Measure representative real backup sizes and agree byte budgets/retention
  policy; diagnostic limits do not limit retained backup space.
- Validate the installed application, manual recovery and actual filesystem
  faults. Mocked browser UI tests do not prove native recovery acceptance.
- No persistent undo, automatic resume/repair or deletion UI is introduced.
  Protected sessions stay protected regardless of diagnostic counts or size.

Version remains 1.0.3. Installation/publication is not part of this increment.

## Settings and Manual Recovery Guidance

The backend increment was committed as `fa2fc05` before the Settings increment,
with its narrower TODO row checked at that point. Settings > Data now contains
a read-only inspection panel using shared `TextField`, button/focus/theme tokens
and the explorer's existing file-size formatter, extracted unchanged into a
shared utility. Settings filters find it by undo/recovery/backups/disk space.

The panel loads when the Data section is rendered, suppresses duplicate refreshes and discards late
successes/errors after destruction. A refresh failure preserves the previous
measurement with an explicit stale label. Partial results never imply an empty
or fully measured directory. No delete, restore or marker-clear action is offered.

Keyboard-accessible guidance tells users to finish operations and close every
Browsey instance before manual work, avoid older marker-unaware builds, inspect
the error's affected paths, copy needed bytes to a separate safe location without
overwriting uncertain files and verify them before considering cleanup. Clearing
the last marker can expose the whole session to startup cleanup. Measurements
exclude legacy/unknown storage; markers can indicate currently running work.

Seven backend tests now cover the six fixture cases above plus the actual JSON
field names used by Settings. New frontend tests cover scan presentation,
repeated requests, stale error/retry behavior, lifecycle disposal, filter matches
and unchanged shared size formatting. Five mocked-browser tests cover 900/620 px
layout with long copyable paths, keyboard-accessible guidance, no destructive
controls, partial measurements, refresh errors and closing/reopening while pending.

The T3 collaborative preview rendered the panel and returned its text via DOM
inspection. Snapshot calls failed and a later resize lost the preview host; no
successful visual screenshot or native installed-build acceptance is claimed.

## Final Verification

- Backend maintenance checks passed PDFium/vendor integrity, four dependency
  policy tests, formatting, cargo check, warnings-denied Clippy and the typed
  error guard. Semgrep is absent and its checks were skipped, not passed.
- After adding the JSON contract test, final offline/locked all-target/all-feature
  tests passed: **593 passed, four opt-in native tests ignored**. Final Clippy
  also passed with warnings denied. Ignored native/device tests were not run here.
- Frontend lint/naming, Svelte and TypeScript checks passed with zero Svelte
  errors/warnings; **296 unit tests in 40 files** and all **60 mocked browser
  tests** passed. The production frontend build passed.
- `git diff --check`, all **20 strict documentation consistency checks** and
  **30 local documentation links** passed. README, CHANGELOG and undo scope
  describe the same read-only guarantees and remaining boundaries.
- Both narrower TODO rows were checked after their respective verification.
  The broad manual-recovery/retention/installed-build row stays unchecked.

No dependency/version change, device operation, installation, app restart,
automatic recovery or protected-backup cleanup was performed.
