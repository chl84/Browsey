# TODO: Engineering Improvements

Created: 2026-02-21  
Scope: Repository-wide (frontend, backend, docs, CI)  
Goal: Raise engineering quality from "good and practical" to "strict and scalable" without destabilizing active development.

## Why This Exists

Current strengths:
- Clearer frontend architecture and module boundaries.
- Naming/import conventions documented and enforced.
- CI quality gate for frontend (`lint`, `check`, `build`).

Current gaps to close:
- Frontend lint passes with warnings that represent technical debt.
- Lint policy is pragmatic but not yet strict for long-term scaling.
- Docs pipeline has typecheck/build, but no dedicated docs lint gate.
- Rust CI lacks explicit style/lint gates (`fmt`, `clippy`) in visible workflows.
- Testing remains heavy on manual smoke checks for critical UI workflows.

## Current Baseline (2026-02-21)

- Frontend:
  - `npm --prefix frontend run lint`: 0 errors, 17 warnings
  - `npm --prefix frontend run check`: green
  - `npm --prefix frontend run build`: green
- Warning breakdown:
  - `@typescript-eslint/no-unused-vars`: 12
  - `no-useless-escape`: 4
  - `no-unsafe-finally`: 1

## Execution Order

1. Phase 1 + Phase 2 (measure and burn down warnings)
2. Phase 3 (promote key rules to error)
3. Phase 4 (docs lint gate)
4. Phase 5 (Rust CI quality gates)
5. Phase 6 (automated UI regression coverage)
6. Phase 7 (maintenance/process guardrails)

## Non-Goals (for this plan)

- No feature work unrelated to quality hardening.
- No backend API contract redesign.
- No broad frontend architecture refactor (already completed in prior tracks).

## Quality Policy for This Plan

- Keep behavior unchanged unless a step explicitly states otherwise.
- Prefer small, focused commits (one phase/sub-phase per commit).
- Keep all new project text and comments in English.
- Required gate per phase:
  - `npm --prefix frontend run lint`
  - `npm --prefix frontend run check`
  - `npm --prefix frontend run build`
  - plus phase-specific checks below.
- Manual smoke-test before commit when user-facing behavior paths are touched.

## Phase 1: Baseline and Tracking

- [ ] Snapshot current frontend lint warnings into a tracked report (`docs/quality/lint-baseline.md`).
- [ ] Categorize warnings by type and owner area:
  - unused variables/imports
  - escape/noise issues
  - unsafe/finally or risky patterns
- [ ] Define a warning budget rule:
  - no new warnings allowed
  - total warning count must trend down

Acceptance:
- Warning baseline is documented and versioned.
- Team has a measurable target for warning reduction.

Exit Criteria:
- Baseline report includes exact warning count, categories, and file ownership map.
- "No new warnings" policy is documented in the report and referenced by contributors.

## Phase 2: Burn Down Existing Frontend Warnings

- [ ] Remove low-risk warnings first:
  - rename unused parameters to `_...`
  - remove dead locals/imports
  - fix no-useless-escape issues
- [ ] Resolve medium-risk warnings with focused diffs and manual verification.
- [ ] Keep warning count updated after each batch.

Acceptance:
- Frontend lint warning count is significantly reduced (target: 0 or near-0).
- No behavior regressions from cleanup changes.

Exit Criteria:
- Target warning count: 0 (preferred) or <= 3 with explicit tracked exceptions.
- All remaining warnings must be listed in `docs/quality/lint-baseline.md` with owners.

## Phase 3: Tighten Lint Rules Incrementally

- [ ] Promote selected warning rules to errors after cleanup:
  - `@typescript-eslint/no-unused-vars`
  - `no-useless-escape`
  - `no-unsafe-finally`
- [ ] Keep temporary exceptions explicit and localized.
- [ ] Remove temporary relaxations once each category reaches stable compliance.

Acceptance:
- Lint policy is stricter and blocks regression instead of only reporting debt.

Exit Criteria:
- Selected rules are `error` in `frontend/eslint.config.js`.
- CI fails on reintroduction of those classes of issues.

## Phase 4: Add Docs Lint Gate

- [ ] Introduce docs lint scripts (ESLint for docs TS/Svelte content).
- [ ] Add `npm --prefix docs run lint`.
- [ ] Wire docs lint into docs workflow before `check`/`build`.

Acceptance:
- Docs quality gate includes lint + typecheck + build.

Exit Criteria:
- `.github/workflows/docs-pages.yml` includes docs lint step.
- `npm --prefix docs run lint` is green in CI and locally.

## Phase 5: Harden Rust CI Quality Gates

- [ ] Add/extend Rust workflow with explicit:
  - `cargo fmt --check`
  - `cargo clippy --all-targets --all-features -- -D warnings`
  - `cargo test` (at minimum for changed crates/modules)
- [ ] Ensure workflow triggers on Rust/backend path changes.

Acceptance:
- Rust style/lint/test checks are enforced automatically in CI.

Exit Criteria:
- Rust workflow fails on formatting/lint/test violations.
- Workflow trigger paths cover backend Rust changes.

## Phase 6: Add Focused Automated UI Regression Coverage

- [ ] Add a minimal deterministic test layer for critical explorer flows:
  - keyboard navigation and selection behavior
  - context menu action dispatch
  - clipboard/drag-drop orchestration boundaries
  - search mode transitions (`address`/`filter`/`search`)
- [ ] Prefer unit/integration tests around hooks/controllers before full E2E expansion.
- [ ] Add one smoke-level E2E for the highest-risk user path.

Acceptance:
- Critical flows are protected by automated regression checks, not only manual smoke tests.

Exit Criteria:
- At least one automated test exists for each listed critical flow area.
- Tests run in CI with stable pass/fail signal (no flaky baseline).

## Phase 7: Release and Maintenance Guardrails

- [ ] Add a short quality checklist to PR template (if template exists):
  - docs updated?
  - changelog updated?
  - lint/check/build green?
  - tests added/updated?
- [ ] Add a monthly "quality debt sweep" issue template:
  - lint warnings
  - flaky tests
  - stale temporary exceptions

Acceptance:
- Quality practices are repeatable and not person-dependent.

Exit Criteria:
- Checklist/template is versioned in repository.
- A recurring quality-debt issue template exists and is usable.

## Definition of Done

- [ ] Frontend lint warnings are reduced to an agreed low level (preferably 0).
- [ ] Key relaxed lint rules are promoted to errors.
- [ ] Docs have an explicit lint gate in CI.
- [ ] Rust has `fmt` + `clippy` + test gates in CI.
- [ ] Critical UI flows have automated regression coverage.
- [ ] PR/release process includes lightweight quality checklists.
