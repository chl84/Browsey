# filey

Minimal file explorer built with Tauri 2 (Rust backend) and Svelte/TypeScript frontend. Heavy work (I/O, sorting, recursive search, metadata) stays in Rust to keep the UI responsive and the bundle small; the frontend handles rendering and simple interactions only.

## Architecture & performance
- **Rust-first**: `list_dir` and `search` (see `src/main.rs`, `src/search.rs`) read directories, skip symlinks, sort folders before files, and return fully formatted entries (size, modified, icon path).
- **Auto-refresh**: A `notify` watcher (`src/watcher.rs`) listens for create/modify/remove in the current directory and emits `dir-changed`; the frontend listens and refreshes with a 300 ms debounce.
- **Virtualized list**: The file list renders only visible rows for large directories to keep hover/scroll smooth.
- **Smooth wheel scrolling**: Wheel deltas are coalesced and applied once per animation frame to avoid stutter on fast mouse wheels.
- **Small binaries**: Release profile in `.cargo/config.toml` uses `opt-level="z"`, thin LTO, `panic=abort`, strip. Uses system WebView (WebKit on Linux, WebView2 on Windows) so bundles stay small.
- **Icons**: Mapped in Rust (`src/icons.rs`) to `frontend/public/icons/scalable/...` so both Linux and Windows get native-looking glyphs.
- **Dynamic mounts**: Partitions list uses sysinfo to enumerate mounted volumes, labels by mount point (so `/` and `/home` don’t duplicate a device), marks removable vs. fixed, and polls every 2 s.
- **Bookmarks & layout prefs**: Stored in SQLite; add via Ctrl+B on a single selected folder (modal lets you rename), remove from sidebar “x”. Column widths are also persisted in the same DB.

## Dependencies
Common:
- Rust stable (`rustup`).
- Node.js LTS + npm (frontend build/dev only).

Linux (Fedora names; adjust per distro):
- `webkit2gtk4.1-devel` `javascriptcoregtk4.1-devel` `libsoup3-devel` `gtk3-devel` `libappindicator-gtk3` `librsvg2-devel` `patchelf` `rpm-build`.
- RPM build example: `sudo dnf install webkit2gtk4.1-devel javascriptcoregtk4.1-devel libsoup3-devel gtk3-devel libappindicator-gtk3 librsvg2-devel patchelf rpm-build`.

Windows:
- WebView2 Runtime (built-in on Win11; otherwise install from Microsoft).
- Visual Studio Build Tools (C++ workload) or full VS.
- Rust via `rustup`, Node LTS.

## Development
1) Install system deps (above).
2) Frontend deps:
   ```bash
   npm --prefix frontend install
   ```
3) Run dev (Vite on 5173 via `beforeDevCommand`):
   ```bash
   cargo tauri dev --no-dev-server
   ```
   Wrapper: `./scripts/dev-server.sh`

Quick checks:
```bash
cargo check
```

## Build
Frontend:
```bash
npm --prefix frontend run build
```

Rust release binary:
```bash
cargo build --release
```
Result: `target/release/filey`

Tauri bundle:
- Linux RPM (smallest on Fedora-like distros):
  ```bash
  cargo tauri build --bundles rpm
  ```
  Output: `target/release/bundle/rpm/filey-<version>.rpm`
- Helper script with local ccache temp:
  ```bash
  ./scripts/build-release.sh
  ```
- Install RPM (Fedora/RHEL/openSUSE):
  ```bash
  sudo rpm -Uvh target/release/bundle/rpm/filey-0.2.0-1.x86_64.rpm
  ```
- Uninstall RPM:
  ```bash
  sudo rpm -e filey
  ```
- Windows:
  ```bash
  cargo tauri build
  ```
  Produces MSI/installer using system WebView2 (no bundled browser).

## Runtime notes
- Dev port: 5173 (see `tauri.conf.json` and `scripts/start-vite.sh`). If occupied, stop the process or adjust the port.
- Mounts refresh automatically every 2 s; removable drives get a USB icon, and if you browse a device that disconnects the app falls back to Home with an error message.
- Hidden files render at half opacity in the list. Sidebar auto-collapses under 700 px width. Fixed 24 px shell padding.
- Search is recursive, case-insensitive, skips symlinks, and matches on the current path subtree. Empty search returns an empty result and preserves the listing.
- Data lives in SQLite at the platform data dir (Linux: `~/.local/share/filey/filey.db`) and holds bookmarks, starred, recent, and column widths.
- Permissions: capability file `capabilities/default.json` grants `core:event` listen/emit so the watcher can refresh the UI.
- Shortcuts: see section below.
- Context menu: right-click rows for “Open with…”, Copy path, Cut/Copy, Rename (F2), Move to wastebasket (Delete), Delete permanently (Shift+Delete with confirmation), Properties (Ctrl+P). Properties lazy-loads accessed/created timestamps; folder sizes reuse the statusbar computation.

## Shortcuts & modes
- Modes share ett inputfelt: **adresse** (standard), **filter** (når du begynner å skrive uten fokus), **søk** (etter Ctrl/⌘+F).
- **Ctrl/⌘+F**: aktiverer søkemodus, fokuserer input. **Esc**: avslutter søk og går til adressemodus.
- **Filtrering**: når input ikke er fokusert og du taster bokstaver/tall, går vi til filtreringsmodus, fokuserer feltet, og filteret oppdateres mens du skriver og når du sletter. Shift+digit ignoreres.
- **Ctrl/⌘+B**: åpner bokmerkemodal for én markert mappe.
- **Ctrl/⌘+A** i fil-listen: marker alt. **Esc** tømmer markering/blur i listen og lukker åpne modaler/menyer (før øvrige snarveier).

## UI
- Dark, neutral greys (no chroma) defined in `frontend/src/app.css`.
- Columns: Name, Type, Modified, Size, ⭐; name is line-clamped to 2 lines. Sidebar sections: Places, Bookmarks, Partitions. Bookmarks show an “x” on hover to remove; drives use different icons for fixed vs removable.
- Virtualized scrolling container keeps hover smooth on large folders.

## Frontend structure
- `frontend/src/App.svelte`: page shell, wiring stores to components (sidebar, topbar, file list, status bar, bookmark modal).
- Stores: `lib/explorer/state.ts` (Tauri I/O: listings, search, bookmarks, partitions, sort), `lib/explorer/stores/listState.ts` (selection, virtual scroll, DOM refs for rows/header).
- Components: `lib/components/explorer/` (Sidebar + sections, Topbar, FileList with header/row/resizer, Statusbar, Notice, BookmarkModal).

## Next steps
- Add Rust commands for copy/move/delete/rename to keep FS ops native.
- Optional debounce tuning or batching for watcher events if directories churn heavily.
