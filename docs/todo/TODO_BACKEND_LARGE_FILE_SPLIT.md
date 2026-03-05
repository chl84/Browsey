# TODO: Backend Large File Split Wave (Commands-first, Structure-aligned)

Created: 2026-03-05  
Goal: Split oversized backend files into smaller responsibility-owned modules, aligned with existing project structure and hardening norms.  
Scope: `src/commands/**` (Rust backend only)

## Why this track exists

Recent findings show multiple backend files are now large enough to reduce maintainability and increase regression risk during feature changes.

Largest current backend candidates (non-test):
- `src/commands/thumbnails/mod.rs` (~1329 LOC)
- `src/commands/decompress/util.rs` (~1100 LOC)
- `src/commands/listing/mod.rs` (~966 LOC)
- `src/commands/decompress/mod.rs` (~925 LOC)
- `src/commands/transfer/execute.rs` (~920 LOC)
- `src/commands/cloud/open.rs` (~908 LOC)
- `src/commands/cloud/cache.rs` (~743 LOC)
- `src/commands/permissions/ownership.rs` (~698 LOC)
- `src/commands/settings/mod.rs` (~686 LOC)
- `src/commands/rename/mod.rs` (~684 LOC)

## Phase 0 baseline snapshot (2026-03-05)

### A) Module ownership baseline

- `thumbnails`: `src/commands/thumbnails/mod.rs`
- `transfer`: `src/commands/transfer/execute.rs`
- `listing`: `src/commands/listing/mod.rs`
- `decompress`: `src/commands/decompress/mod.rs` + `src/commands/decompress/util.rs`
- `cloud-open/cache`: `src/commands/cloud/open.rs` + `src/commands/cloud/cache.rs`
- `settings`: `src/commands/settings/mod.rs`
- `rename`: `src/commands/rename/mod.rs`
- `permissions`: `src/commands/permissions/ownership.rs` + `src/commands/permissions/mod.rs`

### B) Public command signature freeze (split-wave targets)

- `thumbnails`
  - `clear_thumbnail_cache() -> ApiResult<ThumbnailCacheClearResult>`
  - `get_thumbnail(app_handle, path, max_dim, generation) -> ApiResult<ThumbnailResponse>`
- `transfer`
  - `preview_mixed_transfer_conflicts(...) -> ApiResult<Vec<MixedTransferConflictInfo>>`
  - `copy_mixed_entries(...) -> ApiResult<Vec<String>>`
  - `move_mixed_entries(...) -> ApiResult<Vec<String>>`
  - `copy_mixed_entry_to(...) -> ApiResult<String>`
  - `move_mixed_entry_to(...) -> ApiResult<String>`
- `listing`
  - `list_dir(path, sort, app) -> ApiResult<DirListing>`
  - `list_facets(scope, path, include_hidden, app) -> ApiResult<ListingFacets>`
  - `watch_dir(path, state, app) -> ApiResult<()>`
- `decompress`
  - `can_extract_paths(paths) -> ApiResult<bool>`
  - `extract_archive(app, cancel, undo, path, progress_event) -> ApiResult<ExtractResult>`
  - `extract_archives(app, cancel, undo, paths, progress_event) -> ApiResult<Vec<ExtractBatchItem>>`
- `cloud` (open/cache surface touched by split wave)
  - `open_cloud_entry(path, app, cancel, progress_event) -> ApiResult<()>`
  - `clear_cloud_open_cache() -> ApiResult<CloudOpenCacheClearResult>`

### C) Behavior and typed-error lock baseline

Executed before split work:
- `cargo test commands::thumbnails:: -- --nocapture` (9 passed)
- `cargo test commands::transfer::execute::tests -- --nocapture` (18 passed)
- `cargo test commands::listing:: -- --nocapture` (8 passed)
- `cargo test commands::decompress:: -- --nocapture` (10 passed)
- `cargo test commands::cloud:: -- --nocapture` (125 passed)

### D) High-risk timing sample baseline (test-suite proxy)

- `commands::thumbnails::` suite: finished in ~0.00s
- `commands::transfer::execute::tests` suite: finished in ~0.30s
- `commands::listing::` suite: finished in ~0.00s
- `commands::decompress::` suite: finished in ~0.09s
- `commands::cloud::` suite: finished in ~3.27s

Note:
- This is a pre-refactor timing proxy from deterministic test suites.
- Replace with command-level timing samples when we run manual perf pass for each touched phase.

Recent cloud hardening findings (separate track, already implemented in code):
- [x] Delete policy lookup made cache-first with config-dump fallback
- [x] Cloud materialize waiter timeout increased (typed timeout retained)
- [x] OneDrive conflict key normalized with Unicode-aware lowercasing

## Architecture alignment (must follow)

