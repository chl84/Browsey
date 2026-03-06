# Linux Settings Runtime Audit

Created: 2026-03-06
Track: `docs/todo/TODO_PRODUCTION_READY_LINUX.md`
Scope: Step 6 settings, persistence, restore-defaults behavior, and runtime
application on Linux.

## Purpose

Capture the current Linux 1.0 evidence for settings behavior so the remaining
Step 6 items can be closed against something concrete instead of a vague
"settings feel okay" judgment.

## Evidence Reviewed

- `src/commands/settings/commands.rs`
- `src/commands/settings/tests.rs`
- `frontend/src/features/settings/settingsTypes.ts`
- `frontend/src/features/explorer/state/preferencesSlice.ts`
- `frontend/src/features/explorer/pages/ExplorerPage.svelte`
- `frontend/src/features/settings/hooks/useSettingsModalViewModel.test.ts`
- `frontend/e2e/explorer-smoke.e2e.ts`
- `frontend/src/features/settings/sections/PerformanceSection.svelte`
- `frontend/src/features/settings/sections/AdvancedSection.svelte`

## Status Summary

| Area | Current Linux 1.0 status | Basis |
|---|---|---|
| Persisted roundtrip for recently added Linux-facing settings | Partial but meaningful | Backend tests now cover roundtrip, invalid-input rejection, and legacy malformed-value fallback for `mountsPollMs`, `doubleClickMs`, `scrollbarWidth`, trimmed `rclonePath`, `hardwareAcceleration`, and normalized `logLevel` loads. |
| Restore defaults | Partial but meaningful | `ExplorerPage` reset flow explicitly writes all current defaults, and view-model tests now cover representative Linux-facing settings (`cloudEnabled`, `mountsPollMs`, `doubleClickMs`, `logLevel`, `rclonePath`, `scrollbarWidth`) plus filter-preservation behavior, but there is not yet a broader end-to-end reset proof across the full settings surface. |
| Runtime application without restart | Strong enough to close | Most settings update Svelte stores immediately through `preferencesSlice`; `useExplorerData` has direct live-application tests for mount refresh interval plus root-hook coverage for high contrast and scrollbar width; and the Linux e2e smoke now proves that representative settings changes for `confirmDelete`, `highContrast`, and `density` all apply live and restore cleanly without restart. `hardwareAcceleration` remains the only current setting explicitly documented as requiring restart in the UI. |
| Settings clarity | Meaningfully improved | The most internal-feeling Linux-facing rows now explain themselves in the UI (`Mount refresh interval`, `Log level` guidance), but a full pass on every settings row is not separately audited yet. |
| Settings migration across versions | Partial | Bounded loaders safely ignore malformed/out-of-range persisted values, enum loaders now also reject unsupported legacy values such as invalid `defaultView`, `density`, `sortField`, and `sortDirection`, but there is still no explicit migration/versioned-settings plan or upgrade-path validation yet. |

## What Is Now Clearly Verified

- Bounded numeric Linux-facing settings reject out-of-range values through the
  Browsey settings API.
- Legacy malformed or out-of-range stored values for the covered bounded
  settings degrade to `None`/default behavior rather than forcing bad runtime
  state.
- Enum-backed loaders now reject unsupported persisted values instead of
  passing them through blindly, including `defaultView`.
- `rclonePath` persistence trims whitespace before storing and roundtrips
  cleanly.
- `hardwareAcceleration` roundtrips through persisted settings, and persisted
  `logLevel` values normalize cleanly at load time while invalid values degrade
  to `None`.
- Restore defaults in `ExplorerPage` still routes through a single
  `DEFAULT_SETTINGS` source of truth instead of duplicating ad-hoc constants.
- Restore-defaults helper tests now cover representative Linux-facing settings,
  rather than only a single `cloudThumbs` reset seam.
- `Mount refresh interval` now has explicit live-application coverage through
  the `useExplorerData` timer subscription, rather than only code inspection.
- Linux e2e smoke now proves that representative settings changes apply live
  without restart for:
  - `confirmDelete`
  - `highContrast`
  - `density`
- The same Linux e2e smoke also proves that those representative settings
  restore cleanly back to default live state.
- The settings UI now explains the practical effect of:
  - mount refresh interval
  - log level

## Remaining Gaps Before Step 6 Can Be Closed

- add broader restore-defaults verification for representative settings beyond
  narrow view-model seams
- document or implement a more explicit settings migration story for version
  upgrades beyond the current loader-level fallback behavior

## Conclusion

This audit does not justify checking off all of Step 6 yet.

It does justify checking off:

- `Confirm settings changes do not require restart unless explicitly documented`

It does justify treating Step 6 as:

- no longer vague
- partially underpinned by both backend and frontend evidence
- mainly missing broader restore-defaults/runtime-validation coverage rather
  than first-principles design work
