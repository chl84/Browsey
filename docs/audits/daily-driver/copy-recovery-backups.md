# Copy Recovery Backups

Date: 2026-10-03
Baseline: `290e924`
Track: [Daily-driver completeness plan](../../todo/TODO_DAILY_DRIVER_COMPLETENESS.md)
Status: Incremental implementation; overall Priority 0 remains open.

## Reproduction and Scope

A disposable regression failed against the preceding implementation: after
copy and undo, editing the original source made redo restore the new source
bytes instead of the copied bytes. A missing source also prevented redo.
Mixed-batch undo compensation used that same source-based replay after an
earlier action failed, risking failure or changed restored contents.

This increment preserves copied data for session redo and compensation. It
does not make batches atomic or persist undo history across restart.

## Backup and Replay

On first copy undo, Browsey verifies the completed-output receipt, writes a
unique private backup in the existing locked undo session, finalizes writes,
verifies the backup receipt and rechecks the target. Only then can conservative
per-entry target removal begin. Failed backup preparation does not remove the
target and reports any backup candidate left behind; an unverified candidate
is not cached as the original copied bytes.

The complete backup stays associated with the action and is reused across
undo/redo cycles. Redo and mixed-batch compensation copy from it without
overwriting a destination, recheck the backup, and record ownership/version
evidence for restored outputs. They never reread the original source. Changed
or missing backups and occupied targets cause explicit errors.

Copy-before-remove works across filesystems without relying on inode identity
surviving rename. It deliberately needs space and I/O for a full backup. On
disk-full/writeback/marker errors the unchanged target remains. A later removal
or restore failure can leave partial target contents, but retains and reports
the complete backup. No automatic destructive retry or partial-tree adoption is
performed. A manually inspected retry can skip an already undone copy member
when its target is absent and its backup remains verified.

Native OS filenames are retained in backup paths, including Unix non-UTF-8
names. The backup allocator no longer converts basenames lossily.

## Recovery Protection and Cleanup

Before destructive removal or restore, a uniquely named, exclusively created
`.recovery-required` file is written beside the backup bucket in the session.
It contains diagnostic destination/backup paths, not serialized undo state.
Marker ownership is checked before clearing it; a replacement is retained.
Markers remain through nested actions and compensation and are cleared only
after the entire top-level action or batch succeeds. Clear failures are logged
and conservatively retain the marker.

Startup cleanup still respects live-instance locks, but additionally keeps the
entire abandoned session if any recovery marker exists. Marker scan errors or
suspicious marker types fail closed. Failed/interrupted operations therefore
retain their recovery data beyond app restart without claiming persistent undo
or automatically resuming filesystem operations.

Manual recovery means inspecting the error's affected target and backup paths,
copying needed data to a verified safe location without overwriting uncertain
contents, and only then deciding whether the marked session can be discarded.
Do not remove a marker merely to reclaim space: ordinary abandoned-session
cleanup can delete the whole session after its last recovery marker is removed.
No automatic purge of protected sessions or new recovery browser is introduced.

## UI and Execution

Undo/redo filesystem work now runs on a blocking worker rather than the UI
event loop. The manager still serializes history. Shared history actions in
the explorer page suppress repeated undo and redo requests while work or its
refresh is pending. They refresh after either success or failure, retain backend
recovery guidance, and distinguish successful work from a failed listing refresh.
There is no automatic retry. This does not add progress/cancellation for history
or coordinate all other concurrent file operations.

## Regression Coverage

Fourteen new backend test functions (one opt-in) and five mocked frontend cases
cover:

- file/directory redo after original source edits/deletion, including repeated
  undo/redo cycles and refreshed output receipts;
- failed mixed-batch undo restoring the original copied bytes, and failed
  compensation retaining recovery bytes for a manually inspected retry;
- restore conflicts, partial writes and writeback faults preserving backup data;
- backup write/writeback/marker failures leaving targets unchanged;
- partial removal after a successful child deletion preserving a full backup;
- target edits during backup and changed backup refusal;
- lossless Unix backup basenames and asynchronous typed history errors;
- recovery flags preserving abandoned sessions across processes, marker
  replacements surviving cleanup, and a killed fixture owner;
- explicit file and nested-directory recovery between distinct Linux temporary
  filesystems, with different device IDs asserted;
- UI error reconciliation, combined errors, refresh-only failures and suppression
  of repeat undo/redo requests until refresh completes.

The existing clipboard file/directory roundtrip regression now expects preserved
copied bytes after source changes, instead of the old source-based behavior.

## Verification

- The backend maintenance pipeline passed resource/vendor integrity, four
  dependency-policy tests, formatting, cargo check, Clippy with warnings denied,
  the typed-error guard and the then-current backend suite. Semgrep is not
  installed; its advisory/blocking checks were skipped, not passed.
- After the final killed-owner fixture test, formatting, offline/locked
  all-target/all-feature Clippy with `-D warnings`, and the complete suite
  passed: **586 tests**, four opt-in tests ignored.
- The ignored Linux cross-filesystem test was then invoked explicitly and
  passed for regular files and nested directories. It asserts distinct device
  IDs for `/tmp` backup storage and `/dev/shm` targets; it is not physical-media
  acceptance. The other three opt-in native tests remain unexecuted here.
- Frontend lint/naming and Svelte/TypeScript checks passed with zero Svelte
  errors/warnings. All **280 unit tests in 38 files**, **55 browser tests** and
  the production frontend build passed. Five new unit cases cover history
  reconciliation/gating; browser tests use mocked IPC.
- `git diff --check`, all **20 strict documentation consistency checks** and
  all **27 checked local documentation references** passed before committing.

## Remaining Limits

- Metadata receipts are not hashes or locks. Source edits during writing,
  coarse/restored timestamps, mutations after the final checks and power-loss
  durability remain separate validation/design work. Marker and file syncing
  do not establish transactional directory-entry durability.
- A failed partial tree is not automatically resumed or repaired. Recovery
  data survives marked-session cleanup, but history remains in-memory and
  limited to 50 actions. Protection deliberately retains the whole session,
  including other backups, and is not a storage quota.
- First undo writes a full copy; receipts and backups add entry-proportional
  memory and disk space. Very large trees, slow media, history progress and
  cancellation, quotas and manual recovery UX require measurement/acceptance.
- The cross-filesystem test uses disposable RAM-backed mounts, not USB/MTP,
  unplugging, real disk filling or power failure. No Windows, Fedora/Ubuntu or
  installed-build acceptance is claimed. Browser tests use mocked IPC.
- GIO-owned copies without safe output receipts remain conservatively refused
  for undo; this increment does not expand their undo support.
- Protection requires a build with marker-aware startup cleanup. Older Browsey
  builds may not recognize recovery markers; recover data before downgrading
  or launching an older build against the same abandoned-session storage.

See the [undo scope](../../operations/linux-release/undo-scope.md) for current
claims. Version remains 1.0.3; no dependency update, release, installation or
restart is part of this increment. Commit/push follows the requested workflow.