- Keep command surfaces in existing `mod.rs` files.
- Split by cohesive responsibility, not by arbitrary line count.
- Keep provider internals inside existing provider folders (for cloud/rclone paths).
- Preserve typed-error boundaries:
  - command errors remain in module `error.rs` (or existing typed-error owner),
  - no stringly regression (`to_string()` seams in hardened paths).
- Keep tests close to moved logic:
  - use existing `tests.rs` / module-local `#[cfg(test)]` patterns already present in each domain.
- Avoid catch-all module names unless the content is truly shared and small (no new giant `utils.rs`).

## In scope

- Structural refactor only (file/module splits, imports, wiring).
- Moving tests to stay close to moved logic where helpful.
- Small helper extraction to reduce coupling and clarify ownership.

## Out of scope

- New product features.
- Frontend changes.
- Public Tauri command signature changes (unless absolutely required by extraction seams, then must be documented).
- Mixed behavior rewrites hidden inside “split” commits.

## Risk-first execution policy

- One domain at a time:
  - never split `thumbnails`, `transfer`, `listing`, `decompress`, and `cloud` in the same commit.
- No mixed intent commits:
  - split/move commits must not include opportunistic behavior changes.
- Require behavioral baseline before first extraction in each phase:
  - identify and lock highest-risk flows with explicit tests first.
- Keep rollback simple:
  - each step must be revertable independently without partial dependency on later steps.
- Stop immediately on unexpected drift:
  - if smoke/tests reveal behavior change outside touched module boundaries, pause and isolate root cause before continuing.

## Split wave plan

### Phase 0 — Guardrails and baseline

- [x] Capture baseline counts (LOC + key file owners) in PR description.
- [x] Freeze public command signatures for targeted modules.
- [x] Lock typed-error behavior with targeted tests before moving logic in each phase.
- [x] Record baseline command behavior snapshot per domain (happy path + one failure path).
- [x] Record baseline command timing sample for high-risk paths (`cloud open`, listing, thumbnail generation) to catch performance regressions.

Acceptance:
- Baseline and invariants are explicit before structural changes.

### Phase 1 — Highest impact modules

#### A) `src/commands/thumbnails/mod.rs`
- [x] Convert to folder-backed module if needed: `src/commands/thumbnails/`
- [x] Extract cloud-thumbnail precheck/materialization gate to dedicated module (for example `cloud_source.rs`).
- [x] Extract cache/inflight orchestration to dedicated module (for example `cache_flow.rs`).
- [x] Keep `get_thumbnail` command surface in `mod.rs`.
- [x] Keep runtime settings cache/invalidation behavior unchanged (no per-call DB regressions).

#### B) `src/commands/transfer/execute.rs`
- [x] Keep orchestration in `execute.rs` and move operation-specific branches to submodules under `src/commands/transfer/execute/`.
- [x] Separate route resolution, conflict naming, and write execution steps into small helpers.
- [x] Keep existing `src/commands/transfer/execute/tests.rs` aligned with moved code.
- [x] Keep auto-rename and prechecked semantics unchanged across local/cloud/mixed branches.

#### C) `src/commands/listing/mod.rs`
- [x] First slice: extract cloud listing path/mapping/facet helpers into `src/commands/listing/cloud.rs`.
- [x] Second slice: extract local listing/read-dir flow into `src/commands/listing/local.rs` and watch flow into `src/commands/listing/watch.rs`.
- [x] Third slice: extract scope entry resolution (`dir/recent/starred/trash`) into `src/commands/listing/scope.rs`.
- [x] Extract provider-specific mapping (local/cloud/network/trash) into focused sibling modules.
- [x] Keep listing command entry points in `mod.rs`.
- [x] Preserve icon/error mapping behavior exactly.
- [x] Keep sorting and path normalization order stable for all listing sources.

Acceptance:
- Each target file reduced materially and becomes orchestration-only.
- No user-visible behavior drift in thumbnails/transfer/listing smoke checks.

### Phase 2 — Archive/decompress cleanup

#### A) `src/commands/decompress/mod.rs` + `src/commands/decompress/util.rs`
- [x] First slice: extract progress/copy plumbing (`ProgressEmitter`, cancel checks, copy loop, buffered open, io mappers) to `src/commands/decompress/util/stream_io.rs`.
- [x] Second slice: extract path/sanitization + output-path strategy to `src/commands/decompress/util/path_ops.rs`.
- [x] Third slice: extract budget/disk guards to `src/commands/decompress/util/budget.rs`.
- [x] Split `util.rs` into responsibility-owned helpers:
  - [x] path/sanitization + output-path strategy
  - [x] budget/disk guards
  - [x] progress/copy plumbing
- [x] Keep archive format handlers (`zip/tar/7z/rar`) as format-specific modules.
- [x] Keep command entry points and high-level flow in `decompress/mod.rs`.
- [x] Preserve cancellation and disk-guard behavior exactly (no relaxed safety checks).

