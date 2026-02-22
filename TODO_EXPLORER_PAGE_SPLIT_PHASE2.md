# TODO: ExplorerPage.svelte Split (Phase 2)

Created: 2026-02-22
Goal: Continue splitting `frontend/src/features/explorer/pages/ExplorerPage.svelte` (~1880 LOC) into smaller, testable orchestration modules while preserving behavior.

## Why This Track Exists

`ExplorerPage.svelte` is already much better than the old monolithic `App.svelte`, but it still carries too much orchestration in one file:
- Large local UI/modal state surface
- Multiple long dependency wiring objects
- Layout/viewport/density sync logic mixed with feature orchestration
- Many page-level handlers that are glue (not page rendering concerns)

This track focuses on **structural extraction only** (composition cleanup), not UX changes.

## Scope (Quality-Checked)

In scope:
- Extract page-level orchestration hooks/factories from `ExplorerPage.svelte`
- Reduce local state and reactive coordination in the page
- Keep `ExplorerPage.svelte` as the composition root for refs + high-level assembly

Out of scope:
- Backend/API contract changes
- Visual redesign or interaction behavior changes
- Rewriting existing feature hooks unless needed for extraction seams
- New architecture abstractions that duplicate existing feature owners

## Constraints

- Prefer extending existing hooks before creating new ones.
- One focused commit per step (or sub-step where explicitly listed).
- Keep all user-facing behavior unchanged unless a regression fix is required.
- Keep all project docs/comments in English.

## Quality Gates (Every Step)

- [ ] `npm --prefix frontend run check` is green
- [ ] `npm --prefix frontend run lint` is green
- [ ] Relevant step-local tests are green (unit/e2e if touched)
- [ ] Browsey manual smoke for touched area is green
- [ ] `ExplorerPage.svelte` diff mostly shrinks (no accidental logic duplication)
- [ ] Commit message reflects exact step scope

## Dependencies and Risk Hotspots

Sequencing notes (important):
- Step 3 (viewport/layout extraction) is a high-regression step because it touches resize/recompute timing for list/grid virtualization and wheel behavior.
- Step 6 (lifecycle/listener extraction) depends on a clear ownership boundary from earlier steps; do not move feature logic out of existing hooks while extracting registration glue.
- Step 7 (reactive block cleanup) depends on stable behavior after Steps 3 and 6; do this only after manual smoke confirms no reactive ordering regressions.
- Step 8 (dependency builder normalization) should remain thin; it depends on final shapes from earlier extractions and should not be used to hide behavior changes.

Recommended pause points:
- Pause and manual smoke after Step 3
- Pause and manual smoke after Step 6
- Pause and manual smoke after Step 7

## Work Plan

### 1) Baseline and seam mapping (no behavior change)

- [x] Add a short comment block at top of `ExplorerPage.svelte` documenting temporary ownership boundaries (page vs hooks) if clarity is needed.
- [x] Identify extraction seams and mark them with section headers (no logic changes).
- [x] Record current `ExplorerPage.svelte` LOC and main responsibility buckets in this TODO (for progress tracking).

Baseline snapshot (recorded 2026-02-22):
- `ExplorerPage.svelte` current size: `1883` LOC
- Main responsibility buckets currently in page:
  - composition root + DOM refs/render wiring
  - view/list/grid derived state + density/layout metrics
  - navigation/search/input mode orchestration
  - file-ops/modals/context/drag-drop/input handler dependency wiring
  - page-local UI toggles and bookmark modal glue
  - app lifecycle/listener registration glue
  - `ExplorerShell` prop assembly (`sidebar/topbar/listing/menu/modal/status`)

Acceptance:
- Clear extraction map exists before moving code.

### 2) Extract shell prop assembly into a dedicated mapper/factory

Target:
- Large prop/handler object assembly passed into `ExplorerShell`
- Pure wiring/renaming logic only

- [x] Add `frontend/src/features/explorer/pages/createExplorerShellProps.ts` (or similarly named factory).
- [x] Move pure prop assembly (values + callbacks) out of `ExplorerPage.svelte`.
- [x] Keep `ExplorerShell` public API unchanged.
- [x] Keep the factory free of side effects and DOM access where possible.

Acceptance:
- `ExplorerPage.svelte` no longer contains one giant inline `ExplorerShell` props assembly object.

### 3) Extract viewport/layout/density metrics orchestration

Target:
- UI sizing and viewport update coordination
- Grid/list size metric sync
- Resize-driven recompute scheduling

- [x] Add `frontend/src/features/explorer/pages/useExplorerViewportLayout.ts`.
- [x] Move density-to-layout metric synchronization and related recompute triggers from the page.
- [x] Keep DOM refs (`rowsElRef`, `gridElRef`, `headerElRef`) owned by `ExplorerPage.svelte`; pass getters/setters into the hook.
- [x] Do not change virtualization algorithms in this step.

Acceptance:
- Page no longer owns most layout/viewport synchronization details.

### 4) Extract page-local modal/UI shell booleans and simple toggles

