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
