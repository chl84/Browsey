# TODO: Naming Conventions Finetuning

Created: 2026-02-21  
Scope: Frontend (`frontend/src`)  
Goal: Standardize naming and placement conventions without behavior changes.

## Progress

- [x] Create this TODO file in project root.
- [ ] Work through phases below and check off continuously.

## Constraints

- No runtime behavior changes.
- No large mixed commits; keep changes small and reversible.
- Run `lint/check/build` for each rename batch.
- Manual Browsey smoke-test before commit when UI-facing files are touched.

## Phase 1: Define Rules

- [x] Document canonical naming rules:
  - `*.service.ts` only for backend/service boundary calls.
  - `use*.ts` only for stateful hooks/composables.
  - `create*.ts` (or `*.factory.ts`) for factory helpers.
  - `*.store.ts` only for Svelte stores.
  - `types.ts` only for shared domain types.
- [x] Confirm rule exceptions (if any) and list them explicitly.

Acceptance:
- Rules are explicit, short, and unambiguous.

Rule Details:
- `use*.ts` must export a primary `use*` symbol as default naming convention.
- If a file exports only `create*`, filename should be `create*.ts` (or `*.factory.ts`) in a follow-up rename batch.
- `services/*.service.ts` may call Tauri `invoke` directly or other platform bridge APIs (`@tauri-apps/*`) for boundary operations.
- Domain `index.ts` files are allowed to re-export both `use*` and `create*`.

Current Exceptions (temporary, to clean in Phase 3/4):
- Multiple `use*.ts` files currently export only `create*` symbols (see audit findings below).

## Phase 2: Audit Current Code

- [x] Build candidate list for naming mismatches:
  - `use*.ts` files that are pure factories.
  - files in `services/` that are not service-boundary calls.
  - `*.store.ts` files that do not expose stores.
- [x] Group findings by risk:
  - low-risk rename-only
  - medium-risk (touches many imports)
  - postpone/defer

Acceptance:
- We have a concrete, ordered rename list.

Audit Findings (2026-02-21):
- `services/*.service.ts` are consistent with boundary rule.
  - Exception that is still acceptable by rule: `frontend/src/features/explorer/services/nativeDrag.service.ts` (platform bridge via plugin/path APIs, no `invoke`).
- `*.store.ts` are consistent with store rule:
  - `frontend/src/features/explorer/file-ops/clipboard.store.ts`
  - `frontend/src/features/explorer/state/list.store.ts`
- Naming mismatches are concentrated in `use*.ts` files that export only `create*`.

Low-Risk Rename Candidates (rename-only, local import surface):
- `frontend/src/features/explorer/navigation/useTopbarActions.ts` -> `createTopbarActions.ts`
- `frontend/src/features/explorer/navigation/useViewAnchor.ts` -> `createViewAnchor.ts`
- `frontend/src/features/explorer/context/useTextContextMenu.ts` -> `createTextContextMenu.ts`
- `frontend/src/features/explorer/context/useContextMenus.ts` -> `createContextMenus.ts`
- `frontend/src/features/explorer/context/useContextActions.ts` -> `createContextActions.ts`

Medium-Risk Candidates (used broadly or central in page wiring):
- `frontend/src/features/explorer/hooks/useActivity.ts` -> `createActivity.ts`
- `frontend/src/features/explorer/hooks/useAppLifecycle.ts` -> `createAppLifecycle.ts`
- `frontend/src/features/explorer/hooks/useBookmarkModal.ts` -> `createBookmarkModal.ts`
- `frontend/src/features/explorer/hooks/useColumnWidths.ts` -> `createColumnResize.ts`
- `frontend/src/features/explorer/hooks/useGridHandlers.ts` -> `createGridKeyboardHandler.ts`
- `frontend/src/features/explorer/hooks/useNewFileTypeHint.ts` -> `createNewFileTypeHint.ts`
- `frontend/src/features/explorer/hooks/useShortcuts.ts` -> `createGlobalShortcuts.ts`
- `frontend/src/features/explorer/file-ops/useClipboard.ts` -> `createClipboard.ts`
- `frontend/src/features/explorer/file-ops/useNativeFileDrop.ts` -> `createNativeFileDrop.ts`
- `frontend/src/features/explorer/selection/useSelectionBox.ts` -> `createSelectionBox.ts`
- `frontend/src/features/explorer/ui-shell/hooks/useViewObservers.ts` -> `createViewObservers.ts`

