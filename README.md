# Browsey

Browsey is a minimalist and fast, cross-platform file explorer built with Tauri 2 (Rust backend) and a Svelte/TypeScript frontend. The chrome stays light while Rust handles traversal, sorting, search, and metadata; the frontend focuses on rendering, input, and interactions. It is inspired by GNOME Nautilus, aiming for that familiar feel with a lighter, faster stack.

Documentation: https://chl84.github.io/Browsey/

## Status
Early beta: core flows (browse, search, clipboard, trash, compress, duplicate checks, open with, properties) are in place; expect rapid iteration and some rough edges. Permissions editing now works on Unix (POSIX mode bits) **and** Windows (DACLs for owner/group/everyone, plus read-only/executable toggles).

## Highlights
- **Responsive lists**: Virtualized rows, smooth wheel coalescing, and cached metadata keep large folders responsive.
- **Live updates**: A `notify` watcher emits `dir-changed` events; the UI refreshes with a short debounce. Bookmarked paths are automatically allowlisted for watching (handy for mapped network drives).
- **Clipboard & conflicts**: Native copy/cut/paste commands with preview. Pasting into the same folder auto-renames without prompting; other conflicts offer overwrite vs auto-rename. Long copies (e.g., GVFS OneDrive/MTP) run in the background with live progress, size hints, and cancel support.
- **Search**: Recursive, case-insensitive search scoped to the current directory subtree; skips symlinks to avoid loops.
- **Duplicate checks**: Context action for a single selected file. Scan starts from a user-selected folder, ignores symlinks, streams progress in two phases (size filter, then byte compare), and supports clean cancellation when the modal closes.
- **Drag targets**: Internal drag/drop supports breadcrumbs as drop targets with visual highlighting.
- **Drives & bookmarks**: Lists mounts/partitions (marks removable), bookmarks, starred, recent, and trash. Mount polling is configurable (500-10000 ms, default 8000 ms), and SQLite stores bookmarks, stars, recents, and column widths.
- **Context actions**: New Folder…, Open with (associated apps + custom command), copy path, cut/copy/paste, compress to ZIP (name + level), Check for Duplicates (single file), rename, move to wastebasket (Delete), delete permanently (Shift+Delete), properties with lazy-loaded tabs, and “open item location” for recents.
- **Configurable shortcuts**: Settings includes a persisted shortcut keymap editor. Click a shortcut to capture a new key combo; bindings are validated/canonicalized and duplicate conflicts are rejected.
- **Settings data actions**: Settings > Data can clear thumbnail cache, stars, bookmarks, and recents with confirmation dialogs and per-action toast feedback.
- **Properties metadata**: The Extra tab loads on demand and shows type-specific metadata only (for example image resolution/color model/depth, PDF document fields, audio/video codec and timing, archive format and entry stats).
- **Archive extraction**: Zip/Tar(+gz/bz2/xz/zst)/GZ/BZ2/XZ/Zstd/7z and RAR (stored entries); supports multi-archive batch extract with cancel/progress/undo. Unsupported RAR compression methods fail fast instead of writing corrupt data.
- **Drag & drop**: Internal drag/drop with custom ghost and drop-target highlighting; designed to work on Linux and Windows.
- **Thumbnails**: Lazy, virtualized thumbnails with caching and per-file permission checks. SVG rasterized via resvg; PDFs via bundled PDFium; images via `image` crate; **videos** via ffmpeg first-frame grabs with cancellation on navigation.
- **Grid view parity**: Fixed-size cards with virtualization, keyboard navigation and range selection, lasso overlay, hidden-item dimming, and consistent click-to-clear selection; names can span up to three lines but stay aligned to show the start.
- **Interaction tuning**: Double-click speed in Settings directly controls list/grid open timing for mouse-based file/folder opening.
- **Theming & density**: Dark by default plus a light toggle; “cozy” vs “compact” density presets resize rows, grid cards, icons, and even the sidebar, all via CSS variables in `frontend/src/app.css`.
- **Cross-platform details**: Uses system WebView (WebKit on Linux, WebView2 on Windows). Network locations on Windows delete permanently (Explorer parity) because the recycle bin is unavailable there. On Linux, when hardware acceleration is disabled in settings, Browsey keeps compositing enabled and only sets `WEBKIT_DISABLE_DMABUF_RENDERER=1`.
- **Removable & cloud drives**: Detects removable volumes and offers an eject action on Windows (CfgMgr/SetupAPI + IOCTL fallback) and on Linux (`gio`/`umount`/`udisksctl` with lazy fallback); safely-ejected drives are hidden from the list. GVFS mounts for MTP (phones) and OneDrive appear with appropriate icons; busy devices surface a short “in use” hint. On Windows we enumerate only drive-letter volumes; OneDrive works as its synced local folders and MTP devices without drive letters are not listed.
- **UI polish**: Flat, squared styling across inputs/buttons/modals; address bar shows breadcrumbs when unfocused and selects the full path on focus; renaming pre-selects the filename without its extension; browser default context menu and hotkeys are disabled (except Ctrl+Shift+I), while app shortcuts remain.
- **Visual cues for access**: Read-only items show an eye icon; items without read access show a padlock. Multi-select permission changes apply in one batch with undo/rollback on failure (Unix and Windows).
- **User defaults**: Persists show hidden, hidden-last, folders-first, start directory, default view (list/grid), default sort field/direction, delete confirmation, mount polling interval, and double-click speed. Defaults load before the first listing so startup respects your prefs.

