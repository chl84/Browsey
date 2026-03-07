# Linux User-Facing Error Audit

Created: 2026-03-07
Track: `docs/todo/TODO_PRODUCTION_READY_LINUX.md`
Scope: Step 10 user-facing error language for Linux-critical surfaces.

## Purpose

Capture the concrete Linux 1.0 progress on turning backend/domain errors into
user-facing copy at the UI boundary, without claiming every internal error in
the whole codebase has already been rewritten.

## Confirmed Progress

- `frontend/src/features/explorer/services/openWith.service.ts` now normalizes
  backend `open_with` errors by typed code before they reach the modal UI.
- The open-with UI no longer needs to show raw backend phrasing such as:
  - `Selected application is unavailable: ...`
  - `Failed to launch ...`
  - `Path must be absolute: ...`
  - `database_open_failed`
- `frontend/src/features/explorer/modals/propertiesModal.ts` now normalizes
  Linux permission/ownership failures into user-facing modal text instead of
  leaking helper- or rollback-oriented wording.
- The properties modal now replaces raw/internal phrasing such as:
  - `helper protocol error`
  - `helper_start_failed`
  - `helper_wait_failed`
  - `post_change_snapshot_failed`
  - `rollback failed (...). System may be partially changed`
  with clearer user-facing messages.
- Permission-apply failures in the properties modal now surface a toast with
  normalized wording instead of silently snapping the UI back with no clear
  explanation.
- Cloud UI surfaces already had typed-code-based user-facing normalization in
  `frontend/src/features/network/cloud.service.ts`, and this round brings the
  same principle to local Linux-critical permissions/open-with flows.

## Verification

- `frontend/src/features/explorer/modals/propertiesModal.test.ts`
  - ownership helper failures show normalized modal copy
  - permission update failures show normalized toast copy
- `frontend/src/features/explorer/services/openWith.service.test.ts`
  - `app_not_found` maps to user-facing text
  - `launch_failed` maps to user-facing text

## What This Audit Justifies

This audit justifies checking off:

- `Ensure error surfaces show user-facing language rather than internal phrasing`

Reason:

- the remaining obvious Linux-critical UI seams that still exposed internal
  backend language are now normalized at the frontend boundary using typed
  codes.

## Remaining Step 10 Gaps

- Step 10 still includes a broader support/logging quality bar than UI copy
  alone.
- Internal/backend error messages still exist for logs, tests, and low-level
  plumbing; this audit only claims the user-facing Linux-critical surfaces are
  now materially improved.

## Conclusion

This audit supports treating the Linux 1.0 user-facing error-language item as
complete for the Linux-critical UI surfaces that were still leaking internal
phrasing.

It does not yet justify checking off:

- `Ensure logs are useful for real support/debug cases`
