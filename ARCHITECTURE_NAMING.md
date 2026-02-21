# Architecture: Naming Conventions

This document defines lightweight naming rules for frontend files under `frontend/src`.

## Core Rules

- `use*.ts` is for stateful hooks/composables and should export a `use*` symbol.
- `create*.ts` (or `*.factory.ts`) is for factory helpers and should export `create*`.
- `*.service.ts` is for backend/platform boundary calls (Tauri `invoke`, plugin APIs, OS bridge logic).
- `*.store.ts` is for Svelte store modules.
- `types.ts` is for shared domain types.

## Placement Rules

- Keep cross-feature imports on public barrels: `@/features/<feature>`.
- Treat deep imports across features as private implementation details.
- Domain-level `index.ts` files are allowed and recommended when a domain is consumed outside its folder.

## Examples

Good:

```ts
import { ExplorerPage } from '@/features/explorer'
import { useExplorerNavigation } from './useExplorerNavigation'
import { createClipboard } from './createClipboard'
import { copyToClipboard } from './clipboard.service'
```

Avoid:

```ts
import { something } from '@/features/explorer/internal/privateFile'
// useThing.ts exporting only createThing()
```

## Exceptions

- `frontend/src/features/settings/hooks/useSettingsModalViewModel.ts` is a temporary exception and can be revisited later.

## Automated Enforcement

- ESLint enforces import-boundary restrictions (`frontend/eslint.config.js`).
- `npm --prefix frontend run lint` also runs naming validation:
  - `frontend/scripts/check-naming-conventions.mjs`
  - Fails when `use*.ts/js` exports `create*` but no `use*`, except explicit allowlist entries.
- Lint warning policy and current baseline are tracked in:
  - `docs/quality/lint-baseline.md`

## Quick Decision Checklist

1. Does it hold state and behave like a hook/composable?
2. Is it a pure builder/factory?
3. Does it call backend/platform boundaries?
4. Is it a store module?
5. Is it just shared types?

If uncertain, prefer the smallest clear module and keep naming aligned with exported API.
