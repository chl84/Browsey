# TODO: rclone Cloud Integration (CLI-first)

Scope direction:
- OneDrive first (v1)
- Google Drive later (phase 2)
- Nextcloud later (phase 3)
- Use `rclone` as external CLI (not embedded library)

How to use this list:
- Sections are grouped by topic, not strict execution order.
- Use the implementation sequence below to avoid rework.

Recommended implementation sequence (v1 OneDrive):
1. Sections `1`, `11`, and the decision-heavy parts of `12` (scope, security posture, packaging/path strategy).
2. Sections `2`, `10`, and `3` (path model, error contract, backend module shape / command registration plan).
3. Section `4` plus test harness items in `13` (CLI wrapper + fake `rclone` shim and command-builder tests).
4. Sections `5` and `6` (command mapping + routing seams in Browsey).
5. Section `7` backend feature delivery (listing and core ops) before broad frontend wiring.
6. Section `9` conflict model before finalizing paste/overwrite UI behavior in section `8`.
7. Section `8` frontend integration and capability-driven UX.
8. Section `13` full QA gates, then `17`, then validate `18` (tests/checks, docs/migration, DoD).
9. Sections `15` and `16` only after OneDrive v1 stabilizes.

## 1. Decisions and boundaries (before code)

- [x] Lock decision: `CLI-first` (`rclone` commands), not `rc API` in v1.
- [x] Define v1 scope (OneDrive): `list`, `mkdir`, `rename/move`, `copy`, `delete`, `refresh`.
- [x] Define out-of-scope for v1: `undo`, cloud trash, permissions, thumbnails, recursive search, duplicate scan.
- [x] Define minimum supported `rclone` version and how it is validated at first cloud use (runtime probe).
- [x] Decide whether Browsey requires global `rclone` in `PATH` or supports configurable binary path (v1: global `rclone` in `PATH`; custom path deferred).
- [x] Decide how cloud support is surfaced in Browsey UX in v1: `Network` view + explicit `rclone://...` path/open flows.
- [x] Decide whether cloud operations are behind a feature flag / experimental setting in first rollout (v1: no feature flag).
- [x] Decide how cloud `delete` maps to Browsey semantics in v1 (`permanent delete` only vs future trash integration).
- [x] Define whether cross-boundary operations are supported in v1:
  - [x] local -> cloud copy (not supported in v1)
  - [x] cloud -> local copy (not supported in v1)
  - [x] cloud -> cloud within same remote move/copy (supported in v1)
  - [x] cloud -> cloud across remotes (supported via same cloud flow, best-effort / not separately validated yet)
- [x] Define atomicity/rollback expectations for multi-entry cloud operations (current v1 behavior: stop-on-first-error, no rollback of earlier successful cloud ops).

## 2. Domain/path model (critical)

- [x] Introduce an explicit cloud path representation in backend (not local filesystem paths).
- [x] Define internal format for remote paths, e.g. `rclone://<remote>/<path>`.
- [x] Implement parser/validator for `rclone://...` (reject relative segments and ambiguous forms).
- [x] Define escaping rules for names with spaces and special characters.
- [x] Separate display label from remote ID (e.g. `my-onedrive` vs "OneDrive (Work)").
- [x] Define per-path/provider capability metadata (delete, rename, copy, move, trash, permissions, etc.).

## 3. Backend architecture (new cloud module)

- [x] Create `src/commands/cloud/`.
- [x] Add `src/commands/cloud/mod.rs`.
- [x] Add `src/commands/cloud/rclone_cli.rs` (low-level wrapper around `std::process::Command`).
- [x] Add `src/commands/cloud/path.rs` (cloud path parsing/formatting).
- [x] Add `src/commands/cloud/types.rs` (`CloudEntry`, `CloudError`, capabilities, provider kind).
- [x] Add `src/commands/cloud/provider.rs` (provider trait/abstraction).
- [x] Add provider impl module (`providers/rclone.rs` or `providers/onedrive_rclone.rs` for v1).
- [x] Keep cloud code separate from local FS logic in `src/commands/fs/*` and `src/undo/*`.
- [x] Follow existing command-module pattern: local `error.rs` with `map_api_result(...)` and typed domain error codes.
- [x] Return `crate::errors::api_error::ApiResult<T>` from Tauri commands (match existing backend command style).
- [x] Decide which commands are sync vs `async` + `spawn_blocking` (rclone CLI calls should not block Tauri async runtime).
- [x] Add cloud module exports to `src/commands/mod.rs` (both `pub mod cloud;` and re-exports for new commands).
- [x] Register new Tauri commands in `src/main.rs` `tauri::generate_handler![...]`.
- [x] Keep command names aligned with Browsey naming style (`snake_case` invoke names, explicit verbs, minimal ambiguity).

