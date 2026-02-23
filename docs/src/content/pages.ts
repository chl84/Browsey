export type DocSection = {
  id: string
  title: string
  body?: string
  bullets?: string[]
  code?: string
  note?: string
}

export type DocPage = {
  id: string
  title: string
  summary: string
  sections: DocSection[]
}

export const docsPages: DocPage[] = [
  {
    id: 'overview',
    title: 'Overview',
    summary: 'What Browsey is, where it runs, and where the project stands today.',
    sections: [
      {
        id: 'about',
        title: 'What Browsey Is',
        body: 'Browsey is a cross-platform file explorer built with a Rust/Tauri backend and a Svelte/TypeScript frontend. Rust handles filesystem-heavy operations (listing, search, metadata, archive tasks), while the UI layer focuses on rendering, keyboard/mouse interactions, and responsive updates.',
      },
      {
        id: 'ai-assisted-development',
        title: 'AI-Assisted Development',
        body: 'Browsey is developed with AI assistance from OpenAI Codex as part of day-to-day implementation and iteration work.',
      },
      {
        id: 'scope',
        title: 'Product Scope',
        bullets: [
          'Daily file navigation and management in list and grid layouts',
          'Clipboard operations with conflict handling and preview',
          'Search, archive extract/compress, duplicate checks, and metadata editing',
          'Mounts/bookmarks/stars/recent/trash workflows in one shell',
        ],
      },
      {
        id: 'status',
        title: 'Current Status',
        body: 'Browsey is in active development. Core workflows are implemented and stable for daily use, while rapid iteration continues and behavior can still evolve quickly between versions.',
        bullets: [
          'Search and duplicate scanning are streamed and cancellable',
          'Permissions editing works on Unix and Windows',
          'Archive and thumbnail pipelines are broad but still being refined',
        ],
      },
      {
        id: 'platforms',
        title: 'Supported Platforms',
        bullets: [
          'Linux: Tauri webview via WebKit',
          'Primary Linux test environment: Fedora 43',
          'Windows: Tauri webview via WebView2',
          'macOS: not supported yet',
        ],
      },
      {
        id: 'docs-site',
        title: 'Documentation Site',
        body: 'This docs site is a standalone Svelte project in docs/, deployed with GitHub Pages.',
        bullets: [
          'Production URL: https://chl84.github.io/Browsey/',
          'GitHub Pages path uses /Browsey/ (case-sensitive)',
          'Local dev uses Vite and defaults to port 4173 for preview',
        ],
      },
    ],
  },
  {
    id: 'getting-started',
    title: 'Getting Started',
    summary: 'Prerequisites, local run instructions, and build commands for Linux/Windows.',
    sections: [
      {
        id: 'requirements-common',
        title: 'Requirements (Common)',
        bullets: [
          'Rust stable toolchain via rustup',
          'Node.js LTS and npm (frontend/dev/docs)',
          'Bundled PDFium in resources/pdfium-* (no extra system PDF libs needed)',
          'Optional ffmpeg in PATH (or FFMPEG_BIN) for video thumbnails',
          'Linux (GNOME Wayland): install xclip for file clipboard interoperability between Browsey instances without GNOME shell focus/dock side-effects on Ctrl+C/Ctrl+V',
        ],
      },
      {
        id: 'requirements-linux',
        title: 'Linux Requirements (Fedora package names)',
        code: `sudo dnf install webkit2gtk4.1-devel javascriptcoregtk4.1-devel libsoup3-devel gtk3-devel\n# for release packaging and system integration\nsudo dnf install libappindicator-gtk3 librsvg2-devel patchelf rpm-build`,
        note: 'Use equivalent package names on your distro.',
      },
      {
        id: 'requirements-windows',
        title: 'Windows Requirements',
        bullets: [
          'WebView2 Runtime (built-in on Windows 11, otherwise install)',
          'Visual Studio Build Tools (C++ workload) or full Visual Studio',
          'Rust via rustup and Node.js LTS',
        ],
      },
      {
        id: 'run-dev',
        title: 'Run From Source (Development)',
        code: `npm --prefix frontend install\ncargo tauri dev --no-dev-server`,
        note: 'Wrappers: scripts/dev/dev-server.sh (Unix) and scripts/dev/dev-server.bat (Windows).',
      },
      {
        id: 'checks',
        title: 'Quick Verification',
        code: `cargo check\nnpm --prefix frontend run check`,
      },
      {
        id: 'build-artifacts',
        title: 'Build and Bundle Commands',
        bullets: [
          'Frontend build: npm --prefix frontend run build',
          'Rust release binary: cargo build --release',
          'Windows NSIS: cargo tauri build --bundles nsis',
          'Linux RPM: cargo tauri build --bundles rpm',
        ],
        note: 'Helpers: scripts/build/build-release.sh and scripts/build/build-release.bat.',
      },
      {
        id: 'install-packages',
        title: 'Installable Artifacts',
        bullets: [
          'Linux RPM: Browsey-<version>-1.x86_64.rpm from Releases',
          'Windows NSIS installer from Releases',
          'Binaries and bundles are emitted under target/release/',
        ],
      },
    ],
  },
  {
    id: 'user-workflows',
    title: 'User Workflows',
    summary: 'Task-oriented guidance for common operations in Browsey.',
    sections: [
      {
        id: 'navigation-selection',
        title: 'Navigate and Select',
        bullets: [
          'Use list or grid mode depending on file density and preview needs',
          'Ctrl+A selects all; blank-space click clears selection',
          'Lasso selection works in both list and grid',
          'In grid, arrow keys + Shift handle range selection',
        ],
      },
      {
        id: 'search-flow',
        title: 'Search Within Current Scope',
        body: 'Search is recursive and case-insensitive under the active directory subtree. Results stream progressively and symlinks are skipped to avoid loops.',
        bullets: [
          'Press Ctrl+F to toggle search mode',
          'Press Esc once to exit search/filter input and return directly to breadcrumb view',
          'Editing the search text does not auto-run a new backend search; press Enter to refresh results',
          'Empty query returns no search results but keeps listing state',
          'Search results stream in arrival order; sort can be applied manually from the list header',
          'AQS-lite query syntax is supported in backend search (the UI does not parse the syntax locally)',
          'In search mode, the frontend no longer applies a second plain-text filter on top of backend search results',
        ],
      },
      {
        id: 'search-aqs-lite',
        title: 'Search Query Syntax (AQS Lite)',
        body: 'Browsey supports a scoped AQS-like query syntax for recursive search. Parsing and matching are performed in the backend search stream (not in the frontend). Matching is case-insensitive.',
        bullets: [
          'Plain text (for example `foo`) uses `name:` semantics and performs a case-insensitive contains match on the entry basename.',
          'Quoted text (for example `\"black\"`) is treated as an exact phrase/value token (case-insensitive exact match), so it matches `black` but not `black-1`.',
          'Field exact-value syntax uses `=`, for example `name:=file.txt` (case-insensitive exact equality for that field).',
          'Wildcards are supported in text fields: `*` matches zero or more characters, `?` matches exactly one character.',
          'Wildcard patterns match the full field value (not substring mode). Example: `filename:*.rs` matches `main.rs`; `*.` only matches names ending with a dot, while `*.*` is a practical “has extension” pattern.',
          'Boolean operators are supported: `AND`, `OR`, `NOT`, plus grouping with parentheses `(...)`.',
          'Operator precedence is `NOT` > `AND` > `OR`.',
          'Adjacent terms without an explicit operator are treated as implicit `AND` (for example `name:foo hidden:false`).',
          'Supported fields: `name:`, `filename:`, `folder:`, `path:`, `hidden:`, `readonly:`.',
          '`name:` matches the entry basename (`dir`, `file`, and `link`).',
          '`filename:` matches the basename for files and links only (directories do not match `filename:` queries).',
          '`folder:` matches the parent folder path of an entry (where the entry is located), not the entry name itself.',
          '`path:` matches the full entry path.',
          '`hidden:` and `readonly:` accept only `true` or `false` in this syntax scope.',
          'Examples: `folder:Projects AND filename:*.rs`, `(name:foo OR name:bar) AND NOT hidden:true`, `path:/path/to/workspace AND readonly:false`.',
          'Invalid syntax returns a search query error (for example unmatched parentheses or invalid boolean values), which is different from “0 results”.',
          'This is a scoped AQS-lite implementation, not full Windows AQS. Size/date operators (for example `size:` or `modified:`) and `is:` aliases are not supported in this phase.',
        ],
      },
      {
        id: 'column-filters',
        title: 'Column Filters',
        body:
          'Each sortable column has a filter icon that opens a checkbox list. Filters combine with the text filter and search results, and apply before sorting and pagination.',
        bullets: [
          'Name: alphabet buckets (A–F, G–L, M–R, S–Z, 0–9, Other)',
          'Type: values come from current view (extension/kind) or backend column set, sorted alphabetically',
          'Modified: time-bucketed groups (Today, Yesterday, N days/weeks/months/years ago), newest first',
          'Size: file-only buckets (0–10 KB … Over 1 TB), ascending by size band',
          'Filters respect hidden-file setting and never cross symlinks from the backend source',
          'Right-click a filter icon for a Reset action; filter icons turn red when any option is active',
        ],
      },
      {
        id: 'clipboard-flow',
        title: 'Copy/Cut/Paste and Conflict Handling',
        bullets: [
          'Ctrl+C/Ctrl+X/Ctrl+V map to copy/cut/paste actions',
          'Paste into same directory auto-renames duplicates without prompt',
          'Cross-directory conflicts provide overwrite vs auto-rename choices',
          'Long transfers run in background with progress and cancel support',
        ],
      },
      {
        id: 'duplicate-flow',
        title: 'Check for Duplicates Tool',
        body: 'Select exactly one file, then open context menu -> Tools -> Check for Duplicates.',
        bullets: [
          'Choose a start folder for the scan',
          'Traversal ignores symlinks',
          'Backend compares by file length first, then byte-by-byte for matching lengths',
          'Collection enforces safety limits (max scanned files and max candidate files) and returns an explicit error if exceeded',
          'Progress is streamed and scan cancels cleanly when modal closes (including Esc)',
          'Result preview shows the first 3 matches, then a summary line for remaining matches',
          'Use the copy button to copy the full duplicate list to clipboard',
        ],
      },
      {
        id: 'archives-flow',
        title: 'Archive and Extraction Flow',
        bullets: [
          'Supports Zip, Tar variants, GZ/BZ2/XZ/Zstd, 7z, and stored RAR entries',
          'Extraction is guarded by limits: max 2,000,000 archive entries and a byte cap based on min(100 GB, available disk space minus 1 GiB reserve)',
          'Runtime write flow re-checks destination free space periodically and aborts cleanly if reserve would be violated',
          'Batch extraction supports shared progress, cancel, and undo semantics',
          'Unsupported RAR compression methods fail fast to avoid bad output',
        ],
      },
      {
        id: 'delete-trash-flow',
        title: 'Delete and Trash Behavior',
        bullets: [
          'Delete sends to wastebasket when supported',
          'Shift+Delete performs permanent delete',
          'On Windows, network locations use permanent-delete flow (recycle bin is unavailable there)',
        ],
      },
      {
        id: 'network-flow',
        title: 'Network View and Server Addresses',
        bullets: [
          'Network view merges mounted endpoints with discovered LAN devices/resources',
          'Address bar accepts mountable server URIs (for example `sftp://`, `smb://`, `nfs://`, `ftp://`, `dav://`, `davs://`, `afp://`, `onedrive://`)',
          'URI aliases are normalized for compatibility (`ssh://` as SFTP, `webdav://`/`webdavs://` as DAV/DAVS, and `ftps://` in the FTP family)',
          'Context menus adapt to entry type: URI entries show Connect/Open in Browser + Copy Server Address, mounted entries show Open/Disconnect + Copy Mount Path',
          'Connection activity states are explicit: Connecting, Already connected, Connected, Failed',
        ],
      },
      {
        id: 'open-with-flow',
        title: 'Open With and External Apps',
        bullets: [
          'Open with lists associated applications and includes an explicit "Open normally" system default option',
          'Entries are filtered in the modal by app name, comment, and command',
          'System default handler can be selected explicitly',
          'Launches are detached to avoid terminal noise',
        ],
      },
      {
        id: 'properties-flow',
        title: 'Properties and Extra Metadata',
        bullets: [
          'Ctrl+P opens Properties for the current selection',
          'Permissions tab supports editing owner/group directly (platform support and permissions permitting)',
          'Owner/group selectors use searchable dropdowns populated from discovered system users and groups',
          'On Linux, ownership updates can trigger privilege escalation via system auth prompt when required',
          'Multi-select permissions are aggregated across the full selection',
          'Extra tab fetches metadata only when the tab is activated (single selection)',
          'Extra tab shows type-specific metadata only and avoids duplicating Basic-tab fields',
        ],
      },
      {
        id: 'console-flow',
        title: 'Open Console in Current Folder',
        bullets: [
          'Ctrl+T opens a terminal rooted at the current directory view',
          'On Linux, Browsey probes a strict allowlist of terminal emulators: ptyxis, gnome-terminal, konsole, xfce4-terminal, tilix, alacritty, kitty, wezterm, foot, kgx',
          'Terminal launch does not accept environment-provided command strings',
          'The action is available only when the current view is a real directory',
        ],
      },
      {
        id: 'undo-redo-flow',
        title: 'Undo and Redo Scope',
        bullets: [
          'Ctrl+Z and Ctrl+Y execute backend undo/redo actions',
          'Operations are tracked in-memory with capped history (current max: 50 actions)',
          'Permissions and ownership changes are intentionally excluded from undo/redo history',
          'File-operation backups are written under the app undo directory for restoration',
        ],
      },
    ],
  },
  {
    id: 'features',
    title: 'Features',
    summary: 'Detailed capability map and implementation-level behavior.',
    sections: [
      {
        id: 'rendering-virtualization',
        title: 'Rendering and Performance',
        bullets: [
          'Virtualized list and grid rendering for large directories',
          'Wheel/scroll handling optimized to reduce jank',
          'Metadata updates are batched to keep UI responsive during churn',
          'Thumbnails are lazy-loaded with queueing and cancellation',
        ],
      },
      {
        id: 'watcher-live-updates',
        title: 'Watcher and Live Refresh',
        body: 'Browsey uses filesystem watcher events and emits refresh signals to keep views in sync with external changes.',
        bullets: [
          'The backend watcher is non-recursive for the active directory',
          'Debounced refresh avoids thrashing on noisy event bursts',
          'UI listens for dir-changed events and refreshes active views',
        ],
      },
      {
        id: 'search-internals',
        title: 'Search Streaming Internals',
        bullets: [
          'Search runs in a blocking backend task to avoid UI stalls',
          'Matches are streamed in batches (current batch size: 256 entries)',
          'Cancellation is keyed by task id/event id and propagated via cancel_task',
          'Symlinks are excluded from traversal',
        ],
      },
      {
        id: 'duplicate-internals',
        title: 'Duplicate Scan Internals',
        bullets: [
          'Phase 1 collects same-size candidates while counting scanned files',
          'Directory walking is streamed from read_dir iterators (no full per-directory entry buffering)',
          'Collector enforces hard limits (2,000,000 scanned files and 100,000 candidates) and fails with explicit limit errors',
          'Phase 2 compares candidate bytes with early mismatch exit',
          'Progress uses phase weighting: collecting to 40%, comparing from 40% to 100%',
          'Final duplicate output is sorted deterministically by path string',
        ],
      },
      {
        id: 'thumbnails',
        title: 'Thumbnail Pipeline',
        bullets: [
          'Image decode via image crate (including HDR and OpenEXR support)',
          'SVG rasterization via resvg',
          'PDF rendering via bundled PDFium',
          'Video first-frame extraction via ffmpeg when available',
          'HDR/EXR image decode uses a longer timeout budget than standard image formats',
          'Fallback icons are used when decode fails or access is denied',
        ],
      },
      {
        id: 'properties-metadata',
        title: 'Properties Metadata Pipeline',
        bullets: [
          'Properties Basic tab loads lightweight metadata first; heavy metadata stays deferred',
          'Extra metadata is loaded lazily when the Extra tab is opened',
          'Backend providers are type-specific: image, pdf, audio, video, archive',
          'Image metadata routing covers png/jpg/jpeg/gif/bmp/webp/svg/tiff/tif/tga/ico/hdr/exr (and avif when decoder support is available)',
          'Extra tab UI intentionally omits redundant kind/section labels and shows fields directly',
        ],
      },
      {
        id: 'mounts-drives',
        title: 'Mounts, Removable Drives, and Cloud Volumes',
        bullets: [
          'Mount list includes local partitions and removable devices',
          'Windows supports eject with native APIs and fallback paths',
          'Linux eject uses gio/umount/udisksctl with safe fallback behavior',
          'GVFS mounts (including MTP/OneDrive-style integrations) are surfaced when available',
          'Generic GVFS root mounts are hidden from Partitions while concrete GVFS endpoints stay visible',
          'Network discovery combines GVFS, Avahi/mDNS, and SSDP sources for broader endpoint coverage',
          'Mount scanning cadence is user-configurable (default 8000 ms)',
          'Active GVFS locations get periodic refresh (5s) in addition to watcher-driven updates',
        ],
      },
      {
        id: 'safety-rules',
        title: 'Safety Rules and Guardrails',
        bullets: [
          'Symlinks are ignored in search and duplicate traversal',
          'Clipboard copy/cut rejects symlink entries during transfer operations',
          'Clipboard fallback copy uses no-clobber file creation and deterministic rename candidate retries',
          'Critical Linux rename/delete/trash staging paths use descriptor-based no-follow primitives to reduce symlink and check-then-use race exposure',
          'When atomic no-replace rename is unavailable, move/rename falls back to non-overwrite copy+delete with narrower non-atomic guarantees',
          'Permission edits roll back on failure in multi-target updates',
          'Archive extraction enforces global caps (100 GB absolute output, 2,000,000 entries) plus disk-aware reserve checks',
          'Linux extraction write paths use descriptor-based no-follow primitives for directory traversal and file creation',
          'Linux console launch uses a fixed terminal allowlist with fixed argument shapes',
          'Clipboard helper binaries and ffprobe are resolved to canonical executable paths before spawn',
          'Undo cleanup validates path boundaries before deletion actions',
        ],
      },
    ],
  },
  {
    id: 'settings-shortcuts',
    title: 'Settings and Shortcuts',
    summary: 'Persisted defaults, editable keyboard map, interaction tuning, and data-maintenance actions.',
    sections: [
      {
        id: 'persisted-settings',
        title: 'Persisted Settings',
        bullets: [
          'Show hidden, hidden-last, folders-first',
          'Start directory and default view (list/grid)',
          'Sort field and sort direction',
          'Delete confirmation preference',
          'Density profile (cozy/compact)',
          'Archive defaults (name, compression level, open-destination-after-extract)',
          'Thumbnail settings (video thumbnails enabled flag, ffmpeg path override, cache size)',
          'Mount list polling interval',
          'Double-click speed for mouse-based opening in list/grid',
          'Hardware acceleration toggle',
          'Shortcut keymap bindings',
        ],
      },
      {
        id: 'default-settings',
        title: 'Current Default Values',
        bullets: [
          'Default view: list',
          'Density: cozy',
          'Show hidden: enabled',
          'Hidden files last: disabled',
          'Folders first: enabled',
          'Confirm permanent delete: enabled',
          'Archive name: Archive (.zip)',
          'Archive level: 6',
          'Open destination after extract: disabled',
          'Video thumbnails: enabled',
          'FFmpeg path override: empty (auto-detect)',
          'Thumbnail cache: 300 MB',
          'Mount polling: 8000 ms',
          'Double-click speed: 300 ms',
          'Hardware acceleration: enabled',
        ],
      },
      {
        id: 'settings-ranges',
        title: 'Validated Setting Ranges',
        bullets: [
          'Archive compression level: 0-9',
          'Thumbnail cache size: 50-1000 MB',
          'Mount polling interval: 500-10000 ms',
          'Double-click speed: 150-600 ms',
        ],
      },
      {
        id: 'settings-data-actions',
        title: 'Data Maintenance Actions',
        bullets: [
          'Settings > Data provides clear actions for thumbnail cache, stars, bookmarks, and recents',
          'Each clear action requires confirmation and shows per-action toast feedback',
          'Clearing thumbnail cache removes cached files on disk and refreshes visible thumbnails in the UI',
          'Clearing stars/bookmarks/recents applies globally and updates relevant views/state immediately',
        ],
      },
      {
        id: 'startup-behavior',
        title: 'Startup Behavior',
        body: 'Saved defaults load before first listing so startup respects your preferred view and sorting mode immediately.',
      },
      {
        id: 'hardware-accel',
        title: 'Hardware Acceleration on Linux',
        body: 'When hardware acceleration is disabled in Browsey settings, Browsey keeps compositing enabled and sets only WEBKIT_DISABLE_DMABUF_RENDERER=1.',
        note: 'Legacy software-rendering flags were removed in v0.4.1. Restart is required for this setting to take effect.',
      },
      {
        id: 'shortcut-map',
        title: 'Default Keyboard Shortcuts (Remappable)',
        bullets: [
          'Any shortcut can be remapped from Settings by clicking its binding and pressing a new key combo',
          'Shortcut updates are persisted and validated; duplicate/conflicting bindings are rejected',
          'Ctrl+F: toggle search mode',
          'Ctrl+G: toggle list/grid view',
          'Ctrl+C / Ctrl+X / Ctrl+V: copy/cut/paste',
          'Ctrl+A: select all',
          'Ctrl+Z / Ctrl+Y: undo / redo',
          'Ctrl+S: open settings modal',
          'F2: rename',
          'Delete / Shift+Delete: trash / permanent delete',
          'Ctrl+P: properties',
          'Ctrl+H: toggle hidden files',
          'Ctrl+B: bookmark modal for single selected folder',
          'Ctrl+T: open terminal in current directory',
        ],
      },
      {
        id: 'input-focus-behavior',
        title: 'Input and Focus Behavior',
        bullets: [
          'Typing without focus enters address-bar filter mode',
          'Esc exits search/filter input and lands directly in breadcrumb view (address mode, input unfocused)',
          'In address mode, Esc while editing the path restores the current valid location path before returning to breadcrumbs',
          'In filter mode, Enter is intentionally ignored (no path submit)',
          'Browser-level Ctrl hotkeys outside the configured shortcut map are blocked (Ctrl+Shift+I is allowed)',
          'Text inputs keep native editing shortcuts',
        ],
      },
    ],
  },
  {
    id: 'architecture',
    title: 'Architecture',
    summary: 'Technical layout of backend, frontend, persistence, and resources.',
    sections: [
      {
        id: 'backend',
        title: 'Backend (Rust / Tauri Commands)',
        bullets: [
          'src/main.rs wires app startup, command registration, event handlers, and shared runtime state',
          'src/commands/ is capability-split into focused domains (for example listing/fs/search/permissions/rename/network/open_with/system_clipboard/settings/library)',
          'Filesystem command internals are decomposed under src/commands/fs/ (delete/trash/open/error/platform helpers) to keep critical paths isolated and testable',
          'Long-running pipelines are isolated in submodules (src/commands/duplicates/, src/commands/decompress/, src/commands/thumbnails/) with progress + cancellation support',
          'src/metadata/providers/ contains type-specific extra-metadata providers (image, pdf, audio, video, archive, shared media_probe)',
          'src/keymap/ centralizes accelerator parsing/canonicalization and conflict validation for remappable shortcuts',
          'Shared subsystems live outside commands as dedicated modules (for example src/clipboard/, src/undo/, src/db/, src/fs_utils/, src/errors/, src/path_guard/, src/tasks/, src/context_menu/)',
          'Platform-specific behavior is isolated with cfg gates (for example Windows delete/eject details and Linux launcher/mount behavior)',
        ],
      },
      {
        id: 'frontend',
        title: 'Frontend (Svelte)',
        bullets: [
          'Explorer UI composition now lives in frontend/src/features/explorer/pages/ExplorerPage.svelte, while App.svelte is a thin entry wrapper',
          'Explorer internals are organized by domain (`context`, `navigation`, `file-ops`, `selection`, `ui-shell`, `state`) to keep ownership boundaries explicit',
          'frontend/src/features/explorer/ui-shell/ owns shell composition (`ExplorerShell`, `Sidebar`, `Topbar`) and view observer/virtualization hooks',
          'frontend/src/features/explorer/hooks/ and domain modules handle orchestration for navigation, search session, file ops, context menus, and input handlers',
          'frontend/src/features/explorer/services/ is the invoke boundary: UI code calls service wrappers, not invoke directly',
          'frontend/src/features/explorer/state/ contains internal slices/stores while preserving a stable `createExplorerState` API in state.ts',
          'frontend/src/features/settings/ splits `SettingsModal` into section components plus a dedicated view-model hook',
          'frontend/src/features/shortcuts/ provides shortcut mapping metadata and frontend bridge logic',
          'Cross-feature imports are enforced through feature barrels (`@/features/<feature>`) with ESLint import-boundary rules',
          'Naming convention drift is linted via `npm --prefix frontend run lint` (ESLint + naming checks)',
          'Reusable UI primitives live in frontend/src/shared/ui/ (ModalShell, ConfirmActionModal, Toast, context menus, overlays)',
          'Global styling/theme density variables are centralized in frontend/src/app.css',
        ],
      },
      {
        id: 'persistence',
        title: 'Persistence and Cache',
        bullets: [
          'SQLite stores bookmarks, stars, recents, column widths, and shortcut keymap overrides',
          'Thumbnail cache lives in user cache directory and is trimmed periodically',
          'Application logs are written under the user data directory at browsey/logs/browsey.log',
          'Log file rotation keeps a secondary browsey.log.1 when the main log reaches size limit',
          'Undo backups are kept under user data directory (browsey/undo) and stale backups are cleaned at startup',
          'Capabilities configuration grants event listen/emit for backend-to-frontend signaling',
        ],
      },
      {
        id: 'repo-layout',
        title: 'Repository Layout',
        bullets: [
          'README stays intentionally high-level; this docs section is the detailed source of truth for structure',
          'Top-level split: src/ (Rust backend), frontend/ (Svelte app), docs/ (documentation app), scripts/ (helpers), resources/ (assets)',
          'capabilities/default.json defines Tauri permission capabilities used by app event/listen flows',
        ],
        code: `src/
  main.rs
  errors/{mod.rs,api_error.rs,domain.rs}
  commands/
    mod.rs
    about.rs bookmarks.rs console.rs keymap.rs library.rs
    listing/ search/ rename/ permissions/ network/
    fs/{mod.rs,delete_ops.rs,open_ops.rs,error.rs,windows.rs,trash/*}
    duplicates/{mod.rs,scan.rs}
    decompress/{mod.rs,zip_format.rs,tar_format.rs,seven_z_format.rs,rar_format.rs,util.rs}
    entry_metadata/ file_types/ compress/ settings/ open_with/ system_clipboard/ thumbnails/
  clipboard/{mod.rs,ops.rs,drop_mode.rs,error.rs}
  undo/{mod.rs,error.rs,backup.rs,engine.rs,path_ops.rs,path_checks.rs,security.rs,nofollow.rs,types.rs}
  db/{mod.rs,error.rs}
  fs_utils/{mod.rs,error.rs}
  path_guard/{mod.rs,error.rs}
  tasks/{mod.rs,error.rs}
  context_menu/{mod.rs,action.rs}
  metadata/providers/{image.rs,pdf.rs,audio.rs,video.rs,archive.rs,media_probe.rs}
  keymap/{mod.rs,model.rs,accelerator.rs,error.rs}
  watcher.rs runtime_lifecycle.rs

frontend/src/
  App.svelte app.css main.ts
  shared/{index.ts,lib/{tauri.ts,error.ts},ui/{ModalShell.svelte,ConfirmActionModal.svelte,Toast.svelte,...}}
  features/
    explorer/{pages,ui-shell,components,hooks,navigation,context,file-ops,selection,state,services,modals,filters,helpers,model}
    settings/{SettingsModal.svelte,hooks,sections,settingsTypes.ts,index.ts}
    network/{index.ts,uri.ts,services.ts,contextMenu.ts,clipboard.ts}
    shortcuts/{index.ts,keymap.ts,service.ts}

docs/src/content/pages.ts
ARCHITECTURE_IMPORTS.md ARCHITECTURE_NAMING.md CHANGELOG.md
docs/todo-archive/{README.md,TODO_*.md}
scripts/{dev/*,build/*,docs/*,install/*,maintenance/*}
resources/{icons/,schemas/,pdfium-linux-x64/,pdfium-win-x64/}
capabilities/default.json`,
        note: 'When in doubt, add user-facing behavior notes in docs first, then keep README concise with links/summaries.',
      },
    ],
  },
  {
    id: 'development',
    title: 'Development and Docs Workflow',
    summary: 'Day-to-day contributor commands for app and docs work.',
    sections: [
      {
        id: 'app-dev-loop',
        title: 'App Development Loop',
        code: `npm --prefix frontend install\ncargo tauri dev --no-dev-server`,
        note: 'Use scripts/dev/dev-server.sh or scripts/dev/dev-server.bat for convenience.',
      },
      {
        id: 'app-validation',
        title: 'App Validation Loop',
        code: `cargo check\nnpm --prefix frontend run check`,
      },
      {
        id: 'docs-dev-loop',
        title: 'Docs Development Loop',
        code: `npm --prefix docs install\n./scripts/docs/docs-dev.sh\n./scripts/docs/docs-check.sh\n./scripts/docs/docs-build.sh`,
        note: 'Also available: scripts/docs/docs-install.sh + scripts/docs/docs-preview.sh (and .bat equivalents on Windows).',
      },
      {
        id: 'docs-pages-deploy',
        title: 'Docs Deployment (GitHub Pages)',
        bullets: [
          'Repository Pages source should be set to GitHub Actions',
          'Docs workflow builds with PAGES_BASE_PATH=/Browsey/',
          'Published docs resolve at https://chl84.github.io/Browsey/',
        ],
      },
      {
        id: 'contributor-guidance',
        title: 'Contributor Guidance',
        bullets: [
          'Keep README concise and high-level; put deep technical details in docs pages',
          'Prefer updating docs content in docs/src/content/pages.ts for user-facing docs pages',
          'Keep docs statements aligned with README and changelog facts',
          'When behavior changes, update docs and release notes in the same PR',
          'Use English for project documentation and inline technical comments',
        ],
      },
    ],
  },
  {
    id: 'release-notes',
    title: 'Release Notes',
    summary: 'Curated highlights from recent versions.',
    sections: [
      {
        id: 'unreleased',
        title: 'Unreleased',
        bullets: [
          'Frontend explorer modules were reorganized into explicit domains (`context`, `navigation`, `file-ops`, `selection`, `ui-shell`, `state`) with no intended behavior change',
          'Explorer factory naming was standardized (`use*.ts` -> `create*.ts`) for factory modules to reduce naming ambiguity',
          'Settings internals were modularized: `SettingsModal` is now split into section components plus a dedicated view-model hook',
          'Frontend import boundaries are now enforced through feature barrels and ESLint deep-import restrictions',
          'Naming conventions now have an automated lint check (`frontend/scripts/check-naming-conventions.mjs`) wired into `npm --prefix frontend run lint`',
          'Architecture docs were expanded with explicit naming/import convention references (`ARCHITECTURE_NAMING.md`, `ARCHITECTURE_IMPORTS.md`)',
          'Completed TODO execution plans were moved into `docs/todo-archive/`, and project text/comments were normalized to English',
          'Backend error flow migration was expanded across remaining modules, replacing string-based failures with code-based ApiError mapping',
          'Core error modules were added in fs_utils, metadata, statusbar, and undo to standardize domain-level classification',
          'Undo internals were fully migrated to typed errors and split into focused modules (backup/engine/nofollow/path checks/path ops/security/types/error)',
          'Frontend now uses a shared Tauri invoke wrapper plus a central error normalizer, fixing structured-error toasts that previously showed [object Object]',
          'Drag/drop logic was extracted from App.svelte into dedicated explorer hooks to reduce coupling and dead code risk',
          'Drop copy-vs-move policy resolution now lives in backend-facing flows instead of frontend heuristics',
          'URI/network rule handling was further centralized in backend modules, reducing duplicated frontend scheme logic',
          'Extract availability now comes from backend capability checks instead of frontend extension-only heuristics',
          'Backend source layout was tightened with additional modular splits across commands and shared modules',
          'Explorer wheel scrolling was simplified and stabilized with a single always-on wheel assist path, centralized tuning, and deterministic handling for non-cancelable wheel bursts',
          'Extreme wheel-input behavior was tuned by lowering max per-event step, increasing list/grid virtualization overscan, and snapping scroll targets to integer pixels to reduce transient flicker/half-tone rendering artifacts',
          'Sorting behavior and performance were refined: Size sorting now keeps files before links before directories in both directions, directories sort by item count in the Size column, the unused Starred sort path was removed, backend sort keys are cached more aggressively, and frontend in-memory search sorting now uses cached/decorated keys for large result sets',
          'Explorer state internals were further decomposed into dedicated state modules (searchSort, entryMutations, createSortRefreshDispatcher, searchRuntimeHelpers, createSearchSession) while preserving the public createExplorerState API, and a state/README now documents boundaries and naming expectations',
          'Modal shell behavior was hardened: immediate Esc close now works before modal content focus lands, Esc close handlers no longer double-fire via bubbling, and focus is restored to a sensible prior element when modals close',
          'Modal keyboard defaults were improved: duplicate-check search can start with Enter from the search-root field, confirm/conflict/delete dialogs now support immediate Enter actions via explicit default focus policies, and duplicate in-input Esc handlers were removed where ModalShell already handles closing',
          'Conflict handling correctness was fixed for drag-move auto-rename so choosing Auto-rename on move conflicts no longer overwrites existing targets and instead preserves rename-candidate retry behavior',
          'Properties modal UX and stability were improved: ownership moved to a dedicated tab, permission toggles are disabled during async apply without flashing the permissions pane, layout was tightened/resized/responsive-tuned, and owner/group dropdowns can overflow modal bounds when needed',
          'A custom shared slider UI component (square thumb + square track) replaced native range styling inconsistencies across settings and compression controls',
          'Advanced Rename preview updates no longer visibly flicker/repaint the modal while typing; preview now updates in place without swapping the preview pane content',
          'Search now supports a scoped backend AQS-lite syntax (wildcards, boolean logic, grouping, quoted exact phrases, exact-value field matches, and `name`/`filename`/`folder`/`path`/`hidden`/`readonly` filters); search-mode frontend filtering no longer re-filters these results as plain text',
        ],
      },
      {
        id: 'v044',
        title: 'v0.4.4 (2026-02-17)',
        bullets: [
          'Archive extraction write paths on Linux now use descriptor-based no-follow directory/file primitives to reduce symlink and path-race exposure',
          'Extraction byte limits are now disk-aware (min(100 GB, available space minus 1 GiB reserve)) with periodic runtime free-space checks',
          'Clipboard fallback copy now uses no-clobber file creation and deterministic rename candidate retries (no pre-exists probing)',
          'Destructive move/rename no longer uses a check-then-rename Linux compatibility path when renameat2(RENAME_NOREPLACE) is unavailable; controlled copy+delete fallback is used instead',
          'Windows/portable destructive delete fallback now validates no-follow metadata recursively instead of calling raw remove_dir_all',
          'Duplicate candidate collection now streams directory iteration and enforces scanned/candidate caps with explicit limit errors',
          'Properties ownership editing now uses searchable User/Group dropdowns populated from discovered system principals',
          'Wastebasket list mode now maps icons from original item metadata so file-type-specific icons are preserved',
          'Esc now exits both search and filter directly to breadcrumb view',
          'Address-mode Esc now restores the current valid path when input text has been edited',
          'Filter-mode Enter is now a no-op and no longer triggers path navigation',
          'Large-selection copy/cut flows were optimized to avoid quadratic selection scans',
          'Large-selection context-menu and delete path resolution now use Set/Map lookups',
          'Shutdown handling around clipboard/file operations was hardened to reduce late-stage work during exit',
          'Input-mode transitions were centralized for more consistent address/filter/search-session state handling',
          'Search state semantics were tightened so searchRunning now reflects active backend search execution',
          'Wastebasket deletion now resolves entries by stable trash ID to reduce redundant .trashinfo scanning',
          'Unix trash/undo rename-delete paths now use descriptor-based no-follow primitives to reduce symlink and check-then-use race exposure',
          'No-overwrite rename fallback semantics now use explicit non-overwrite copy+delete paths when atomic rename no-replace is unavailable',
          'Staged trash renames are now journaled and recovered on startup after interrupted operations',
          'Windows trash moves no longer use staged names, so restore keeps the original path and filename',
          'Trash internals were refactored behind a backend abstraction and hardened with rollback/fallback/cleanup unit tests',
          'Linux Open With now resolves selected app IDs only from canonical in-scope .desktop files',
          'Properties ownership editing now supports Linux privilege escalation via pkexec helper fallback',
          'Permissions/ownership changes are now intentionally excluded from undo/redo history',
          'Permissions/ownership rollback paths were decoupled from undo action types and hardened with partial-rollback failure tests',
          'Properties permissions UI is now density-aware (cozy/compact), including a smaller Apply ownership button',
          'Long error messages now wrap consistently in modal error pills, ownership errors, and notice banners',
          'Expected ownership validation/auth cancellation failures no longer spam frontend dev warnings',
          'App log timestamps now use local time with timezone offset instead of UTC Z formatting',
          'Network layer was split into dedicated backend/frontend modules (`src/commands/network/*`, `frontend/src/features/network/*`) and wired into the Network view lifecycle',
          'Network discovery now aggregates GVFS (`gio mount -li`), Avahi/mDNS, and SSDP sources to surface broader SFTP/SMB/NFS/FTP/WebDAV/AFP/HTTP/HTTPS endpoints',
          'Address bar + URI handling now supports broader server-address aliases (`ssh`→`sftp`, `webdav`/`webdavs`→`dav`/`davs`, `ftps` accepted as FTP-family alias for normalization/matching)',
          'Mount UX now reports explicit outcomes (Connecting, Already connected, Connected, Failed) from backend to frontend activity labels',
          'GVFS mount visibility checks were hardened with retries and stricter mounted-URI validation; OneDrive mountables no longer false-positive as already connected',
          'Linux partitions now hide the generic GVFS root mount while still surfacing concrete GVFS endpoints (for example active OneDrive/MTP mounts)',
          'Network context menu is now URI-aware: mountable URIs show Connect/Copy Server Address, HTTP(S) URIs show Open in Browser, mounted paths keep Open/Disconnect',
          'Properties modal now supports virtual network URIs in the Extra tab by showing parsed URI fields (address/protocol/user/host/port/path/query/fragment) without failing filesystem metadata probes',
          'OneDrive entries in Network are now account-deduplicated during mount/eject transition windows to avoid temporary duplicate rows',
          'Column-filter UX was refined: facet staleness/parity issues were fixed, active filter indicators were improved for both list and grid modes, and grid now shows an explicit active-filter notice when headers are hidden',
        ],
      },
      {
        id: 'v043',
        title: 'v0.4.3 (2026-02-13)',
        bullets: [
          'Added a topbar main action menu (hamburger) with wired actions for Settings, Keyboard Shortcuts, Search, view-mode toggle (List/Grid), hidden-files toggle, Refresh, and About',
          'Added a dedicated About modal with three tabs: Version (embedded changelog), Build (runtime/build target details), and License',
          'License tab now shows both LICENSE and THIRD_PARTY_NOTICES in one combined scrollable text field',
        ],
      },
      {
        id: 'v042',
        title: 'v0.4.2 (2026-02-13)',
        bullets: [
          'Column filters now apply real filtering on top of text filter/search, with name/type/modified/size buckets, reset via right-click, and red active indicators',
          'Size/modified/type filter options are sourced from the current listing or backend column sets; hidden files are respected and size buckets skip folders',
          'Double-click speed setting is now wired to list/grid open behavior and persisted with range validation',
          'Settings > Data actions now clear thumbnail cache, stars, bookmarks, and recents with confirmation dialogs and per-action toasts',
          'Thumbnail cache clear now removes cached files on disk and refreshes visible thumbnails',
          'Extra metadata now lazy-loads when the Extra tab is activated',
          'Extra metadata backend split into type-specific providers and avoids Basic-tab duplication',
          'Extra tab UI simplified by removing the redundant kind row and section title chrome',
          'Image extra-metadata routing now includes .tif, .tga, .hdr, and .exr',
          'Bundled Linux PDFium updated to 146.0.7678.0 (library, headers, and licenses refreshed)',
          'Linux open-console launch now uses a strict terminal allowlist and fixed arguments (env-based terminal command overrides removed)',
          'Archive extraction safety guardrails expanded with a total output cap (100 GB) and a total-entry cap (2,000,000)',
          'RAR extraction now streams entry data in chunks instead of buffering whole entries in memory',
          'System clipboard helper binaries (wl-copy/wl-paste/xclip) and ffprobe now resolve through canonical path checks before process spawn',
        ],
      },
      {
        id: 'v041',
        title: 'v0.4.1 (2026-02-08)',
        bullets: [
          'Linux rendering fallback simplified to WEBKIT_DISABLE_DMABUF_RENDERER=1 when acceleration is disabled',
          'New Check for Duplicates tool with dedicated modal (single-file context action)',
          'Duplicate backend pipeline: size pre-filter + byte-for-byte compare with early mismatch exit',
          'Duplicate scan UX: progress bar, robust cancellation on modal close/Esc, and better result preview/copy',
          'README/docs updated for duplicate scanning and hardware-acceleration policy',
        ],
      },
      {
        id: 'v040',
        title: 'v0.4.0 (2026-02-06)',
        bullets: [
          'Search moved to blocking backend task to reduce UI freezes on large trees',
          'Archive support expanded (7z/RAR plus batch extraction with progress/cancel/undo)',
          'Thumbnails moved to PDFium-backed pipeline with performance and reliability fixes',
          'GVFS/OneDrive/MTP reliability improved for mounts and transfer flows',
          'Persistent defaults and broad UX polish across core file flows',
        ],
      },
      {
        id: 'v030',
        title: 'v0.3.0-beta1 (2026-01-25)',
        bullets: [
          'Introduced thumbnail pipeline with caching, allowlist, permissions checks, and concurrency limits',
          'Grid view lazy thumbnail loading with queue and icon fallback behavior',
          'Asset protocol scope and cache trimming hardening',
        ],
      },
      {
        id: 'v020',
        title: 'v0.2.0-beta1 (2025-01-18)',
        bullets: [
          'Custom icon set rollout across folders/files/status indicators',
          'Theme toggle redesign and layout polish in list/grid',
          'General dependency maintenance and stability fixes',
        ],
      },
      {
        id: 'v010',
        title: 'v0.1.0-beta1 (2025-01-11)',
        bullets: [
          'Initial public beta with browsing/search/bookmarks/starring/trash/compression',
          'Virtualized list/grid foundations and cross-platform Tauri 2 architecture',
        ],
      },
    ],
  },
  {
    id: 'known-limitations',
    title: 'Known Limitations',
    summary: 'Current constraints and caveats verified against the codebase.',
    sections: [
      {
        id: 'platform-coverage',
        title: 'Platform Coverage',
        bullets: [
          'Browsey currently targets Linux and Windows; macOS is not supported yet',
          'Console-launch behavior is implemented per-platform and may fail if no terminal emulator is available',
        ],
      },
      {
        id: 'open-with-limitations',
        title: 'Open With Limitations',
        bullets: [
          'Open With behavior depends on platform app registration quality and may vary by file type',
          'Custom command launching is supported, but command availability and arguments are user-managed',
        ],
      },
      {
        id: 'archive-limitations',
        title: 'Archive Limitations',
        bullets: [
          'RAR entries using unsupported compression methods are rejected (fail-fast)',
          'Symlink archive entries are skipped or rejected depending on archive format and safety rules',
          'Archives exceeding extraction safety caps are rejected (2,000,000 entries, plus byte cap = min(100 GB, available destination space minus 1 GiB reserve))',
          'Extraction reports skipped symlink and skipped unsupported-entry counts',
        ],
      },
      {
        id: 'duplicate-scan-limits',
        title: 'Duplicate Scan Limits',
        bullets: [
          'Duplicate scans enforce hard collection limits (2,000,000 scanned files and 100,000 same-size candidates)',
          'When limits are exceeded, the scan ends with an explicit limit error instead of unbounded memory growth',
        ],
      },
      {
        id: 'symlink-policy',
        title: 'Symlink Policy',
        bullets: [
          'Search and duplicate scans do not traverse symlinks',
          'Clipboard copy/cut logic refuses symlink entries',
          'Permission editing on symlinks is not supported',
        ],
      },
      {
        id: 'undo-lifecycle',
        title: 'Undo Lifecycle',
        bullets: [
          'Undo/redo history is in-memory and therefore resets when the app restarts',
          'Backup paths under browsey/undo are cleaned at startup to prevent stale leftovers',
          'When atomic no-replace rename is unavailable on a platform/filesystem, move/rename fallback is copy+delete and therefore non-atomic',
        ],
      },
      {
        id: 'windows-mount-visibility',
        title: 'Windows Mount Visibility',
        bullets: [
          'Windows mount enumeration is drive-letter based',
          'Volumes/devices without drive letters are outside the standard mount list',
        ],
      },
      {
        id: 'settings-change-scope',
        title: 'Settings Change Scope',
        bullets: [
          'Hardware acceleration changes require application restart',
          'Shortcut editing currently supports single-step accelerators (no multi-stroke chords)',
        ],
      },
    ],
  },
  {
    id: 'troubleshooting',
    title: 'Troubleshooting',
    summary: 'Practical fixes for setup, build, and docs deployment issues.',
    sections: [
      {
        id: 'linux-build-deps',
        title: 'Linux Build Fails on WebKit/Soup/GTK',
        body: 'Install distro equivalents of webkit2gtk4.1, javascriptcoregtk4.1, libsoup3, and gtk3 development packages.',
      },
      {
        id: 'setting-change-restart',
        title: 'Hardware Acceleration Change Has No Immediate Effect',
        bullets: [
          'Hardware acceleration is read during app startup',
          'After toggling this setting, restart Browsey to apply renderer policy changes',
        ],
      },
      {
        id: 'thumbnails-video',
        title: 'Video Thumbnails Missing',
        bullets: [
          'Verify ffmpeg is available in PATH (or set FFMPEG_BIN)',
          'Browsey can also use a persisted ffmpeg override path from Settings',
          'Without ffmpeg, Browsey falls back to file-type icons for videos',
        ],
      },
      {
        id: 'thumbnails-hdr-exr',
        title: 'HDR/EXR Thumbnails Slow or Missing',
        bullets: [
          'Large .hdr/.exr images are heavier to decode than standard images and may appear later',
          'Browsey uses a longer decode timeout budget for HDR/EXR, but very large files can still fail',
          'On decode failure, Browsey falls back to the file-type icon',
        ],
      },
      {
        id: 'open-console-fails',
        title: 'Open Console Fails on Linux',
        bullets: [
          'Ensure a terminal emulator is installed (Browsey probes common terminal commands)',
          'Supported launch targets are: ptyxis, gnome-terminal, konsole, xfce4-terminal, tilix, alacritty, kitty, wezterm, foot, kgx',
          'The target path must be an existing directory',
        ],
      },
      {
        id: 'gnome-wayland-clipboard',
        title: 'GNOME Wayland: Ctrl+C/Ctrl+V Triggers Dock/Top Bar',
        bullets: [
          'Install xclip and restart Browsey',
          'Browsey avoids wl-clipboard for file clipboard operations on GNOME Wayland because it can briefly trigger shell focus transitions',
          'With xclip installed, file clipboard copy/paste between Browsey instances works without dock/top-bar side-effects',
        ],
      },
      {
        id: 'logs-location',
        title: 'Finding Logs',
        bullets: [
          'Browsey writes logs under your user data directory in browsey/logs/',
          'Primary log file is browsey.log with rotation to browsey.log.1',
          'Log timestamps use local wall-clock time and include timezone offset (for example +01:00)',
        ],
      },
      {
        id: 'issue-reporting',
        title: 'Reporting Issues Effectively',
        bullets: [
          'Include OS/version, Browsey version, and reproduction steps',
          'Attach relevant logs/screenshots where possible',
          'State whether issue is deterministic or intermittent',
        ],
      },
    ],
  },
]

export const docsPageMap = new Map(docsPages.map((page) => [page.id, page]))
