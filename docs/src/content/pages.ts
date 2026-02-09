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
        body: 'Browsey is in early beta. Core workflows are implemented and production-like for many use cases, but rapid iteration is still expected and behavior can evolve quickly between versions.',
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
        note: 'Wrappers: scripts/dev-server.sh (Unix) and scripts/dev-server.bat (Windows).',
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
        note: 'Helpers: scripts/build-release.sh and scripts/build-release.bat.',
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
          'Press Esc to exit search mode',
          'Empty query returns no search results but keeps listing state',
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
          'Progress is streamed and scan cancels cleanly when modal closes (including Esc)',
        ],
      },
      {
        id: 'archives-flow',
        title: 'Archive and Extraction Flow',
        bullets: [
          'Supports Zip, Tar variants, GZ/BZ2/XZ/Zstd, 7z, and stored RAR entries',
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
          'Windows network paths delete permanently (recycle bin unavailable there)',
        ],
      },
      {
        id: 'open-with-flow',
        title: 'Open With and External Apps',
        bullets: [
          'Open with lists matching applications and allows custom command',
          'System default handler can be selected explicitly',
          'Launches are detached to avoid terminal noise',
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
          'Debounced refresh avoids thrashing on noisy event bursts',
          'Bookmarked paths are allowlisted for watching',
          'UI listens for dir-changed events and refreshes active views',
        ],
      },
      {
        id: 'thumbnails',
        title: 'Thumbnail Pipeline',
        bullets: [
          'Image decode via image crate',
          'SVG rasterization via resvg',
          'PDF rendering via bundled PDFium',
          'Video first-frame extraction via ffmpeg when available',
          'Fallback icons are used when decode fails or access is denied',
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
        ],
      },
      {
        id: 'safety-rules',
        title: 'Safety Rules and Guardrails',
        bullets: [
          'Symlinks are ignored in search and duplicate traversal',
          'Symlink copy/move is rejected on Windows paths that cannot guarantee safe semantics',
          'Permission edits roll back on failure in multi-target updates',
          'Undo cleanup validates path boundaries before deletion actions',
        ],
      },
    ],
  },
  {
    id: 'settings-shortcuts',
    title: 'Settings and Shortcuts',
    summary: 'Persisted defaults, keyboard map, and Linux hardware acceleration behavior.',
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
          'Hardware acceleration toggle',
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
        note: 'Legacy software-rendering flags were removed in v0.4.1.',
      },
      {
        id: 'shortcut-map',
        title: 'Primary Keyboard Shortcuts',
        bullets: [
          'Ctrl+F: toggle search mode',
          'Ctrl+G: toggle list/grid view',
          'Ctrl+C / Ctrl+X / Ctrl+V: copy/cut/paste',
          'Ctrl+A: select all',
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
          'Esc exits search/filter context where applicable',
          'Default browser context menu/hotkeys are blocked (except Ctrl+Shift+I)',
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
          'Command modules under src/commands for fs, search, settings, metadata, and library features',
          'Streaming commands for long-running operations (search, duplicate scan, transfers)',
          'Clipboard preview/execute path for safe conflict handling before writes',
          'Platform-specific behavior isolated behind cfg gates where needed',
        ],
      },
      {
        id: 'frontend',
        title: 'Frontend (Svelte)',
        bullets: [
          'Explorer shell and feature modules under frontend/src/features/explorer/',
          'All Tauri invoke calls wrapped in service modules',
          'Shared modal structure through frontend/src/ui/ModalShell.svelte',
          'Shared styles and density/theming variables in frontend/src/app.css',
        ],
      },
      {
        id: 'persistence',
        title: 'Persistence and Cache',
        bullets: [
          'SQLite stores bookmarks, stars, recents, and column widths',
          'Thumbnail cache lives in user cache directory and is trimmed periodically',
          'Capabilities configuration grants event listen/emit for backend-to-frontend signaling',
        ],
      },
      {
        id: 'repo-layout',
        title: 'Repository Layout',
        bullets: [
          'src/: Rust backend',
          'frontend/: Browsey application UI',
          'docs/: standalone docs application',
          'scripts/: developer and build helper scripts',
          'resources/: icons, schemas, bundled binaries (including PDFium)',
        ],
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
        note: 'Use scripts/dev-server.sh or scripts/dev-server.bat for convenience.',
      },
      {
        id: 'app-validation',
        title: 'App Validation Loop',
        code: `cargo check\nnpm --prefix frontend run check`,
      },
      {
        id: 'docs-dev-loop',
        title: 'Docs Development Loop',
        code: `npm --prefix docs install\n./scripts/docs-dev.sh\n./scripts/docs-check.sh\n./scripts/docs-build.sh`,
        note: 'Equivalent npm commands are also available directly with --prefix docs.',
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
          'Prefer updating docs content in docs/src/content/pages.ts for user-facing docs pages',
          'Keep docs statements aligned with README and changelog facts',
          'When behavior changes, update docs and release notes in the same PR',
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
    id: 'troubleshooting',
    title: 'Troubleshooting',
    summary: 'Practical fixes for setup, build, and docs deployment issues.',
    sections: [
      {
        id: 'docs-pages-404',
        title: 'Docs URL Returns 404',
        bullets: [
          'Confirm latest Docs Pages run completed successfully in GitHub Actions',
          'Ensure repository Settings -> Pages uses Source: GitHub Actions',
          'Use the exact repository path: /Browsey/ (case-sensitive)',
          'Wait briefly after successful deploy; Pages propagation can lag',
        ],
      },
      {
        id: 'actions-500',
        title: 'GitHub Actions UI Shows 500 Errors',
        body: 'GitHub occasionally has transient UI/API errors when opening workflow pages.',
        bullets: [
          'Retry from the workflow list or refresh after a short wait',
          'Trigger a fresh run from main if a run page fails to render',
          'If run status is green and Pages URL works, deployment is usually fine',
        ],
      },
      {
        id: 'docs-tools-missing',
        title: 'Missing vite or svelte-check',
        body: 'Tool-not-found errors in docs commands usually mean dependencies are not installed for docs workspace.',
        code: `npm --prefix docs install\n./scripts/docs-check.sh`,
      },
      {
        id: 'linux-build-deps',
        title: 'Linux Build Fails on WebKit/Soup/GTK',
        body: 'Install distro equivalents of webkit2gtk4.1, javascriptcoregtk4.1, libsoup3, and gtk3 development packages.',
      },
      {
        id: 'thumbnails-video',
        title: 'Video Thumbnails Missing',
        bullets: [
          'Verify ffmpeg is available in PATH (or set FFMPEG_BIN)',
          'Without ffmpeg, Browsey falls back to file-type icons for videos',
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
