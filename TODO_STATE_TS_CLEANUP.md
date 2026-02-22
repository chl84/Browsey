# TODO: Explorer `state.ts` Cleanup

Created: 2026-02-22
Goal: Reduce maintenance load in `frontend/src/features/explorer/state.ts` while preserving the public API (`createExplorerState`) and behavior.

## Why This Track Exists

`frontend/src/features/explorer/state.ts` currently mixes multiple responsibilities:
- Svelte store composition/orchestration (expected)
- Async navigation/load/search orchestration (expected)
- Pure in-memory sorting/comparison logic (should move out)
- Reusable entry mutation/update helpers (should move out)
- Large branching dispatch helpers (can be reduced)

The file works, but it has grown into a high-churn hotspot. This track focuses on **structural cleanup and module boundaries**, not UX changes.

## Architecture Alignment (Reviewed)

This plan is aligned with:
- `ARCHITECTURE_NAMING.md`
- `ARCHITECTURE_IMPORTS.md`

Implications for this work:
- Keep `createExplorerState` as the stable public API.
- Keep new modules inside `frontend/src/features/explorer/state/` (same-feature internal imports are allowed).
- Use naming that matches exported API:
  - `create*.ts` for factories/builders
  - `use*.ts` only for stateful hook/composable-style modules
  - plain `*.ts` for pure helpers/algorithms
- Do not introduce cross-feature deep imports.

## Scope

In scope:
- Move pure algorithms/helpers out of `state.ts`
- Thin out branching helpers where practical
- Keep behavior stable
- Add small state-folder docs for contributor clarity

Out of scope:
- Backend/API changes
- UX changes
- Rewriting existing slices (`filteringSlice`, `preferencesSlice`) unless needed for extraction seams
- Changing the `createExplorerState` return shape

## Constraints

- Prefer pure helper extraction before moving async orchestration.
- One focused commit per step.
- Keep docs/comments in English.
- Preserve behavior; any bug fix discovered during extraction should be isolated in a separate commit if possible.

## Quality Gates (Every Step)

- [ ] `npm --prefix frontend run check` is green
- [ ] `npm --prefix frontend run lint` is green
- [ ] Manual Browsey smoke is green for the touched flow
- [ ] `createExplorerState` public API shape remains unchanged
- [ ] No new cross-feature deep imports introduced

## Dependencies and Risk Hotspots

Sequencing notes:
- Start with pure sort/helper extraction (low risk, immediate readability gain).
- Delay search-stream extraction until smaller cleanup steps have reduced noise in `state.ts`.
- Any step touching `runSearch(...)` is high-risk because it affects streaming, cancellation, batching, and UI responsiveness.
- `refreshForSort(...)` is medium risk because it routes multiple views (`Recent`, `Starred`, `Network`, `Trash`, dir, active search).

Recommended pause points:
- Pause after Step 3 (sort logic fully extracted)
- Pause after Step 5 (`refreshForSort` extraction)
- Pause after Step 7 (`runSearch` extraction, if executed)

## Work Plan

### 1) Baseline and responsibility map (no behavior change)

- [x] Record current `state.ts` LOC and identify responsibility buckets in this TODO.
- [x] Add short section comments in `state.ts` if needed to mark extraction seams (no logic changes).
- [x] Confirm current `createExplorerState` return keys are the compatibility boundary.

Baseline snapshot (recorded 2026-02-22):
- `frontend/src/features/explorer/state.ts` current size before extraction: `867` LOC
- Main responsibility buckets currently in file:
  - store composition and slice wiring (`createExplorerStores`, filtering/preferences slices)
  - navigation/history + listing load flows
  - sort payload + sort refresh routing
  - search mode + search stream orchestration/cancellation
  - entry mutation/update glue (`entries.update(...)`)
  - mounts/bookmarks/layout persistence helpers
  - final `createExplorerState` public API assembly (return object)
- Compatibility boundary for this track:
  - Keep the `createExplorerState` returned object shape stable unless explicitly changing feature public API.

Acceptance:
- Clear extraction map exists before moving code.

### 2) Extract in-memory search/list sorting function (low risk)

Target:
- `sortSearchEntries(...)` and directly related local sort code

- [x] Add `frontend/src/features/explorer/state/searchSort.ts` (pure helper module).
- [x] Move `sortSearchEntries(...)` out of `state.ts`.
- [x] Export a pure function (for example `sortExplorerEntriesInMemory(...)`) with no store access.
- [x] Keep sort semantics unchanged (`Name`, `Type`, `Modified`, `Size`).

Naming rationale:
- Plain `searchSort.ts` is a pure helper module, not a hook/factory/service/store.

Acceptance:
- `state.ts` no longer contains the main in-memory sorting implementation.

### 3) Extract sort compare/key helpers (low risk)

Target:
- Small pure helpers currently embedded near in-memory sorting

