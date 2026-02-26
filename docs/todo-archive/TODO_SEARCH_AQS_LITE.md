# TODO: Search AQS Lite (Scoped Syntax)

## Goal

Implement a **limited AQS-like search syntax** for Browsey search that is:

- backend-evaluated (single source of truth)
- streaming-compatible with the current `search_stream` flow
- small enough to maintain
- explicitly scoped (no "full AQS" ambition in this phase)

Keep the frontend parser-free. The frontend sends the raw query string and displays backend parse errors.

## Supported Syntax (This Scope Only)

### Text / Pattern

- `*` matches zero or more characters
- `?` matches exactly one character
- `"text"` means exact phrase (single token; no splitting)
- `=:` means exact value in fielded queries (e.g. `name:=file.txt`)

### Boolean Logic

- `AND` (both must match)
- `OR` (at least one must match)
- `NOT` (exclude)
- `(...)` grouping

### Fields (only these)

- `name:text`
  - matches basename (`FsEntry.name`)
  - includes folders, files, and links
- `filename:text`
  - matches basename, but only for files and links
- `folder:text`
  - matches folder context (entry is in a matching parent folder path)
  - applies to folders/files/links based on parent path
- `path:text`
  - matches full path (`FsEntry.path`)
- `hidden:true|false`
- `readonly:true|false`

## Explicit Non-Goals (for this TODO)

- Full AQS compatibility
- Date/size operators (`size:`, `modified:`, `>`, `<`, ranges)
- `is:` aliases
- Frontend-side parser
- Query autocomplete/suggestions
- Locale-aware advanced tokenization

## Semantics (Lock Before Coding)

- Matching is case-insensitive.
- Unfielded text terms match `name:` semantics.
- `"` creates one phrase token (exact value token that may contain spaces).
- `=:` in a fielded value means exact value (case-insensitive equality).
- Wildcards apply to text fields (`name`, `filename`, `folder`, `path`).
- `hidden:` and `readonly:` accept only boolean values in this phase.
- Invalid syntax returns a structured backend error through the existing search progress error field.

### Field Semantics Details

- `name:` -> `FsEntry.name` for `dir|file|link`
- `filename:` -> `FsEntry.name` for `file|link` only
- `folder:` -> parent directory path of `FsEntry.path` (contains/wildcard on parent path)
- `path:` -> full `FsEntry.path`
- `readonly:` -> `FsEntry.read_only`
- `hidden:` -> `FsEntry.hidden`

## Architecture (Recommended)

Create a small backend query module under:

- `src/commands/search/query/`

Suggested files:

- `ast.rs` (expression + predicate types)
- `lexer.rs` (tokens)
- `parser.rs` (AND/OR/NOT + grouping parser)
- `eval.rs` (match against `FsEntry`)
- `error.rs` (parse errors with position/span)
- `mod.rs` (exports)

Keep `src/commands/search/worker.rs` focused on:

- traversal
- batching
- cancel checks
- progress event emission

Do not embed parser logic in `worker.rs`.

## Implementation Plan

### Step 1: Add query module skeleton + AST (no behavior change yet)

- [x] Create `src/commands/search/query/` with `mod.rs`, `ast.rs`, `error.rs`
- [x] Define `Expr` (`And`, `Or`, `Not`, `Predicate`)
- [x] Define `Predicate` variants for this scope only
- [x] Define `TextMatch`/`MatchMode` types for:
  - plain contains
  - quoted exact phrase token
  - field `=:` exact value
  - wildcard pattern

Quality gate:
- [x] `cargo check`

### Step 2: Implement lexer (scoped tokens only)

- [x] Tokenize identifiers, quoted strings, booleans, `:`, `(`, `)`, keywords (`AND/OR/NOT`)
- [x] Preserve spans/positions for errors
- [x] Support `*` and `?` inside text tokens (no expansion yet)
- [x] Support `=:` form in fielded values (`name:=foo`)

Quality gate:
- [x] Lexer unit tests for happy path and syntax edge cases
- [x] `cargo test` (targeted lexer tests)

