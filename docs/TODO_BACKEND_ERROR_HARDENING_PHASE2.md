# TODO: Backend Error Hardening (Phase 2)

Created: 2026-03-02
Goal: Strengthen Browsey's backend error model after the completed Error API
Migration by removing string round-trips, reducing duplicated classification
logic, and preserving typed error semantics further across module boundaries.

## Why This Track Exists

The first backend error migration appears complete:

- command modules now generally expose typed module errors
- `ApiError` mapping happens at command boundaries
- raw `Result<_, String>` seams have been reduced significantly

What remains is a subtler maintenance problem:

- typed errors are still often degraded to strings and then re-classified
- similar classification tables are duplicated across modules
- some core modules still expose `impl From<...> for String`
- remote/cloud error families overlap heavily without a stronger shared model

This is no longer a "move away from `Result<_, String>`" track.
It is a semantic preservation and deduplication track.

## Findings Driving This Plan

### 1. Typed core errors are still round-tripped through strings

Representative cases:

- `src/clipboard/mod.rs` uses a generic `map_clipboard_result(...)` that maps
  any `Display` error via `ClipboardError::from_external_message(error.to_string())`
- `src/commands/rename/error.rs` converts `PathGuardError`, `FsUtilsError`, and
  `UndoError` by stringifying and re-classifying them
- `src/commands/fs/error.rs` does the same for some incoming typed errors
- `src/commands/decompress/` still relies on `from_external_message(...)` in many
  typed orchestration paths

Impact:

- original error codes are lost
- behavior depends more on message wording than necessary
- maintenance becomes fragile when messages are edited

### 2. Core modules still allow degradation back into `String`

Representative cases:

- `src/fs_utils/error.rs`
- `src/path_guard/error.rs`
- `src/tasks/error.rs`
- `src/undo/error.rs`

These `impl From<...> for String` conversions make it easy for typed internals
to regress into string-based flow again.

### 3. Message classification is duplicated across multiple file-operation domains

Modules such as:

- `src/clipboard/error.rs`
- `src/commands/fs/error.rs`
- `src/commands/rename/error.rs`
- `src/undo/error.rs`
- `src/commands/decompress/error.rs`

all classify variants of:

- not found
- permission denied
- already exists / target exists
- invalid input / invalid path
- symlink unsupported

Impact:

- easy drift between domains
- harder to reason about consistency
- more work to harden or extend classifications safely

### 4. Remote/cloud error taxonomies overlap but are not strongly unified

`TransferErrorCode` and `CloudCommandErrorCode` are close in shape and intent,
but remain parallel definitions.

Impact:

- code duplication
- adapter churn
- more places for subtle mismatch in future remote/cloud error handling

### 5. Current tests focus more on behavior than error-code stability

That is good, but it leaves a blind spot:

- the code/message mapping contract itself is not strongly defended
- classification can drift silently without obvious failures

## Architecture Alignment

This track should stay aligned with:

- `ARCHITECTURE_NAMING.md`
- existing backend module ownership
- the already-completed archived tracks:
  - `docs/todo-archive/TODO_ERROR_MIGRATION.md`
  - `docs/todo-archive/TODO_BACKEND_ERROR_API_STANDARDIZATION.md`
  - `docs/todo-archive/TODO_BACKEND_ERROR_API_REMAINING.md`

Implications:

- do not reopen broad `Result<_, String>` migration work that is already done
- prefer narrow semantic fixes over large command rewrites
- preserve existing frontend-facing `code` strings unless a change is deliberate
- treat this as phase 2 of the backend error work, not a restart of phase 1

## Scope

In scope:

- remove avoidable typed-error -> string -> typed-error round-trips
- reduce duplicated classification logic where the same semantics already exist
- strengthen typed conversions between core modules and command modules
- review overlapping cloud/transfer error families
- add regression tests for code stability and conversion behavior

Out of scope:

- frontend error UI changes
- message wording churn without structural purpose
- broad feature work
- changing stable API codes without explicit migration reason

## Duplication Guardrails

- This plan must not duplicate the archived "migrate away from raw string
  results" TODOs; that work is considered complete unless a concrete hole is found.