Postpone/Defer (ambiguous naming intent, revisit after rename batches):
- `frontend/src/features/settings/hooks/useSettingsModalViewModel.ts` (create-style export, but “hook” semantics may be intentional)
- `frontend/src/features/explorer/ui-shell/hooks/useGridVirtualizer.ts` (`use*` export aligns with name; keep)

## Phase 3: Low-Risk Renames

- [x] Rename low-risk files first (small batches).
- [x] Update imports and barrels after each batch.
- [x] Validate each batch:
  - `npm --prefix frontend run lint`
  - `npm --prefix frontend run check`
  - `npm --prefix frontend run build`
- [x] Manual smoke-test on UI-relevant batches.
- [x] Commit each batch separately.

Completed in current batch (2026-02-21):
- `frontend/src/features/explorer/context/useContextActions.ts` -> `frontend/src/features/explorer/context/createContextActions.ts`
- `frontend/src/features/explorer/context/useContextMenus.ts` -> `frontend/src/features/explorer/context/createContextMenus.ts`
- `frontend/src/features/explorer/context/useTextContextMenu.ts` -> `frontend/src/features/explorer/context/createTextContextMenu.ts`
- `frontend/src/features/explorer/navigation/useTopbarActions.ts` -> `frontend/src/features/explorer/navigation/createTopbarActions.ts`
- `frontend/src/features/explorer/navigation/useViewAnchor.ts` -> `frontend/src/features/explorer/navigation/createViewAnchor.ts`

Acceptance:
- Low-risk inconsistencies are cleaned up with green checks/tests.

## Phase 4: Domain Consistency Cleanup

- [ ] Standardize naming within each explorer domain:
  - `context`
  - `navigation`
  - `selection`
  - `file-ops`
  - `ui-shell`
  - `state`
- [ ] Ensure each domain has clear entrypoint (`index.ts`) if externally consumed.
- [ ] Remove leftover ambiguous names where safe.

Started (current uncommitted batch, 2026-02-21):
- `frontend/src/features/explorer/hooks/useActivity.ts` -> `frontend/src/features/explorer/hooks/createActivity.ts`
- `frontend/src/features/explorer/hooks/useAppLifecycle.ts` -> `frontend/src/features/explorer/hooks/createAppLifecycle.ts`
- `frontend/src/features/explorer/hooks/useBookmarkModal.ts` -> `frontend/src/features/explorer/hooks/createBookmarkModal.ts`
- `frontend/src/features/explorer/hooks/useColumnWidths.ts` -> `frontend/src/features/explorer/hooks/createColumnResize.ts`
- `frontend/src/features/explorer/hooks/useGridHandlers.ts` -> `frontend/src/features/explorer/hooks/createGridKeyboardHandler.ts`
- `frontend/src/features/explorer/hooks/useNewFileTypeHint.ts` -> `frontend/src/features/explorer/hooks/createNewFileTypeHint.ts`
- `frontend/src/features/explorer/hooks/useShortcuts.ts` -> `frontend/src/features/explorer/hooks/createGlobalShortcuts.ts`
- `lint/check/build` green.
- Manual Browsey smoke-test green.

Acceptance:
- Naming is consistent within and across domains.

## Phase 5: Enforcement

- [ ] Add lightweight enforcement for naming/placement drift:
  - lint restrictions and/or static checks.
- [ ] Keep enforcement focused (avoid noisy broad rules).

Acceptance:
- New violations are caught automatically in CI/local lint.

## Phase 6: Documentation

- [ ] Add `ARCHITECTURE_NAMING.md` with:
  - conventions
  - examples
  - quick decision checklist
- [ ] Link naming doc from relevant README/architecture docs if needed.

Acceptance:
- Team has one clear naming reference.

## Final Definition of Done

- [ ] Naming rules documented and agreed.
- [ ] Low-risk rename backlog completed.
- [ ] Medium-risk rename backlog completed.
- [ ] Enforcement active.
- [ ] `lint/check/build` green.
- [ ] Manual smoke-test green.
