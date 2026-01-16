# filey

A fast, cross-platform file explorer built with Tauri 2 (Rust backend) and a Svelte/TypeScript frontend. Heavy lifting such as directory traversal, sorting, search, and metadata happens natively in Rust; the frontend focuses on rendering, input, and interactions.

## Highlights
- **Responsive lists**: Virtualized rows, smooth wheel coalescing, and cached metadata keep large folders responsive.
- **Live updates**: A `notify` watcher emits `dir-changed` events; the UI refreshes with a short debounce.
- **Clipboard & conflicts**: Native copy/cut/paste commands with preview. Pasting into the same folder auto-renames without prompting; other conflicts offer overwrite vs auto-rename.
- **Search**: Recursive, case-insensitive search scoped to the current directory subtree; skips symlinks to avoid loops.
- **Drives & bookmarks**: Lists mounts/partitions (marks removable), bookmarks, starred, recent, and trash. Mounts are polled every 2s and SQLite stores bookmarks, stars, recents, and column widths.
- **Context actions**: New Folder…, Open with…, copy path, cut/copy/paste, compress to ZIP (name + level), rename, move to wastebasket (Delete), delete permanently (Shift+Delete), properties with lazy-loaded timestamps, and “open item location” for recents.
- **Drag & drop**: Internal drag/drop with custom ghost and drop-target highlighting; designed to work on Linux and Windows.
- **Cross-platform details**: Uses system WebView (WebKit on Linux, WebView2 on Windows). Network locations on Windows delete permanently (Explorer parity) because the recycle bin is unavailable there.
- **UI polish**: Address bar shows breadcrumbs when unfocused and selects the full path on focus; renaming pre-selects the filename without its extension; browser default context menu and hotkeys are disabled (except Ctrl+Shift+I), while app shortcuts remain.

## Requirements
Common:
- Rust (stable) via `rustup`
- Node.js LTS + npm (frontend build/dev only)

Linux build deps (Fedora names; adapt to your distro):
- `webkit2gtk4.1-devel` `javascriptcoregtk4.1-devel` `libsoup3-devel` `gtk3-devel`
- `libappindicator-gtk3` `librsvg2-devel` `patchelf` `rpm-build`

Windows:
- WebView2 Runtime (built-in on Win11; otherwise install from Microsoft)
- Visual Studio Build Tools (C++ workload) or full Visual Studio
- Rust via `rustup`, Node LTS

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
Produces `target/release/filey`.

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

## Keyboard & interaction map
- **Typing without focus**: Enters filter mode on the address bar; Esc exits.
- **Search**: `Ctrl+F` (or `Cmd+F` on macOS keyboards) toggles search mode; Esc leaves search mode.
- **Bookmarks**: `Ctrl+B` on a single folder opens the bookmark modal; remove via sidebar close icon.
- **Selection**: `Ctrl+A` selects all. Click-drag draws a selection box.
- **Clipboard**: `Ctrl+C`/`Ctrl+X` copy/cut; `Ctrl+V` paste. Pasting into the same directory auto-renames duplicates; other conflicts prompt overwrite vs auto-rename.
- **Rename**: `F2` or context menu.
- **Delete**: `Delete` moves to wastebasket (or permanently on Windows network paths); `Shift+Delete` deletes permanently with confirmation.
- **Properties**: `Ctrl+P` opens properties with lazy-loaded timestamps; folder sizes reuse the status bar computation.

## Architecture notes
- **Backend (`src/`)**: Tauri commands for listing, search, mounts, bookmarks, starring, trash, rename/delete, open with, clipboard preview/execute, compression to ZIP, and a filesystem watcher. Windows-specific behaviors (e.g., network delete fallback, resilient `read_dir`) are isolated behind cfg gates.
- **Frontend (`frontend/src/`)**: Explorer UI in Svelte with virtualized rows, drag/drop hook, clipboard/context-menu helpers, selection box, toast, and conflict modal. Layout and theme live in `frontend/src/app.css`.
- **Data & persistence**: SQLite DB in the platform data dir stores bookmarks, starred items, recents, and column widths. Capability file `capabilities/default.json` grants event listen/emit so the watcher can signal the UI.
- **Icons**: Mapped in Rust (`src/icons.rs`) to `frontend/public/icons/scalable/...` for native-looking glyphs on both platforms.

## Project layout
- `src/commands/` — Tauri command modules (fs, search, bookmarks, settings, meta, library).
- `src/fs_utils.rs` — Path sanitation, platform helpers, logging.
- `src/clipboard.rs` — Clipboard state, conflict preview, paste/rename/overwrite handling.
- `src/watcher.rs` — Notify-based watcher emitting `dir-changed`.
- `frontend/src/features/explorer/` — Explorer components, hooks, stores, utils, selection.
- `frontend/src/ui/` — Shared UI atoms (toasts, modals, drag ghost, etc.).
- `scripts/` — Dev/build helpers for both shells.
- `resources/` — Icons and generated schemas; `capabilities/` for Tauri permissions.

## Behavior specifics
- Listings sort folders before files and skip symlinks for safety.
- Search is scoped to the current root; empty queries return no results but preserve the listing.
- “Open item location” jumps to the parent and reselects the item.
- Windows network paths delete permanently (recycle bin is unavailable there). Symlink copy/move is rejected.
- Drag/drop uses a custom ghost image; Tauri window drag/drop is disabled to allow HTML5 DnD on Windows.

## License
MIT (see `LICENSE`).
