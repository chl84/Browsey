# Linux 1.0 Core Workflow Gap Audit

Created: 2026-03-06
Track: `docs/todo/TODO_PRODUCTION_READY_LINUX.md`
Release-bar reference: `docs/operations/linux-release/release-bar.md`

## Purpose

Review the Linux 1.0 core workflows end-to-end and identify which areas
already inherit release-level validation from existing operations documents,
which areas only have partial automated coverage, and which areas still need
Linux 1.0-specific bugbash/release attention.

This audit is intentionally review-oriented. It does not claim that the gaps
below are the only bugs that exist; it defines where Linux 1.0 hardening effort
should focus first.

## Evidence Reviewed

Existing release operations documents:

- `docs/operations/core-operations/matrix.md`
- `docs/operations/core-operations/release-checklist.md`
- `docs/operations/core-operations/release-blocking-policy.md`
- `docs/audits/core-operations/local-gap-audit.md`
- `docs/audits/core-operations/mixed-gap-audit.md`
- `docs/audits/core-operations/extract-gap-audit.md`

Explorer/frontend evidence:

- `frontend/src/features/explorer/hooks/useExplorerData.test.ts`
- `frontend/src/features/explorer/navigation/useExplorerNavigation.test.ts`
- `frontend/src/features/explorer/navigation/useExplorerSearchSession.test.ts`
- `frontend/src/features/explorer/state.test.ts`
- `frontend/src/features/explorer/modals/propertiesModal.test.ts`
- `frontend/src/features/explorer/services/files.service.test.ts`
- `frontend/src/features/explorer/context/createContextActions.test.ts`

Backend evidence:

- `src/commands/search/worker.rs` (`#[cfg(test)] mod tests`)
- `src/commands/search/query/lexer.rs` (`#[cfg(test)] mod tests`)
- `src/commands/search/query/parser.rs` (`#[cfg(test)] mod tests`)
- `src/commands/search/query/eval.rs` (`#[cfg(test)] mod tests`)
- `src/commands/open_with/linux.rs` (`#[cfg(test)] mod tests`)
- `src/commands/permissions/mod.rs` + related permission/ownership tests
- `src/commands/entry_metadata/mod.rs`

## Workflow Review

| Workflow | Current release/automation status | Evidence | Linux 1.0 gap summary |
|---|---|---|---|
| Browse directories | Partial but meaningful | `useExplorerData.test.ts`, `useExplorerNavigation.test.ts`, `state.test.ts`, `src/commands/listing/*` | Good automation for navigation/state plumbing, but no Linux 1.0 release checklist rows yet for browse/network/mount regressions outside core destructive operations. |
| Copy and move | Stronger release treatment | Existing `LCM` + `MTC` matrix/checklist/policy and related audits | Already part of the release-blocking operating model; remaining work is gap closure, not missing release structure. |
| Rename and advanced rename | Partial to strong | Existing `LRN` matrix/checklist/policy and local gap audit | Local rename is in the core release model; advanced rename still needs Linux 1.0 bugbash attention because it is not separately called out in release operations docs. |
| Trash and permanent delete | Stronger release treatment | Existing `LTD` matrix/checklist/policy and local gap audit | Already part of the release-blocking operating model; remaining work is gap closure, especially around restore/purge edge cases. |
| Compress and extract | Mixed | Existing `EXT` matrix/checklist/policy and extract gap audit | Extract has release structure; compress does not yet have equivalent Linux 1.0 release rows and should be covered by Linux bugbash until/unless promoted into core operations docs. |
| Search | Partial but credible | `useExplorerSearchSession.test.ts`, backend search/query tests | Search has solid parser/session coverage, but no explicit Linux release checklist rows for end-user search execution, cancellation, and error presentation. |
| Properties and permissions | Partial | `propertiesModal.test.ts`, permission/ownership backend tests, `entry_metadata` module | Permissions and metadata logic are covered in pieces, but Linux 1.0 still needs end-to-end validation for properties open/edit/apply behavior on real Linux paths. |
| Open with / open file | Partial | `files.service.test.ts`, `createContextActions.test.ts`, `src/commands/open_with/linux.rs` tests | Local/cloud open dispatch exists, but release-grade Linux validation still needs explicit manual rows for default-open and open-with behavior. |

## Primary Hardening Gaps for Linux 1.0

1. Keep using the existing core-operations release checklist for `LCM`, `LRN`,
   `LTD`, `MTC`, and `EXT`.
2. Add Linux 1.0 bugbash coverage for workflows that are production-critical
   but not fully represented in the existing core-operations release docs:
   - browse directories
   - advanced rename
   - compress
   - search
   - properties and permissions
   - open with / open file
3. Use that bugbash coverage to decide whether some Linux 1.0 workflows should
   later be promoted into the formal release operations docs.

## Conclusion

The Linux 1.0 core workflow review is strong for trust-critical destructive
operations because the project already has a core-operations matrix, checklist,
and release-blocking policy. The main Linux 1.0 gap is not absence of all
release discipline, but uneven coverage for non-destructive but still
production-critical workflows such as browse, search, properties, open-with,
and compress.
