# Linux File-Operation Safety Audit

Created: 2026-03-06
Track: `docs/todo/TODO_PRODUCTION_READY_LINUX.md`
Scope: Step 3 (`Tighten file operation safety`) for the Linux 1.0 track.

## Purpose

Review which Step 3 safety claims are already supported by the current code and
evidence base, and which ones still remain open. This audit is intentionally
strict: an item is only treated as verified when the current implementation and
existing automated/manual release evidence support a concrete Linux 1.0 claim.

## Evidence Reviewed

- `frontend/src/features/settings/settingsTypes.ts`
- `frontend/src/features/settings/sections/GeneralSection.svelte`
- `frontend/src/features/explorer/context/createContextActions.ts`
- `frontend/src/features/explorer/context/createContextActions.test.ts`
- `frontend/src/features/explorer/modals/deleteConfirmModal.ts`
- `frontend/src/features/explorer/modals/advancedRenameModal.ts`
- `frontend/src/features/explorer/modals/cloudBackgroundRefresh.test.ts`
- `frontend/src/features/explorer/file-ops/useExplorerFileOps.test.ts`
- `frontend/src/features/explorer/context/createContextActions.test.ts`
- `frontend/src/features/explorer/state/createSearchSession.test.ts`
- `src/clipboard/mod.rs`
- `src/clipboard/tests.rs`
- `src/commands/decompress/mod.rs`
- `src/commands/decompress/tar_format.rs`
- `src/commands/transfer/preview.rs`
- `frontend/src/features/explorer/file-ops/useExplorerFileOps.test.ts`
- `docs/operations/core-operations/release-checklist.md`
- `docs/operations/core-operations/release-blocking-policy.md`
- `docs/audits/core-operations/extract-gap-audit.md`
- `docs/audits/core-operations/local-gap-audit.md`
- `docs/audits/core-operations/mixed-gap-audit.md`

## Step 3 Status Summary

| TODO item | Current status | Basis |
|---|---|---|
| Verify destructive operations always have correct guardrails | Verified for Linux 1.0 claim | Default permanent-delete confirmation is enabled, move-to-trash is routed separately from permanent delete, and destructive `LTD` release rows are already classified as release-blocking. |
| Verify conflict preview always matches the real operation | Partial | Local, mixed, and cloud preview/execute flows have meaningful coverage, but existing gap audits still call out partial coverage and missing hostile-condition variants. |
| Test aborted copy/move/extract flows for partial outputs and recovery | Open | Core docs and tests show some rollback/cancel behavior, but explicit Linux 1.0 closeout is not complete. |
| Verify undo/redo boundaries and document the actual supported scope | Verified | Locked by `docs/operations/linux-release/undo-scope.md`. |
| Ensure errors never leave the UI in an unknown state without a clear recovery path | Verified for Linux 1.0 claim | Automated coverage now spans delete, new-folder, rename, advanced rename, extract, properties, trash restore/purge, and search recovery, and Linux bugbash rows explicitly require sane failure-state recovery on the supported target surface. |

## Verified: Destructive Operation Guardrails

For the Linux 1.0 claim, the current destructive-operation guardrails are
strong enough to count as verified:

- `confirmDelete` defaults to `true`, so permanent delete is confirmation-gated
  by default.
- Settings exposes this as an explicit user preference: `Ask before permanent
  delete`.
- Explorer action routing keeps `move to trash` separate from `delete
  permanently`; the trash path does not silently fall through to permanent
  delete.
- When confirmation is enabled, permanent delete routes through the explicit
  delete-confirm modal rather than executing immediately.
- Core release policy already treats all local trash/delete/restore/purge
  scenarios (`LTD`) as release-blocking trust-sensitive behavior.

This is sufficient to support the Linux 1.0 guardrail claim:

- permanent delete is intentionally separate from move-to-trash
- permanent delete is confirmation-gated by default
- destructive local delete/trash flows remain release-blocking in candidate
  validation

### Guardrail Boundary

This verification does not mean every destructive edge case is fully closed.
Restore/purge/backend-hostile-condition gaps still exist in the broader local
gap audit and must continue to be tracked separately. The claim closed here is
specifically the presence of correct destructive-operation guardrails, not
complete bug closure for every destructive-path failure mode.

## Still Open: Conflict Preview Consistency

Conflict preview is not ready to be closed yet.

What is already strong:

- local clipboard preview enumerates existing destination conflicts before
  execute
- local clipboard preview now has direct backend tests for file conflict,
  directory conflict, and non-conflicting filtering behavior
- local clipboard tests assert no-overwrite semantics for copy and move
- mixed local<->cloud preview has direct preview/execute alignment tests
- cloud paste UI tests cover rename-on-conflict and overwrite intent plumbing

Why this remains open:

- the local gap audit still marks key local copy/move and rename conflict rows
  as partial
- the mixed gap audit still marks `CO-MTC-006` as partial, mainly around
  hostile-condition execute coverage
- extraction conflict behavior still has partial coverage in the extract audit

