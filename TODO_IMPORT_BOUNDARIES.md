# TODO: Import Boundaries and Barrels

Created: 2026-02-21  
Scope: Frontend (`frontend/src`)  
Goal: Keep feature/shared boundaries strict by introducing clear public APIs (barrels) and enforcing import rules.

## Target Architecture

- `shared` is reusable and can be imported by all features.
- Each feature exposes a **public API** via `index.ts` barrel.
- Cross-feature imports must go through the feature barrel:
  - Allowed: `@/features/network`
  - Not allowed: `@/features/network/services`
- Internal feature modules stay private unless re-exported in that feature’s barrel.

## Constraints

- No behavior changes.
- No API break in runtime behavior.
- Incremental rollout: warn first, then error.
- Keep changes reversible in small commits.

## Quality Gates (each phase)

- [x] `npm --prefix frontend run check` passes.
- [x] `npm --prefix frontend run build` passes.
- [ ] Smoke-test: navigation, search/filter, settings modal, context menus.
- [x] `rg` validation commands for import patterns pass.

## Baseline Audit

- [x] Map current deep imports:
  - `rg -n "from '@/features/[^']+/.+'" frontend/src`
- [x] Map shared->feature violations (should be none):
  - `rg -n "from '@/features/" frontend/src/shared`
- [x] List existing feature barrels:
  - `find frontend/src/features -maxdepth 2 -name index.ts | sort`

Acceptance:
- We have a baseline list of violations and barrel gaps.

Audit Findings (2026-02-21):
- Deep feature imports (alias form) are common; majority are same-feature internals.
- Cross-feature alias imports found: `explorer -> settings` (1), `explorer -> shortcuts` (4).
- `shared -> features` violations: none.
- Existing barrels before Phase 1: only `frontend/src/features/network/index.ts`.

## Phase 1: Add Public Barrels

- [x] Add/normalize:
  - `frontend/src/features/explorer/index.ts`
  - `frontend/src/features/settings/index.ts`
  - `frontend/src/features/network/index.ts` (already exists; verify consistency)
  - `frontend/src/features/shortcuts/index.ts`
  - `frontend/src/shared/index.ts`
- [x] Export only intended surface (pages/components/hooks/services/types meant for external use).
- [x] Avoid wildcard exports when they expose internals accidentally.

Acceptance:
- Each feature has one clear import entrypoint.
- No runtime behavior change.

## Phase 2: Migrate Imports to Barrels

- [x] Replace cross-feature deep imports with barrel imports.
- [x] Keep in-feature relative imports as-is unless cleanup is trivial.
- [x] Verify no forbidden cross-feature deep import remains:
  - `rg -n "from ['\"]@/features/[^/'\"]+/[^'\"]+['\"]" frontend/src frontend/src/App.svelte` + compare importer feature vs imported feature (result: `violations=0`).

Acceptance:
- Cross-feature imports only use `@/features/<feature>`.

## Phase 3: Enforce Rules in Lint (Warn Mode)

- [x] Introduce lint stack for frontend if missing:
  - ESLint + TypeScript + Svelte + import rules.
- [x] Add `no-restricted-imports` patterns for boundaries:
  - Block deep cross-feature imports.
  - Allow same-feature internals.
  - Allow shared imports.
- [x] Add script:
  - `npm --prefix frontend run lint`
- [x] Run lint as warning-level for boundary rules initially.

Acceptance:
- CI/local lint reports boundary violations without blocking.

## Phase 4: Tighten to Error Mode

- [x] Fix remaining warnings.
- [x] Switch boundary rules from warning to error.
- [x] Add lint to CI gate.

Acceptance:
- Boundary violations fail CI.

## Import Policy (to codify in docs)

- Allowed:
  - `@/shared/...`
  - `@/features/<same-feature>/...`
  - `@/features/<other-feature>` (barrel only)
- Disallowed:
  - `@/features/<other-feature>/<private-path>`

## Suggested Rule Sketch

- `no-restricted-imports` with patterns similar to:
  - `@/features/*/*` (deny by default)
  - per-feature overrides for same-feature paths
  - explicit allowlist for `@/features/*` barrel paths

Note:
- Exact ESLint config depends on chosen Svelte + TS lint stack; implement in a dedicated commit.

## Risks and Mitigations

- Risk: Barrel exports become too broad.
  - Mitigation: Explicit named exports only.
- Risk: Circular imports after re-export.
  - Mitigation: Keep barrels thin; avoid barrels importing from other barrels unnecessarily.
- Risk: Large migration diff.
  - Mitigation: Migrate one feature pair at a time and commit incrementally.

## Rollback Plan

- Keep separate commits per phase:
  - Barrels
  - Import migration
  - Lint setup
  - Rule tightening
- If regressions appear:
  - Revert latest phase commit only.

## Definition of Done

- [x] All cross-feature imports use barrels.
- [x] Boundary rules are enforced as errors in CI.
- [x] `ARCHITECTURE_IMPORTS.md` exists with policy examples.
- [ ] Frontend check/build/smoke tests pass.
