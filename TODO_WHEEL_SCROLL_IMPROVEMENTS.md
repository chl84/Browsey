# TODO: Wheel Scroll Improvements

## Scope
- Review target: Explorer list/grid wheel scrolling behavior.
- Goal: make fast wheel rotation feel consistent near top/bottom edges.

## Audit Notes
- [x] Checked `frontend/src/app.css` for wheel-scroll logic.
- [x] Confirmed there is no JavaScript wheel handler in `app.css`.
- [x] Noted `scroll-behavior: smooth` on `html, body` (`frontend/src/app.css`) as a potential indirect contributor to perceived latency.

## Findings-Driven TODO
- [x] Keep only one wheel strategy (`always`) and remove dual-mode branching.
- [x] Centralize wheel tuning in one place (`explorerWheelAssistConfig` in helper).
- [x] Remove no-op wheel plumbing from list/grid state layers.
- [x] Fix inconsistent ownership of wheel gestures in `frontend/src/features/explorer/helpers/wheelScrollHelper.ts`.
- [x] Remove mid-gesture custom/native switching near edges (clamp path should not silently fall back to native behavior).
- [x] Rework acceleration to use burst-aware velocity (time-window or frame-based), not only per-event `deltaY`.
- [x] Recalibrate edge-distance math to align with visible content bounds (account for container padding/gutters).
- [x] Define one deterministic strategy for non-cancelable wheel events during high-rate input.
- [x] Verify interaction with `overscroll-behavior-y: contain` in list/grid containers and keep behavior consistent.
- [x] Enable full custom wheel ownership in Explorer (`mode: always`) instead of native-only long-range handling.
- [ ] Compare wheel feel against Nautilus baseline and tune constants (`minWheelStepPx`, `accelerationScale`, `velocityGain`, `wheelTickBoost`).
- [x] Decide whether `scroll-behavior: smooth` should be disabled for app shell scrolling contexts.

## Test Plan
- [x] Add unit test: cancelable vs non-cancelable wheel event sequences.
- [x] Add unit test: rapid wheel bursts near top/bottom.
- [x] Add unit test: line/page/pixel `deltaMode` normalization.
- [x] Add E2E smoke coverage for short list/grid wheel behavior at boundaries.
- [ ] Manual verification: list view with short content.
- [ ] Manual verification: grid view with short content.
- [ ] Manual verification: mouse wheel and touchpad devices.
- [ ] Manual verification: top edge, bottom edge, and mid-range behavior.

## Exit Criteria
- [ ] No visible speed drop when rapidly scrolling near top/bottom in short lists/grids.
- [ ] No oscillation between custom and native behavior within one wheel burst.
- [ ] Behavior remains stable on Fedora/WebKitGTK runtime used by Browsey.
