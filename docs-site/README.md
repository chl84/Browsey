# Browsey Docs Site Workspace

This folder is the standalone Svelte + Vite app that renders Browsey docs.
Markdown strategy/operations docs live under `../docs/`.

For GitHub Pages deploys, the build uses `PAGES_BASE_PATH` so repository pages like
`https://chl84.github.io/Browsey/` resolve assets correctly.

## Commands

- Install deps: `npm --prefix docs-site install`
- Start local dev server: `npm --prefix docs-site run dev`
- Build static site: `npm --prefix docs-site run build`
- Preview build: `npm --prefix docs-site run preview`
- Typecheck: `npm --prefix docs-site run check`

The docs build output is `docs-site/dist/`.

## Release updates

User-facing documentation and release notes are maintained in
`src/content/pages.ts`; keep them aligned with `../README.md`, `../CHANGELOG.md`,
and the version-specific notes under `../docs/releases/`. The docs workspace's
own package version is independent of the desktop app version in `Cargo.toml`.

Run `bash scripts/maintenance/check-docs-consistency.sh --strict` from the
repository root as well as the docs lint, check, and build commands before a
release. Pushing `main` deploys the site through the Docs Pages workflow.