Acceptance:
- `decompress/mod.rs` remains readable orchestration, utilities become focused and testable.
- Extraction security posture remains unchanged (symlink/path traversal protections preserved).

### Phase 3 — Cloud and shared command infrastructure

#### A) `src/commands/cloud/open.rs`
- [x] First slice: extract inflight dedupe/waiter timeout flow to `src/commands/cloud/open/inflight.rs`.
- [x] Separate cache metadata/storage operations from materialization orchestration.
- [x] Keep dedupe and timeout policy in a focused module with explicit tests.
- [x] Keep public cloud open command flow unchanged.
- [x] Preserve existing cache freshness semantics and permission hardening.

#### B) `src/commands/cloud/cache.rs`
- [x] Separate listing cache store/lookup from background refresh scheduling and retry policy.
- [x] Keep invalidation semantics unchanged.
- [x] Preserve stale-while-refresh behavior and inflight refresh dedupe guarantees.

Acceptance:
- Cloud open/cache modules have clear ownership and lower coupling.
- Cloud timing/behavior remains stable under current cloud test matrix.

### Phase 4 — Secondary large modules

- [x] `src/commands/settings/mod.rs`: split persistence I/O, defaults, and command wrappers.
- [x] `settings`: extracted DB/error mapping + validation helpers to `settings/persistence.rs`.
- [x] `settings`: moved module-local tests to `settings/tests.rs`.
- [x] `settings`: split command wrappers into focused submodules.
- [x] `src/commands/rename/mod.rs`: split batch rename plan/build/apply responsibilities.
- [x] `src/commands/permissions/ownership.rs` and `src/commands/permissions/mod.rs`: separate platform-specific handling from shared orchestration.
- [x] `src/commands/compress/mod.rs` and `src/commands/cloud/rclone_cli.rs`: split only if Phase 1-3 remain stable.

Acceptance:
- Remaining top-size modules each have a clear module map and reduced single-file complexity.

## Quality gates (run per phase)

- [x] `cargo fmt --all -- --check`
- [x] `cargo clippy --all-targets --all-features -- -D warnings`
- [x] `cargo test --all-targets --all-features`
- [x] `bash scripts/maintenance/check-backend-error-hardening-guard.sh`
- [x] Domain-focused test subset for touched area (for example `cargo test commands::cloud:: -- --nocapture`)
- [x] Ensure no new warnings in touched modules (`cargo test` output clean for touched domain tests).

## Phase-gate stop criteria (mandatory)

- [ ] Stop phase if command signature diff appears in touched domain without explicit approval.
- [ ] Stop phase if typed-error code mapping changes unintentionally for touched domain.
- [ ] Stop phase if domain smoke checklist fails on previously passing baseline scenario.
- [ ] Stop phase if clippy or hardening guard starts failing due to new conversion seams.
- [ ] Stop phase if change-set includes unrelated module edits not required by extraction.

## Rollback protocol

- [ ] Each extraction step lands in a dedicated commit with a clear rollback scope.
- [ ] If a regression appears, revert only the smallest offending extraction commit.
- [ ] Re-run touched domain tests + smoke checklist immediately after rollback.
- [ ] Do not proceed to next phase until regression root cause is documented.

## Manual regression smoke (per touched domain)

- [ ] Cloud: list/stat/open/delete/mkdir/copy/move for OneDrive/GDrive/Nextcloud basics
- [ ] Transfer: mixed copy/move with conflict preview and auto-rename path
- [ ] Listing: local/cloud/network/trash load and sort stability
- [ ] Decompress: zip/tar/7z/rar extract happy-path + cancellation
- [ ] Thumbnails: local + cloud thumbs gate behavior (disabled/enabled, supported/unsupported extensions)

## Commit strategy

- [ ] One focused commit per extraction step (or very small sub-step).
- [ ] Pure moves/extractions separated from behavior fixes.
- [ ] Include short “module ownership after split” note in each PR description.
- [ ] Keep PRs reviewable: avoid mega-PRs; prefer one phase per PR.

## Residual risk register (track during execution)

- [ ] Performance drift from moved orchestration (especially listing + thumbnails + cloud refresh).
- [ ] Hidden behavior coupling surfaced by module extraction order.
- [ ] Typed-error drift in refactored command seams.
- [ ] Test fragility from parallel fake-rclone and timing-sensitive paths.
- [ ] Incomplete smoke coverage for edge providers/archives.

## Exit / archive

- [ ] When all phases are complete, move this file to `docs/todo-archive/`.
- [ ] Add completion note (date + final module map + residual risk summary).

## Assumptions and defaults

1. Commands-first backend structure remains the primary organizing principle.
2. Structural refactor must be behavior-preserving by default.
3. Typed-error discipline remains a hard requirement in all touched modules.
4. This track prioritizes maintainability and regression containment over feature throughput.
