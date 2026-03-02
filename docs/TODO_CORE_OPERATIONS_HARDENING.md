# TODO: Core Operations Hardening

Created: 2026-03-02
Goal: Increase trust in Browsey's highest-risk file operations by adding a
deliberate hardening program for copy/move/rename/trash/delete/extract/conflict
flows, with stronger regression coverage and a repeatable release checklist.

## Why This Track Exists

Browsey already has broad capability across local, mixed local/cloud, and
archive workflows, but trust now depends less on feature breadth and more on
how confidently the project can prove operation correctness under failure,
interruption, and edge conditions.

This track exists to improve the areas users judge most harshly:

- destructive or recovery-sensitive file operations
- conflict resolution correctness
- cancellation and partial-failure behavior
- UI/backend consistency after errors
- release-time confidence for critical workflows

This is a hardening track, not a feature-expansion track.

## Current Surface Area

This plan intentionally spans multiple existing modules because the risk surface
already crosses command and UI boundaries:

- local clipboard-backed copy/move and drag/drop semantics: `src/clipboard/`
- local delete flows: `src/commands/fs/`
- trash lifecycle: `src/commands/fs/trash/`
- rename flows: `src/commands/rename/`
- mixed local/cloud transfers and conflict preview:
  `src/commands/transfer/`
- archive extraction: `src/commands/decompress/`
- cloud write integration and refresh signaling: `src/commands/cloud/`
- explorer operation UI and follow-up refresh behavior:
  `frontend/src/features/explorer/`

## Architecture Alignment

This track should stay aligned with:

- `ARCHITECTURE_NAMING.md`
- `ARCHITECTURE_IMPORTS.md`

Implications:

- preserve existing command contracts unless a change is explicitly part of a
  behavior fix
- STRICT: core operation implementations must use typed domain errors end-to-end
  (`*ErrorCode` + `map_api_result`), not stringly error construction
- prefer targeted regression coverage and fault injection over broad rewrites
- keep behavior fixes separate from harness/buildout commits where practical
- keep test helpers close to the domains they validate
- avoid introducing cross-feature frontend coupling just to test workflows

## Duplication Guardrails

This track should reduce duplicated planning and duplicated validation assets:

- The critical operations matrix is the source of truth for expected behavior.
- The release checklist is a runnable subset derived from the matrix, not a
  second place to redefine semantics.
- Provider-specific real-account checklists should stay as supplements for
  environment-specific validation, not as separate definitions of product
  behavior.
- Reuse existing mixed-transfer fake-`rclone` coverage and existing manual cloud
  checklists where still accurate; extend them only for gaps found by the
  matrix.
- Do not reopen completed `rclone rc`, cloud split, or refactor TODO tracks
  unless this matrix exposes a concrete trust regression they did not cover.

Current accuracy note:

- `docs/onedrive-rclone-v1-manual-checklist.md` predates the current mixed
  local/cloud support described in README/docs, so it must be updated or
  replaced before being treated as an authoritative validation artifact.

## Scope

In scope:

- local clipboard-backed `copy` / `move`, plus local `rename`, `trash`, `delete`
- mixed local<->cloud copy/move flows and conflict preview correctness
- extract flows that can destroy, overwrite, partially write, or leave unclear
  state
- cancellation, retry, and partial-failure handling where supported
- user-facing operation state after failure or cancellation
- release-time validation assets for critical workflows

Out of scope:

- new product features
- broad UX redesign unrelated to operation trust
- full cloud parity work beyond correctness in already-supported flows
- macOS support
- unrelated refactors not required for test seams or bug fixes

## Success Criteria

This track is complete when:

- the project has a maintained critical-operations release checklist
- core operation regressions are covered by repeatable automated tests
- local and mixed transfer failure paths degrade predictably
- release candidates can be validated against a defined trust-critical matrix
- no major known ambiguity remains around "did the operation succeed, fail, or
  partially complete?"

## Non-Goals

- do not use this track to smuggle in feature work
- do not widen supported semantics silently
- do not replace manual validation with automation alone
- do not claim broader platform/cloud guarantees than the tests can support

## Quality Gates (Every Step)

