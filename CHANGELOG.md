# Changelog

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
