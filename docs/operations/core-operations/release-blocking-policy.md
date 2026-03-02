# Core Operations Release-Blocking Policy

Created: 2026-03-02
Scope: Trust-critical operation regressions for
`docs/todo-archive/TODO_CORE_OPERATIONS_HARDENING.md` Step 8.
Source of truth for behavior: `docs/operations/core-operations/matrix.md`.

## Purpose

Define a small, operational release gate for trust-critical operation behavior.
This policy is intentionally strict for destructive/recovery-sensitive flows.

## Blocking Rule

A release is blocked if any scenario marked `Release-Blocking` below fails in
`docs/operations/core-operations/release-checklist.md` without a merged fix.

If a blocking scenario is `N/A`, the run notes must include a concrete scope
reason.

## Scenario Classification

### Release-Blocking

- All `LTD` scenarios (`CO-LTD-001` … `CO-LTD-005`)
- All `MTC` scenarios (`CO-MTC-001` … `CO-MTC-006`)
- `CO-LCM-003`, `CO-LCM-004` (move semantics)
- `CO-LRN-003` (rename conflict/no-overwrite semantics)
- `CO-EXT-003`, `CO-EXT-004`, `CO-EXT-005`

Rationale:
- These scenarios can cause silent data loss, ambiguous partial state, or
  incorrect user trust signals after failure/cancel.

### Non-Blocking With Follow-Up Required

- `CO-LCM-001`, `CO-LCM-002`
- `CO-LRN-001`, `CO-LRN-002`
- `CO-EXT-001`, `CO-EXT-002`

Rule:
- A failure requires a linked issue before release.
- Issue must include impact, workaround, and target milestone.

## Classification of Findings

### Release-Blocking Trust Bug

Classify as release-blocking when at least one is true:

- operation result is wrong (reported success but failed/partial)
- destructive flow can lose data or overwrite against policy
- cancel path leaves inconsistent source/destination state without clear error
- UI state implies completion while backend failed/partial

### Acceptable Known Limitation

Acceptable only when all are true:

- behavior matches current matrix/checklist expectation
- no silent data loss risk
- user can reliably detect and recover
- limitation is documented in checklist notes (or provider appendix for cloud)

### Follow-Up Issue (Non-Blocking)

Use when:

- impact is real but does not violate release-blocking criteria
- workaround exists and is documented
- issue includes reproducible steps and owner

## Triage Workflow

1. Map failed checklist row to matrix scenario ID.
2. Classify using the three finding classes above.
3. For blocking findings: stop release and require fix + rerun of affected rows.
4. For non-blocking findings: require issue link and note in release run metadata.
