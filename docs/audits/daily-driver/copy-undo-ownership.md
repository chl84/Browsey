# Copy Undo Ownership and Batch Rollback

Date: 2026-10-03
Baseline: `47fde96`
Track: [Daily-driver completeness plan](../../todo/TODO_DAILY_DRIVER_COMPLETENESS.md)
Status: Incremental implementation; overall Priority 0 remains open.

Follow-up: the later [copy recovery backup increment](copy-recovery-backups.md)
replaces source-based redo/compensation and protects failure backups across
startup cleanup. The observations below describe the `290e924` increment.

## Reproduction

A new disposable-fixture regression failed against the preceding code: copy a
file, edit its destination, then undo. Undo returned success and deleted the
edited destination. The old `Action::Copy` recorded only source and destination
paths; backward replay blindly deleted the target recursively. Clipboard
rollback of already completed copies used the same replay path.

## Implementation

`CopyReceipt` stores an in-memory `TreeSnapshot` assembled from directory
identities recorded at creation and file versions taken from completed open
writers. The clipboard pipeline carries the receipt into ordinary paste and
both directory/file merge branches. Forward undo replay also records a new
receipt, including after redo. A later path-only snapshot is not treated as
ownership evidence for outputs created by another process.

Copy undo first compares the complete target tree with its receipt. Changes to
regular-file identity/version, missing or renamed entries, new children, or
unverifiable ownership cause an error. Removal rechecks each entry and its
recorded directory ancestors, removes only recorded files, and removes
directories only when empty. It never recursively deletes newly added children.
Errors include the affected root and note that remaining paths are retained.

Pure-copy batches preflight every target before removing any member. This
prevents an already edited early copy from causing removal of an unchanged
later copy followed by unsuccessful compensation from a missing source.
Mixed batches cannot use that simple preflight: reverse moves may first need
to restore a copy target to its recorded location. They retain per-action checks.

GIO still owns its copy writers. Untracked GIO outputs are not adopted by a
fresh path lookup. Copy undo refuses them explicitly and retains the target;
partial directory receipts also fail the full-tree comparison. Successful copy
itself is not reported as failed simply because safe undo evidence is unavailable.

## Regression Coverage

Eight new test functions cover:

- edited/replaced copied files and preservation of failed-undo history;
- edited, added, removed and renamed children of copied trees;
- edits, foreign children and directory replacements injected after the full
  undo scan, before per-entry removal;
- refusal to undo a copy without owned-writer evidence;
- preflight preservation of unchanged pure-copy batch members when a peer was
  edited and an original source has disappeared;
- cancellation rollback of completed file/directory copies modified by others;
- both merge branches retaining edited output and the pre-overwrite backup;
- successful file/directory paste undo/redo with renewed receipts, followed by
  refusal to delete an output edited after redo.

Tests use unique temporary fixtures and deterministic thread-local hooks. No
real devices, personal files, sleeps, disk filling or media disconnection are used.

## Verification

- `CARGO_NET_OFFLINE=true bash scripts/maintenance/test-backend.sh` passed
  PDFium/vendor integrity, four dependency-policy tests, formatting, cargo
  check, Clippy with warnings denied, the typed-error guard and backend tests.
  Semgrep is unavailable; advisory/blocking runs were skipped, not passed.
- Final `cargo fmt --all -- --check`, offline/locked all-target/all-feature
  Clippy with `-D warnings`, and the complete backend suite passed after the
  last code edit: **573 passed**, three opt-in native tests ignored.
- All **64 copy-filtered backend tests** passed, including the eight new
  regressions and existing cancellation, overwrite, merge and fallback checks.
- Frontend lint/naming and Svelte/TypeScript checks passed with zero Svelte
  errors/warnings. All **275 unit tests in 37 files**, **55 browser tests** and
  the production frontend build passed. Browser tests use mock IPC, not native
  filesystem/device acceptance.
- `git diff --check`, all **20 strict documentation consistency checks** and
  all **21 checked local documentation links** passed before committing.

## Remaining Boundaries

- Receipts are metadata evidence, not content hashes or filesystem locks. An
  edit during active writing or before receipt capture can become part of the
  recorded final version.
  Coarse/restored timestamps and the interval between the last per-entry check
  and unlink remain unresolved consistency boundaries. Directory ownership is
  captured after creation, not atomically with directory creation.
- A late foreign child is preserved, but unchanged recorded children may already
  have been removed before an empty-directory removal fails. Failed undo keeps
  its history item, not an automatic resumable transaction.
- Copy redo and mixed-batch compensation still reread original sources. They
  do not preserve the original copied bytes for restoration after partial undo.
  Recoverable backup design, backup faults and cross-filesystem behavior remain
  the next explicit safety task; pure-copy preflight is not a general solution.
- Receipts use entry-proportional history memory. Undo scans target metadata;
  pure-copy batch preflight adds an extra scan. Large trees and slow media need
  measurement; no responsiveness improvement is claimed.
- No new native GIO/MTP, physical-device, Windows or Fedora/Ubuntu runtime
  acceptance is claimed. Frontend behavior is unchanged in this increment.

The [undo scope](../../operations/linux-release/undo-scope.md) now documents these
limits. Version remains 1.0.3. No dependency update, publication, installation or
restart is part of this increment; the installed `47fde96` build is left unchanged.
