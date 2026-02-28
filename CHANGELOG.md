# Changelog

## v0.4.5 — 2026-02-26
- Added rclone-backed cloud file support (Linux-first) with direct `rclone://...` paths and Network-view discovery for supported remotes (OneDrive primary target in v1, plus Google Drive/Nextcloud provider groundwork).
- Added core cloud file operations via rclone (`list`, `mkdir`, `copy`, `move/rename`, `delete`) with provider-aware conflict preview, overwrite/auto-rename handling, and capability-driven UI restrictions for unsupported cloud actions.
- Added mixed local-disk <-> cloud copy/move support (files and folders) for clipboard and in-app drag/drop flows, including conflict preview integration, rename-on-conflict retries, provider-aware error mapping, and refresh soft-fail behavior.
- Cloud files can now be opened directly from `rclone://...` paths through a managed local cache, including `Enter`/double-click behavior for supported file entries.
- Cloud file open and mixed cloud/local file transfers now report real byte progress when rclone rc progress data is available, with aggregated batch progress for multi-file uploads/downloads and richer byte detail in the activity pill.
- Mixed local-to-cloud writes now invalidate cloud listing cache correctly, so successful uploads appear after refresh without stale cached listings hiding the result.
- Recent view now prunes dead entries automatically and bounds slow network/GVFS metadata probes, reducing cases where an empty or mostly-stale Recent view opens slowly.
- Explorer drag/drop now supports dropping entries onto bookmark targets in the sidebar, using the same copy/move routing and conflict behavior as breadcrumb drops.
- Cloud UX/performance improvements: background refresh for cloud write operations, refresh coalescing, reduced conflict-preview metadata calls, cloud remote/listing caches with invalidation, bounded per-remote concurrency, and retry/backoff for transient metadata/listing failures.
- Cloud routing hardening: `rclone://` paths no longer enter local FS/undo/GVFS paths, cloud sorting avoids remote reloads on column-sort clicks, and breadcrumbs/direct navigation now handle `rclone://` paths correctly.
- Added cloud-specific UX polish: indeterminate activity indicator for operations without meaningful byte progress, session-only manual-refresh hint for cloud folders, and corrected activity labels (`Copying` vs `Moving`) across paste/drag flows.
- OneDrive/GVFS backend support was removed in favor of rclone-backed cloud integration, and generic Linux GIO/GVFS mount helpers were renamed/refactored (`gvfs.rs` -> `gio_mounts.rs`).
- Added cloud/rclone observability and diagnostics (timing logs, scrubbed command failure logs, perf summary helper script, fake-rclone test shim, and expanded backend/frontend test coverage for cloud and mixed-transfer flows).
- Frontend architecture cleanup (no intended behavior change): explorer modules were reorganized into explicit domains (`context`, `navigation`, `file-ops`, `selection`, `ui-shell`, `state`) and old wrapper paths were removed.
- Explorer factory naming was standardized from `use*.ts` to `create*.ts` where files exported factory APIs, reducing naming ambiguity across hooks/helpers.
- Explorer state internals were split into focused slices/stores while preserving the public `createExplorerState` API to avoid integration breakage.
- Settings UI internals were modularized: `SettingsModal` is now split into tab/section components with a dedicated view-model hook.
- Frontend boundaries are now enforced with feature barrels and ESLint restrictions for cross-feature deep imports (wired into CI/local lint).
- Naming conventions now have automated lint enforcement via `frontend/scripts/check-naming-conventions.mjs` (`npm --prefix frontend run lint` runs ESLint + naming checks).
- Architecture docs were expanded with naming/import guidance (`ARCHITECTURE_NAMING.md`, `ARCHITECTURE_IMPORTS.md`) and README cross-links.
- Fedora/GNOME Software packaging metadata was added (AppStream + desktop metadata + packaging wiring) to improve distribution readiness.
- Completed implementation TODO plans were archived under `docs/todo-archive/`, and remaining project text/comments were normalized to English.
- Frontend structure split: the former monolithic `App.svelte` explorer logic is now decomposed into feature hooks (`navigation`, `search session`, `file ops`, `context menu`, `input handlers`) with `ExplorerPage.svelte` as the composition root.
- Backend error flow migration was expanded across remaining modules, replacing string-based failures with code-based `ApiError` mapping in command and core subsystems.
- New domain-level error modules were introduced in core areas (`fs_utils`, `metadata`, `statusbar`, `undo`) to standardize classification and reduce ad-hoc text matching.
- Undo internals were fully migrated to typed errors and split into focused internal modules (`backup`, `engine`, `nofollow`, `path_checks`, `path_ops`, `security`, `types`, `error`) with updated tests.
- Frontend error handling now uses a shared Tauri invoke wrapper plus error normalization utilities, eliminating `[object Object]` toast output for structured backend errors.
- Drag/drop handling was moved out of `App.svelte` into dedicated explorer hooks, and backend policy resolution now owns copy-vs-move decisions for drop operations.
- URI/network classification rules were further centralized in backend command modules, reducing duplicated frontend scheme mapping logic.
- Extract-action availability is now sourced from backend command capabilities instead of frontend extension-only heuristics.
- Backend source layout was tightened with additional modular splits across commands/core (for example `open_with`, `clipboard`, `undo`, `fs/trash`, `context_menu`, and related command domains).
- Explorer wheel scrolling was simplified and stabilized: a single always-on wheel assist strategy now owns scrolling in list/grid, with centralized tuning and deterministic handling of non-cancelable wheel bursts.
- Explorer wheel/rendering behavior under extreme wheel input was tuned by reducing per-event max step, increasing list/grid virtualization overscan, and snapping scroll targets to integer pixels to reduce transient flicker/half-tone artifacts.
- Sorting behavior and performance were refined: `Size` sorting now keeps files before links before directories in both directions, directories sort by item count in the `Size` column, the unused `Starred` sort path was removed, backend sort keys are cached more aggressively, and frontend in-memory search sorting now uses cached/decorated keys for large result sets.
- Explorer state internals were further decomposed into dedicated `state/*` modules (`searchSort`, `entryMutations`, `createSortRefreshDispatcher`, `searchRuntimeHelpers`, `createSearchSession`) while preserving the public `createExplorerState` API, plus a new `frontend/src/features/explorer/state/README.md` documents folder boundaries.
- Modal shell behavior was hardened: immediate `Esc` close now works even before modal content receives focus, `Esc` close handlers no longer double-fire via bubbling, and focus is restored to a sensible prior element when modals close.
- Modal keyboard defaults were improved: duplicate-check search can start with `Enter` from the search-root field, confirm/conflict/delete dialogs now support immediate `Enter` actions via explicit default focus policies, and duplicate in-input `Esc` handlers were removed where `ModalShell` already handles closing.
- Conflict handling correctness was fixed for drag-move auto-rename: choosing `Auto-rename` on name conflicts no longer overwrites existing targets during move operations on Linux/Unix paths; backend now preserves no-overwrite behavior so rename-candidate retries can run.
- Properties modal UX and stability were improved: ownership moved to a dedicated tab, permission toggles are temporarily disabled during async apply (without flashing the whole permissions pane), the layout was tightened/resized/responsive-tuned, and owner/group dropdowns can overflow beyond modal bounds when needed.
- A custom shared slider UI component was added (square thumb + square track) and wired into settings/compression controls, replacing native range styling inconsistencies.
- Advanced Rename preview updates no longer visibly flicker/repaint the modal while typing; preview now updates in place without swapping the preview pane content.
- Search now supports a scoped AQS-lite query syntax in backend streaming search (`AND`/`OR`/`NOT`, grouping, wildcards `*`/`?`, quoted exact phrases, exact-value `=`, and field filters for `name`, `filename`, `folder`, `path`, `hidden`, and `readonly`), and search-mode frontend filtering no longer re-filters backend AQS results as plain text.

