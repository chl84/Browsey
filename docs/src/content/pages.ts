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
    summary: 'High-level product scope and current status.',
    sections: [
      {
        id: 'about',
        title: 'What Browsey Is',
        body: 'Browsey is a cross-platform file explorer built with a Rust/Tauri backend and a Svelte/TypeScript frontend. Rust handles filesystem-heavy operations while the UI focuses on responsive rendering and interaction.',
      },
      {
        id: 'status',
        title: 'Current Status',
        body: 'Early beta. Core file-management flows are in place and actively iterated.',
        bullets: [
          'Browse, search, clipboard, trash, compression, and extraction',
          'Duplicate checks with streaming progress and cancellation',
          'Properties and permissions editing on Unix and Windows',
        ],
      },
      {
        id: 'platforms',
        title: 'Supported Platforms',
        bullets: [
          'Linux: WebKit-based Tauri webview',
          'Windows: WebView2-based Tauri webview',
          'macOS: not supported yet',
        ],
      },
      {
        id: 'docs-site',
        title: 'Docs Site',
        body: 'Documentation is built from docs/ as a standalone Svelte project and deployed to GitHub Pages.',
        bullets: [
          'Production URL: https://chl84.github.io/Browsey/',
          'Local docs dev server: port 4173 by default',
        ],
      },
    ],
  },
  {
    id: 'getting-started',
    title: 'Getting Started',
    summary: 'Install prerequisites, run Browsey, and build artifacts.',
    sections: [
      {
        id: 'requirements',
        title: 'Requirements',
        bullets: [
          'Rust (stable) via rustup',
          'Node.js LTS + npm',
          'Linux build deps (Fedora names): webkit2gtk4.1-devel, javascriptcoregtk4.1-devel, libsoup3-devel, gtk3-devel',
          'Windows: WebView2 Runtime + Visual Studio Build Tools',
          'Optional: ffmpeg for video thumbnails',
        ],
      },
      {
        id: 'run-from-source',
        title: 'Run From Source',
        code: `npm --prefix frontend install\ncargo tauri dev --no-dev-server`,
        note: 'Convenience wrappers: scripts/dev-server.sh (Unix), scripts/dev-server.bat (Windows).',
      },
      {
        id: 'quality-checks',
        title: 'Quality Checks',
        code: `cargo check\nnpm --prefix frontend run check`,
      },
      {
        id: 'build',
        title: 'Build and Bundles',
        bullets: [
          'Frontend build: npm --prefix frontend run build',
          'Rust release binary: cargo build --release',
          'Windows NSIS bundle: cargo tauri build --bundles nsis',
          'Linux RPM bundle: cargo tauri build --bundles rpm',
        ],
        note: 'Helper scripts: scripts/build-release.sh and scripts/build-release.bat.',
      },
    ],
  },
  {
    id: 'features',
    title: 'Features',
    summary: 'Core capabilities and notable behavior.',
    sections: [
      {
        id: 'file-browsing',
        title: 'Browsing and Interaction',
        bullets: [
          'Virtualized list and grid rendering for large directories',
          'Live directory updates through watcher events',
          'Drag and drop with breadcrumb targets and visual affordances',
          'Bookmarks, starred, recent, trash, and mount listing',
        ],
      },
      {
        id: 'search',
        title: 'Search',
        bullets: [
          'Recursive, case-insensitive, streamed search',
          'Scoped to current directory subtree',
          'Symlinks ignored to reduce loop/safety risks',
        ],
      },
      {
        id: 'duplicates',
        title: 'Duplicate Checks',
        bullets: [
          'Context action for single selected file',
          'User-selectable start folder',
          'Two-phase matching: size prefilter then byte-by-byte compare',
          'Streaming progress with clean cancellation on modal close/Esc',
          'Symlinks ignored',
        ],
      },
      {
        id: 'archives',
        title: 'Archives and Thumbnails',
        bullets: [
          'Extraction support for Zip, Tar variants, 7z, and stored RAR',
          'Batch extraction with progress and cancellation',
          'Thumbnail pipeline for images, SVG, PDFs (PDFium), and videos (ffmpeg)',
        ],
      },
      {
        id: 'permissions',
        title: 'Permissions and Safety',
        bullets: [
          'Unix mode-bit edits and Windows DACL-based access changes',
          'Rollback on failed multi-target permission updates',
          'Windows network paths delete permanently (no recycle bin support)',
        ],
      },
    ],
  },
  {
    id: 'settings-shortcuts',
    title: 'Settings and Shortcuts',
    summary: 'Persisted defaults and keyboard map.',
    sections: [
      {
        id: 'persisted-settings',
        title: 'Persisted Settings',
        bullets: [
          'Show hidden, hidden-last, folders-first',
          'Start directory and default view (list/grid)',
          'Sort field/direction and delete confirmation',
          'Density profile (cozy/compact)',
          'Hardware acceleration toggle',
        ],
      },
      {
        id: 'hardware-accel',
        title: 'Hardware Acceleration (Linux)',
        body: 'When hardware acceleration is disabled, Browsey keeps compositing enabled and sets WEBKIT_DISABLE_DMABUF_RENDERER=1.',
      },
      {
        id: 'shortcuts',
        title: 'Primary Shortcuts',
        bullets: [
          'Ctrl+F: toggle search mode',
          'Ctrl+G: toggle list/grid view',
          'Ctrl+C / Ctrl+X / Ctrl+V: copy/cut/paste',
          'F2: rename',
          'Delete / Shift+Delete: trash / permanent delete',
          'Ctrl+P: properties',
          'Ctrl+H: toggle hidden files',
        ],
      },
    ],
  },
  {
    id: 'architecture',
    title: 'Architecture',
    summary: 'Backend/frontend split and repository layout.',
    sections: [
      {
        id: 'backend',
        title: 'Backend (Rust / Tauri Commands)',
        bullets: [
          'Directory listing, search_stream, and duplicate scan commands',
          'Clipboard preview/execute and filesystem operations',
          'Archive extraction/compression and mount handling',
          'Watcher-driven refresh events and metadata services',
        ],
      },
      {
        id: 'frontend',
        title: 'Frontend (Svelte)',
        bullets: [
          'Explorer components, hooks, stores, and service wrappers',
          'Modal shell and shared UI primitives',
          'Virtualized list/grid and selection systems',
          'Per-feature service modules under frontend/src/features/explorer/services/',
        ],
      },
      {
        id: 'data',
        title: 'Data and Persistence',
        bullets: [
          'SQLite stores bookmarks, stars, recents, and column widths',
          'Thumbnail cache lives in user cache dir with periodic trimming',
          'Capability config enables event listen/emit between backend and UI',
        ],
      },
      {
        id: 'repo-layout',
        title: 'Repository Layout',
        bullets: [
          'src/: Rust backend modules',
          'frontend/: main app UI',
          'docs/: standalone docs app',
          'scripts/: dev/build helper scripts',
          'resources/: icons, schemas, and bundled binaries',
        ],
      },
    ],
  },
  {
    id: 'release-notes',
    title: 'Release Notes',
    summary: 'Recent changes by version.',
    sections: [
      {
        id: 'v041',
        title: 'v0.4.1 (2026-02-08)',
        bullets: [
          'Linux rendering fallback simplified to WEBKIT_DISABLE_DMABUF_RENDERER=1 when acceleration is disabled',
          'New Check for Duplicates tool and dedicated modal',
          'Duplicate backend matching pipeline: size filter + byte compare',
          'Streaming duplicate progress and robust cancellation behavior',
          'README and docs updates for duplicate-check behavior',
        ],
      },
      {
        id: 'v040',
        title: 'v0.4.0 (2026-02-06)',
        bullets: [
          'Search moved to blocking backend task to prevent UI freezes',
          'Archive support expansion (7z/RAR + batch extraction)',
          'PDFium-backed thumbnail pipeline and performance refinements',
          'GVFS/OneDrive/MTP mount and transfer reliability improvements',
          'Persistent defaults and UX polish across core flows',
        ],
      },
    ],
  },
  {
    id: 'troubleshooting',
    title: 'Troubleshooting',
    summary: 'Common setup and runtime issues.',
    sections: [
      {
        id: 'docs-pages-404',
        title: 'Docs URL Returns 404',
        bullets: [
          'Confirm successful Docs Pages run in GitHub Actions',
          'Ensure Settings > Pages uses Source: GitHub Actions',
          'Use the repository URL path: /Browsey/ (case-sensitive)',
        ],
      },
      {
        id: 'docs-local-tools',
        title: 'Missing vite or svelte-check',
        body: 'If docs commands fail with tool-not-found errors, dependencies are not installed yet.',
        code: `npm --prefix docs install\nnpm --prefix docs run check`,
      },
      {
        id: 'linux-webview',
        title: 'Linux Build Fails on WebKit/Soup',
        body: 'Install required system development packages (webkit2gtk4.1, javascriptcoregtk4.1, libsoup3, gtk3) for your distro.',
      },
      {
        id: 'reporting',
        title: 'Reporting Issues',
        body: 'Include platform, app version, reproduction steps, and relevant logs/screenshots when reporting bugs.',
      },
    ],
  },
]

export const docsPageMap = new Map(docsPages.map((page) => [page.id, page]))
