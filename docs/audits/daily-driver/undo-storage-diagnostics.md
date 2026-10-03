# Undo Storage Diagnostics

Date: 2026-10-03
Baseline: `c8949ca`
Track: [Daily-driver completeness plan](../../todo/TODO_DAILY_DRIVER_COMPLETENESS.md)
Status: Backend diagnostics verified; Settings integration in progress.

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

- Finish and verify the shared Settings UI and manual guidance.
- Measure representative real backup sizes and agree byte budgets/retention
  policy; diagnostic limits do not limit retained backup space.
- Validate the installed application, manual recovery and actual filesystem
  faults. Mocked browser UI tests do not prove native recovery acceptance.
- No persistent undo, automatic resume/repair or deletion UI is introduced.
  Protected sessions stay protected regardless of diagnostic counts or size.

Version remains 1.0.3. Installation/publication is not part of this increment.