## v0.4.4 — 2026-02-17
- Destructive move hardening: removed Linux check-then-rename compatibility fallback when `renameat2(RENAME_NOREPLACE)` is unavailable; operations now use a controlled non-overwrite copy+delete fallback with explicit narrower (non-atomic) guarantees.
- Windows/portable destructive-op hardening: Windows rename path now uses the native move API with explicit destination-exists mapping, and non-Linux recursive delete now validates no-follow metadata recursively instead of calling raw `remove_dir_all`.
- Archive extraction hardening: Linux extraction now uses descriptor-based no-follow directory/file primitives across tar/zip/7z/rar and single-file decompress paths to reduce symlink and path-race exposure.
- Archive safety limits are now disk-aware: effective extraction byte cap is computed from available destination disk space with a 1 GiB reserve, plus periodic runtime free-space checks during writes.
- Clipboard copy hardening: fallback copy now uses no-clobber file creation (`create_new`), and rename conflict handling uses deterministic candidate retries without pre-`exists()` probing.
- Duplicate scan pressure controls: collection now enforces scanned/candidate file caps and iterates `read_dir` streams directly (no full directory-entry buffering).
- Properties modal ownership editing now uses searchable User/Group dropdowns populated from discovered system principals.
- Wastebasket list mode now resolves icon type from original item metadata so entries show file-type-specific icons instead of a generic file icon.
- Keyboard UX: `Esc` now exits both search mode and filter mode directly to breadcrumb view (address mode with unfocused path input).
- Address mode UX: pressing `Esc` while editing the path now restores the current valid location path before returning to breadcrumbs.
- Filter mode UX: pressing `Enter` is now a no-op (it no longer triggers path navigation).
- Clipboard UX/performance: large-selection `Ctrl+C`/`Ctrl+X` now use path-based flows that avoid quadratic selection scans.
- Context-menu and delete flows were optimized for large selections (Set/Map lookups and reduced repeated selection reconstruction).
- Clipboard/file-operation shutdown handling was hardened to reduce late-stage work and event emissions during app exit.
- Input/search refactor: mode transitions (`address`/`filter`/search session) are now centralized for more consistent state resets.
- Search state cleanup: `searchRunning` now represents active backend search execution, with state ownership moved to the explorer state layer.
- Wastebasket delete performance: trash entries are now resolved and purged by stable trash IDs, reducing unnecessary `.trashinfo` scans.
- Wastebasket reliability/security hardening: Unix trash/undo rename-delete paths now use descriptor-based no-follow primitives to reduce symlink and check-then-use race exposure.
- Wastebasket compatibility: no-overwrite rename now falls back on Linux when `renameat2(RENAME_NOREPLACE)` is unavailable, with documented narrower race guarantees instead of hard failure.
- Wastebasket crash recovery: staged trash renames are now journaled and recovered on startup if a previous trash operation was interrupted.
- Windows wastebasket correctness: trash moves no longer use staged renames on Windows, so restore keeps the original path and filename.
- Wastebasket internals were refactored behind a backend abstraction and covered with rollback/fallback/cleanup unit tests.
- Linux Open With hardening: selected app IDs are now resolved only from canonical in-scope `.desktop` files (symlink/out-of-scope entries are rejected).
- Properties permissions: ownership editing (`user`/`group`) now supports privilege escalation on Linux via `pkexec` helper fallback when needed.
- Permissions/ownership behavior: changes from the Properties modal are now intentionally excluded from undo/redo history.
- Permissions/ownership safety: rollback paths are now decoupled from undo action types and validated with dedicated partial-rollback failure tests.
- Properties modal polish: permissions/ownership layout now follows density-aware sizing (cozy/compact), with a smaller ownership apply button.
- Error readability: long error messages now wrap in modal error pills, properties ownership errors, and notice banners.
- Frontend ownership flow now suppresses expected warning-noise in dev logs (for example auth dismissal or unknown user/group validation errors).
- App logs now use local timestamps with timezone offset (for example `+01:00`) instead of UTC `Z` formatting.
- Network layer was split into dedicated backend/frontend modules (`src/commands/network/*`, `frontend/src/features/network/*`) and wired into the `Network` view lifecycle.
- Network discovery now aggregates GVFS (`gio mount -li`), Avahi/mDNS, and SSDP sources to surface broader SFTP/SMB/NFS/FTP/WebDAV/AFP/HTTP/HTTPS endpoints.
- Address bar + URI handling now supports broader server-address aliases (`ssh`→`sftp`, `webdav`/`webdavs`→`dav`/`davs`, `ftps` accepted as FTP-family alias for normalization/matching).
- Mount UX now reports explicit outcomes (`Connecting`, `Already connected`, `Connected`, `Failed`) from backend to frontend activity labels.
- GVFS mount visibility checks were hardened with retries and stricter mounted-URI validation.
- Linux partitions now hide the generic GVFS root mount while still surfacing concrete GVFS endpoints (for example active MTP mounts).
- Network context menu is now URI-aware: mountable URIs show `Connect`/`Copy Server Address`, HTTP(S) URIs show `Open in Browser`, mounted paths keep `Open`/`Disconnect`.
- Properties modal now supports virtual network URIs in the Extra tab by showing parsed URI fields (address/protocol/user/host/port/path/query/fragment) without failing filesystem metadata probes.
- Column-filter UX was refined: facet staleness/parity issues were fixed, active filter indicators were improved for both list and grid modes, and grid now shows an explicit active-filter notice when headers are hidden.

