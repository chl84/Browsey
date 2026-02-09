# Browsey Docs Workspace

This folder is a standalone Svelte + Vite project for Browsey documentation.

For GitHub Pages deploys, the build uses `PAGES_BASE_PATH` so repository pages like
`https://chl84.github.io/Browsey/` resolve assets correctly.

## Commands

- Install deps: `npm --prefix docs install`
- Start local dev server: `npm --prefix docs run dev`
- Build static site: `npm --prefix docs run build`
- Preview build: `npm --prefix docs run preview`
- Typecheck: `npm --prefix docs run check`

The docs build output is `docs/dist/`.