- [x] Move helper functions (kind ranks, optional number compare, cached-key helpers) into `searchSort.ts` or `sortHelpers.ts`.
- [x] Keep `state.ts` free of algorithmic comparator details.
- [x] Add unit tests for sort edge-cases if there is no existing test coverage (optional but recommended). (Deferred as optional in this step; no behavior changes introduced by extraction.)

Acceptance:
- In-memory sort module is self-contained and testable.

### 4) Extract entry mutation helpers used by `entries.update(...)` (low/moderate risk)

Target:
- Repeated list update patterns (toggle star patching, remove/replace entry mutations)

- [x] Add `frontend/src/features/explorer/state/entryMutations.ts` (pure helpers).
- [x] Move repeated `entries.update(...)` transformation callbacks into named helpers.
- [x] Keep side effects and service calls in `state.ts`; move only pure list transforms.

Acceptance:
- `state.ts` entry update blocks are shorter and read as intent-level operations.

### 5) Extract sort refresh dispatch routing (moderate risk)

Target:
- `refreshForSort(...)` branching tree

- [x] Add `frontend/src/features/explorer/state/createSortRefreshDispatcher.ts` (factory) if dependencies/callbacks are injected.
- [x] Keep branch behavior unchanged for:
  - active search mode
  - `Recent`
  - `Starred`
  - `Network`
  - `Trash`
  - regular directories
- [x] Keep the factory thin (routing only, no hidden business logic).

Naming rationale:
- `create*` because this likely builds a callback from injected state accessors/loaders.

Acceptance:
- `state.ts` no longer owns a large view-routing sort-refresh branch block.

### 6) Extract small shared search-run helpers (moderate risk)

Target:
- Pure or near-pure helpers used by `runSearch(...)` (not the full stream flow yet)

- [x] Add `frontend/src/features/explorer/state/searchRuntimeHelpers.ts` (or similarly named helper file).
- [x] Move small helpers used by `runSearch(...)` that do not require store ownership (e.g. local transforms, event payload normalization).
- [x] Keep cancellation/cleanup lifecycle in `state.ts` for now.

Acceptance:
- `runSearch(...)` is still in `state.ts` but visually smaller and easier to inspect.

### 7) Extract search stream orchestration into a factory (high risk)

Target:
- `runSearch(...)` stream lifecycle (subscribe, buffer, raf flush, cleanup, cancel interplay)

- [x] Add `frontend/src/features/explorer/state/createSearchSession.ts` (factory).
- [x] Move streaming orchestration into the factory with injected store setters/getters/services/callbacks.
- [x] Keep behavior unchanged:
  - stream batching/flush timing
  - completion handling
  - cancel and stale-run protection
  - facet updates
  - error handling
- [x] Do not change UX/perf semantics in this step (no debounce policy changes here).

Naming rationale:
- `createSearchSession.ts` matches a stateful orchestration factory and `create*` export convention.

Acceptance:
- `state.ts` delegates search-session orchestration to a dedicated module while preserving observable behavior.

### 8) Import cleanup and state-folder boundary pass (no-op)

- [x] Normalize local import ordering in `state.ts` and new `state/*` modules.
- [x] Confirm no cross-feature deep imports were introduced.
- [x] Ensure each new file name matches exported API (`use*`, `create*`, helper naming).

Acceptance:
- New state modules are naming-compliant and boundary-clean.

### 9) Add `state/` README for contributor guidance (docs-only)

- [x] Add `frontend/src/features/explorer/state/README.md`.
- [x] Document:
  - what belongs in `state.ts` (composition/orchestration root)
  - what belongs in `state/*.ts` helpers/factories
  - naming rules used in this folder
  - rule: keep `createExplorerState` stable unless intentionally changing public API

Acceptance:
- A contributor can understand the state-folder boundaries in a few minutes.

## Suggested Commit Boundaries

- [ ] Commit 1: baseline map + sort extraction (`searchSort.ts`)
- [ ] Commit 2: sort helper cleanup (and optional unit tests)
- [ ] Commit 3: entry mutation helper extraction
- [ ] Commit 4: sort refresh dispatcher extraction
- [ ] Commit 5: search helper extraction (non-stream lifecycle)
- [ ] Commit 6: search session factory extraction (high-risk)
- [ ] Commit 7: import cleanup + `state/README.md`

## Manual Smoke Checklist (Run After Relevant Steps)

- [ ] Start app and open a normal directory
- [ ] Sort by `Name`, `Type`, `Modified`, `Size`
- [ ] Open `Recent`, `Starred`, `Network`, `Trash` and sort there
- [ ] Run search and sort results while search mode is active
- [ ] Cancel an active search and run a new one
- [ ] Apply text filter + column filters on a large search result set
- [ ] Toggle star on an entry in normal view and in `Starred` view

## Progress

- [x] Step 1 complete
- [x] Step 2 complete
- [x] Step 3 complete
- [x] Step 4 complete
- [x] Step 5 complete
- [x] Step 6 complete
- [x] Step 7 complete
- [x] Step 8 complete
- [x] Step 9 complete