## 4. rclone CLI wrapper (security + robustness)

- [x] Use only `Command::new(...).args([...])` (never shell strings).
- [x] Add allowlist of `rclone` subcommands Browsey may invoke.
- [x] Add default timeout policy per command type (short for `list/stat`, longer for copy/move).
- [x] Add retry policy for transient failures (network, timeout, rate-limit).
- [x] Normalize `stdout/stderr/exit code` into structured `CloudError`.
- [x] Scrub/redact logs so secrets/tokens/config details are never logged.
- [x] Add version check (`rclone version`) and capability check on first use.
- [x] Add clear error for missing binary (`rclone` not found).
- [x] Ensure command execution uses `spawn_blocking` or equivalent isolation for blocking process I/O.
- [x] Decide stdout/stderr size limits to avoid huge buffers in logs/UI on provider failures.
- [ ] Ensure child processes are terminated/cancelled cleanly on app shutdown (Browsey runtime lifecycle integration).

## 5. rclone command mapping (OneDrive v1)

- [x] Folder listing via `lsjson` (name, type, size, modified time, optional hashes if needed later).
- [x] Stat/existence check for a single path.
- [x] Create directory (`mkdir`).
- [x] Delete file (validate correct command/flags for supported `rclone` version).
- [x] Delete directory recursively (`purge` or explicit strategy).
- [x] Delete empty directory (`rmdir`) as optional optimization.
- [x] Rename/move (map to `moveto` for file and directory).
- [x] Copy (map to `copyto` for file and directory).
- [x] Define overwrite behavior and map `rclone` errors to Browsey conflict model.
- [x] Centralize standard flags (JSON output, retries, reduced noise, etc.).

## 6. Routing in existing backend

- [x] Add early routing for local path vs cloud path in relevant commands.
- [x] Prevent cloud paths from entering `src/commands/fs/*`, `src/undo/*`, `nofollow`, or GVFS flows.
- [x] Start with separate Tauri commands for cloud instead of rewriting all FS commands at once.
- [x] Ensure `src/commands/network/*` does not try to own rclone cloud operations.
- [x] Keep `src/commands/network/gio_mounts.rs` for generic GIO/GVFS only (MTP/other mounts).
- [x] Identify exact integration seams in Browsey first (likely explorer listing + clipboard/paste preview + file ops service layer), and route there instead of adding ad-hoc bypasses.
- [x] Decide whether cloud entries reuse `Entry` directly or need a cloud-specific variant before conversion into `Entry`.
- [x] Ensure local-only helpers (`path_guard`, local metadata probes, `std::fs::*`) are never called on cloud paths during previews/actions.

## 7. OneDrive v1 feature delivery

- [x] Define how a OneDrive account is represented (rclone remote name + optional subpath).
- [x] Add backend command to list configured remotes (or a Browsey allowlist subset).
- [x] Add backend command to select/validate a remote and normalize root path.
- [x] Implement browsing/listing for OneDrive via `rclone`.
- [x] Implement core file ops in OneDrive (`copy`, `move`, `rename`, `delete`, `mkdir`).
- [x] Trigger refresh after operations.
- [x] Implement cloud-aware paste conflict preview (no `Path::exists()`).
- [x] Disable or hide unsupported actions in v1 (trash, undo, permissions, etc.) with clear UI messaging.

## 8. Frontend integration (v1)