## Screenshots
![Browsey showing a Fedora workspace](resources/01_screenshot_browsey_fedora.png)
![Browsey in grid view with thumbnails](resources/02_screenshot_browsey_fedora.png)

## Requirements
Supported platforms: Linux and Windows (macOS is not supported yet).

Common:
- Rust (stable) via `rustup`
- Node.js LTS + npm (frontend build/dev only)
- PDFium is bundled in `resources/pdfium-<platform>/` so no system PDF libs are needed.
- Optional for video thumbnails: `ffmpeg` in PATH (or `FFMPEG_BIN`), otherwise video files fall back to icons.

Linux build deps (Fedora names; adapt to your distro):
- `webkit2gtk4.1-devel` `javascriptcoregtk4.1-devel` `libsoup3-devel` `gtk3-devel`
- `libappindicator-gtk3` `librsvg2-devel` `patchelf` `rpm-build`

Windows:
- WebView2 Runtime (built-in on Win11; otherwise install from Microsoft)
- Visual Studio Build Tools (C++ workload) or full Visual Studio
- Rust via `rustup`, Node LTS

## Install
- Fedora/RPM: download the latest `Browsey-<version>-1.x86_64.rpm` from Releases and install with `sudo rpm -Uvh --replacepkgs Browsey-<version>-1.x86_64.rpm`.
- Windows: grab the NSIS installer from Releases and run it (bundled by `cargo tauri build --bundles nsis`).
- From source: clone, run `npm --prefix frontend install`, then `cargo tauri dev --no-dev-server` (or `cargo tauri build` for a release bundle).

## Development
1) Install system deps (above).
2) Install frontend deps:
   ```bash
   npm --prefix frontend install
   ```
3) Run dev (Vite on 5173 is started by the Tauri hook):
   ```bash
   cargo tauri dev --no-dev-server
   ```
   Convenience wrappers: `scripts/dev-server.sh` (Unix) or `scripts/dev-server.bat` (Windows).

Quick checks:
```bash
cargo check
npm --prefix frontend run check
```

## Building
Frontend only:
```bash
npm --prefix frontend run build
```

Rust release binary:
```bash
cargo build --release
```
Produces `target/release/browsey`.

Tauri bundles:
- Windows NSIS:
  ```bash
  cargo tauri build --bundles nsis
  ```
  or use `scripts/build-release.bat` (cleans old bundles, builds frontend, then bundles). Output lands in `target/release/bundle/nsis/`.
- Linux RPM (smallest on Fedora-like distros):
  ```bash
  cargo tauri build --bundles rpm
  ```
  Helper: `scripts/build-release.sh`. Output in `target/release/bundle/rpm/`.

## Keyboard & interaction map (defaults)
- Shortcuts below are default bindings and can be remapped in Settings.
- **Typing without focus**: Enters filter mode on the address bar; Esc exits.
- **Search**: `Ctrl+F` toggles search mode; Esc leaves search mode.
- **View toggle**: `Ctrl+G` toggles between list and grid.
- **Bookmarks**: `Ctrl+B` on a single folder opens the bookmark modal; remove via sidebar close icon.
- **Open console**: `Ctrl+T` opens a terminal at the current directory when in a folder view.
- **Selection**: `Ctrl+A` selects all. Click-drag draws a selection box (works in both list and grid). In grid view, arrow keys + Shift handle range selection; clicking blank space clears selection.
- **Clipboard**: `Ctrl+C`/`Ctrl+X` copy/cut; `Ctrl+V` paste. Pasting into the same directory auto-renames duplicates; other conflicts prompt overwrite vs auto-rename.
- **Rename**: `F2` or context menu.
- **Delete**: `Delete` moves to wastebasket (or permanently on Windows network paths); `Shift+Delete` deletes permanently with confirmation.
- **Properties**: `Ctrl+P` opens properties; folder sizes reuse the status bar computation, and the Extra tab loads metadata only when activated.
- **Hidden files**: `Ctrl+H` toggles showing hidden files (hidden items are shown by default).