- A new shared abstraction is justified only if it removes real duplication in
  at least two active domains.
- Do not create a giant global error enum for the whole backend.
- Keep domain ownership local; share only the parts that are genuinely common.

## Success Criteria

This track is complete when:

- core typed errors no longer routinely degrade into strings before crossing
  adjacent module boundaries
- duplicated classification logic is reduced in the highest-value hotspots
- remote/cloud overlap is either unified or intentionally documented and bounded
- regression tests defend stable error-code outcomes for representative flows

## Quality Gates (Every Step)

- [x] `cargo fmt --all` is green
- [x] `cargo clippy --all-targets --all-features -- -D warnings` is green
- [x] `cargo test --all-targets --all-features` stays green
- [x] touched modules keep API code strings stable unless explicitly noted
- [x] new conversions preserve semantics more directly than the code they replace
- [x] no new `impl From<...> for String` is introduced in typed backend modules

## Work Plan

### 1) Inventory string round-trips and lock the target list

- [x] Create a focused inventory of:
  - `from_external_message(error.to_string())`
  - `map_err(|error| ...error.to_string())`
  - `impl From<...> for String`
- [x] Label each case as:
  - necessary boundary
  - acceptable low-level leaf
  - removable semantic loss
- [x] Start with the highest-value trust-critical modules:
  - `src/clipboard/`
  - `src/commands/fs/`
  - `src/commands/rename/`
  - `src/undo/`
  - `src/commands/decompress/`

Acceptance:

- The phase-2 target set is explicit and does not sprawl into already-completed migration work.

Inventory snapshot (2026-03-02):

- `impl From<...> for String` still exists in:
  - `src/fs_utils/error.rs`
  - `src/path_guard/error.rs`
  - plus non-phase-2 modules such as `metadata` and `watcher`
- clear removable semantic-loss cases include:
  - `src/commands/fs/error.rs` typed-error fallback through `to_string()`
  - `src/commands/decompress/` typed orchestration paths that still rely on
    `from_external_message(...)`
- first completed slice:
  - `src/clipboard/mod.rs` now maps typed upstream errors via `Into<ClipboardError>`
  - `src/tasks/error.rs` and `src/undo/error.rs` no longer expose
    `impl From<...> for String`
  - `src/commands/rename/error.rs` conversions from `PathGuardError`,
    `FsUtilsError`, and `UndoError` are now direct typed mappings
  - `src/commands/permissions/error.rs` and `src/clipboard/error.rs` now map
    `UndoError` via `UndoErrorCode` instead of `code_str()` string matching
- removable-but-deferred semantic-loss cases include:
  - `src/commands/fs/error.rs` typed-error fallback through `to_string()`
- likely acceptable low-level leaf/boundary cases still need review in:
  - external process integrations
  - archive format adapters
  - some cloud/provider adapter edges

### 2) Remove avoidable `impl From<...> for String` in core modules

Target modules:

- `src/fs_utils/error.rs`
- `src/path_guard/error.rs`
- `src/tasks/error.rs`
- `src/undo/error.rs`

- [ ] Audit current call sites for these conversions.
- [ ] Remove the conversion impls where they are no longer justified.
- [ ] Replace remaining call sites with typed conversions or explicit boundary mapping.
- [ ] If one conversion must remain, document why it is still a true boundary.

Acceptance:

- Core typed errors are no longer trivially collapsible back into raw strings.

Progress update (2026-03-02):

- completed in this slice:
  - removed `impl From<TaskError> for String`
  - removed `impl From<UndoError> for String`
- intentionally deferred:
  - `FsUtilsError -> String`
  - `PathGuardError -> String`
  - both still have broad caller fan-out and should be removed only alongside a
    larger `fs`/`decompress` slice

### 3) Replace string re-classification in local file-operation paths

Target modules:

- `src/clipboard/`
- `src/commands/fs/`
- `src/commands/rename/`
- `src/undo/`

- [ ] Replace generic `Display`-based mapping where typed upstream errors are known.
- [ ] Add direct `From<...>` conversions where semantic preservation is clear.
- [ ] Keep fallback string classification only for true external/leaf failures.
- [ ] Preserve current frontend-facing code strings unless a concrete bug requires change.

