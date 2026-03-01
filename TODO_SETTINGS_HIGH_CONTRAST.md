# TODO: Finish Settings High Contrast Wiring

Created: 2026-03-01
Goal: Fully wire the `Accessibility -> High contrast` setting so it persists correctly, affects the live app globally, and remains consistent with the rest of the Settings architecture.

## Current State

What exists:
- `High contrast` row is present in the Settings modal UI.
- `highContrast` exists in the frontend `Settings` type and default settings model.
- Search/filter metadata in the Settings modal already knows about the row.

What is missing:
- No frontend settings-service load/store wiring for `highContrast`
- No backend settings commands for `highContrast`
- No explorer state store for `highContrast`
- No preference-slice load/store flow for `highContrast`
- No global root hook (`body` / `:root` attribute or class) that turns the setting into active UI behavior
- No actual contrast overrides tied to the setting

## Architecture Alignment

This track should stay aligned with:
- `ARCHITECTURE_NAMING.md`
- existing Settings/data-flow conventions already used for other persisted preferences

Implications:
- Persist the setting through the same backend settings pipeline as other booleans
- Keep Settings modal as a view surface, not the owner of side effects
- Apply the visual effect through one global app hook, not ad-hoc component conditionals
- Treat high contrast as an overlay on top of the existing theme system, not a separate theme

## Scope

In scope:
- Persistence for `highContrast`
- Explorer state/store wiring
- Global DOM/theme hook
- First-pass contrast improvements for core UI surfaces
- Test coverage for persistence and UI hook behavior

Out of scope:
- A full accessibility redesign
- Per-component exceptions unless required for readability or regressions
- New theme families

## Work Plan

### 1) Add persistence wiring

- [x] Add `store_high_contrast` and `load_high_contrast` in `src/commands/settings/mod.rs`
- [x] Add matching frontend service functions in `frontend/src/features/explorer/services/settings.service.ts`
- [x] Follow the same naming and storage conventions used by the other boolean settings

Acceptance:
- `highContrast` can be stored and loaded from the existing settings database.

### 2) Add explorer state wiring

- [x] Add a `highContrast` writable store in `frontend/src/features/explorer/state/stores.ts`
- [x] Load persisted value in `frontend/src/features/explorer/state/preferencesSlice.ts`
- [x] Add a setter/toggle flow in `preferencesSlice.ts`
- [x] Pass the live value through the existing `ExplorerPage -> SettingsModal` prop chain

Acceptance:
- Toggling the setting changes live frontend state and survives restart.

### 3) Add a single global app hook

- [x] Choose one root mechanism:
  - `document.documentElement.dataset.highContrast = "true" | "false"`
  - or `body.classList.toggle(...)`
- [x] Apply the hook in one central frontend location, not in multiple components
- [x] Ensure it is updated both on initial load and on live setting changes

Acceptance:
- The app has exactly one global source of truth for “high contrast is active”.

### 4) Implement first-pass contrast overrides

- [ ] Add scoped CSS variable overrides driven by the global hook
- [ ] Start with the highest-value surfaces:
  - focus ring visibility
  - borders and separators
  - muted text readability
  - selection highlight
  - input/button contrast
  - modal/context-menu surfaces
- [ ] Keep the delta deliberate and small; avoid introducing a third full theme

Acceptance:
- The setting has a visible, global effect in both dark and light themes.

### 5) Validate against theme interactions

- [ ] Confirm the high-contrast overlay works in dark theme
- [ ] Confirm the high-contrast overlay works in light theme
- [ ] Confirm no unreadable combinations are introduced for hover/selected/focus states

Acceptance:
- High contrast behaves as a theme overlay rather than breaking theme assumptions.

### 6) Add tests

- [ ] Backend test coverage for `store_high_contrast` / `load_high_contrast`
- [ ] Frontend test coverage for settings-service / preference-slice wiring
- [ ] Frontend test for the global root hook toggle
- [ ] Add at least one focused regression test for “setting persists and reapplies on load”

Acceptance:
- The behavior is protected at both the persistence and UI-application layers.

## Quality Gates

- [ ] `cargo fmt --all`
- [ ] `cargo check -q`
- [ ] `npm --prefix frontend run check`
- [ ] Focused frontend tests pass
- [ ] No duplicate high-contrast logic appears across multiple components

## Commit Strategy

- [ ] Commit 1: persistence + service wiring
- [ ] Commit 2: explorer state wiring + global hook
- [ ] Commit 3: CSS variable overrides + tests
- [ ] Archive this TODO under `docs/todo-archive/` when complete
