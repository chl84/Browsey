# Linux Hook/State Coverage Audit

Created: 2026-03-06
Track: `docs/todo/TODO_PRODUCTION_READY_LINUX.md`
Scope: Step 7 frontend hooks/state seams that are prone to regress under small changes.

## Purpose

Capture whether the Linux 1.0 stabilization track now has enough targeted
frontend hook/state coverage to stop treating this as an obvious testing gap.

## Evidence Reviewed

- `frontend/src/features/explorer/state.test.ts`
- `frontend/src/features/explorer/state/stores.test.ts`
- `frontend/src/features/explorer/hooks/useExplorerData.test.ts`
- `frontend/src/features/explorer/hooks/createActivity.test.ts`
- `frontend/src/features/explorer/navigation/useExplorerNavigation.test.ts`
- `frontend/src/features/explorer/file-ops/useExplorerFileOps.test.ts`
- `frontend/src/features/explorer/file-ops/useExplorerDragDrop.test.ts`
- `frontend/src/features/explorer/context/useExplorerContextMenuOps.test.ts`
- `frontend/src/features/explorer/context/createContextActions.test.ts`
- `frontend/src/features/settings/hooks/useSettingsModalViewModel.test.ts`
- `frontend/src/features/settings/cloudSetup.test.ts`
- `frontend/e2e/explorer-smoke.e2e.ts`

## Confirmed Coverage

- `createExplorerState` has regression coverage for:
  - local vs cloud sort-refresh behavior
  - network onboarding hints and non-cloud resilience
  - cloud-disabled behavior
- `createExplorerStores` has direct timer/store behavior coverage for delayed
  loading visibility.
- `useExplorerData` has direct subscription/runtime-application coverage for:
  - cloud background refresh routing
  - root high-contrast hook
  - root scrollbar-width hook
  - mount refresh interval live updates
- `createActivity` has direct activity-pill state coverage for byte progress
  and cancel-state transitions.
- `useExplorerNavigation` has targeted cloud path routing coverage for file vs
  directory vs missing-stat cases.
- `useExplorerFileOps` has explicit conflict-preview coverage across:
  - cloud-to-cloud
  - local-to-cloud
  - cloud-to-local
  - self-paste auto-rename behavior
- `useExplorerDragDrop` has direct bookmark-drop coverage through the shared
  paste/move routing path.
- `useExplorerContextMenuOps` and `createContextActions` cover menu filtering,
  action routing, and clipboard/system-clipboard fallback behavior.
- `createSettingsModalViewModel` and `cloudSetup` cover settings filter state,
  restore-defaults behavior, onboarding state mapping, and cloud section
  visibility.
- Linux-focused e2e smoke now exercises representative cross-layer state
  behavior for:
  - search
  - paste failure and recovery
  - settings live updates and restore defaults

## Conclusion

This audit supports checking off:

- `Increase coverage around hooks/state seams that break under small changes`

It does not imply that every frontend hook/state seam is fully closed forever.
It means the Linux 1.0 track now has meaningful, targeted regression coverage
over the areas that were most likely to break from small reactive/store changes.
