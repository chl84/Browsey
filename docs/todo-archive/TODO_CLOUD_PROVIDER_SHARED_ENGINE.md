# TODO: Cloud Provider Shared Engine (OneDrive + Google Drive + Nextcloud)

Created: 2026-03-05
Target: Reduce provider drift without regressions
Scope: `src/commands/cloud/**` (backend only)

## Goal
Extract shared cloud operation flow into a common engine while keeping provider-specific behavior behind explicit policy hooks.  
Primary goal: less duplication and fewer regressions in `mkdir/delete/move/copy/open` flows with stable typed errors.

## Baseline lock (before refactor)
- [x] Lock current typed error `code_str()` behavior for `mkdir/delete/move/copy/open` in tests.
- [x] Lock provider-specific policy behavior in tests (not comments only):
- [x] OneDrive: hard-delete and case/conflict quirks covered by assertions.
- [x] Google Drive: no-trash delete policy and current conflict behavior covered by assertions.
- [x] Nextcloud: current WebDAV/Nextcloud behavior and defaults covered by assertions.
- [x] Add a short policy table comment that references concrete test names.
- [x] Ensure current provider regression scenarios are green before first extraction PR.

## Principles (locked)
- [x] Shared engine handles common flow only (precheck, retries, normalization, dedupe patterns).
- [x] Provider policy handles quirks only (trash/delete flags, case/conflict semantics, provider-specific errors).
- [x] No frontend contract changes.
- [x] No Tauri command signature changes.
- [x] No stringly error regressions; preserve typed error mapping.

## In scope
- [ ] `src/commands/cloud/providers/rclone/**`
- [ ] `src/commands/cloud/mod.rs`
- [ ] Cloud provider tests in `src/commands/cloud/providers/rclone/tests.rs`
- [ ] Related typed-error mapping tests

## Out of scope
- [ ] New cloud providers
- [ ] UI/UX changes
- [ ] Large architecture changes outside cloud command stack

## Workstream 1: Policy boundary
- [x] Define a `ProviderPolicy` abstraction (trait/struct strategy) for:
- [x] Delete/trash behavior
- [x] Conflict/case behavior
- [x] Provider-specific error hints/classification hooks
- [x] Retry/backoff hints where needed
- [x] Keep explicit provider defaults in one place (no hidden behavior in call sites)
- [x] Document shared vs provider-specific boundaries in code comments

## Workstream 2: Shared engine extraction (no behavior change)
- [x] Extract shared precheck/execute/retry skeleton for cloud write operations
- [x] Keep existing behavior parity for OneDrive/GDrive/Nextcloud
- [x] Route existing provider code through the shared engine incrementally
- [x] Keep total/pure helper boundaries where possible to simplify tests
- [x] Keep thin wrappers at old entry points during migration (avoid big-bang call-site rewrites)

## Workstream 3: Operation-by-operation migration
- [x] Phase A: `mkdir`
- [x] Phase B: `delete`
- [x] Phase C: `move` + `copy`
- [x] Phase D: `open/materialize` shared patterns where safe
- [x] Validate each phase completely before moving to the next
- [x] Rule: one operation family per PR unless changes are purely mechanical

## Workstream 4: Contract and regression tests
- [x] Add provider contract test matrix for OneDrive/GDrive/Nextcloud:
- [x] create -> delete -> recreate same name
- [x] destination exists
- [x] not found
- [x] timeout / retry mapping
- [x] rate limit mapping
- [x] case/conflict behavior checks (provider-appropriate)
- [x] Keep and update existing provider-specific tests
- [x] Ensure typed error `code_str()` stability in touched flows
- [x] Add at least one negative-path and one retry-path test for every touched operation

## Regression guardrails (per PR)
- [x] No behavior-changing refactor PR unless behavior delta is explicitly scoped, tested, and documented in the same PR.
- [x] For touched operations, typed error code snapshots must stay unchanged unless explicitly approved.
- [x] No unbounded waits in shared engine paths (timeouts/cancellation must remain enforced).
- [x] Run focused provider tests locally before full suite to catch regressions early.
- [x] Record any intentional behavioral delta in PR notes with provider impact.

## Maintainability guardrails
- [x] Keep shared logic in dedicated helper modules (engine/policy/error mapping) instead of provider files.
- [x] Avoid duplicate retry/error parsing logic across providers after extraction.
- [x] Add concise doc comments on each policy hook: input, output, and provider responsibility.
- [x] Prefer pure helper functions for decision logic to maximize unit-test coverage.
- [x] Keep migration logs/temporary diagnostics removable and tracked by checklist.

## Workstream 5: Rollout and safeguards
- [x] Land as small PR sequence (refactor PRs + behavior-safe migrations)
- [x] Use temporary debug logging around policy hooks during migration
- [x] Remove migration-only logging before final closeout
- [x] Keep rollback simple: each PR must be independently revertible

## Quality gates
- [x] `cargo fmt --all -- --check`
- [x] `cargo clippy --all-targets --all-features -- -D warnings`
- [x] `bash scripts/maintenance/check-backend-error-hardening-guard.sh`
- [x] `cargo test commands::cloud::providers::rclone::tests -- --nocapture`
- [x] `cargo test --all-targets --all-features`

## Acceptance criteria
- [x] Shared cloud logic is centralized without hiding provider quirks
- [x] OneDrive/GDrive/Nextcloud keep existing expected behavior
- [x] No typed-error regressions in affected paths
- [x] Shared engine precheck/retry skeleton is used by at least `mkdir/delete/move/copy`.
- [x] Provider files no longer duplicate precheck+retry flow for migrated operations.
- [x] Every migrated operation has explicit contract tests across all three providers
- [x] All quality gates pass

## Residual risk
- [ ] Risk: over-generalization can break provider edge cases.
- [ ] Mitigation: strict policy hooks + contract tests per provider.
- [ ] Risk: hidden semantic drift during refactor.
- [ ] Mitigation: no-behavior-change phases and small PRs.

## Exit / archive
- [x] Move this file to `docs/todo-archive/` when complete.
- [x] Add completion note with date and summary in archived file.

## Completion note
Completed: 2026-03-05
Result:
- Introduced provider policy hooks for delete behavior, conflict casing, error hints, and mkdir retry hints.
- Refactored cloud write/open paths into shared helpers without public API changes.
- Added cross-provider contract coverage (OneDrive/Google Drive/Nextcloud) for create-delete-recreate and copy/move roundtrips.
- Passed quality gates: fmt, clippy `-D warnings`, backend hardening guard, and full Rust test suite.
