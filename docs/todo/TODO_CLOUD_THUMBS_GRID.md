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
- [ ] Behavioral extension: `get_thumbnail` supports cloud paths when `cloudThumbs` is enabled and extension is allowed

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
- [ ] Pass `cloudThumbs` through shell prop chain:
  - [ ] `createExplorerShellProps.ts`
  - [ ] `ExplorerShell.svelte`
  - [ ] `FileGrid.svelte`
- [ ] Extend loader options in `frontend/src/features/explorer/thumbnailLoader.ts` with `allowCloudThumbs`
- [ ] Enqueue rules:
  - [ ] local files: unchanged
  - [ ] cloud files: require `allowCloudThumbs == true`
  - [ ] cloud extension allowlist: image/pdf/svg only
  - [ ] cloud video extensions: always blocked in phase 1
- [ ] Reset loader generation when `cloudThumbs` toggles

## Workstream D: Backend hardening (thumbnail + cloud)
- [ ] Introduce runtime settings cache in `src/commands/thumbnails/mod.rs`
  - [ ] include `thumb_cache_mb`, `video_thumbs`, `ffmpeg_path`, `cloud_thumbs`
  - [ ] first-read DB load, then in-memory reads
  - [ ] invalidation hook
- [ ] Invalidate thumbnail runtime settings cache from settings store commands:
  - [ ] `store_thumb_cache_mb`
  - [ ] `store_video_thumbs`
  - [ ] `store_ffmpeg_path`
  - [ ] `store_cloud_thumbs`
- [ ] Refactor cloud materialization helper out of cloud-open flow in `src/commands/cloud/open.rs`
  - [ ] reusable “materialize to cloud-open cache without opening”
- [ ] Add in-flight dedupe for cloud materialization
  - [ ] key: stable tuple/hash of source path + size + modified
  - [ ] waiters receive same result
- [ ] Extend thumbnail command to cloud path handling in `src/commands/thumbnails/mod.rs`
  - [ ] detect `rclone://`
  - [ ] enforce `cloudThumbs` enabled
  - [ ] enforce allowlisted extension
  - [ ] enforce known size and `<= 50 MB`
  - [ ] materialize via shared helper, then run existing thumbnail generation path
- [ ] Preserve typed error mapping in `src/commands/thumbnails/error.rs`
  - [ ] disabled cloud thumbs
  - [ ] unsupported cloud extension
  - [ ] missing size (guard)
  - [ ] too large cloud source

## Workstream E: Test coverage
- [ ] Rust tests: settings command roundtrip for `cloudThumbs`
- [ ] Rust tests: thumbnail runtime settings cache load + invalidation behavior
- [ ] Rust tests: cloud thumbnail guard behavior
  - [ ] disabled
  - [ ] unsupported extension
  - [ ] unknown size
  - [ ] over size limit
- [ ] Rust tests: cloud materialization dedupe (single materialization under concurrency)
- [ ] Rust tests: cloud-open existing behavior unchanged after helper extraction
- [ ] Frontend tests/mocks:
  - [x] add `load_cloud_thumbs` mock in `frontend/src/test/mocks/tauri/core.ts`
  - [ ] settings/model tests for Cloud thumbs row and filtering
  - [ ] loader behavior tests (cloud eligible/ineligible paths)

## Quality gates
- [x] `npm --prefix frontend run check`
- [ ] `cargo test --all-targets --all-features`
- [ ] `cargo clippy --all-targets --all-features -- -D warnings`

## Acceptance criteria
- [ ] `Cloud thumbs` appears in Settings > Thumbnails and persists
- [ ] Default install behavior does not fetch cloud thumbnails
- [ ] With setting enabled, cloud image/pdf/svg thumbnails render in Grid
- [ ] Cloud video thumbnails are not attempted
- [ ] No DB-read on every thumbnail call for runtime settings
- [ ] Concurrent requests for same cloud source do not duplicate materialization downloads
- [ ] Cloud files with unknown size or size > 50 MB are rejected with stable typed errors
- [ ] Local thumbnail behavior remains unchanged

## Exit / archive
- [ ] Move this file to `docs/todo-archive/` when all checkboxes are complete
- [ ] Add a short completion note (date + result summary) in archived file
