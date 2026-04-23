# Linux Logging Surface Audit

Created: 2026-03-06
Track: `docs/todo/TODO_PRODUCTION_READY_LINUX.md`
Scope: Step 10 observability/supportability for Linux-critical browse/open/search
and duplicates flows.

## Purpose

Capture the concrete Linux 1.0 progress on backend logging quality without
pretending the whole observability track is done.

## Confirmed Progress

- `src/commands/fs/open_ops.rs` now treats recent-list bookkeeping as a
  debug-only best-effort side effect instead of warning-level noise.
- `src/commands/fs/open_ops.rs` now emits structured warning logs for failed
  local opens, GVFS open failures, GVFS timeouts, and GVFS channel failures,
  including explicit `path`, `error`, and `timeout_secs` fields where
  applicable.
- `src/commands/open_with/mod.rs` now treats `touch_recent(...)` failure as a
  structured debug log instead of a warning, keeping app-launch logs focused on
  the actual launch outcome.
- `src/commands/search/worker.rs` now logs directory traversal failures with
  structured `path` and `error` fields, while keeping permission-denied cases
  at `debug` and only escalating other traversal failures to `warn`.
- `src/commands/duplicates/scan.rs` now logs duplicate-tree walk failures with
  structured `path` and `error` fields.
- `src/commands/duplicates/scan.rs` now downgrades per-candidate compare
  failures to `debug`, because the scan continues and these failures were a
  noisy best-effort detail rather than a release-blocking warning by
  themselves.
- `src/main.rs` now records startup context in the structured log stream when
  Info/Debug logging is enabled, including Browsey version, build profile,
  target OS/arch, and the active log file path.
- `src/main.rs` now records runtime log-level changes, which makes support logs
  clearer when a user intentionally turns on Debug logging to reproduce an
  issue.
- `docs-site/src/content/pages.ts` now tells Linux users exactly how to collect
  support logs: turn log level up to `Debug`, reproduce, and attach
  `browsey.log` / `browsey.log.1`.
- Existing Linux-critical areas already had structured logs in place and remain
  aligned with this direction:
  - `src/commands/listing/local.rs`
  - `src/commands/listing/watch.rs`
  - `src/commands/permissions/mod.rs`
  - `src/commands/permissions/set_permissions.rs`

## What This Audit Justifies

This audit justifies checking off:

- `Ensure logs are useful for real support/debug cases`
- `Remove or reduce noisy low-value logging`

Reason:

- the known low-value warning paths in Linux-critical browse/open/search/
  duplicates flows have been either removed, downgraded to `debug`, or made
  clearly actionable with structured fields.
- support/debug runs now have a documented collection path plus structured
  startup context that makes attached logs materially more useful in real bug
  reports.

## Remaining Step 10 Gaps

- User-facing error phrasing is still a separate track from backend log
  quality.

## Conclusion

The Linux 1.0 observability track has now meaningfully reduced noisy,
low-signal backend logging in Linux-critical workflows and made the remaining
logs materially more useful for real support/debug capture.

It does not yet justify checking off:

- `Ensure error surfaces show user-facing language rather than internal phrasing`
