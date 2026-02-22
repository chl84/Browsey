# Explorer State Folder

This folder contains internal state modules for the Explorer feature.

## Purpose

- `state.ts` is the composition/orchestration root for `createExplorerState`.
- `state/*.ts` contains internal helpers, factories, and slice modules used by `state.ts`.
- The public Explorer state API is the return shape of `createExplorerState` (unless intentionally changed).

## What Belongs in `state.ts`

- Store composition (`createExplorerStores`)
- Slice wiring (`filteringSlice`, `preferencesSlice`, etc.)
- High-level feature orchestration
- Public API assembly (the returned object)

## What Belongs in `state/*.ts`

- Pure helper logic (sorting, list transforms, payload normalization)
- Thin factories that build callbacks from injected dependencies
- Slice modules and store modules

## Naming Rules (Applied Here)

- `create*.ts`: factories/builders that return callbacks or orchestrators
- `*.store.ts`: Svelte store modules
- Plain helper names (`searchSort.ts`, `entryMutations.ts`, `searchRuntimeHelpers.ts`) for pure logic
- Avoid `use*.ts` here unless the module is clearly a stateful hook/composable with a `use*` export

## Import Boundary Rules

- Keep imports inside the Explorer feature unless using shared APIs.
- Do not add cross-feature deep imports from `state/*`.
- If something must be shared across features, expose it via a feature barrel instead of importing private files directly.

## Practical Guidance

- Prefer extracting pure helpers before moving async orchestration.
- Keep factories thin; do not hide business behavior in dependency builders.
- When refactoring, preserve `createExplorerState` behavior first, then improve structure.

