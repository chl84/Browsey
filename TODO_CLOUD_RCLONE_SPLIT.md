# TODO: Cloud/rclone Module Split

Created: 2026-02-28
Goal: Split the largest cloud/rclone files into smaller modules without changing behavior, while following the existing `src/commands/cloud/` structure and current project naming conventions.

## Architecture Alignment

This track should stay aligned with:
- `ARCHITECTURE_NAMING.md`
- `ARCHITECTURE_IMPORTS.md`

Implications:
- Keep cloud-specific modules under `src/commands/cloud/`.
- Keep rclone provider internals under `src/commands/cloud/providers/rclone/` if a folder split is introduced.
- Use plain `*.rs` names for focused modules by responsibility.
- Avoid creating generic catch-all names like `utils.rs` unless the contents are truly shared and small.
- Preserve current public command entry points in `src/commands/cloud/mod.rs`.

## Scope

In scope:
- Split large cloud/rclone Rust files by responsibility
- Preserve behavior and existing command contracts
- Keep test coverage close to moved logic

Out of scope:
- New cloud features
- Backend/API contract changes unless required by extraction seams
- Large behavioral rewrites during the split

## Target Files

- `src/commands/cloud/mod.rs`
- `src/commands/cloud/rclone_rc.rs`
- `src/commands/cloud/providers/rclone.rs`

## Split Rules

- Prefer extracting cohesive responsibility buckets over mechanical line-count splits.
- Prefer existing directory ownership before adding new top-level folders.
- If a file becomes a folder-backed module, keep the old file name as the module directory name:
  - `mod.rs` may delegate to sibling modules inside `src/commands/cloud/`
  - `rclone_rc.rs` should become `src/commands/cloud/rclone_rc/` only if the extracted pieces remain rc-specific
  - `providers/rclone.rs` should become `src/commands/cloud/providers/rclone/` if split
- Keep provider-facing orchestration separate from parsing, runtime checks, daemon lifecycle, and fallback policy.

## Work Plan

### 1) Split `src/commands/cloud/mod.rs` first

- [x] Extract directory-listing cache and invalidation into `src/commands/cloud/cache.rs`
- [x] Extract remote permit/cooldown limiter into `src/commands/cloud/limits.rs`
- [x] Extract cloud refresh event payload + emit helpers into `src/commands/cloud/events.rs`
- [x] Keep Tauri command entry points in `src/commands/cloud/mod.rs`
- [ ] Keep command-specific orchestration in clearly named sibling modules if needed:
  - `list.rs`
  - `write.rs`
  - `conflicts.rs`

Acceptance:
- `mod.rs` remains the command surface, not the main implementation dump.

### 2) Split `src/commands/cloud/rclone_rc.rs`

- [ ] Extract daemon lifecycle/health/recycle logic into `src/commands/cloud/rclone_rc/daemon.rs`
- [ ] Extract generic request/client helpers into `src/commands/cloud/rclone_rc/client.rs`
- [ ] Extract async job control (`job/status`, `job/stop`, cancel flow) into `src/commands/cloud/rclone_rc/jobs.rs`
- [ ] Extract typed `operations/*` and `config/*` wrappers into `src/commands/cloud/rclone_rc/methods.rs`
- [ ] Keep shared rc state and public entry points in `src/commands/cloud/rclone_rc/mod.rs`

Acceptance:
- rc daemon, request transport, and async job logic are no longer mixed in one file.

### 3) Split `src/commands/cloud/providers/rclone.rs`

- [ ] Convert to `src/commands/cloud/providers/rclone/`
- [ ] Extract runtime probe/cache into `runtime.rs`
- [ ] Extract read-path logic (`list_dir`, `stat_path`) into `read.rs`
- [ ] Extract write-path logic (`mkdir`, `delete`, `move`, `copy`) into `write.rs`
- [ ] Extract rc/cli backend selection + logging helpers into `logging.rs` if still large enough
- [ ] Extract `lsjson`/stat parsing helpers into `parse.rs`
- [ ] Keep the `RcloneCloudProvider` type and trait impl in `mod.rs`

Acceptance:
- Provider orchestration stays readable and does not own all parsing/fallback/runtime details inline.

## Quality Gates

- [x] `cargo fmt --all`
- [x] `cargo test cloud_ --quiet`
- [x] `cargo test rclone_ --quiet`
- [x] Any moved tests still live next to the code they validate
- [x] No new circular module dependencies
- [ ] No behavior change hidden inside a “split” commit

## Commit Strategy

- [ ] One focused commit per extraction step
- [ ] Separate pure moves from any behavior fix discovered during refactor
- [ ] Archive this TODO under `docs/todo-archive/` when the split is complete