## v0.4.3 — 2026-02-13
- Added a topbar main action menu (hamburger) with wired actions for Settings, Keyboard Shortcuts, Search, view-mode toggle (List/Grid), hidden-files toggle, Refresh, and About.
- Added a dedicated About modal with three tabs: `Version` (embedded changelog), `Build` (runtime/build target details), and `License`.
- License tab now shows both `LICENSE` and `THIRD_PARTY_NOTICES` in one combined scrollable text field.

## v0.4.2 — 2026-02-13
- Column filters now apply real filtering on top of text filter/search, with name/type/modified/size buckets, reset via right-click, and red active indicators.
- Size/modified/type filter options are sourced from the current listing or backend column sets; hidden files are respected and size buckets skip folders.
- Settings: `Double-click speed` is now wired to actual list/grid mouse-open behavior and persisted as a validated preference.
- Settings: Added Data maintenance actions to clear thumbnail cache, stars, bookmarks, and recents with confirmation dialogs and per-action toasts.
- Thumbnail cache clear now removes cached files on disk and refreshes visible thumbnails in the UI.
- Properties modal: Extra metadata is now lazy-loaded when the **Extra** tab is opened (no eager metadata fetch on modal open).
- Extra metadata backend reorganized into type-specific providers (image, pdf, audio, video, archive) and no longer duplicates Basic-tab fields.
- Extra tab UI simplified by removing the redundant `kind` row and section headings; it now focuses on the metadata fields directly.
- Image extra-metadata routing now includes `.tif` and `.tga`.
- Image extra-metadata routing now includes `.hdr` and `.exr`.
- Bundled Linux PDFium updated to `146.0.7678.0` (including refreshed `libpdfium.so`, headers, and license set).
- Linux open-console launcher now uses a strict terminal allowlist and fixed arguments (removed env-driven terminal command overrides).
- Extraction safety guardrails expanded: total output cap (`100 GB`) and total-entry cap (`2,000,000`) are both enforced.
- `RAR` extraction now streams entry data in chunks instead of buffering whole entries in memory.
- Clipboard helper binaries (`wl-copy`, `wl-paste`, `xclip`) and `ffprobe` now resolve through canonical path checks before process spawn.

