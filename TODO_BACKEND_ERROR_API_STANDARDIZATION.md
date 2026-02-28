# TODO: Backend Error API Standardization

Created: 2026-02-28
Goal: Make backend command modules consistent in how they build and expose errors: internal typed module errors first, `ApiError`/`ApiResult` only at the command surface.

## Architecture Alignment

This track should stay aligned with:
- `ARCHITECTURE_NAMING.md`
- existing `src/commands/*` module ownership

Implications:
- If a command folder has meaningful internal logic, it should own an `error.rs`.
- Internal helpers should prefer typed module results over `Result<_, String>` when crossing module boundaries or representing domain failures.
- Mapping to `ApiError` should happen at the outer command entry points, not deep inside the implementation.
- Keep narrow std/native result types where appropriate for leaf helpers:
  - `std::io::Result<_>` for true I/O leaf operations
  - parser-local `Result<_, String>` only when the string does not cross the module boundary unchanged

## Standard To Reach

Each backend command module should follow this shape:
- `error.rs` defines:
  - `<Module>ErrorCode`
  - `<Module>Error`
  - `<Module>Result<T>`
  - `map_api_result(...)`
- command entry points return `ApiResult<_>`
- internal orchestration returns `<Module>Result<_>`
- string errors are converted to typed module errors close to where they originate

## Priority Order

1. `src/commands/thumbnails/`
2. `src/commands/network/`
3. `src/commands/decompress/`
4. `src/commands/compress/`
5. `src/commands/fs/trash/`

## Scope

In scope:
- Standardize backend error flow
- Introduce or strengthen typed internal error layers
- Reduce raw `Result<_, String>` in orchestration-heavy code
- Keep behavior and user-visible API contracts stable

Out of scope:
- Feature work
- Error message wording churn without structural reason
- Broad behavioral rewrites unrelated to error flow

## Work Plan

### 1) `src/commands/thumbnails/`

Current issue:
- `error.rs` exists, but `mod.rs` still uses raw `Result<_, String>` across much of the main pipeline and async coordination.

Tasks:
- [x] Introduce `ThumbnailResult<T>` alias in call sites instead of raw `Result<T, String>` where errors cross internal boundaries
- [x] Replace stringly-typed in-flight coordination payloads with `ThumbnailResult<ThumbnailResponse>` where practical
- [x] Convert major orchestration helpers in `mod.rs` to return `ThumbnailResult<_>`
- [ ] Keep low-level library adapters string-based only if immediately mapped at the boundary
- [x] Ensure `map_api_result(...)` stays only at Tauri command entry points

Acceptance:
- Main thumbnail pipeline no longer passes raw strings between top-level internal functions.

### 2) `src/commands/network/`

Current issue:
- `error.rs` exists, but `mounts.rs` mixes `NetworkResult<_>` with raw `Result<_, String>`, especially on non-Windows paths.

Tasks:
- [ ] Standardize `mounts.rs` to return `NetworkResult<_>` for orchestration paths
- [ ] Convert `eject_drive_impl` and `mount_partition_impl` to typed errors on all supported platforms
- [ ] Map command/process execution failures into `NetworkError` close to the process boundary
- [ ] Keep discovery helpers simple, but do not leak raw string errors through command orchestration

Acceptance:
- `network` command flows are platform-consistent and do not mix raw strings with typed errors in the same layer.

### 3) `src/commands/decompress/`

Current issue:
- `error.rs` exists, but extraction orchestration and many helper seams still use `Result<_, String>`.

Tasks:
- [ ] Convert top-level extract orchestration to `DecompressResult<_>` end-to-end
- [ ] Wrap `spawn_blocking` task results into typed errors at the task boundary, not later
- [ ] Introduce typed conversions for archive detection, output-dir preparation, and extraction planning helpers
- [ ] Keep format-specific leaf readers string-based only if mapped before returning into `mod.rs`
- [ ] Reduce direct `String` error propagation in `util.rs` seams that are used by orchestration

Acceptance:
- `decompress/mod.rs` uses typed module errors for orchestration and batch control paths.

### 4) `src/commands/compress/`

Current issue:
- `error.rs` exists, but compression planning and zip-writing orchestration still lean on `Result<_, String>`.

Tasks:
- [ ] Convert top-level compression orchestration to `CompressResult<_>`
- [ ] Wrap `spawn_blocking` task failures into `CompressError` at the task boundary
- [ ] Convert planning helpers (`ensure_same_parent`, destination resolution, entry gathering) to typed errors where they cross orchestration boundaries
- [ ] Keep low-level zip/IO leaf helpers narrow, but map them before they return to orchestration

Acceptance:
- `compress/mod.rs` no longer uses raw string errors for the main command pipeline.

### 5) `src/commands/fs/trash/`

Current issue:
- Trash/delete internals rely heavily on `Result<_, String>` and only map to `FsError` later.

Tasks:
- [ ] Convert `move_ops.rs` orchestration to `FsResult<_>` where it crosses module boundaries
- [ ] Convert `delete_ops.rs` blocking/orchestration helpers to `FsResult<_>`
- [ ] Keep raw string helpers only for truly local formatting convenience, not module interfaces
- [ ] Reuse existing `FsError::from_external_message(...)` less by constructing typed `FsError` earlier

Acceptance:
- `fs` trash/delete flows behave like the rest of the standardized backend modules.

## Quality Gates

- [ ] `cargo fmt --all`
- [ ] `cargo check -q`
- [ ] Module-specific tests stay green after each step
- [ ] No change to frontend/backend API contracts unless explicitly required
- [ ] No new mixed layers where typed errors are converted back into raw strings

## Commit Strategy

- [ ] One focused commit per module family (`thumbnails`, `network`, `decompress`, `compress`, `fs/trash`)
- [ ] Separate structural error refactors from behavior changes
- [ ] Archive this TODO under `docs/todo-archive/` when the track is complete
