# Local Transfer Fault Injection

Date: 2026-10-03
Baseline: `12e6a35` (the preceding file-safety batch).
Track: [Daily-driver completeness plan](../../todo/TODO_DAILY_DRIVER_COMPLETENESS.md)
Status: Follow-up implementation; final verification results recorded below.

## Scope

Exercise the real local clipboard copy/move and undo copy/move error handlers
with disposable files. A thread-local, RAII-scoped test hook wraps actual file
reads/writes and finalization. Hooks are excluded from application builds and
reset when their scope exits. Short writes exercise `write_all` and `io::copy`;
faults occur after some data has actually reached the test destination.

Rename reports an injected unsupported-atomic-rename error to enter the existing
cross-device/unsupported-rename fallback. This proves fallback behavior, not a
physical cross-device mount or a kernel disk-full condition. The test undo copy
uses generic `io::copy` with wrapped files; it does not exercise the production
kernel-offloaded `File` copy optimization under a real media fault.

No disk was filled, physical media disconnected, personal file changed, or
application installed. Windows runtime and native GVfs/GIO are not validated.

## Coverage and Expected Outcomes

| Scenario | Clipboard copy and forced move fallback | Undo copy and forced move fallback |
|---|---|---|
| Mid-stream storage-full write error | Error, source intact, owned partial file removed | Error, source intact, partial target retained and reported |
| Mid-stream source-read `NotFound` | Error, source intact, owned partial file removed | Error, source intact, partial target retained and reported |
| Mid-stream destination-write `BrokenPipe` | Error, source intact, owned partial file removed | Error, source intact, partial target retained and reported |
| Final storage-full/writeback error | Error, source intact, owned target removed | Error, source intact, target retained and reported |
| Open source unlinked by fixture, Unix | Reads continue through open handle; move returns missing-source error and retains complete destination | Same |
| Open target unlinked by fixture, Unix | Error after finalization; source retained | Same |
| Target replaced after sync, Unix | Error; source and competing file retained | Same |
| Target replaced before an injected sync failure, Unix | Error cleanup preserves competing file and renamed output | Undo does not automatically clean targets on copy failure |
| Cancellation mid-stream | Source intact, owned partial target removed | Undo fallback has no cancellation token; not claimed |
| Cancellation after successful sync | Both complete copies retained; cancellation reports destination path | Not claimed |

Unrelated fixture files are checked byte-for-byte in the fault matrix. A separate
test verifies that exclusive target creation rejects an existing file before
the I/O hook and that a dropped hook does not affect the next normal copy.

## New Defects and Corrections

1. **Synced but no longer reachable target.** Both move fallbacks could delete
   the source after successful writes/sync to a destination inode already
   unlinked by another process. Both Unix regression tests failed before the
   correction. Check that the destination path still identifies the open output
   before reporting a completed copy or permitting fallback source deletion.
2. **Replaced destination accepted as successful delivery.** A clipboard move
   regression also failed when the completed output was renamed elsewhere and
   a competing file appeared at the destination. Stable identity checks now
   reject that completion, keeping the source and competing file.
3. **Cleanup trusted initial exclusive creation indefinitely.** Code review
   found unconditional partial-file unlink on copy errors. A replacement is
   not owned by Browsey's copy. Check ownership before error cleanup, retain
   unverifiable/replaced outputs, and add the sync-failure replacement case.

The archive output identity guard is promoted to shared `fs_utils::FileIdentity`,
with capture from an open file handle as well as from a path. Extraction retains
its existing alias and conservative cleanup policy. Unix uses device/inode;
Windows retains the existing volume/file-index guard and rejects reparse points
or unavailable IDs. No size/mtime heuristic is introduced for output ownership.

Undo failure messages now explicitly state that the source is retained and a
partial or completed destination may remain. Automatic deletion/retry would be
unsafe; inspect both paths before any recovery action.

## Verification

- `CARGO_NET_OFFLINE=true bash scripts/maintenance/test-backend.sh`
  passed bundled PDFium/vendor integrity, four dependency-policy tests, formatting,
  all-target/all-feature cargo check, Clippy with warnings denied, typed-error guard,
  and **557 backend tests**. Three opt-in native tests were ignored; no fresh
  native acceptance is claimed. Eleven new test functions cover this batch.
- Semgrep is not installed; advisory and blocking Semgrep runs were skipped,
  not passed. Existing advisory typed-error conversions in USB/network code are
  unchanged and outside this batch.
- `cargo test --offline --locked faults_ -- --nocapture` passed three selected
  tests, including both copy/move fault matrices, after the short-write seam update.
- `git diff --check` and the strict documentation consistency gate passed
  (20 checks). All 18 checked local documentation references resolved.
- Frontend code is unchanged. Its preceding 271-unit/55-browser results are
  historical evidence for the prior batch, not rerun results for this batch.

## Remaining Boundaries

- Identity checks narrow replacement races; they do not make path-based unlink
  or copy/delete transactions atomic against a change after the final check.
- Nested-directory cleanup, concurrent content edits on the same inode, and
  directory durability still require separate work. Do not extend these file
  assertions to entire mutable trees.
  Update: [nested-copy follow-up](nested-copy-recovery.md) covers failed-copy
  cleanup and metadata-based source changes; transaction/durability limits remain.
- Device disconnection and full-disk behavior remain injected OS-error semantics,
  not physical-media acceptance. Native GIO/MTP and other platform validation
  remain open in the parent plan.
- UI partial-state feedback is the next separate validation task. These tests
  establish backend messages and filesystem state, not fresh installed UI behavior.
  Update: the same follow-up adds mocked local UI reconciliation; installed
  UI/device acceptance is still open.

Commit/push is authorized for this batch. Publication, installation, and real
device formatting are outside this run.