Linux 1.0 should therefore keep this item open until preview-vs-execute
consistency is closed across the trust-critical conflict families, not just the
best-covered flows.

## Still Open: Abort/Partial Output Recovery

This item remains open.

There is real existing evidence:

- local delete batch cancellation rolls back completed items
- local delete batch now has direct permission-denied coverage that keeps the
  source in place and reports a recoverable failure before any progress is
  emitted
- local multi-item clipboard paste now has direct rollback coverage when a
  later source disappears after clipboard capture, for both copy and cut
  paths
- local multi-item clipboard copy now also has direct mid-batch cancellation
  coverage that rolls back already created targets before returning
  `cancelled`
- local multi-item clipboard cut now also has direct mid-batch cancellation
  coverage that restores already moved sources and removes created targets
  before returning `cancelled`
- local overwrite-merge directory copy and cut now also have direct recursive
  cancellation rollback coverage after the first merged item
- local overwrite-merge directory copy and cut now also have direct recursive
  rollback coverage when a later merged source disappears mid-operation
- mixed local<->cloud copy and move now have direct execute-phase cancellation
  coverage while the second transfer command is actively running, in both
  directions
- mixed local<->cloud progress-aware copy loops now also have direct
  execute-phase cancellation coverage while the second transfer command is
  actively running, in both directions
- mixed local<->cloud progress-aware move loops now also have direct
  execute-phase cancellation coverage while the second transfer command is
  actively running, in both directions
- mixed local<->cloud directory loops now also have direct execute-phase
  cancellation coverage while the second transfer command is actively running,
  in both directions for both copy and move
- local copy cancellation now has direct backend coverage for both file cleanup
  and directory-destination cleanup
- zip extraction now has direct archive-level cancellation + rollback coverage
  for a partially written entry
- tar extraction now also has direct archive-level cancellation + rollback
  coverage for a partially written entry
- `do_extract` now also has direct real-archive success-path and archive-root
  conflict-path coverage
- `do_extract` now has direct cancellation + rollback coverage with a real
  archive, and batch extract helper behavior is covered with real archive
  success/failure/cancel inputs
- the blocking batch extract entrypoint now also has direct real-archive
  continuation and cancel-stop coverage
- extract release docs explicitly define non-transactional boundaries
- mixed transfer docs/tests already track partial completion and refresh
  reconciliation

But the Linux 1.0 closeout is not finished because:

- local multi-item cancellation/summary coverage is stronger, including
  recursive merge-cancel rollback and recursive later-source-failure rollback,
  but it is still narrower than the full trust-sensitive matrix
- mixed execute-phase cancellation coverage is stronger, but the mixed audit
  now keeps mainly broader partial-summary/hostile-condition breadth open after
  the added direct directory-loop cancellation coverage
- extract cancel/failure filesystem-state validation is stronger, but broader
  conflict-path and format-confidence decisions still belong to active
  release-checklist work

## Verified: UI Recovery After Errors

This item is now strong enough to count as verified for the Linux 1.0 claim.

Current evidence covers the major trust-sensitive UI families:

- cloud rename/new-folder/delete modals already soft-fail refresh and tell the
  user to press `F5` when background reconciliation times out
- trash restore now has explicit conflict-path coverage that confirms
  destination-collision failures do not emit a false `trash-changed` signal
- trash restore/purge now also have explicit list-failure coverage that
  confirms backend enumeration failure does not emit a false `trash-changed`
  signal or pretend that work was attempted
- delete modal closes cleanly after success/failure handling
- delete modal failure now has explicit frontend coverage for toast + cleanup +
  closed-state recovery
- rename modal failure now has explicit frontend coverage for kept-open state,
  surfaced error text, and single-pass activity cleanup
- advanced rename modal failure now has explicit frontend coverage for both
  preview-failure and confirm-failure paths, with kept-open state and surfaced
  error text
- extract failure/cancel paths now have explicit frontend coverage for toast +
  activity cleanup + non-ambiguous recovery state
- new-folder modal failure now has explicit frontend coverage for kept-open
  state, surfaced error text, and single-pass activity cleanup
- trash restore/purge actions now have explicit frontend coverage for surfaced
  recovery toast + skipped reload when the backend restore/purge call fails
- search session setup/runtime failures now have explicit frontend coverage for
  surfaced error text + cleared loading/running state + cleared active cancel
  handles

This automated evidence is also backed by explicit Linux manual recovery rows
already present in the release bugbash:

- `LX-SRC-002` and `LX-SRC-003` for search error/cancel recovery
- `LX-TRH-002` for restore/purge conflict and failure recovery

That combination is enough to support the Linux 1.0 claim being made here:

- failure/cancel states in the major trust-sensitive UI flows are surfaced in a
  recoverable, non-ambiguous way
- the release process already requires Linux-target manual validation for the
  remaining user-facing recovery seams on supported desktops/sessions

## Conclusion

Step 3 should currently be treated as:

- safe to check off:
  - destructive guardrails
  - undo/redo scope documentation
- still open:
  - conflict preview consistency
  - abort/partial-output recovery

That is the honest Linux 1.0 state based on the current code and audit trail.
