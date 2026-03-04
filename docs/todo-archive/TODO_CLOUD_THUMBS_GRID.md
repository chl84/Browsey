# TODO: Cloud Thumbs for Grid (Images/PDF/SVG) + Backend Hardening

Created: 2026-03-04
Target: Reliable cloud thumbnails without performance regressions
Scope: Grid thumbnails + settings + thumbnail/cloud backend integration

## Goal
Add an opt-in `Cloud thumbs` setting for cloud entries (`rclone://`) in Grid view, with strict safeguards:
- avoid per-call DB reads in thumbnail path,
- dedupe concurrent cloud materializations,
- enforce cloud thumbnail download size guard.

## Locked decisions
- [x] Cloud thumbs scope (phase 1): `image + pdf + svg` (no video)
- [x] Cache strategy: reuse existing `cloud-open` cache (no separate cloud-thumb cache)
- [x] Default: `Cloud thumbs = off`
- [x] Unknown cloud file size: fail fast (strict guard)
- [x] Cloud size limit for thumbnail materialization: `50 MB`

## Public API / interface changes
- [x] Add Tauri settings commands:
  - [x] `store_cloud_thumbs(value: bool)`
  - [x] `load_cloud_thumbs() -> Option<bool>`
- [x] Extend frontend settings types/props:
  - [x] `Settings.cloudThumbs: boolean`
  - [x] `SettingsModal.cloudThumbsValue`
  - [x] `SettingsModal.onToggleCloudThumbs`
- [x] Keep `get_thumbnail(path, max_dim, generation)` signature unchanged
- [x] Behavioral extension: `get_thumbnail` supports cloud paths when `cloudThumbs` is enabled and extension is allowed

## Workstream A: Settings persistence and wiring
- [x] Rust settings command implementation in `src/commands/settings/mod.rs`
- [x] Re-export new settings commands in `src/commands/mod.rs`
- [x] Register new commands in `src/main.rs`
- [x] Frontend settings service methods in `frontend/src/features/explorer/services/settings.service.ts`
- [x] Add `cloudThumbs` to explorer stores in `frontend/src/features/explorer/state/stores.ts`
- [x] Add preference load/toggle/set methods in `frontend/src/features/explorer/state/preferencesSlice.ts`
- [x] Ensure state API includes new preference in `frontend/src/features/explorer/state.ts`
- [x] Wire setting through `ExplorerPage.svelte` and `createExplorerSettingsModalProps.ts`

## Workstream B: Settings modal UX
- [x] Add `cloudThumbs` to `frontend/src/features/settings/settingsTypes.ts` (default `false`)
- [x] Add SettingsModal props in `frontend/src/features/settings/SettingsModal.svelte`
- [x] Add row in `frontend/src/features/settings/sections/ThumbnailsSection.svelte`
  - [x] Label: `Cloud thumbs`
  - [x] Description: `Enable thumbnails for cloud images, PDF and SVG (network usage)`
- [x] Add filter model support in `frontend/src/features/settings/hooks/useSettingsModalViewModel.ts`
  - [x] `showCloudThumbsRow`
  - [x] searchable text for “cloud thumbs”
- [x] Ensure restore-defaults path resets `cloudThumbs` to `false`

## Workstream C: Grid thumbnail pipeline gating (frontend)
- [x] Pass `cloudThumbs` through shell prop chain:
  - [x] `createExplorerShellProps.ts`
  - [x] `ExplorerShell.svelte`
  - [x] `FileGrid.svelte`
- [x] Extend loader options in `frontend/src/features/explorer/thumbnailLoader.ts` with `allowCloudThumbs`
- [x] Enqueue rules:
  - [x] local files: unchanged
  - [x] cloud files: require `allowCloudThumbs == true`
  - [x] cloud extension allowlist: image/pdf/svg only
  - [x] cloud video extensions: always blocked in phase 1
- [x] Reset loader generation when `cloudThumbs` toggles

## Workstream D: Backend hardening (thumbnail + cloud)
- [x] Introduce runtime settings cache in `src/commands/thumbnails/mod.rs`
  - [x] include `thumb_cache_mb`, `video_thumbs`, `ffmpeg_path`, `cloud_thumbs`
  - [x] first-read DB load, then in-memory reads
  - [x] invalidation hook
- [x] Invalidate thumbnail runtime settings cache from settings store commands:
  - [x] `store_thumb_cache_mb`
  - [x] `store_video_thumbs`
  - [x] `store_ffmpeg_path`
  - [x] `store_cloud_thumbs`
- [x] Refactor cloud materialization helper out of cloud-open flow in `src/commands/cloud/open.rs`
  - [x] reusable “materialize to cloud-open cache without opening”
- [x] Add in-flight dedupe for cloud materialization
  - [x] key: stable tuple/hash of source path + size + modified
  - [x] waiters receive same result
- [x] Extend thumbnail command to cloud path handling in `src/commands/thumbnails/mod.rs`
  - [x] detect `rclone://`
  - [x] enforce `cloudThumbs` enabled
  - [x] enforce allowlisted extension
  - [x] enforce known size and `<= 50 MB`
  - [x] materialize via shared helper, then run existing thumbnail generation path
- [x] Preserve typed error mapping in `src/commands/thumbnails/error.rs`
  - [x] disabled cloud thumbs
  - [x] unsupported cloud extension
  - [x] missing size (guard)
  - [x] too large cloud source

## Workstream E: Test coverage
- [x] Rust tests: settings command roundtrip for `cloudThumbs`
- [x] Rust tests: thumbnail runtime settings cache load + invalidation behavior
- [x] Rust tests: cloud thumbnail guard behavior
  - [x] disabled
  - [x] unsupported extension
  - [x] unknown size
  - [x] over size limit
- [x] Rust tests: cloud materialization dedupe (single materialization under concurrency)
- [x] Rust tests: cloud-open existing behavior unchanged after helper extraction
- [x] Frontend tests/mocks:
  - [x] add `load_cloud_thumbs` mock in `frontend/src/test/mocks/tauri/core.ts`
  - [x] settings/model tests for Cloud thumbs row and filtering
  - [x] loader behavior tests (cloud eligible/ineligible paths)

## Quality gates
- [x] `npm --prefix frontend run check`
- [x] `cargo test --all-targets --all-features`
- [x] `cargo clippy --all-targets --all-features -- -D warnings`

## Acceptance criteria
- [x] `Cloud thumbs` appears in Settings > Thumbnails and persists
- [x] Default install behavior does not fetch cloud thumbnails
- [x] With setting enabled, cloud image/pdf/svg thumbnails render in Grid
- [x] Cloud video thumbnails are not attempted
- [x] No DB-read on every thumbnail call for runtime settings
- [x] Concurrent requests for same cloud source do not duplicate materialization downloads
- [x] Cloud files with unknown size or size > 50 MB are rejected with stable typed errors
- [x] Local thumbnail behavior remains unchanged

## Exit / archive
- [x] Move this file to `docs/todo-archive/` when all checkboxes are complete
- [x] Add a short completion note (date + result summary) in archived file

## Completion note
Completed: 2026-03-04
Result: Cloud thumbs is now wired end-to-end (settings, grid gating, backend thumbnail cloud path handling), with runtime setting cache hardening, cloud materialization dedupe, guard/error tests, and green Rust/frontend quality gates.
