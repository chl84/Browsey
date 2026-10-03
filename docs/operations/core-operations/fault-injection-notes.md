# Core Operations Fault-Injection Notes

Created: 2026-03-02
Track reference: `docs/todo-archive/TODO_CORE_OPERATIONS_HARDENING.md` (Step 7)

## Goal

Use deterministic, seam-level fault injection for hostile-condition behavior
instead of timing-sensitive tests.

## Implemented Fault-Injection Coverage

Primary test seam:
- `src/commands/decompress/util.rs` (`copy_with_progress` and `CreatedPaths`)

Covered hostile conditions:
- permission denied:
  - `open_unique_file_reports_permission_denied_in_read_only_directory`
  - platform-scoped to Unix read-only semantics (`#[cfg(unix)]`)
- source disappearing during operation:
  - `copy_with_progress_surfaces_source_disappeared_error`
  - synthetic reader emits `io::ErrorKind::NotFound` after deterministic reads
- destination becoming unavailable:
  - `copy_with_progress_surfaces_destination_unavailable_error`
  - synthetic writer emits `io::ErrorKind::BrokenPipe` on deterministic write
- backend cancellation while work is in progress:
  - `copy_with_progress_stops_when_cancel_is_triggered_during_large_copy`
  - cancellation token flips mid-stream via deterministic read counter

## Design Constraints Applied

- Explicit fault injection only (custom reader/writer and deterministic counters).
- No sleep/race-based assertions.
- Platform-specific behavior isolated where OS semantics differ.

## Local Transfer Follow-Up (2026-10-03)

Shared test-only seam: `src/fs_utils/copy_test_hooks.rs`. Thread-local RAII
scopes wrap real fixture files, force short writes, inject read/write/sync
errors, and enter the normal move fallback through an unsupported-rename error.
No runtime configuration or dependency is added.

Coverage in `src/clipboard/tests.rs` and `src/undo/tests.rs` includes injected
storage-full, missing-source read, disconnected-destination write and final
writeback failures; actual Unix unlink/replacement of open paths; cancellation
mid-stream and after sync; exclusive destination creation and scope isolation.
Inspect source/target/unrelated contents, not just the returned error.

See the [local-transfer report](../../audits/daily-driver/local-transfer-fault-injection.md)
for per-operation partial-output policies and remaining native/media/race limits.

The test-only `CopyUndoVerified` phase runs after a copy receipt's full tree
scan and before per-entry removal. Tests add a foreign child, edit a file or
replace a copied directory at this exact boundary. Copy undo must retain these
paths and never recursively delete new contents. Clipboard completion hooks
also mutate completed outputs before cancellation triggers batch rollback.
See the [copy undo ownership report](../../audits/daily-driver/copy-undo-ownership.md).

Copy recovery adds `CopyUndoEntry` immediately before individual removal and
`RecoveryMarker` before creating the protection marker. Disposable tests inject
a failure after one child was removed, write/sync failure while preparing or
restoring a backup, marker creation failure and target edits during backup.
Subprocess tests verify cleanup after a recovery-session owner is killed.
The opt-in Linux `copy_recovery_roundtrip_across_disposable_filesystems` test
uses distinct `/tmp` and `/dev/shm` filesystems and requires explicit invocation
with `--ignored`; it is not physical removable-media acceptance.
See the [copy recovery report](../../audits/daily-driver/copy-recovery-backups.md).

Streaming-copy fixtures also rewrite/truncate/append to the source at a read
boundary and edit the same target inode at `Sync`/`Synced`. Verify source
preservation, explicit refusal of mixed-version success and preservation of
uncertain output/foreign edits on finalization errors. These are metadata-version
checks, not hashing, locks or atomic snapshots.
See the [copy version report](../../audits/daily-driver/copy-version-validation.md).

`BeforeSourceDelete` is a test-only boundary after fallback copying and before
the final move gate. Fixtures mutate source/destination files and children there;
the move must refuse unsafe deletion. Recorded source removal reuses the
`CopyUndoVerified`/`CopyUndoEntry` seams: a child added after scanning must survive,
an edited remaining file must stop removal, and clipboard cancellation after an
earlier removal must preserve all destination data and report partial source state.
An opaque-writer fixture exercises the real completion gate with no output
receipt; it does not simulate a live GIO/MTP device.
