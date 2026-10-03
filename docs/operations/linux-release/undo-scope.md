# Linux 1.0 Undo/Redo Scope

Created: 2026-03-06
Track: `docs/todo-archive/TODO_PRODUCTION_READY_LINUX.md`
Status: Active source of truth for the Linux 1.0 undo/redo claim.

## Purpose

Define the actual undo/redo scope that Browsey supports for the Linux 1.0
production claim, so release validation and user-facing docs do not imply a
broader guarantee than the code currently provides.

This document is intentionally narrower than a full feature inventory. It only
states which actions are part of the Linux 1.0 undo/redo claim, which
boundaries apply, and which areas are explicitly outside that claim.

## Core Guarantees

For the Linux 1.0 production claim, Browsey undo/redo is defined as:

- local-operation only
- in-memory only for the lifetime of the running app
- capped to the most recent 50 recorded actions
- cleared forward-redo history when a new action is recorded

This means Browsey may only claim Linux 1.0 undo/redo support for actions that
are explicitly recorded into the shared `UndoState` and replayed through the
central undo engine.

## Supported Undo/Redo Scope for Linux 1.0

The following local filesystem actions are inside the Linux 1.0 undo/redo
claim:

- copy and cut/paste operations that complete through the clipboard file-op
  pipeline
- rename and batch rename for local filesystem paths
- create file
- create folder
- permanent delete flows that move the original path into the undo backup area
- move to trash flows that record either the final trash move or the backup
  fallback action
- archive compression when the operation creates a new archive successfully
- archive extraction when the operation creates output successfully

These are the concrete action types currently recorded into the shared undo
history on Linux:

- `Action::Copy`
- `Action::Move`
- `Action::Rename`
- `Action::Create`
- `Action::Delete`
- `Action::CreateFolder`
- `Action::Batch(...)`

## Boundaries That Must Be Documented Clearly

The Linux 1.0 undo/redo claim is subject to these hard boundaries:

- undo/redo history is not persisted across app restart
- startup cleanup removes abandoned, unlocked undo sessions from previous runs;
  backups belonging to running instances are protected by OS file locks
- copy recovery markers also preserve abandoned sessions after failed or
  interrupted undo/redo; those sessions require explicit manual recovery and
  are not reconstructed into persistent undo history
- version 1.0.1 stores sessions under `browsey/undo-sessions/`; legacy `browsey/undo/`
  backups are left intact because older processes do not provide ownership locks
- only actions that successfully completed and were recorded are undoable
- multi-item operations may be recorded as one `Batch(...)` history item rather
  than many separate undo steps
- a newly recorded action clears redo history
- history depth is capped at 50 recorded items
- copy undo checks a receipt of created directory identities and completed
  regular-file versions; changed, replaced, missing or unverifiable targets
  cause an explicit error instead of blind recursive deletion
- GIO-owned copy writers (including some GVfs/MTP transfers) do not provide
  this ownership evidence; their targets are retained when copy undo is refused
- copy undo removes only recorded unchanged files and empty directories;
  pure-copy batches preflight all targets, but late changes or I/O failures can
  still leave a partially undone operation. Inspect the reported paths before
  retrying; there is no automatic retry or transaction guarantee
- copy undo first writes and verifies a complete private backup before removing
  the unchanged target. Backup/writeback/marker failures leave the target
  untouched; redo and mixed-batch compensation restore from the preserved bytes,
  not from an original source that may have changed or disappeared
- failed removal/restoration can still leave partial targets; complete backups
  are retained and reported for manual recovery. Changed backups and occupied
  restore targets are refused; uncertain partial trees are not overwritten or
  adopted automatically
- recovery markers remain until the entire action/batch completes successfully.
  On failure/interruption, startup cleanup keeps the whole marked session.
  Marker-clear failure is logged and conservatively retains the session
- older builds may not recognize recovery markers; recover data before a
  downgrade or launching an older build against abandoned-session storage
- backups are reused across copy undo/redo cycles and may remain until session
  cleanup. The 50-action history cap is not a byte quota; first undo needs space
  for an additional full copy, and marked sessions can grow across restarts
- history filesystem work runs on a blocking worker; repeated undo/redo requests
  in the same explorer page are suppressed until both operation and refresh
  finish. Listing refresh is attempted after errors, without retrying file work
- manual local copies validate source versions/copy lengths after streaming and
  pre-sync output versions after writeback. Uncertain finalized output is kept
  for inspection instead of treated as a successful undoable copy
- both manual local copy engines compare BLAKE3 digests of the written stream
  with a bounded readback of the still-open output. Versions are rechecked before
  and after reading; mismatches/read errors refuse completion and source deletion.
  Readback needs a readable/seekable output and adds one target read pass, using
  a 256 KiB buffer; it is not a second source read or a persistent receipt hash
