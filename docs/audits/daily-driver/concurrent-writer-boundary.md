# Concurrent Writer Support Boundary

Date: 2026-10-03.
Reviewed source baseline: `c73bf2c`; the accompanying frontend extraction fix
does not change these filesystem paths.
Scope: local manual clipboard/history copies, copy/delete move fallback and
receipt-based removal. This is a design review, not an atomicity improvement.

## Decision

The supported completion/recovery claim requires a stable source, destination
and parent namespace throughout copying, moving and undo/redo. Stop other
programs writing, replacing or renaming those entries before starting. Detection
of concurrent edits is a conservative guard, not permission to operate on an
actively changing tree. A progress value of 100 percent does not lock the files.

Retain the existing checks and explicit inspection guidance. Do not add automatic
retry, delete uncertain output, or silently adopt changed data into a receipt.
The smallest justified change here is an explicit support boundary in the README
and undo scope, backed by installed native observations and existing deterministic
tests. A fully coordinated/transactional move requires a separate design.

This closes the engineering TODO to **review and document** the remaining windows.
Acceptance A0-7 remains partial: it includes races and platforms not established
by this run. It does not mean the races below have been eliminated.

## What the Current Checks Establish

The [content-verification audit](copy-content-verification.md) explains the two
local copy engines. Both compare a BLAKE3 digest of the written stream with a
bounded readback through the still-open destination handle. Path and handle
versions are checked around readback; the source version/length is checked after
streaming and readback. Detected differences refuse completion and fallback
source deletion, retaining uncertain output for inspection.

[FileState](../../../src/fs_utils/file_state.rs) contains identity, length,
modification time and, on Unix, change time. It is metadata, not a stored content
hash or filesystem lock. Copy receipts record created directory identities and
completed regular-file versions, rather than adopting a fresh target lookup.

[Clipboard fallback](../../../src/clipboard/ops.rs) and
[history fallback](../../../src/undo/path_ops.rs) verify the completed destination
receipt before removing recorded source entries. Removal checks recorded parent
identities and each entry again, removes children before parents and uses only
empty-directory removal. An added foreign child therefore survives, but an
error can leave the source partially removed. The destination is retained.

## Remaining Windows

| Boundary | Existing protection | What it cannot promise |
| --- | --- | --- |
| Stream/readback/version checks | Detect observed content, length, version and identity changes | An atomic point-in-time source/output snapshot or all changes on every filesystem |
| Destination receipt before source removal | Refuse an already changed or unverifiable output | Freeze the destination while a long source-tree removal continues |
| Per-entry check before unlink/rmdir | Refuse observed source/parent changes; never recursively remove foreign children | Make the final check and removal indivisible against another process |
| Same-filesystem no-replace rename | Do not clobber an occupied destination through the native rename path | Stop a program with an open handle from continuing to edit the moved inode |

Readback can itself race with a writer. Metadata resolution differs by filesystem,
and timestamps alone are not content identity. A writer can change an entry after
the final successful check; path-based removal also has a check-to-use window if
the namespace changes then. Repeating another check narrows a window but cannot
remove it. The current checks are not power-loss durability or atomic batch undo.

An application-local mutex coordinates only Browsey's own participating work.
Advisory locks do not stop uncooperative writers. Private staging could reduce
exposure of an in-progress destination, but does not by itself close source
check-to-unlink or post-publication edits. It would also change output visibility,
publication portability, cancellation and recovery behavior. No new staging or
locking architecture is introduced without reviewing those tradeoffs.

## Evidence and Follow-Up

The [native error/writer run](../../operations/linux-release/runs/2026-10-03-a0-errors-and-writers.md)
used a separate real process to edit an already-written destination prefix and
then an active source prefix. The installed copy engine rejected both outcomes,
retained the source and uncertain output, and did not retry automatically. A
fixture-scoped write-delay shim made these windows observable; this is not slow
USB/network acceptance. Native same-filesystem moves and overwrite undo were
also tested, but native cross-device fallback races were not.

Existing deterministic cases in [clipboard tests](../../../src/clipboard/tests.rs)
and [undo tests](../../../src/undo/tests.rs) cover edits before fallback deletion,
changes after tree preflight, individual source-entry changes, added foreign
children, replaced directories and cancellation during partial removal. They
passed in the full backend suite. Their injected phase checks are not native
proof of arbitrary timing after the last per-entry check.

Before claiming active-writer-safe destructive moves, agree a coordination or
snapshot/publication design and test real cross-device paths, namespace changes
and the remaining final-check windows. Until then, stop competing writers and
inspect both reported paths after any uncertain or partial result; do not
blindly repeat the operation. See [undo scope](../../operations/linux-release/undo-scope.md).