### Step 3: Implement parser (boolean precedence + grouping)

- [x] Parse fielded predicates (`name:`, `filename:`, `folder:`, `path:`, `hidden:`, `readonly:`)
- [x] Parse unfielded text tokens as `name:` predicates
- [x] Parse `NOT`, `AND`, `OR` with precedence (`NOT` > `AND` > `OR`)
- [x] Parse grouped expressions with parentheses
- [x] Return useful parse errors with positions

Quality gate:
- [x] Parser unit tests (precedence, grouping, invalid syntax)
- [x] `cargo test` (targeted parser tests)

### Step 4: Implement evaluator (`FsEntry` matcher)

- [x] Implement case-insensitive text matching for:
  - contains
  - exact phrase token
  - exact value
  - wildcard (`*`, `?`)
- [x] Implement `name`, `filename`, `folder`, `path` field semantics
- [x] Implement `hidden` / `readonly` boolean predicates
- [x] Add safe wildcard matcher (escape regex metacharacters before wildcard expansion, or implement non-regex matcher)

Quality gate:
- [x] Evaluator unit tests against sample `FsEntry` values
- [x] `cargo test` (targeted eval tests)

### Step 5: Integrate parser/evaluator into `search_stream`

- [x] Parse/compile query once at search start in `src/commands/search/worker.rs`
- [x] On parse error, emit search error payload and finish cleanly
- [x] Replace current `name contains` check with query evaluator
- [x] Keep batching/cancel/facets behavior unchanged

Quality gate:
- [x] Existing search tests pass
- [x] Add integration test for parse-error payload
- [x] `cargo test`

### Step 6: Frontend error handling polish (minimal)

- [x] Confirm backend parse error is surfaced distinctly from "0 results"
- [x] If needed, adjust search error display text (no parser in frontend)
- [x] No syntax interpretation in frontend

Quality gate:
- [x] `npm --prefix frontend run check`
- [x] `npm --prefix frontend run lint`
- [x] Manual search test with invalid query (e.g. unmatched `(`)

### Step 7: Documentation (scoped syntax only)

- [x] Document supported AQS-lite syntax in user docs/help text
- [x] Document unsupported syntax explicitly to avoid false expectations
- [x] Add examples for `name`, `filename`, `folder`, `path`, `hidden`, `readonly`

Quality gate:
- [x] `npm --prefix docs run check` (if docs content changes there)

## Test Matrix (Must Pass Before Calling It Done)

### Text / Wildcards

- [x] `foo`
- [x] `"foo bar"`
- [x] `name:rep*`
- [x] `name:?.txt`
- [x] `name:=file.txt` (exact value in this scope)

### Fields

- [x] `filename:report`
- [x] `folder:Downloads`
- [x] `path:/home/user`
- [x] `hidden:true`
- [x] `hidden:false`
- [x] `readonly:true`
- [x] `readonly:false`

### Boolean / Grouping

- [x] `name:foo AND NOT hidden:true`
- [x] `name:foo OR name:bar`
- [x] `(name:foo OR name:bar) AND readonly:false`
- [x] `folder:Projects AND filename:*.rs`

### Error Cases

- [x] `(` (unclosed group)
- [x] `name:` (missing value)
- [x] `hidden:maybe` (invalid boolean)
- [x] `name:foo AND OR bar` (invalid boolean sequence)

## Risks / Watchouts

- Wildcard matching must not be implemented as raw regex injection.
- `folder:` semantics can be ambiguous; keep it explicitly on **parent path** in this phase.
- Keep parser/evaluator isolated so future `size:` / `modified:` support does not bloat `worker.rs`.
- Avoid frontend parser duplication.

## Suggested Commit Boundaries

- [x] Commit 1: query module skeleton + AST + errors
- [x] Commit 2: lexer + tests
- [x] Commit 3: parser + tests
- [x] Commit 4: evaluator + tests
- [x] Commit 5: backend search integration + integration tests
- [x] Commit 6: frontend error polish + docs