- clipboard readback checks cancellation between chunks; history replay still
  has no new cancellation API. Verification reads do not inflate transferred
  byte totals and no successful completion event precedes verification
- failed/aborted local file streams retain their current uncertain output, even
  when its inode still matches: another writer's changes can be masked by our
  writes. Cancellation detected before opening a target creates no output.
  Inspect retained files before retrying; they may be incomplete or edited
- clipboard overwrites create and sync recovery markers before moving original
  destinations into backups. Protection creation/finalization failure leaves the
  original unmoved. Paste/merge and rollback retain protection until the whole
  operation succeeds; history replay reactivates it before moving the original
- occupied restore targets are not overwritten. Blocked rollback reports backup
  paths and keeps markers through startup cleanup, including process interruption
  before failure handling. Uncertain failed backup candidates are also retained
- protection verification failure refuses rollback/history movement. Inspect the
  reported backup and destination paths rather than retrying automatically. If no
  marker could be created for an unprotected rollback action, manually recover
  originals before closing Browsey or allowing startup cleanup
- this protection is scoped to clipboard overwrite backups, not every delete,
  trash or history action. It does not imply atomic writes or power-loss durability;
  no directory-sync/journal recovery or persistent history guarantee is added
- copy/delete move fallback revalidates destination receipts before source
  removal and checks recorded source entries individually; new source children
  survive because directories are only removed when empty. Errors can leave a
  partially removed source tree and retain the destination without automatic retry
- opaque output writers (including GIO-owned copies) cannot justify fallback
  source deletion without ownership receipts. Such a move may copy successfully
  but then refuse source removal; inspect both paths rather than repeat the move
- these checks do not close after-readback/final check-to-unlink races or make
  copy/move atomic. Source snapshots and later removal receipts remain metadata
  checks, not stored content hashes or locks. Readback compares the bytes it reads
  to the written stream; it does not freeze concurrent writers or guarantee a
  consistent point-in-time source/directory snapshot or power-loss recovery
- supported local copy/move/undo completion requires stable source, destination
  and parent entries throughout the operation. Stop other programs writing,
  replacing or renaming them first; detection of some concurrent edits is not
  an active-writer safety guarantee. See the
  [concurrent-writer boundary decision](../../audits/daily-driver/concurrent-writer-boundary.md)
- Settings > Data inspects undo-session storage without changing files, locks
  or markers. It shows measured file-content lengths, session/marker counts,
  a copyable directory path and manual recovery guidance. Incomplete scans are
  explicitly labelled, and marker counts also include work still in progress;
  this is not allocated disk usage, a quota, or automatic recovery/cleanup

These boundaries are part of the supported behavior, not incidental
implementation details.

See the [copy undo ownership follow-up](../../audits/daily-driver/copy-undo-ownership.md)
for disposable-fixture evidence and remaining safety work.
The [copy recovery backup follow-up](../../audits/daily-driver/copy-recovery-backups.md)
documents preserved bytes and manual recovery boundaries.
The [storage diagnostics follow-up](../../audits/daily-driver/undo-storage-diagnostics.md)
records scan limits, Settings behavior and outstanding retention acceptance.
The [copy version follow-up](../../audits/daily-driver/copy-version-validation.md)
records source/read and output/writeback checks plus fallback removal boundaries.

## Outside the Linux 1.0 Undo/Redo Claim

The following areas are explicitly outside the Linux 1.0 undo/redo claim unless
the code and this document are both updated later:

- all cloud (`rclone`) operations
- search
- open/open-with launches
- settings changes
- mount/discovery/network connection flows
- permissions editing and ownership changes
- any operation that does not record an action into the shared `UndoState`

Cloud is especially important here: Browsey currently advertises
`can_undo: false` for cloud capabilities, and Linux 1.0 must continue to treat
cloud undo/redo as unsupported.

## Release Validation Use

Representative disposable measurements and the conservative budget/retention
decision are recorded in the [recovery storage policy](../../audits/daily-driver/recovery-storage-policy.md).
On Unix, Settings also reports filesystem-allocated blocks for scanned files and
directories. This is not exclusive physical usage on CoW/compressed filesystems,
and an incomplete scan does not establish the complete storage requirement.

This document is the Linux 1.0 source of truth for undo/redo scope. Release
validation, bugbash work, and user-facing Linux docs should use it to answer:

- which workflows must keep working with undo/redo on Linux
- which workflows must not be described as undoable
- which regressions count as Linux trust regressions versus expected scope

If Browsey expands undo/redo later, the release checklist and this document
must be updated together.
