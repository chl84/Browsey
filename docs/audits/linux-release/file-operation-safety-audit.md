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
- `frontend/src/features/explorer/modals/cloudBackgroundRefresh.test.ts`
- `src/clipboard/mod.rs`
- `src/clipboard/tests.rs`
- `src/commands/transfer/preview.rs`
- `frontend/src/features/explorer/file-ops/useExplorerFileOps.test.ts`
- `docs/operations/core-operations/release-checklist.md`
- `docs/operations/core-operations/release-blocking-policy.md`
- `docs/audits/core-operations/local-gap-audit.md`
- `docs/audits/core-operations/mixed-gap-audit.md`

## Step 3 Status Summary

| TODO item | Current status | Basis |
|---|---|---|
| Verify destructive operations always have correct guardrails | Verified for Linux 1.0 claim | Default permanent-delete confirmation is enabled, move-to-trash is routed separately from permanent delete, and destructive `LTD` release rows are already classified as release-blocking. |
| Verify conflict preview always matches the real operation | Partial | Local, mixed, and cloud preview/execute flows have meaningful coverage, but existing gap audits still call out partial coverage and missing hostile-condition variants. |
| Test aborted copy/move/extract flows for partial outputs and recovery | Open | Core docs and tests show some rollback/cancel behavior, but explicit Linux 1.0 closeout is not complete. |
| Verify undo/redo boundaries and document the actual supported scope | Verified | Locked by `docs/operations/linux-release/undo-scope.md`. |
| Ensure errors never leave the UI in an unknown state without a clear recovery path | Partial | Some cloud refresh soft-fail flows are covered, but this is not yet broad enough to claim across Linux-critical operations. |

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
- extract release docs explicitly define non-transactional boundaries
- mixed transfer docs/tests already track partial completion and refresh
  reconciliation

But the Linux 1.0 closeout is not finished because:

- local copy cancellation remains a named gap in the local audit
- mixed execute-phase cancellation during active work remains a named gap in the
  mixed audit
- extract cancel/failure filesystem-state validation still belongs to active
  release-checklist work

## Still Open: UI Recovery After Errors

This item remains open.

Current evidence is useful but incomplete:

- cloud rename/new-folder/delete modals already soft-fail refresh and tell the
  user to press `F5` when background reconciliation times out
- delete modal closes cleanly after success/failure handling

What is still missing for Linux 1.0:

- broader verification across local destructive flows, search, extract, and
  other trust-sensitive operations
- an explicit Linux 1.0 rule that failure/partial states are always surfaced in
  a recoverable and non-ambiguous way
- broader release-checklist coverage for UI-state recovery after backend error

## Conclusion

Step 3 should currently be treated as:

- safe to check off:
  - destructive guardrails
  - undo/redo scope documentation
- still open:
  - conflict preview consistency
  - abort/partial-output recovery
  - UI recovery after errors

That is the honest Linux 1.0 state based on the current code and audit trail.
