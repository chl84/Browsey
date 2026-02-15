# Changelog

## Unreleased
- Keyboard UX: `Esc` now exits both search mode and filter mode directly to breadcrumb view (address mode with unfocused path input).
- Address mode UX: pressing `Esc` while editing the path now restores the current valid location path before returning to breadcrumbs.
- Filter mode UX: pressing `Enter` is now a no-op (it no longer triggers path navigation).
- Clipboard UX/performance: large-selection `Ctrl+C`/`Ctrl+X` now use path-based flows that avoid quadratic selection scans.
- Context-menu and delete flows were optimized for large selections (Set/Map lookups and reduced repeated selection reconstruction).
- Clipboard/file-operation shutdown handling was hardened to reduce late-stage work and event emissions during app exit.
- Input/search refactor: mode transitions (`address`/`filter`/search session) are now centralized for more consistent state resets.
- Search state cleanup: `searchRunning` now represents active backend search execution, with state ownership moved to the explorer state layer.
- Wastebasket delete performance: trash entries are now resolved and purged by stable trash IDs, reducing unnecessary `.trashinfo` scans.
- Properties permissions: ownership editing (`user`/`group`) now supports privilege escalation on Linux via `pkexec` helper fallback when needed.
- Properties modal polish: permissions/ownership layout now follows density-aware sizing (cozy/compact), with a smaller ownership apply button.
- Error readability: long error messages now wrap in modal error pills, properties ownership errors, and notice banners.
- Frontend ownership flow now suppresses expected warning-noise in dev logs (for example auth dismissal or unknown user/group validation errors).
- App logs now use local timestamps with timezone offset (for example `+01:00`) instead of UTC `Z` formatting.

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
- GVFS/OneDrive/MTP: better mount detection, polling/cancel/debounce to avoid UI hangs; copy/move now supports progress, cancel, and gio fallback; clearer cloud labels and icons.
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
