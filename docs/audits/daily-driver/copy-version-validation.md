# Copy Version Validation

Date: 2026-10-03
Baseline: `c95b0d7`
Track: [Daily-driver completeness plan](../../todo/TODO_DAILY_DRIVER_COMPLETENESS.md)
Status: Streaming-source, post-write and fallback-removal validation implemented; broader Priority 0
and concurrency acceptance remain open.

## Confirmed Reproductions

Four new fixture tests failed on the baseline:

- clipboard and undo plain copies reported success when the source was rewritten
  during streaming, potentially combining different source versions;
- clipboard and undo forced fallback moves accepted an in-place edit of their
  target during finalization. Inode identity alone did not reject the edit before
  the move continued to source deletion;
- clipboard finalization-error cleanup also used only inode identity, allowing
  an in-place edit to be removed if sync failed. This case is included in the
  corrected target-edit regression's parameter combinations.

All fixtures use unique temporary paths. Tests force deterministic short reads
and writes through the existing thread-local hooks; no personal files or real
devices are used.

## First Verified Increment

Both manual local copy engines record a source version from the opened reader
before reading contents. After output writeback, they require the named source
and open descriptor to match that version and the copied byte count to equal
its original length. Rewrites, truncation, appends and disappearance cause an
explicit error, not success or fallback source deletion.

The engines also record the completed writer's version after writing/applying
permissions but before sync, then compare it with the named target and open
writer after sync. The receipt carries that already checked version; it never
adopts a fresh version after verification. This detects edits during that window.

Clipboard error cleanup keeps finalized outputs when source verification fails
or the target's pre-sync version cannot be verified. It also checks that version
on sync-error cleanup, preserving another writer's edits instead of unlinking
them. Unchanged owned partial output still follows the existing cleanup policy.
The undo engine retains its existing explicit partial-output policy.

Uncertain outputs may contain mixed versions or incomplete data. They are kept
for inspection, not described as trustworthy backups or recorded as successful
undoable copies. File work is never automatically retried. GIO-owned copy writers
are outside this manual-copy increment.

## Verification at the First Checkpoint

- All four initially failing copy/move regressions passed after the correction.
- Two shared file-state tests verify path/handle identity, copied length,
  disappearance and Unix same-length edits with restored modification time.
  Unix ctime adds change evidence; it is not a content hash.
- The backend maintenance pipeline passed resource/vendor integrity, four
  dependency-policy tests, formatting, cargo check, warnings-denied Clippy,
  typed-error guards and **599 tests**. Four native opt-in tests remain ignored.
- Semgrep is absent; its checks were skipped, not passed.

## Open Boundaries

- A source can change after its final verification; there is no exclusive lock
  or filesystem transaction. A whole directory copy is not an atomic snapshot.
- A foreign target edit before the pre-sync version is captured can still be
  masked by subsequent writes and adopted into that snapshot. Mid-write failure
  cleanup before a finalization snapshot still has its prior identity-only
  boundary. The new guards do not claim to solve these windows.
- At the first checkpoint, fallback source deletion still needed destination
  receipt rechecking and per-entry checks. The follow-up below adds these checks;
  final check-to-unlink races still require a separate native design review.
- Coarse/restored timestamps and non-Unix timestamp behavior are not equivalent
  to content verification. Power-loss, installed application, removable-media
  and cross-distribution acceptance remain separate.

Version stays 1.0.3. No dependency change, installation, restart or release is
part of this increment. Both narrower TODO rows were checked after verification;
the broad active-write/race row stays open.

## Second Verified Increment: Fallback Move Completion

The first correction was committed as `87a401a`, with two verified TODO rows
checked before this increment. Three new tests then failed against that checkpoint:
the clipboard/undo move accepted a target edit after copying but before source
deletion, and clipboard removal could not honor cancellation between source
entries because it still used whole-tree deletion. These are deterministic
test-only boundaries, not timing or physical-device tests.

Fallback copying now carries its completed destination receipt into a final
verification gate. Missing/unverifiable receipts, changed output versions or
changed child sets prevent source deletion and leave the destination untouched.
Clipboard destination inspection is cancel-aware rather than an uninterruptible
new scan. Ordinary native no-replace rename is unchanged.

Both local fallback engines use the same recorded-entry removal algorithm as
conservative copy undo, now named `remove_recorded`. It compares the whole source
snapshot, rechecks ancestor identities and each leaf before removal, and only
unlinks recorded files/removes empty recorded directories. A child added after
the scan cannot be recursively deleted. An edited remaining file stops removal,
even when an earlier file was already removed. Clipboard cancellation is checked
at each removal boundary. Undo history still has no new cancel/progress API.

Partial removal retains destination contents and reports remaining source paths
for inspection. This is not rollback or automatic recovery: earlier source entries
may already be gone. No operation is automatically repeated. A destination can
still change after its final gate, including during a long source-removal phase;
final per-entry check-to-unlink races are not closed.

GIO-owned writers provide no safe output ownership receipt. A fallback move using
such a writer now refuses source deletion even if copying returned success.
This deliberately favors retaining both paths over deleting an unverified source.
Use copy and inspect its result; automatic move/undo parity for GIO writers needs
its own design and native acceptance. The ordinary GIO copy path is unchanged.

Four new test functions cover late output edits/additions, late source edits/
additions before scanning and between individual removals, preserved children
after the scan, cancellation after one source removal and the real completion
gate with an opaque output receipt. The opaque fixture is not a live GIO device.

Final backend maintenance checks passed resource/vendor integrity, dependency
policy, formatting, cargo check, Clippy with warnings denied, typed-error guards
and **603 tests**, with four native opt-in tests ignored. Semgrep remains absent
and skipped. All **296 frontend unit tests in 40 files** also passed; frontend
source and IPC command signatures were not changed, and the browser suite was
not rerun for this backend-only increment.

The opt-in Linux cross-filesystem copy recovery test was also explicitly rerun
and passed for files and nested directories across distinct disposable `/tmp`
and `/dev/shm` filesystems. The other three opt-in native tests were not invoked;
this is not USB/MTP, unplugging, power-loss or installed-build acceptance.

The fallback completion and per-entry cancellation TODO rows were checked after
verification. Broad active-write/race, retention, installed-build and real-media
acceptance remain open. No installation, restart or dependency/version change.

`git diff --check`, all 20 strict documentation consistency checks and 37 local
documentation links passed before the second commit. README and MTP acceptance
notes describe the same conservative opaque-writer fallback restriction.
