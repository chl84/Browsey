# TODO: App.svelte Split

Created: 2026-02-19
Goal: Split `frontend/src/App.svelte` (~3100 LOC) into smaller feature-owned modules while preserving behavior.

## Quality-Checked Scope

This plan is validated against current structure:
- Existing hooks already cover large areas (`useExplorerData`, `useExplorerDragDrop`, `createContextActions`, `createAppLifecycle`, `createTopbarActions`, etc.).
- Remaining heavy logic in `App.svelte` is mostly orchestration glue and handlers.
- Strategy: extend existing hooks first, add new hooks only where there is no clear owner.

## Constraints (keep it simple)

- No backend contract changes.
- No visual/UX redesign.
- No `state.ts` split in this track.
- No behavior refactor mixed with structural moves.
- One focused commit per step (or sub-step where explicitly listed).

## Quality Gates (every step)

- [ ] `npm --prefix frontend run check` is green.
- [ ] `npm --prefix frontend run build` is green.
- [ ] App starts (`cargo tauri dev --no-dev-server`) and basic navigation works.
- [ ] Step-local `rg` checks for moved symbols/imports are green.
- [ ] Commit message reflects exact scope.

## Work Plan

### 1) Create thin entry wrapper

- [x] Create `frontend/src/features/explorer/pages/ExplorerPage.svelte` by moving current `App.svelte` content as-is.
- [x] Reduce `frontend/src/App.svelte` to a thin wrapper that only renders `ExplorerPage`.
- [x] Run step-local checks:
  - `rg -n "from './features/" frontend/src/features/explorer/pages/ExplorerPage.svelte` returns empty.
  - `rg -n "ExplorerShell" frontend/src/App.svelte` returns empty.

Acceptance:
- No behavior change.
- Diff is mostly move/import adjustments.

### 2) Extract navigation/load orchestration

Target from current `App.svelte` sections:
- `loadDir/loadRecent/loadStarred/loadNetwork/loadTrash`, `goBack/goForward`, `goToPath/openPartition`, pending-nav queue.

- [x] Add `frontend/src/features/explorer/hooks/useExplorerNavigation.ts`.
- [x] Reuse existing services/state calls; do not duplicate logic in multiple hooks.
- [x] Keep selection snapshot/restore behavior tied to navigation (`captureSelectionSnapshot`, `restoreSelectionForCurrent`, `centerSelectionIfAny`).
- [x] Run step-local check:
  - `rg -n "const (loadDir|loadRecent|loadStarred|loadNetwork|loadTrash|goBack|goForward|goToPath|openPartition) = async" frontend/src/features/explorer/pages/ExplorerPage.svelte` returns empty.

Acceptance:
- Back/forward/home/up/network connect/navigation remains stable.

### 3) Extract search/session + path input orchestration

Target from current `App.svelte` sections:
- Search session helpers and mode transitions (`normalizeSearchQuery`, `resetSearchSession`, `markSearchResultsStale`, `transitionTo*`, `setSearchModeState`, path submit/search submit).

- [x] Add `frontend/src/features/explorer/hooks/useExplorerSearchSession.ts`.
- [x] Move only search/path-session orchestration there.
- [x] Keep component event bindings unchanged.
- [x] Run step-local check:
  - `rg -n "const (normalizeSearchQuery|resetSearchSession|markSearchResultsStale|transitionToAddressMode|transitionToFilterMode|setSearchModeState) =" frontend/src/features/explorer/pages/ExplorerPage.svelte` returns empty.

Acceptance:
- Search mode behaves exactly as before.

### 4) Extract file operation orchestration (paste/extract/duplicates/stats)

Target from current `App.svelte` sections:
- `computeDirStats`, paste/conflict flow (`handlePasteOrMove`, `pasteIntoCurrent`, `resolveConflicts`), extract flow, duplicate progress lifecycle.

- [x] Add `frontend/src/features/explorer/hooks/useExplorerFileOps.ts`.
- [x] Sub-step A: move paste/conflict flow.
- [x] Sub-step B: move extraction flow.
- [x] Sub-step C: move duplicate scan progress lifecycle.
- [x] Sub-step D: move `computeDirStats` orchestration.
- [ ] Keep existing service calls, activity labels, and toast messages unless strictly needed.

Acceptance:
- Copy/cut/paste + conflict modal + extract + duplicate scan still behave identically.

### 5) Extract context menu orchestration

Target from current `App.svelte` sections:
- `loadAndOpenContextMenu`, blank context menu open/select, row context menu delegates.

- [ ] Add `frontend/src/features/explorer/hooks/useExplorerContextMenuOps.ts`.
- [ ] Reuse `createContextMenus` and `createContextActions`; avoid parallel abstractions.
- [ ] Run step-local check:
  - `rg -n "const (loadAndOpenContextMenu|handleBlankContextMenu|handleBlankContextAction|handleContextSelect) =" frontend/src/features/explorer/pages/ExplorerPage.svelte` returns empty.

Acceptance:
- Context menus unchanged in dir/recent/trash/network/search contexts.

### 6) Extract input/pointer handler orchestration

Target from current `App.svelte` sections:
- Document keydown/keyup, row/grid pointer handlers, click-open/double-click flow, scroll/hover suppression.

- [ ] Prefer extending existing hooks first (`createGlobalShortcuts`, `createGridKeyboardHandler`, `createSelectionBox`, `useExplorerDragDrop`).
- [ ] Add `frontend/src/features/explorer/hooks/useExplorerInputHandlers.ts` only if composition still needs a dedicated owner after extensions.
- [ ] Keep all handler signatures that `ExplorerShell` depends on.

Acceptance:
- Selection, lasso, keyboard nav, and open-on-click behavior unchanged.

### 7) Final cleanup

- [ ] Remove dead locals/imports left in `ExplorerPage.svelte`.
- [ ] Keep `ExplorerPage.svelte` as composition root only.
- [ ] Add short note to docs/changelog describing structural split.

Acceptance:
- `ExplorerPage.svelte` is significantly smaller and mostly wiring.

## Smoke Checklist (manual)

- [ ] Navigate: home/up/back/forward/network/trash.
- [ ] Selection: click/shift/ctrl/meta/lasso.
- [ ] DnD and clipboard: copy/cut/paste + conflict modal.
- [ ] Context menus: row + blank area in each view.
- [ ] Modals: rename, advanced rename, properties, open with, settings.
- [ ] Undo/redo.
- [ ] Extract/compress/duplicate scan flows.
- [ ] Search/filter transitions: address mode, filter mode, full search mode, and `Esc` exit behavior.

## Progress

- [x] Step 1 complete
- [x] Step 2 complete
- [x] Step 3 complete
- [x] Step 4 complete
- [x] Step 4A complete
- [x] Step 4B complete
- [x] Step 4C complete
- [x] Step 4D complete
- [ ] Step 5 complete
- [ ] Step 6 complete
- [ ] Step 7 complete
