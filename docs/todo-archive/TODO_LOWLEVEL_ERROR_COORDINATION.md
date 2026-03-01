# TODO Low-Level Error Coordination

Scope: low-level Browsey layers below `src/commands/`, with focus on consistent typed error flow, coordinated classification, and avoiding stringly typed internal seams.

Principles:
- Keep domain-specific typed errors at each low-level boundary.
- Map to `ApiError` only in command-surface layers, not in low-level modules.
- Introduce `error.rs` where a low-level area has enough internal error semantics to justify a dedicated domain error model.
- Avoid `Result<_, String>` and `Option`-only failure signaling for important infrastructure seams.

## Priority Order

- [x] `src/undo/`
- [x] `src/binary_resolver/`
- [x] `src/db/`
- [x] `src/tasks/` + `src/runtime_lifecycle.rs` + `src/watcher.rs`

## 1. Undo

- [x] Introduce or complete a dedicated typed internal undo error flow rooted in existing `src/undo/error.rs`
- [x] Remove remaining stringly typed seams in:
  - `src/undo/engine.rs`
  - `src/undo/path_ops.rs`
  - `src/undo/path_checks.rs`
  - `src/undo/security.rs`
  - `src/undo/nofollow.rs`
  - `src/undo/backup.rs`
  - `src/undo/types.rs`
- [x] Replace `Into<String>`/`From<String>`-driven internal control flow with typed `UndoResult<_>` across module boundaries
- [x] Make rollback/batch failure aggregation preserve stable error codes instead of only formatted messages

Status note:
- [x] `error.rs` now exposes explicit typed constructors and I/O classification helpers
- [x] `types.rs`, `path_checks.rs`, and `backup.rs` no longer rely on raw string errors for their main internal seams
- [x] `engine.rs` and `path_ops.rs` now map their main I/O and validation seams through typed `UndoError`
- [x] `path_ops.rs` no longer uses text parsing to detect destination-exists failures in fallback move callers
- [x] `path_ops.rs` now decides cross-device and no-replace fallback from stable `UndoErrorCode` instead of raw `io::Error`
- [x] batch rollback aggregation in `engine.rs` now preserves stable `UndoErrorCode` when wrapping failures
- [x] fallback move cleanup in `path_ops.rs` now preserves the original delete failure code when adding context
- [x] `error.rs` no longer relies on generic `From<String>`/`From<&str>` conversions, and `FsUtilsError` now maps to `UndoError` by stable code
- [x] `nofollow.rs` now exposes typed deletion results at its public low-level seam, and `engine.rs` / `path_ops.rs` consume them by `UndoErrorCode`
- [x] `nofollow.rs` delete mapping now preserves symlink/invalid-input semantics instead of collapsing them into generic `io_error`
- [x] `nofollow.rs` rename mapping now preserves target-exists, cross-device, unsupported, and symlink semantics as stable `UndoErrorCode`
- [x] `nofollow.rs` no longer relies on string comparison to recover embedded symlink semantics from `io::Error`
- [x] `security.rs` now centralizes typed symlink/metadata validation and uses typed Win32 failure mapping for DACL reads
- [x] `security.rs` and `nofollow.rs` now keep their remaining platform-specific raw I/O shims internal; typed `UndoResult<_>` boundaries carry the stable semantics outward

## 2. Binary Resolver

- [x] Add `src/binary_resolver/error.rs`
- [x] Introduce `BinaryResolverError`, `BinaryResolverErrorCode`, and `BinaryResolverResult<T>`
- [x] Replace `Option<PathBuf>` returns in `src/binary_resolver/mod.rs` with typed results where failure semantics matter
- [x] Distinguish at least:
  - invalid binary name
  - explicit path invalid
  - not found
  - not executable
  - canonicalize/stat failure
- [x] Update callers so they stop reverse-engineering resolver state from `None`