## Architecture notes
- **Backend (`src/`)**: Tauri commands for listing, streaming search, mounts, bookmarks, starring, trash, rename/delete, open with (desktop entries on Linux, custom commands, and default handler), clipboard preview/execute, compression to ZIP, duplicate scanning, shortcut keymap management, and a filesystem watcher. Search uses the `search_stream` command with incremental batches and cancellation support. Duplicate scanning supports a streaming command with progress and cancel tokens. Thumbnail pipeline uses resvg for SVG and bundled PDFium for PDFs, keeping heavy files in check. Windows-specific behaviors (e.g., network delete fallback, resilient `read_dir`) sit behind cfg gates.
- **Frontend (`frontend/src/`)**: Explorer UI in Svelte with virtualized rows/cards, drag/drop, lasso selection, context menus, modals, and a thumbnail loader. All Tauri `invoke` calls are wrapped in `features/explorer/services/` (clipboard, trash, listing, files, layout, history, activity, star, bookmarks, data). Layout and theming live in `frontend/src/app.css`. Modals share structure via `frontend/src/ui/ModalShell.svelte` and `frontend/src/ui/modalUtils.ts`.
- **Data & persistence**: SQLite DB in the platform data dir stores bookmarks, starred items, recents, column widths, and shortcut keymap overrides. Thumbnail cache lives under the user cache dir with periodic trimming. Capability file `capabilities/default.json` grants event listen/emit so the watcher can signal the UI.
- **Icons**: Uses a custom Browsey icon set in `frontend/public/icons/scalable/browsey/` mapped via `src/icons.rs`, covering sidebar items, folders (incl. templates, public, desktop, etc.), files (images, text, pdf, spreadsheets, presentations), compressed archives, and shortcuts. Removable disks and bookmarks also use the new set.

## Project layout
- `src/commands/` — Tauri command modules (fs, search, bookmarks, settings, keymap, meta, library).
- `src/keymap/` — Shortcut keymap definitions, accelerator validation/canonicalization, and conflict checks.
- `src/fs_utils.rs` — Path sanitation, platform helpers, logging.
- `src/clipboard.rs` — Clipboard state, conflict preview, paste/rename/overwrite handling.
- `src/watcher.rs` — Notify-based watcher emitting `dir-changed`.
- `frontend/src/features/explorer/` — Explorer components, hooks, stores, services, utils, selection.
- `frontend/src/features/explorer/services/` — All Tauri API calls wrapped as small modules (clipboard, trash, listing, files, layout, history, activity, star, bookmarks, data).
- `frontend/src/ui/` — Shared UI atoms (toasts, modals, drag ghost, etc.).
- `scripts/` — Dev/build helpers for both shells.
- `resources/` — Icons and generated schemas; `capabilities/` for Tauri permissions.
- `resources/pdfium-*/` — Bundled PDFium binaries and licenses for Linux/Windows.

## Behavior specifics
- Listings sort folders before files and skip symlinks for safety.
- Search is scoped to the current root and streamed incrementally; final ordering is applied in the frontend using the active sort setting. Empty queries return no results but preserve the listing.
- Duplicate checks compare by file size first, then byte-for-byte with early exit on mismatch; symlinks are ignored.
- “Open item location” jumps to the parent and reselects the item.
- Windows network paths delete permanently (recycle bin is unavailable there). Symlink copy/move is rejected.
- Permissions: Owner/group/other (Everyone) access bits can be edited on Unix and Windows; Windows maps to the file DACL and honors read-only and executable toggles. Multi-select permission state is aggregated across the full selection with bounded parallelism, and unsupported targets (for example symlinks) are handled without aborting the full batch.
- Removable volumes on Windows expose an eject action; once a device is successfully ejected the UI removes it and filters out NOT_READY/DEVICE_NOT_CONNECTED remnants.
- Open with modal lists matching applications (fallbacks included), allows a custom command, uses the system default when chosen, and launches apps detached without console noise.
- Drag/drop uses a custom ghost image; Tauri window drag/drop is disabled to allow HTML5 DnD on Windows.

## Disclaimer
Browsey performs file operations (copy, move, rename, compress, trash, delete). Use it at your own risk, keep backups of important data, and verify paths before destructive actions. The software is provided as-is without warranties; contributors are not liable for data loss or other damage.

## License
MIT (see `LICENSE`).