Target:
- `settingsOpen`, `aboutOpen`, and similar page-only UI shell toggles

- [x] Add `frontend/src/features/explorer/pages/useExplorerPageUiState.ts`.
- [x] Move simple open/close/toggle handlers for page-owned UI booleans.
- [x] Keep feature-specific modal business logic inside existing modal controllers/hooks.
- [x] Do not move bookmark modal orchestration here (covered by Step 5).
- [x] Return a small typed API for page and shell wiring.

Acceptance:
- Page local `let ...Open = ...` noise is reduced without moving business logic to the wrong layer.

### 5) Extract bookmark modal orchestration glue (page-owned flow)

Target:
- `createBookmarkModal()` integration and page glue (`openBookmarkModal`, `closeBookmarkModal`, confirm/apply path)

- [x] Add `frontend/src/features/explorer/pages/useBookmarkModalFlow.ts` (or extend existing bookmark hook if a better seam exists).
- [x] Move bookmark modal state sync + confirm flow glue out of the page.
- [x] Keep `bookmarkModalOpen` ownership clear (either inside this flow or `useExplorerPageUiState`, not split across both).
- [x] Keep bookmark service calls and toast behavior unchanged.

Acceptance:
- Bookmark modal orchestration is no longer spread across page locals + lifecycle blocks.

### 6) Extract lifecycle/listener registration glue

Target:
- `onMount`/`onDestroy` listener registration glue
- Global event subscription cleanup wiring

- [x] Add `frontend/src/features/explorer/pages/useExplorerPageLifecycle.ts`.
- [x] Move listener registration/cleanup orchestration that does not belong to feature-specific hooks.
- [x] Keep actual feature behavior in existing hooks (navigation, input, drag-drop, app lifecycle, etc.).
- [x] Avoid absorbing bookmark/modal business flows into lifecycle extraction (only registration/cleanup glue belongs here).
- [x] Avoid burying core page startup sequence; keep top-level flow readable.

Acceptance:
- `ExplorerPage.svelte` lifecycle sections become thin and declarative.

### 7) Reduce reactive block density by extracting derived-state calculators

Target:
- Large `$:` blocks that mostly compute derived values and synchronize state

- [x] Identify pure derived-state computations vs side effects.
- [x] Move pure calculations into helper functions/factories under `frontend/src/features/explorer/pages/`.
- [x] Keep side-effectful `$:` blocks in page (or in dedicated hooks if clearly owned).
- [x] Avoid changing reactive ordering semantics in this step.
- [x] Add step-local smoke focus on search/filter mode transitions and selection/view sync after reactive cleanup.

Acceptance:
- Fewer large `$:` blocks in page; remaining ones are easier to reason about.

### 8) Normalize dependency object wiring for major hooks

Target:
- Long inline dependency objects passed to:
  - `useExplorerNavigation`
  - `useExplorerFileOps`
  - `useExplorerDragDrop`
  - `useExplorerInputHandlers`
  - `useModalsController`

- [ ] Extract typed dependency builders in `pages/` (one builder per major hook, or grouped by concern).
- [ ] Keep builders close to page to avoid cross-feature leakage.
- [ ] Ensure builders are thin and do not hide behavior.
- [ ] If a builder starts owning branching logic, stop and extract a real hook/helper instead.

Acceptance:
- Hook calls remain readable and the page no longer has multiple 50+ line inline dependency object literals.

### 9) Final cleanup + dead code pass

- [ ] Remove dead locals/imports introduced by extractions.
- [ ] Re-run search checks for duplicate handlers/helpers after moves.
- [ ] Confirm no new deep-import boundary regressions were introduced.
- [ ] Update docs/changelog only if the refactor materially changes contributor-facing structure.

Acceptance:
- `ExplorerPage.svelte` is materially smaller and clearly a composition root.

## Suggested Commit Boundaries

- [ ] Commit 1: shell prop factory extraction
- [ ] Commit 2: viewport/layout orchestration extraction
- [ ] Commit 3: page UI state + bookmark modal flow extraction
- [ ] Commit 4: lifecycle glue extraction
- [ ] Commit 5: derived-state/helper extraction
- [ ] Commit 6: dependency builder normalization + cleanup

## Manual Smoke Checklist (Run After Each Relevant Step)

- [ ] Start app and open a normal directory
- [ ] Switch list/grid view and verify virtualization still renders correctly
- [ ] Keyboard navigation + Enter open + double-click open
- [ ] Context menu on item and blank area
- [ ] Drag/drop hover + drop indicators (no obvious regressions)
- [ ] Bookmark modal open/confirm/cancel
- [ ] Settings modal and About modal open/close
- [ ] Search/filter/address mode transitions + `Esc`
- [ ] Wheel scroll in list/grid (especially after viewport/layout extraction)

## Progress

- [x] Step 1 complete
- [x] Step 2 complete
- [x] Step 3 complete
- [x] Step 4 complete
- [x] Step 5 complete
- [x] Step 6 complete
- [x] Step 7 complete
- [ ] Step 8 complete
- [ ] Step 9 complete