- [x] Add UI model support for cloud entries that are not local filesystem paths.
- [x] Update network/explorer flows to display rclone-based cloud endpoints.
- [x] Define OneDrive presentation in Network/sidebar without GVFS-specific assumptions.
- [x] Add clear labels/icons for rclone OneDrive endpoints.
- [x] Add basic operation activity/progress UI (at least busy state).
- [x] Gate context-menu actions based on backend capability flags.
- [x] Ensure keyboard actions (`Delete`, `F2`, etc.) respect cloud capabilities.
- [x] Translate raw `rclone` errors into user-friendly UI messages.
- [x] Follow frontend naming conventions from `ARCHITECTURE_NAMING.md` (e.g. `*.service.ts` for Tauri invoke boundary).
- [x] Respect import-boundary rules from `ARCHITECTURE_IMPORTS.md` (cross-feature imports via public barrels only).
- [x] Add/extend feature barrel exports (`frontend/src/features/network/index.ts`, `frontend/src/features/explorer/index.ts`) instead of deep cross-feature imports.
- [x] Keep cloud `invoke` calls inside service modules (avoid calling `invoke(...)` directly from Svelte components).
- [x] Decide where cloud state lives (existing explorer state vs dedicated network/cloud store) before wiring multiple components (v1 uses existing explorer state/services + cloud path routing).
- [x] Ensure refresh/watch UX is clear for cloud paths (no `watch_dir` support; use manual/poll refresh semantics).

## 9. Conflict model and overwrite policy

- [x] Define cloud conflict model aligned with existing Browsey overwrite/rename/cancel flow.
- [x] Add cloud preview command that returns conflicts without local filesystem checks.
- [x] Map `destination exists` style `rclone` errors to existing UI conflict behavior.
- [x] Define rename-on-conflict strategy in cloud (Browsey-generated new name vs provider-specific behavior).
- [x] Test edge cases: same name, case-only rename, file-vs-directory conflicts.
- [x] Align conflict preview payload shape with existing clipboard preview UI to minimize frontend branching.
- [x] Define normalization rules consistently (provider casing/path separators) so preview and execution agree.

## 10. Error model, logging, observability

- [x] Introduce `CloudErrorCode` (auth, network, rate_limit, timeout, binary_missing, invalid_path, unsupported, destination_exists, etc.).
- [x] Log command name and duration, but not secrets/tokens.
- [x] Log scrubbed `stderr` on failures.
- [x] Add debug logging path for development (`RUST_LOG`) for rclone invocations.
- [x] Standardize user-facing error messages for common cloud failures (auth expired, remote missing, connectivity).
- [x] Map cloud errors into Browsey's existing `ApiError { code, message }` contract so frontend handling remains consistent.
- [x] Reuse module-local `error.rs` patterns (`*_ErrorCode`, `map_api_result`) for cloud commands instead of ad-hoc string errors.
- [x] Add telemetry-friendly stable error codes before broad UI integration (avoid string-parsing in frontend).

## 11. Security requirements (must be early)

- [x] No shell execution; argument-list invocation only.
- [x] Strict validation of remote names and path segments.
- [x] Do not accept arbitrary user-provided `rclone` flags.
- [x] Never log `rclone` config content or tokens.
- [x] Consider a Browsey remote allowlist/prefix policy (implemented optional env-based policy: `BROWSEY_RCLONE_REMOTE_ALLOWLIST` / `BROWSEY_RCLONE_REMOTE_PREFIX`).
- [x] Decide whether to use default `rclone` config path or explicit configurable path (v1: default config path only; override deferred).

## 12. Cross-distro and packaging

- [x] Define Linux strategy for v1: `rclone` from `PATH`.
- [x] Show clear install/setup error when `rclone` is missing.
- [x] Update docs/install guide with `rclone` requirement for cloud features.
- [x] Consider later support for custom binary path or bundling (defer until packaging/runtime validation shows a concrete need).
- [ ] Test at least two Linux environments (e.g. Fedora + Ubuntu) with different `rclone` versions.
- [x] Decide whether `rclone` path is persisted in Browsey settings (and add settings keys through `src/commands/settings/mod.rs` if yes) (v1: no, because PATH-only strategy).
- [ ] Validate packaging impact for AppImage/Flatpak builds (PATH visibility and external binary discovery).

## 13. Test strategy (important)

- [x] Unit tests for cloud path parser/formatter.
- [x] Unit tests for `rclone` command builder (expected args).
- [x] Unit tests for `stderr`/exit-code mapping to `CloudErrorCode`.
- [x] Create a fake-`rclone` shim for deterministic backend tests.
- [x] Integration tests for list/copy/move/delete using fake `rclone` JSON output.
- [x] Frontend tests for conflict preview and disabled actions based on capabilities.
- [x] Manual test checklist for OneDrive v1 (auth, large files, rename, delete, conflict, refresh).
- [x] Add backend tests for command registration wiring / argument serialization at Tauri boundary where practical.
- [x] Run Browsey standard quality gates before each milestone PR:
  - [x] `cargo check`
  - [x] `cargo test`
  - [x] `cargo fmt`
  - [x] `npm --prefix frontend run check`
  - [x] `npm --prefix frontend run lint`
