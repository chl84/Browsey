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
- `src/commands/open_with/mod.rs` now rejects relative paths explicitly before
  filesystem sanitization in both `list_open_with_apps` and `open_with`.
- `src/commands/open_with/error.rs` no longer needs message-pattern
  reclassification for `path_not_absolute`.
- `src/commands/open_with/mod.rs` now maps `fs::open_entry` failures through
  typed `ApiError.code` values instead of reclassifying via
  `from_external_message(error.message)`.
- `src/commands/open_with/linux.rs` now uses explicit typed constructors for
  Linux desktop-entry failures instead of text-driven reclassification for
  `app_not_found`, `launch_failed`, and empty `Exec` input.
- `src/commands/listing/local.rs` now maps `read_dir` failures through typed
  `ListingError::from_io_error(...)` instead of
  `ListingError::from_external_message(format!(...))`.
- `src/commands/listing/error.rs` no longer carries an unused
  message-classification path for listing I/O failures after that seam was
  removed.
- `src/commands/fs/error.rs` now exposes typed `FsError::from_io_error(...)`
  for direct `std::io::Error` classification instead of forcing Linux-critical
  fs/trash flows back through message parsing.
- `src/commands/fs/trash/move_ops.rs` now maps backup-directory
  `create_dir_all(...)` failures through typed `FsError::from_io_error(...)`
  instead of `FsError::from_external_message(format!(...))`.
- `src/commands/fs/delete_ops.rs` now maps backup-directory
  `create_dir_all(...)` failures through typed `FsError::from_io_error(...)`
  instead of constructing string-only `delete_failed` errors.
- `src/commands/fs/trash/staging.rs` now maps trash stage journal directory
  creation failures through typed `FsError::from_io_error(...)` instead of
  relying on string-only `trash_failed` construction.
- `src/commands/fs/trash/staging.rs` now also maps trash stage journal
  `remove_file(...)`, `write(...)`, and `rename(...)` failures through typed
  `FsError::from_io_error(...)` instead of one-off string-only `trash_failed`
  construction.
- Existing backend hardening controls already provide a meaningful baseline:
  - `.semgrep/typed-errors-blocking.yml`
  - `.semgrep/typed-errors.yml`
  - `scripts/maintenance/check-backend-error-hardening-guard.sh`
  - `docs/ERROR_HARDENING_EXCEPTION_POLICY.md`
- Linux 1.0 release gating now references those controls explicitly in:
  - `docs/operations/linux-release/pre-release-checklist.md`
  - `docs/operations/linux-release/release-bar.md`

## Remaining Step 10 Gaps

- Linux-critical flows still contain some runtime string-based classification
  seams, but the obvious `console`, ownership/pkexec retry, and `open_with`
  path/app-launch classification examples are now removed.
- Several backend domains still use one-off error mapping or raw message
  forwarding even when they already have typed error containers; the local
  delete/trash backup-dir seams are reduced, but Step 10 still has broader
  cleanup left.
- Step 10 also includes supportability/logging quality, which is broader than
  typed-error cleanup alone.

## Conclusion

This audit supports treating Step 10 as `in progress`, not complete.

It justifies saying that Linux-critical error handling is moving in the right
direction.

It also justifies checking off the policy/process item:

- `Require all new or modified error-handling code to use the Browsey error API`

It does not yet justify checking off:

- `Remove remaining stringly or one-off error seams from Linux-critical flows`
- `Ensure logs are useful for real support/debug cases`
- `Ensure error surfaces show user-facing language rather than internal phrasing`
