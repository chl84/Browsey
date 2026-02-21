# Architecture: Import Boundaries

This project uses strict frontend import boundaries to keep features isolated and make refactoring safer.

## Rules

- `shared` is reusable and can be imported by any feature.
- Every feature exposes a public API through `index.ts`.
- Cross-feature imports must use a feature barrel.
- Deep imports across feature boundaries are private and forbidden.

## Allowed

```ts
import { invoke } from '@/shared'
import { ExplorerPage } from '@/features/explorer'
import { createListState } from '@/features/explorer/stores/list.store' // same-feature internal import
```

## Not Allowed

```ts
import { setShortcutBinding } from '@/features/shortcuts/service'
import SettingsModal from '@/features/settings/SettingsModal.svelte'
import type { Entry } from '@/features/explorer/model/types'
```

Use these instead:

```ts
import { setShortcutBinding } from '@/features/shortcuts'
import { SettingsModal } from '@/features/settings'
import type { Entry } from '@/features/explorer'
```

## Enforcement

- ESLint enforces boundary rules via `no-restricted-imports`.
- Rule severity is `error`.
- Naming drift is checked by `frontend/scripts/check-naming-conventions.mjs`.
- CI gate runs:
  - `npm --prefix frontend run lint`
  - `npm --prefix frontend run check`
  - `npm --prefix frontend run build`

Related:
- See `ARCHITECTURE_NAMING.md` for file naming and placement conventions.