- [x] Run `npm --prefix frontend run build` before merge of major frontend integration changes.

## 14. Explicitly unsupported in v1 (document clearly)

- [x] Undo for cloud operations (intentionally unsupported in v1; documented).
- [x] Trash/recycle bin for cloud operations (intentionally unsupported in v1; documented).
- [x] Permissions/ownership management for cloud (intentionally unsupported in v1; documented).
- [x] Open-with directly from cloud without local temp-cache strategy (intentionally unsupported in v1; documented).
- [x] Thumbnails from cloud without explicit download/cache pipeline (intentionally unsupported in v1; documented).
- [x] Recursive search / duplicate scan on cloud (intentionally unsupported in v1; documented).

## 15. Prepare for Google Drive (phase 2)

- [x] Keep provider model generic so OneDrive is not hardcoded into shared types/commands.
- [x] Add `ProviderKind` from the start (`onedrive`, `gdrive`, `nextcloud`).
- [x] Add capability matrix per provider.
- [x] Track Google Drive semantic differences (shortcuts, native docs types, trash behavior) as provider-specific TODOs.
  - [x] Decide v1/v2 handling of Google Drive shortcuts (`application/vnd.google-apps.shortcut`) in listing and copy/move flows (defer shortcut-specific handling to v2; treat as unsupported special-case until validated).
  - [x] Decide how native Google Docs/Sheets/Slides types appear in Browsey (`entry kind`, size, open/export affordances) (initial plan: treat as cloud file entries; no direct open/export UX in v1).
  - [x] Define delete/trash semantics for Google Drive vs Browsey permanent delete/trash UI expectations (align with v1 cloud permanent-delete semantics; revisit with cloud trash support).
  - [ ] Verify duplicate-name and parent-folder semantics on Google Drive through `rclone` (conflict preview assumptions may differ).
  - [x] Decide whether Shared Drives are supported in initial Google Drive rollout and how they are surfaced in Network view (initial rollout: not explicitly supported/tested; defer until Google Drive validation phase).
  - [ ] Map provider-specific rate-limit/auth error texts for Google Drive into existing cloud error codes (without polluting common mapping).
- [x] Keep provider-specific error mapping isolated from shared `rclone` wrapper.

## 16. Prepare for Nextcloud (phase 3)

- [x] Model Nextcloud as its own provider (likely via `rclone` WebDAV backend).
- [x] Reserve provider-specific config/validation fields (URL, vendor, app password).
- [x] Handle TLS/certificate errors cleanly in backend error model and UI.
- [x] Test path normalization and special characters carefully for WebDAV.
- [ ] Verify conflict/rename semantics for Nextcloud via `rclone`.

## 17. Documentation and migration

- [x] Update README/docs when first `rclone`-OneDrive support lands.
- [x] Document that GVFS OneDrive is no longer supported and why (robustness / FUSE ghost-state issues).
- [x] Add short migration guide from GVFS/GOA OneDrive to `rclone` remote.
- [x] Document v1 limitations clearly.
- [x] Document Browsey-specific setup flow (where to configure remote name/path in UI/settings).
- [x] Document security posture and limitations (external `rclone` dependency, PATH lookup, config ownership).

## 18. Definition of Done (OneDrive v1)

- [ ] OneDrive can be browsed via `rclone` without GVFS paths.
- [ ] `copy`/`move`/`rename`/`delete`/`mkdir` work reliably without GVFS ghost/`ENOTEMPTY` issues.
- [ ] Conflict preview and overwrite flow work against cloud storage.
- [x] Backend + frontend checks are green (`cargo check`, `cargo test`, frontend check).
- [ ] Manual test checklist completed on at least one OneDrive setup.
- [x] Docs updated with install/setup steps and known limitations.
- [x] Cloud integration follows frontend import/naming rules and does not introduce deep cross-feature imports.
- [x] New backend commands are registered/typed consistently with Browsey's `ApiResult` + error-code conventions.
