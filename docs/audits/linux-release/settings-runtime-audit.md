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
- `frontend/src/features/settings/sections/PerformanceSection.svelte`
- `frontend/src/features/settings/sections/AdvancedSection.svelte`

## Status Summary

| Area | Current Linux 1.0 status | Basis |
|---|---|---|
| Persisted roundtrip for recently added Linux-facing settings | Partial but meaningful | Backend tests now cover roundtrip, invalid-input rejection, and legacy malformed-value fallback for `mountsPollMs`, `doubleClickMs`, `scrollbarWidth`, and trimmed `rclonePath`. |
| Restore defaults | Partial | `ExplorerPage` reset flow explicitly writes all current defaults, and view-model tests cover the restore-defaults seam, but there is not yet a broad integration test proving representative settings all reset end-to-end. |
| Runtime application without restart | Partial | Most settings update Svelte stores immediately through `preferencesSlice`, `useExplorerData` now has direct live-application tests for mount refresh interval plus existing root-hook coverage for high contrast and scrollbar width, and `hardwareAcceleration` is the only current setting explicitly documented as requiring restart in the UI. This is meaningful evidence, but not yet broad enough to close the whole item. |
| Settings clarity | Meaningfully improved | The most internal-feeling Linux-facing rows now explain themselves in the UI (`Mount refresh interval`, `Log level` guidance), but a full pass on every settings row is not separately audited yet. |
| Settings migration across versions | Open | Bounded loaders safely ignore malformed/out-of-range persisted values for some settings, but there is no explicit migration/versioned-settings plan or upgrade-path validation yet. |

## What Is Now Clearly Verified

- Bounded numeric Linux-facing settings reject out-of-range values through the
  Browsey settings API.
- Legacy malformed or out-of-range stored values for the covered bounded
  settings degrade to `None`/default behavior rather than forcing bad runtime
  state.
- `rclonePath` persistence trims whitespace before storing and roundtrips
  cleanly.
- Restore defaults in `ExplorerPage` still routes through a single
  `DEFAULT_SETTINGS` source of truth instead of duplicating ad-hoc constants.
- `Mount refresh interval` now has explicit live-application coverage through
  the `useExplorerData` timer subscription, rather than only code inspection.
- The settings UI now explains the practical effect of:
  - mount refresh interval
  - log level

## Remaining Gaps Before Step 6 Can Be Closed

- add broader restore-defaults verification for representative settings beyond
  narrow view-model seams
- add runtime-focused checks that representative settings actually apply live in
  the Linux UI without restart where that is the intended contract
- document or implement a more explicit settings migration story for version
  upgrades

## Conclusion

This audit does not justify checking off all of Step 6 yet.

It does justify treating Step 6 as:

- no longer vague
- partially underpinned by both backend and frontend evidence
- mainly missing broader restore-defaults/runtime-validation coverage rather
  than first-principles design work
