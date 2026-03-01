# TODO Low-Level Error Coordination

Scope: low-level Browsey layers below `src/commands/`, with focus on consistent typed error flow, coordinated classification, and avoiding stringly typed internal seams.

Principles:
- Keep domain-specific typed errors at each low-level boundary.
- Map to `ApiError` only in command-surface layers, not in low-level modules.
- Introduce `error.rs` where a low-level area has enough internal error semantics to justify a dedicated domain error model.
- Avoid `Result<_, String>` and `Option`-only failure signaling for important infrastructure seams.

## Priority Order

- [ ] `src/undo/`
- [ ] `src/binary_resolver/`
- [ ] `src/db/`
- [ ] `src/tasks/` + `src/runtime_lifecycle.rs` + `src/watcher.rs`

## 1. Undo

- [x] Introduce or complete a dedicated typed internal undo error flow rooted in existing `src/undo/error.rs`
- [ ] Remove remaining stringly typed seams in:
  - `src/undo/engine.rs`
  - `src/undo/path_ops.rs`
  - `src/undo/path_checks.rs`
  - `src/undo/security.rs`
  - `src/undo/nofollow.rs`
  - `src/undo/backup.rs`
  - `src/undo/types.rs`
- [ ] Replace `Into<String>`/`From<String>`-driven internal control flow with typed `UndoResult<_>` across module boundaries
- [ ] Make rollback/batch failure aggregation preserve stable error codes instead of only formatted messages

Status note:
- [x] `error.rs` now exposes explicit typed constructors and I/O classification helpers
- [x] `types.rs`, `path_checks.rs`, and `backup.rs` no longer rely on raw string errors for their main internal seams
- [x] `engine.rs` and `path_ops.rs` now map their main I/O and validation seams through typed `UndoError`
- [x] `path_ops.rs` no longer uses text parsing to detect destination-exists failures in fallback move callers
- [x] batch rollback aggregation in `engine.rs` now preserves stable `UndoErrorCode` when wrapping failures
- [x] fallback move cleanup in `path_ops.rs` now preserves the original delete failure code when adding context
- [x] `error.rs` no longer relies on generic `From<String>`/`From<&str>` conversions, and `FsUtilsError` now maps to `UndoError` by stable code
- [x] `nofollow.rs` now exposes typed deletion results at its public low-level seam, and `engine.rs` / `path_ops.rs` consume them by `UndoErrorCode`
- [ ] `security.rs` and `nofollow.rs` still contain the largest remaining platform-specific stringly typed control flow

## 2. Binary Resolver

- [x] Add `src/binary_resolver/error.rs`
- [x] Introduce `BinaryResolverError`, `BinaryResolverErrorCode`, and `BinaryResolverResult<T>`
- [ ] Replace `Option<PathBuf>` returns in `src/binary_resolver/mod.rs` with typed results where failure semantics matter
- [ ] Distinguish at least:
  - invalid binary name
  - explicit path invalid
  - not found
  - not executable
  - canonicalize/stat failure
- [ ] Update callers so they stop reverse-engineering resolver state from `None`

Status note:
- [x] typed checked resolver functions now exist alongside compatibility wrappers in `src/binary_resolver/mod.rs`
- [x] `src/commands/cloud/rclone_path.rs` now uses typed resolver results instead of plain `Option`
- [x] `src/commands/system_clipboard/mod.rs` now uses typed resolver results instead of plain `Option`
- [x] `src/metadata/providers/media_probe.rs` now degrades from typed resolver results explicitly instead of calling plain `Option` wrappers

## 3. DB

- [ ] Keep `src/db/error.rs`, but strengthen it
- [ ] Add lower-level classification for important filesystem/SQLite-adjacent conditions:
  - permission denied
  - read-only filesystem
  - not found/data-dir unavailable
- [ ] Reduce dependence on message-pattern reclassification in upper layers
- [ ] Ensure command modules consuming `DbError` can map from stable low-level codes instead of reparsing strings

Status note:
- [x] `src/db/error.rs` now classifies I/O and SQLite failures into stable low-level codes
- [x] `src/db/mod.rs` now maps main open/read/write/transaction seams through typed DB helpers instead of generic message parsing
- [x] `src/commands/settings/mod.rs` now maps from `DbError.code()` instead of reparsing DB error text
- [x] `src/commands/listing/mod.rs` now maps DB failures from `DbError.code()` at its main direct DB seam
- [x] `src/commands/library.rs` now maps its direct DB seams without reparsing DB error text
- [x] `src/commands/open_with/mod.rs` and `src/commands/bookmarks.rs` now avoid reparsing DB error text at their direct DB seams
- [x] `src/commands/fs/open_ops.rs`, `src/commands/keymap.rs`, and `src/keymap/mod.rs` now avoid reparsing DB error text at their direct DB seams
- [x] `src/commands/cloud/rclone_path.rs` now avoids reparsing DB error text at its direct settings seam
- [ ] other command modules that consume `DbError` still need the same direct code-based mapping where it materially matters

## 4. Tasks / Runtime / Watcher

- [ ] Add `src/watcher_error.rs` only if watcher is expanded further; otherwise keep current inline error type but standardize behavior
- [ ] Define one explicit policy for coordination-layer failures:
  - shutdown-time emit failures
  - poisoned locks
  - cleanup/drop failures
  - watcher replacement failures
- [ ] Remove `expect(...)` from coordination state where failure should stay recoverable
- [ ] Decide which failures are intentionally best-effort and document that in code comments where needed
- [ ] Ensure helper APIs do not silently swallow operationally relevant failures unless best-effort is deliberate

Status note:
- [x] `src/watcher.rs` no longer uses `expect(...)` for watcher-state locking
- [x] watcher replacement/stop paths now return typed `WatcherResult<_>` and callers map them explicitly where relevant
- [x] best-effort behavior is now documented in `src/runtime_lifecycle.rs`, `src/tasks/mod.rs`, and watcher callback emits
- [ ] broader coordination policy is improved but not yet fully unified across every caller and helper

## Quality Gates

- [ ] no new `Result<_, String>` in touched low-level paths
- [ ] no important infra helper uses bare `Option` when the reason for failure matters
- [ ] each touched low-level area exposes a typed `...Result<T>`
- [ ] `cargo fmt --all`
- [ ] `cargo check -q`
- [ ] `cargo clippy --all-targets --all-features -- -D warnings`
- [ ] targeted tests for each touched area

## Commit Strategy

- [ ] one focused commit per low-level area
- [ ] keep `undo` separate from `binary_resolver`
- [ ] keep `db` separate from coordination-layer cleanup
