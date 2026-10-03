# Browsey Docs Site Workspace

This folder is the standalone Svelte + Vite app that renders Browsey docs.
Markdown strategy/operations docs live under `../docs/`.

For GitHub Pages deploys, the build uses `PAGES_BASE_PATH` so repository pages like
`https://chl84.github.io/Browsey/` resolve assets correctly.

## Commands

- Install locked deps: `npm --prefix docs-site ci`
- Start local dev server: `npm --prefix docs-site run dev`
- Build static site: `npm --prefix docs-site run build`
- Preview build: `npm --prefix docs-site run preview`
- Typecheck: `npm --prefix docs-site run check`
- Lint: `npm --prefix docs-site run lint`
- Test routes, search, links and content: `npm --prefix docs-site test` (Node 22.6+)

The docs build output is `docs-site/dist/`.

## Release updates

User-facing documentation and release notes are maintained in
`src/content/pages.ts`; keep them aligned with `../README.md`, `../CHANGELOG.md`,
and the version-specific notes under `../docs/releases/`. The docs workspace's
own package version is independent of the desktop app version in `Cargo.toml`.

Run `bash scripts/maintenance/check-docs-consistency.sh --strict` from the
repository root as well as the docs lint, check, test and build commands before a
release. Docs-related pull requests run checks without deploying. Pushing `main`
deploys the site through the Docs Pages workflow.

Use `PAGES_BASE_PATH=/Browsey/ npm --prefix docs-site run build` to verify the
production asset prefix locally. Preview with
`PAGES_BASE_PATH=/Browsey/ npm --prefix docs-site run preview`
and open `/Browsey/`. Ordinary development uses `/`.

Keep navigation and search helpers in `src/lib/docs.ts`. Content strings support
inline backtick code, escaped as text; use section `links` for clickable references,
not raw HTML. Preserve page/section IDs when updating wording so old deep links work.
