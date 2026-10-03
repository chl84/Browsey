# Copy Version Validation

Date: 2026-10-03
Baseline: `c95b0d7`
Track: [Daily-driver completeness plan](../../todo/TODO_DAILY_DRIVER_COMPLETENESS.md)
Status: Streaming-source and post-write validation implemented; broader Priority 0
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
- Source deletion after fallback copy still needs destination receipt rechecking
  and per-entry source-version checks. Final check-to-unlink races remain even
  with these checks; atomic removal requires a separate native design review.
- Coarse/restored timestamps and non-Unix timestamp behavior are not equivalent
  to content verification. Power-loss, installed application, removable-media
  and cross-distribution acceptance remain separate.

Version stays 1.0.3. No dependency change, installation, restart or release is
part of this increment. Both narrower TODO rows were checked after verification;
the broad active-write/race row stays open.