Acceptance:

- Local file-operation paths preserve codes through typed conversions instead of re-parsing messages.

Progress update (2026-03-02):

- completed in this slice:
  - `src/clipboard/mod.rs` now requires `Into<ClipboardError>` instead of
    generic `Display`
  - dead string re-classification helpers were removed from
    `src/clipboard/error.rs`, `src/tasks/error.rs`, and `src/undo/error.rs`
  - `UndoErrorCode` is re-exported from `crate::undo`
  - `src/clipboard/error.rs`, `src/commands/rename/error.rs`, and
    `src/commands/permissions/error.rs` now use typed `UndoErrorCode` matching
- deferred to next slice:
  - `src/commands/fs/`
  - `src/commands/decompress/`
  - `src/path_guard/` and `src/fs_utils/` caller cleanups needed before their
    remaining string conversions can be removed

Verification run for this slice:

- `cargo fmt --all`
- `cargo check -q`
- `cargo clippy --all-targets --all-features -- -D warnings`
- `cargo test -q undo::tests`
- `cargo test -q clipboard`
- `cargo test -q commands::permissions::tests`
- `cargo test --all-targets --all-features`

### 4) Reduce duplicated file-operation classification logic

Target modules:

- `src/clipboard/error.rs`
- `src/commands/fs/error.rs`
- `src/commands/rename/error.rs`
- `src/undo/error.rs`
- `src/commands/decompress/error.rs`

- [ ] Identify the smallest shared classification seams worth extracting.
- [ ] Prefer shared helpers for repeated IO/path semantics, not a shared mega-enum.
- [ ] Keep domain-only rules local (for example rollback, snapshot, archive-specific rules).
- [ ] Verify no user-visible code drift is introduced during deduplication.

Acceptance:

- Common file-operation semantics are less duplicated while domain-specific semantics remain local.

### 5) Review and tighten cloud/transfer error overlap

Target modules:

- `src/commands/transfer/error.rs`
- `src/commands/cloud/error.rs`
- related adapters/callers in `src/commands/cloud/` and `src/commands/transfer/`

- [ ] Audit overlap between `TransferErrorCode` and `CloudCommandErrorCode`.
- [ ] Decide whether to:
  - keep them separate but explicitly mapped, or
  - extract a narrow shared remote/cloud error core
- [ ] Implement the smaller option that reduces duplication without flattening domain ownership.

Acceptance:

- Remote/cloud error overlap is either structurally reduced or explicitly bounded and documented.

### 6) Add regression tests for conversion and code stability

- [ ] Add tests that assert representative upstream typed errors keep expected API codes.
- [ ] Add tests that prove message wording changes do not silently remap codes in key flows.
- [ ] Add at least one regression test per high-value domain:
  - clipboard/local copy-move
  - fs delete/trash
  - rename
  - undo
  - decompress
  - transfer/cloud adapter boundary

Acceptance:

- The error-code contract is tested directly instead of being only an incidental side effect.

### 7) Add a lightweight guard against regression

- [ ] Add a small maintenance check, script, or review checklist note that flags:
  - new `impl From<...> for String` in typed backend modules
  - new `from_external_message(error.to_string())` in typed module-to-module seams
- [ ] Keep the guard lightweight enough that it will actually stay enabled.

Acceptance:

- The codebase becomes less likely to drift back toward string-based semantic loss.

## Suggested Commit Boundaries

- [ ] Commit 1: inventory + target list
- [ ] Commit 2: remove `From<...> for String` in core modules
- [ ] Commit 3: local file-operation typed conversions
- [ ] Commit 4: shared classification cleanup
- [ ] Commit 5: cloud/transfer overlap tightening
- [ ] Commit 6: regression tests for code stability
- [ ] Commit 7: lightweight regression guard

## Exit Notes

When this track is complete:

- archive this TODO under `docs/todo-archive/`
- update any backend architecture/error notes if shared helpers or boundaries changed
- record any intentionally retained string-classification boundaries so they do
  not get rediscovered as "mystery debt"