Status note:
- [x] typed checked resolver functions now exist alongside compatibility wrappers in `src/binary_resolver/mod.rs`
- [x] `src/commands/cloud/rclone_path.rs` now uses typed resolver results instead of plain `Option`
- [x] `src/commands/system_clipboard/mod.rs` now uses typed resolver results instead of plain `Option`
- [x] `src/metadata/providers/media_probe.rs` now degrades from typed resolver results explicitly instead of calling plain `Option` wrappers
- [x] the old plain `Option` resolver wrappers have been removed from `src/binary_resolver/mod.rs`

## 3. DB

- [x] Keep `src/db/error.rs`, but strengthen it
- [x] Add lower-level classification for important filesystem/SQLite-adjacent conditions:
  - permission denied
  - read-only filesystem
  - not found/data-dir unavailable
- [x] Reduce dependence on message-pattern reclassification in upper layers
- [x] Ensure command modules consuming `DbError` can map from stable low-level codes instead of reparsing strings

Status note:
- [x] `src/db/error.rs` now classifies I/O and SQLite failures into stable low-level codes
- [x] `src/db/mod.rs` now maps main open/read/write/transaction seams through typed DB helpers instead of generic message parsing
- [x] `src/commands/settings/mod.rs` now maps from `DbError.code()` instead of reparsing DB error text
- [x] `src/commands/listing/mod.rs` now maps DB failures from `DbError.code()` at its main direct DB seam
- [x] `src/commands/library.rs` now maps its direct DB seams without reparsing DB error text
- [x] `src/commands/open_with/mod.rs` and `src/commands/bookmarks.rs` now avoid reparsing DB error text at their direct DB seams
- [x] `src/commands/fs/open_ops.rs`, `src/commands/keymap.rs`, and `src/keymap/mod.rs` now avoid reparsing DB error text at their direct DB seams
- [x] `src/commands/cloud/rclone_path.rs` now avoids reparsing DB error text at its direct settings seam
- [x] `src/commands/search/worker.rs` now maps search DB failures from `DbError.code()` instead of collapsing them into one generic open failure
- [x] `src/main.rs`, `src/commands/thumbnails/mod.rs`, and `src/metadata/providers/media_probe.rs` now treat best-effort DB setting reads explicitly and log stable DB codes instead of silently swallowing failures
- [x] remaining direct `DbError` consumers are now either code-based mappers or clearly intentional best-effort fallbacks

## 4. Tasks / Runtime / Watcher

- [x] Add `src/watcher_error.rs` only if watcher is expanded further; otherwise keep current inline error type but standardize behavior
- [x] Define one explicit policy for coordination-layer failures:
  - shutdown-time emit failures
  - poisoned locks
  - cleanup/drop failures
  - watcher replacement failures
- [x] Remove `expect(...)` from coordination state where failure should stay recoverable
- [x] Decide which failures are intentionally best-effort and document that in code comments where needed
- [x] Ensure helper APIs do not silently swallow operationally relevant failures unless best-effort is deliberate

Status note:
- [x] `src/watcher.rs` no longer uses `expect(...)` for watcher-state locking
- [x] watcher replacement/stop paths now return typed `WatcherResult<_>` and callers map them explicitly where relevant
- [x] best-effort behavior is now documented in `src/runtime_lifecycle.rs`, `src/tasks/mod.rs`, and watcher callback emits
- [x] `src/runtime_lifecycle.rs` now logs dropped emit failures at the helper boundary instead of silently returning `false`
- [x] `src/main.rs` shutdown cleanup now logs recoverable cancel/watcher/rclone teardown failures explicitly
- [x] `src/commands/network/mounts.rs` now uses the shared runtime emit helper instead of raw `app.emit(...)`
- [x] watcher coordination keeps its inline error type intentionally; a separate `src/watcher_error.rs` is not warranted at current scope

## Quality Gates

- [x] no new `Result<_, String>` in touched low-level paths
- [x] no important infra helper uses bare `Option` when the reason for failure matters
- [x] each touched low-level area exposes a typed `...Result<T>`
- [x] `cargo fmt --all`
- [x] `cargo check -q`
- [x] `cargo clippy --all-targets --all-features -- -D warnings`
- [x] targeted tests for each touched area

## Commit Strategy

- [x] one focused commit per low-level area
- [x] keep `undo` separate from `binary_resolver`
- [x] keep `db` separate from coordination-layer cleanup
