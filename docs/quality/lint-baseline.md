# Frontend Lint Baseline

Created: 2026-02-21  
Scope: `frontend/src`

## Purpose

Track lint debt, ownership, and policy so warning count trends are measurable and regressions are blocked.

## Policy

- No new frontend lint warnings are allowed.
- Warning count must stay at `0` unless an explicit temporary exception is documented in this file.
- Any temporary exception must include:
  - reason
  - owner
  - expiry/removal plan

## Snapshot History

### 2026-02-21 (pre-cleanup)

- `npm --prefix frontend run lint`: `0 errors`, `17 warnings`
- Warning categories:
  - `@typescript-eslint/no-unused-vars`: `12`
  - `no-useless-escape`: `4`
  - `no-unsafe-finally`: `1`

Owner-area map:
- `explorer/context`: 1 (`@typescript-eslint/no-unused-vars`)
- `explorer/hooks`: 7 (3 unused-vars, 4 useless-escape)
- `explorer/modals`: 2 (1 unused-vars, 1 unsafe-finally)
- `explorer/pages`: 3 (`@typescript-eslint/no-unused-vars`)
- `explorer/state`: 3 (`@typescript-eslint/no-unused-vars`)
- `explorer/thumbnailLoader`: 1 (`@typescript-eslint/no-unused-vars`)

### 2026-02-21 (post-cleanup)

- `npm --prefix frontend run lint`: `0 errors`, `0 warnings`
- `npm --prefix frontend run check`: green
- `npm --prefix frontend run build`: green

## Contributor Rule

Before merge:
- Run `npm --prefix frontend run lint`.
- If warning count is non-zero, fix or explicitly document a temporary exception here before requesting review.
