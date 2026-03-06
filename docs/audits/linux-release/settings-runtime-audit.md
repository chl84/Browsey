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
| Persisted roundtrip for settings surface | Strong enough to close | Backend tests now cover direct roundtrip across the Linux-facing settings surface (`showHidden`, `hiddenFilesLast`, `highContrast`, `foldersFirst`, `defaultView`, `startDir`, `confirmDelete`, `sortField`, `sortDirection`, `archiveName`, `density`, `archiveLevel`, `openDestAfterExtract`, `videoThumbs`, `cloudThumbs`, `cloudEnabled`, `hardwareAcceleration`, `ffmpegPath`, `thumbCacheMb`, `mountsPollMs`, `doubleClickMs`, `scrollbarWidth`, `rclonePath`) with dedicated companion tests for `logLevel` normalization/rejection and bounded-value fallback behavior. |
| Restore defaults | Strong enough to close | `ExplorerPage` reset flow explicitly writes all current defaults; the view-model now has a full-object equality test proving `restoreDefaults` resets the entire settings payload to `DEFAULT_SETTINGS`; and Linux e2e smoke confirms representative settings revert live in the running UI. |
| Runtime application without restart | Strong enough to close | Most settings update Svelte stores immediately through `preferencesSlice`; `useExplorerData` has direct live-application tests for mount refresh interval plus root-hook coverage for high contrast and scrollbar width; and the Linux e2e smoke now proves that representative settings changes for `confirmDelete`, `highContrast`, and `density` all apply live and restore cleanly without restart. `hardwareAcceleration` remains the only current setting explicitly documented as requiring restart in the UI. |
| Settings clarity | Meaningfully improved | The most internal-feeling Linux-facing rows now explain themselves in the UI (`Mount refresh interval`, `Log level` guidance), but a full pass on every settings row is not separately audited yet. |
| Settings migration across versions | Strong enough to close | Settings loading now runs an explicit versioned migration (`settingsSchemaVersion=1`) that normalizes/prunes legacy persisted values in the DB itself instead of only ignoring them at load time. Regression tests verify normalization for trimmed string settings and `logLevel`, canonicalization of bounded integers, and pruning of invalid enum/bool/out-of-range legacy values. |

## What Is Now Clearly Verified

- Bounded numeric Linux-facing settings reject out-of-range values through the
  Browsey settings API.
- The Linux-facing settings surface now has an explicit backend roundtrip test
  covering the persisted store/load path for:
  - `showHidden`
  - `hiddenFilesLast`
  - `highContrast`
  - `foldersFirst`
  - `defaultView`
  - `startDir`
  - `confirmDelete`
  - `sortField`
  - `sortDirection`
  - `archiveName`
  - `density`
  - `archiveLevel`
  - `openDestAfterExtract`
  - `videoThumbs`
  - `cloudThumbs`
  - `cloudEnabled`
  - `hardwareAcceleration`
  - `ffmpegPath`
  - `thumbCacheMb`
  - `mountsPollMs`
  - `doubleClickMs`
  - `scrollbarWidth`
  - `rclonePath`
- Legacy malformed or out-of-range stored values for the covered bounded
  settings degrade to `None`/default behavior rather than forcing bad runtime
  state.
- Settings loading now performs an explicit versioned migration pass and stores
  `settingsSchemaVersion = 1` after normalization/pruning.
- The migration path now rewrites persisted legacy values in-place for:
  - `archiveName`
  - `ffmpegPath`
  - `rclonePath`
  - `logLevel`
  - bounded integer settings such as `thumbCacheMb`
- The same migration path now prunes invalid persisted legacy values for:
  - enum settings such as `density`, `defaultView`, `sortField`, `sortDirection`
  - bool settings with non-boolean payloads
  - out-of-range bounded integer settings such as `mountsPollMs`
- Enum-backed loaders now reject unsupported persisted values instead of
  passing them through blindly, including `defaultView`.
- `rclonePath` persistence trims whitespace before storing and roundtrips
  cleanly.
- `hardwareAcceleration` roundtrips through persisted settings, and persisted
  `logLevel` values normalize cleanly at load time while invalid values degrade
  to `None`.
- `logLevel` remains explicitly covered through dedicated runtime-sensitive
  store/load tests in addition to migration coverage, because its store path
  applies runtime logging and therefore has a different seam than pure
  persistence-only settings.
- Restore defaults in `ExplorerPage` still routes through a single
  `DEFAULT_SETTINGS` source of truth instead of duplicating ad-hoc constants.
- `restoreDefaults` now has a full-object equality test that proves the entire
  settings payload resets back to `DEFAULT_SETTINGS`, not just a small set of
  representative fields.
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

## Conclusion

This audit still does not justify checking off all of Step 6 as a whole.

It does justify checking off:

- `Test all settings for roundtrip and restore-defaults behavior`
- `Confirm settings changes do not require restart unless explicitly documented`
- `Verify persisted settings migration across app versions`

It does justify treating Step 6 as:

- no longer vague
- partially underpinned by both backend and frontend evidence
- now closed for settings roundtrip, restore-defaults, restart behavior, and
  persisted migration on the Linux 1.0 track
