# Active Copy Failure Retention

Date: 2026-10-03
Baseline: `23787e7`
Scope: local manual clipboard file copies and failure rollback, disposable fixtures.

## Reproduced Defect

The new `failed_streaming_copy_preserves_in_place_target_edits` test failed
against the baseline: a foreign write to the same destination inode followed by
a stream error resulted in the destination being deleted. Exclusive creation and
inode matching cannot justify this cleanup. Our own writes may mask another
writer's metadata changes, even before the finalization snapshot is taken.

## Correction and Behavioral Tradeoff

Local clipboard file streams no longer unlink their failed output. Read, write,
permission, finalization errors and mid-stream cancellation report an uncertain
retained path for inspection, or report a missing/replaced/unverifiable path
without pretending its contents survived. They never register a successful copy
receipt. Undo's local file-copy engine already retains failed outputs.

Cancellation checked before opening the destination creates no file. Completed
unchanged peers in a failed batch/directory still use their receipt-based rollback;
the currently failed unregistered output and its nonempty parents are retained.
This is deliberately conservative: even an uncontested disk-full error or
cancellation may leave a partial file. No automatic retry, purge, quota or
automatic repair is introduced. These outputs are outside undo storage metrics.

When an overwrite failed previously, removal of its partial output allowed the
old destination to be restored. Retaining uncertain output now blocks that
restoration. Clipboard failure rollback therefore marks all original overwrite
backups before starting any rollback action. An occupied destination is not
overwritten, and failure reports original backup paths for manual recovery.
Markers remain on failure; successful whole rollback clears them, with failed
marker clearing logged and conservatively retained. Existing startup cleanup
recognizes these same markers and preserves abandoned marked sessions.

If marker creation fails, rollback does not start. The error reports all original
backups and instructs manual recovery before closing Browsey/allowing startup
cleanup. A partially written marker is still conservatively recognized by
cleanup; an error before marker creation is not a retention guarantee. Protection
begins at failure rollback, not at the initial overwrite backup: a crash before
rollback begins remains an open boundary. Markers are diagnostic paths, not
persistent undo history or a restore plan.

## Verified Checkpoint

The 72 clipboard-related tests passed, including five new test functions:

- Six copy/forced-move read/write/cancellation cases preserve foreign in-place
  target edits and original source bytes.
- Copy and forced move overwrite failures preserve edited output, original
  backup and source, keep the clipboard and report blocked rollback paths.
- A nested rollback action with an occupied target pins its original backup;
  after its owner is dropped, a separate cleanup process retains the session.
- Successful restoration clears its marker and permits abandoned-session cleanup.
- Failure while protecting the second of two originals prevents any rollback
  action from starting and reports both backup paths. The earlier marker also
  retains the entire session across separate-process cleanup.

Existing fault tests now assert retention instead of automatic active-file
deletion. The isolated real Unix file-size-limit test retains the partial target
and original backup, with an explicit protection error for its partial marker.
Normal completed-peer rollback, overwrite undo and source preservation continue
to pass. The two narrow TODO rows were checked at this checkpoint; broader
acceptance remains unchecked.

Final backend maintenance gates passed PDFium/vendor integrity, dependency
policy, formatting, cargo check, warnings-denied Clippy, typed-error guards and
**608 tests**, with four opt-in native tests ignored. Semgrep is absent and was
skipped, not passed. The ignored tests were not explicitly invoked in this
increment; cross-filesystem/native acceptance is not claimed from this run.

All **296 frontend unit tests in 40 files** passed. Frontend source and IPC
signatures are unchanged; the browser suite was not rerun. All 20 strict
documentation consistency checks, 39 local documentation links and
`git diff --check` passed.

## Remaining Boundaries

Successful streaming can still adopt a foreign edit masked by later writes
before version capture. No private staging/publication, content hashing, file
lock or transaction has been added. Final check-to-unlink races in completed
receipt/source removal also remain open. GIO/cloud writers are unchanged.

Manual recovery UX, retained-output space budgets, installed UI behavior,
physical USB/MTP faults, power loss and cross-distribution acceptance have not
been validated here. No install, restart, release, dependency or version change.
