# Early Overwrite Backup Protection

Date: 2026-10-03
Baseline: `697116d`
Scope: clipboard overwrite backups, including nested merge branches and their
local undo/redo; disposable temporary files only.

## Reproduced Defect

`killed_overwrite_keeps_original_backup_before_failure_rollback` failed against
the baseline. The real clipboard overwrite pipeline moved an original into its
undo session, then a test subprocess paused before writing replacement contents.
After killing this process, a separate cleanup process removed the unmarked
original backup. This was not a simulated error-return/rollback test: failure
handling never ran. Source bytes survived, but the pre-existing destination did
not. The fixture root was removed after collecting the failure evidence.

## Correction

`backup_existing_target` now creates and syncs the existing recovery-marker
format before moving the original. It rechecks marker ownership immediately
before movement. Creation/writeback failure stops the overwrite without moving
the original. A failed backup move retains its protected candidate and reports
both candidate and original paths for inspection; no uncertain candidate or
competing output is deleted.

The existing `Action::Delete` representation carries optional opaque
`BackupProtection`, avoiding a parallel overwrite history implementation.
Ordinary delete/trash constructors explicitly use no early protection and retain
their prior behavior. Protection has no destructive Drop cleanup. Clipboard
overwrites keep it through the entire paste or directory merge, not just one
copied item. Successful whole paste/rollback finalizes it through the shared undo
engine; failures retain it. Marker-clear failure is logged and conservatively
keeps the session protected.

Failure rollback verifies/reuses existing overwrite protection instead of trying
to create the same marker again. Older unprotected rollback actions retain the
prior protect-before-rollback behavior. Protection verification failure refuses
rollback and reports backup paths. Completed clipboard overwrite history
reactivates protection before moving originals during undo/redo and only clears
it after whole-action/batch success; normal copied-byte recovery remains in use.

## Verified Checkpoint

All 76 clipboard-related tests passed. Four new functions include a subprocess
helper and these cases:

- Twelve actual killed-process fixtures: copy and forced move fallback, flat
  files and nested merges, stopped before original movement, after movement but
  before action registration, or before the first replacement write. Separate
  cleanup retains the marker and, where created, the exact original backup.
  Source bytes survive; the original is intact before movement, the replacement
  path is absent just after movement, and it is empty before streaming writes.
- Marker creation/writeback failure and replaced-marker verification leave the
  original unmoved. A competing backup candidate survives no-replace failure.
- A two-item overwrite keeps both markers through completed peers, clears them
  after whole-paste success and round-trips original/new bytes through undo/redo.

The existing real Unix file-size-limit test now applies the write limit only
after the original is safely backed up. It still triggers actual partial copy
failure, and rollback reuses the already-written marker without requiring a new
diagnostic write under the file-size limit. Existing source/target version,
merge rollback, native-name and completed-copy tests remain green.

The narrow TODO row was checked after this targeted checkpoint. Final backend
maintenance gates passed resource/vendor integrity, dependency policy, formatting,
cargo check, warnings-denied Clippy, typed-error guards and **612 tests**, with
four native opt-in tests ignored. Semgrep is absent and was skipped, not passed.
The ignored tests were not explicitly invoked in this increment.

All **296 frontend unit tests in 40 files** passed. Frontend source and IPC
signatures are unchanged; the browser suite was not rerun. All 20 strict
documentation consistency checks, 40 local documentation links and
`git diff --check` passed. Delete/trash changes outside clipboard are mechanical
initialization/pattern updates for the optional protection field only.

## Remaining Boundaries

Markers pin sessions for manual recovery; they are not a persistent undo journal
or an automatic restore plan. An interrupted backup may be partial, and errors
require inspection of both original and candidate paths. Marker/backup directory
entries are not additionally synced as a power-loss transaction. No battery-loss,
hardware fault, removable-device or cross-distribution guarantee is made here.

The killed fixtures use temporary local paths, not physical USB/MTP, and do not
exercise a kill midway through a cross-filesystem original-backup stream. Other
delete/trash crash windows remain separate scope. Completed-stream foreign edits
masked before version capture and final check-to-unlink races are unchanged.

No installation, restart, release, dependency or version change. Broader manual
recovery, space-budget, installed UI and real-media TODO rows remain open.