- [ ] `cargo fmt --all` is green
- [ ] `cargo clippy --all-targets --all-features -- -D warnings` is green
- [ ] Relevant Rust tests are green for the touched domain
- [ ] `npm --prefix frontend run test` is green when frontend operation flows are touched
- [ ] `npm --prefix frontend run test:e2e` is green when user-visible critical
      operation flows are touched
- [ ] Manual Browsey smoke is green for the touched operation family
- [ ] Any new assumption or invariant is documented in the checklist or test notes
- [ ] STRICT typed-error guard is green for touched core operation modules
      (`scripts/maintenance/check-backend-error-hardening-guard.sh`)

## Risk Hotspots

Highest-risk behaviors in this track:

- move semantics after partial destination creation
- rename-on-conflict behavior across local and cloud-backed paths
- trash/delete divergence across Linux, Windows, network, and cloud paths
- extract cleanup after interrupted or denied writes
- stale selection/progress/error state in the explorer after failed operations
- cancellation timing around long-running or backgrounded tasks
- refresh/invalidation correctness after write operations

## Work Plan

### 1) Define the critical operations matrix

- [x] Create a matrix of trust-critical flows and expected outcomes.
- [x] Assign stable scenario IDs so tests and checklist rows can reference the
      same source entry instead of duplicating behavior descriptions.
- [x] Group the matrix by operation family:
  - local clipboard-backed copy/move
  - local rename
  - local trash/delete/restore/purge
  - mixed local<->cloud copy/move
  - extract with existing destination/error/cancel cases
- [x] For each flow, record:
  - happy path expectation
  - conflict behavior expectation
  - cancellation expectation
  - partial-failure expectation
  - visible UI expectation after completion/failure
- [x] Use exact module ownership notes so follow-up work is easy to route.

Acceptance:

- There is a concrete, reviewable trust matrix instead of implicit maintainer knowledge.

### 2) Add a critical-operations release checklist

- [x] Add `docs/core-operations-release-checklist.md`.
- [x] Make the checklist executable by a maintainer, not only descriptive.
- [x] Structure the checklist as matrix-derived validation rows keyed by
      scenario ID, not a rewritten narrative copy of the matrix.
- [x] Cover at minimum:
  - copy/move file
  - copy/move directory
  - rename file/folder
  - trash/restore/purge
  - permanent delete
  - extract archive to new destination
  - extract archive into conflicting destination
  - mixed local<->cloud copy/move with conflict preview
- [x] Record environment notes where relevant:
  - Linux-first required checks
  - Windows checks if a touched area claims Windows support
  - cloud preconditions for `rclone` flows
- [x] Keep provider-specific real-account cloud checks in separate appendix docs
      and update the existing OneDrive checklist instead of forking a second
      cloud manual-validation document.

Acceptance:

- A release candidate can be validated against a repeatable manual checklist.

### 3) Strengthen local destructive-operation regression coverage

Target modules:

- `src/clipboard/`
- `src/commands/fs/`
- `src/commands/fs/trash/`
- `src/commands/rename/`

- [x] Audit existing Rust tests for local file operations and mark matrix gaps.
- [x] Add missing tests for:
  - local clipboard-backed copy/move failure and rollback-sensitive cases
  - rename conflicts and no-overwrite semantics
  - delete vs trash routing
  - restore/purge edge cases
  - disappearing-source and disappearing-destination behavior
  - permission-denied and read-only variants where practical
- [x] Prefer fixture/setup helpers that make failure modes readable, not opaque.
- [x] Keep security/no-follow expectations explicit in test names where relevant.

Progress notes (2026-03-02):

- Added matrix gap audit: `docs/core-operations-local-gap-audit.md`.
- Added local regression tests for disappearing source/destination and explicit
  no-overwrite rename behavior in:
  - `src/clipboard/tests.rs`
  - `src/commands/rename/mod.rs`
- Added permanent-delete undo-path tests in:
  - `src/commands/fs/delete_ops.rs`
- Added restore/purge core tests via injectable trash ops in:
  - `src/commands/fs/trash/mod.rs`
  - `src/commands/fs/trash/tests.rs`
- Added no-follow and read-only permission regression tests in:
  - `src/clipboard/tests.rs`
  - `src/commands/rename/mod.rs`
- Added delete batch cancellation/rollback/progress-path tests in:
  - `src/commands/fs/delete_ops.rs`
