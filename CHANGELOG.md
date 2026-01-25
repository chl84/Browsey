# Changelog

## v0.3.0-beta1
- Added thumbnail pipeline with caching, format allowlist, permission checks, decode timeouts, and global concurrency limits.
- Grid view now loads thumbnails lazily via IntersectionObserver + queue; falls back to icons instantly on error.
- Asset protocol scope enabled for loading cached thumbnails; cache trimming with size/file caps.
- UI tweaks: larger grid icons, tighter card spacing, custom file-name tooltips, and refined theme toggle spacing.
- Dependency updates and safety hardening around path canonicalization, symlink/device rejection, and symlink-safe temp paths.

## v0.2.0-beta1
- New custom icon set for folders/files/status; refreshed bookmarks, network, trash, and drive icons.
- Theme toggle redesign with clearer affordance and spacing.
- Grid/list polish: badge placement, spacing adjustments, and smoother scrolling.
- Maintenance: dependency bumps and minor fixes.

## v0.1.0-beta1
- Initial public beta with browsing, search, bookmarks, starring, trash, compression, permissions editing, and virtualized grid/list views.
- Cross-platform support via Tauri 2 with Svelte/TypeScript frontend and Rust backend.
