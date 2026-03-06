# Linux Error Surface Audit

Created: 2026-03-06
Track: `docs/todo/TODO_PRODUCTION_READY_LINUX.md`
Scope: Step 10 observability/supportability and Linux-critical error handling.

## Purpose

Capture concrete Linux 1.0 progress on error-surface hardening without
pretending the whole Step 10 track is done after isolated seam fixes.

## Confirmed Progress

- `src/commands/console.rs` now rejects relative paths explicitly before
  filesystem sanitization and returns typed `path_not_absolute`.
- `src/commands/console/error.rs` no longer reclassifies `FsUtilsError` by
  parsing `error.to_string().contains("path must be absolute")`.
- `src/commands/permissions/ownership/unix.rs` no longer decides pkexec retry
  by parsing `"requires elevated privileges"` from the error message; it now
  uses typed `UndoErrorCode::PermissionDenied`.
- Existing backend hardening controls already provide a meaningful baseline:
  - `.semgrep/typed-errors-blocking.yml`
  - `.semgrep/typed-errors.yml`
  - `scripts/maintenance/check-backend-error-hardening-guard.sh`
  - `docs/ERROR_HARDENING_EXCEPTION_POLICY.md`

## Remaining Step 10 Gaps

- Linux-critical flows still contain some runtime string-based classification
  seams, but the obvious `console` and ownership/pkexec retry examples are now
  removed.
- Several backend domains still use one-off error mapping or raw message
  forwarding even when they already have typed error containers.
- Step 10 also includes supportability/logging quality, which is broader than
  typed-error cleanup alone.

## Conclusion

This audit supports treating Step 10 as `in progress`, not complete.

It justifies saying that Linux-critical error handling is moving in the right
direction, but not yet checking off:

- `Require all new or modified error-handling code to use the Browsey error API`
- `Remove remaining stringly or one-off error seams from Linux-critical flows`
- `Ensure logs are useful for real support/debug cases`
- `Ensure error surfaces show user-facing language rather than internal phrasing`