- Added explicit delete-vs-trash routing regression test in:
  - `frontend/src/features/explorer/context/createContextActions.test.ts`

Acceptance:

- Local trust-critical operation paths have targeted regression coverage, not only incidental coverage.

### 4) Strengthen mixed transfer and conflict-path coverage

Target modules:

- `src/commands/transfer/execute.rs`
- `src/commands/transfer/preview.rs`
- `src/commands/transfer/route.rs`
- `tests/support/fake-rclone.sh`

- [x] Reuse the existing fake-`rclone` execution tests as the baseline and add
      only trust-matrix gaps instead of rebuilding parallel coverage from scratch.
- [ ] Expand mixed local<->cloud tests to cover the highest-risk conflict paths.
- [ ] Add or tighten cases for:
  - rename-on-conflict parity
  - copy vs move source cleanup semantics
  - partial completion and follow-up invalidation behavior
  - error mapping consistency when the provider fails mid-operation
  - cancellation behavior where progress-aware paths are active
- [ ] Keep fake-`rclone` scenarios deterministic and easy to extend.

Progress notes (2026-03-02):

- Added mixed transfer execution tests on existing fake-`rclone` harness in:
  - `src/commands/transfer/execute.rs`
- New coverage includes:
  - early-cancel behavior (`cancelled` before write begins)
  - `destination_exists` policy when `prechecked=false` in local->cloud copy

Acceptance:

- Mixed transfer correctness is defended by a deliberate regression suite, not only ad hoc spot tests.

### 5) Harden extraction and partial-write behavior

Target modules:

- `src/commands/decompress/`

- [ ] Audit existing extraction tests against overwrite, limit, and interruption scenarios.
- [ ] Add missing tests for:
  - extraction into partially existing targets
  - permission-denied during extraction
  - cancellation/interruption cleanup expectations
  - multi-archive batch behavior where partial completion is possible
- [ ] Document any intentionally non-transactional behavior clearly in checklist notes.

Acceptance:

- Extract behavior is predictable under failure and no longer depends on informal assumptions.

### 6) Add frontend regression coverage for operation-state integrity

Target areas:

- `frontend/src/features/explorer/`
- `frontend/e2e/`

- [ ] Add frontend tests for error/progress/cancel state where backend outcomes are mocked.
- [ ] Cover at minimum:
  - conflict modal follow-up behavior
  - progress state after cancel/error
  - refresh behavior after successful write flows
  - operation failure surfaces that currently risk stale UI state
- [ ] Add at least one Playwright scenario beyond the current smoke test if a
  critical flow is not realistically protected by unit tests alone.

Acceptance:

- User-visible operation state after failure/cancel is covered by targeted regression tests.

### 7) Add fault-injection passes for hostile conditions

- [ ] Introduce or extend helpers for simulated:
  - permission denied
  - source disappearing during operation
  - destination becoming unavailable
  - backend cancellation while work is in progress
- [ ] Prefer explicit fault injection over fragile timing-based tests.
- [ ] Keep platform-specific fault cases separated when semantics differ.

Acceptance:

- The project can test hard failure modes intentionally instead of waiting for them to appear in the field.

### 8) Define release blocking policy for critical regressions

- [ ] Document which failures in this matrix should block a release.
- [ ] Define what counts as:
  - release-blocking trust bug
  - acceptable known limitation
  - follow-up issue
- [ ] Keep the policy small and operational, not aspirational.

Acceptance:

- Critical operation bugs are triaged by trust impact, not only by convenience.

## Suggested Commit Boundaries

- [ ] Commit 1: critical operations matrix
- [ ] Commit 2: release checklist
- [ ] Commit 3: local destructive-operation regression additions
- [ ] Commit 4: mixed transfer/conflict regression additions
- [ ] Commit 5: extraction hardening coverage
- [ ] Commit 6: frontend operation-state regression additions
- [ ] Commit 7: fault-injection helpers and hostile-condition tests
- [ ] Commit 8: release-blocking policy notes

## Exit Notes

When this track is complete:

- archive this TODO under `docs/todo-archive/`
- keep the release checklist as a live document in `docs/`
- link the checklist from contributor/release notes if it becomes part of the
  normal release flow