## v0.4.1 — 2026-02-08
- Linux rendering fallback simplified: when hardware acceleration is disabled, Browsey now sets only `WEBKIT_DISABLE_DMABUF_RENDERER=1` (removed legacy compositing/software flags).
- Added **Check for Duplicates** tool in the file context menu (single-file selection), with a dedicated modal built on the shared modal shell and app-wide modal styling.
- Duplicate scan backend: two-stage matching (file size pre-filter, then byte-for-byte compare with early mismatch exit), symlink-safe traversal, and deterministic sorted output.
- Duplicate scan UX: streaming progress, modal progress bar, clean cancellation when closing the modal (including Esc), and improved result preview/copy behavior.
- Docs: README updated with duplicate-check behavior and current Linux hardware-acceleration policy.

## v0.4.0 — 2026-02-06
- Hardened undo backup cleanup: validates undo dir location before deleting contents and avoids deleting the root folder outright.
- Search command now runs in a blocking task to prevent UI freezes on large trees.
- Asset protocol scope made portable via cache-dir placeholder, restoring thumbnail access across machines.
- Archive handling: added 7z and RAR extraction, batch extract with shared progress/cancel and undo, plus safer single-root handling and ZIP level/name persistence.
- Thumbnails: switched to PDFium backend with bundled binaries, faster decode pipeline (pool scaling, retries, timeouts), two-generation cache and video-thumb preference; PDF caching and resource lookup fixed.
- GVFS/MTP: better mount detection, polling/cancel/debounce to avoid UI hangs; copy/move now supports progress, cancel, and gio fallback; clearer cloud labels and icons.
- Scroll/viewport perf: list scroll and wheel events now rAF-throttled; entry-meta refresh batched to reduce jank.
- Drag/drop & clipboard: native file drop support with correct copy/move hints; system clipboard cut/copy integration and conflict modal readability improvements.
- Settings & UX: persistent defaults for view, start directory, folders-first, hidden-last, show hidden, confirm delete, density (cozy/compact); cleaned settings UI and removed unused theme/icon controls.
- Docs: README notes inspiration from GNOME Nautilus; version bumped to 0.4.0.

## v0.3.0-beta1 — 2026-01-25
- Added thumbnail pipeline with caching, format allowlist, permission checks, decode timeouts, and global concurrency limits.
- Grid view now loads thumbnails lazily via IntersectionObserver + queue; falls back to icons instantly on error.
- Asset protocol scope enabled for loading cached thumbnails; cache trimming with size/file caps.
- UI tweaks: larger grid icons, tighter card spacing, custom file-name tooltips, and refined theme toggle spacing.
- Dependency updates and safety hardening around path canonicalization, symlink/device rejection, and symlink-safe temp paths.

## v0.2.0-beta1 — 2025-01-18
- New custom icon set for folders/files/status; refreshed bookmarks, network, trash, and drive icons.
- Theme toggle redesign with clearer affordance and spacing.
- Grid/list polish: badge placement, spacing adjustments, and smoother scrolling.
- Maintenance: dependency bumps and minor fixes.

## v0.1.0-beta1 — 2025-01-11
- Initial public beta with browsing, search, bookmarks, starring, trash, compression, permissions editing, and virtualized grid/list views.
- Cross-platform support via Tauri 2 with Svelte/TypeScript frontend and Rust backend.
