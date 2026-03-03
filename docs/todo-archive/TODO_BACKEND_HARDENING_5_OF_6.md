# TODO: Backend Hardening to 5/6 (Commands-first)

## Short Summary

This was an active execution TODO in `docs/todo/` and served as the operational
plan to reach 5/6 hardening (commands-first, 2 sprints, progressively blocking
CI, including Semgrep). The plan was structured to be directly executable in
PR work.

## 1) Header and Goal

Created: 2026-03-03  
Target: 5/6 hardening confidence  
Scope: `src/commands/**`

Goal: prevent typed-error regressions through layered controls (guard + semantic lint + targeted cleanup).

## 2) Baseline (locked in document)

- [x] `from_external_message(...to_string())` in `src/commands/**`: **0** hits
- [x] `map_err(...to_string())` in `cloud/transfer/network/permissions`: **0** hits
- [x] `impl From<...> for String` in `src/commands/**`: **0** hits (outside scope, `metadata`/`watcher` still exist)
- [x] Rust quality workflow active (`fmt`, `clippy`, guard, tests)

## 3) Scope / Out of scope

In scope:
- `src/commands/**`
- `scripts/maintenance/check-backend-error-hardening-guard.sh`
- `.github/workflows/rust-quality.yml`
- `scripts/maintenance/test-backend.sh`

Out of scope (for this TODO):
- `src/metadata/**`, `src/watcher.rs`, `src/statusbar/**`
- frontend feature changes

## 4) Sprint 1 TODO (rules + low-risk cleanup)

- [x] Add Semgrep ruleset in `.semgrep/typed-errors.yml`
- [x] Add Semgrep to CI as **advisory** first
- [x] Clean up `from_external_message(...to_string())` in:
- [x] `src/commands/keymap.rs`
- [x] `src/commands/listing/mod.rs`
- [x] `src/commands/search/worker.rs`
- [x] `src/commands/entry_metadata/mod.rs`
- [x] `src/commands/duplicates/mod.rs`
- [x] `src/commands/compress/mod.rs`
- [x] Add/update tests that lock `code_str()` for affected mappings
- [x] Update guard policy: no new allowlist exceptions without rationale + reference

## 5) Sprint 2 TODO (progressive blocking)

- [x] Make Semgrep **blocking** for `transfer/cloud/network/permissions`
- [x] Move more modules to blocking when cleaned
- [x] Remove remaining findings in commands-first scope or document required boundaries
- [x] Document exception policy in docs (why/requirements/tests)

## 6) Quality Gates (must be green)

- [x] `cargo fmt --all -- --check`
- [x] `cargo clippy --all-targets --all-features -- -D warnings`
- [x] `bash scripts/maintenance/check-backend-error-hardening-guard.sh`
- [x] `cargo test --all-targets --all-features`
- [x] `semgrep --config .semgrep/typed-errors.yml src/commands`

## 7) Acceptance Criteria for 5/6

- [x] No new typed-error regressions in `src/commands/**`
- [x] 0 blocking findings in Semgrep scope
- [x] 0 blocking findings in guard
- [x] Stable API codes (no unintended `code` changes)

## 8) Exit + Archiving

- [x] When all items are green: move file to `docs/todo-archive/`
- [x] Add short completion note in archive file with date and result

## Test Scenarios Explicitly Covered in TODO Work

1. Mapping preservation: upstream typed error -> same expected `code_str()`.
2. No new string roundtrip in modules set to blocking.
3. CI regression: PR must fail on new blocking findings.
4. Guard regression: new exceptions without rationale must be rejected in review.

## Assumptions and Defaults

1. Scope is `commands-first`.
2. Delivery over two sprints.
3. Enforcement is progressive blocking, not big-bang.
4. Semgrep is introduced in CI (advisory -> blocking).
5. Frontend/API contracts remain stable.

## Completion note

Completed: 2026-03-03  
Result: Commands-first hardening 5/6 TODO fully completed and archived.  
Evidence: full backend quality suite green (`fmt`, `check`, `clippy`, semgrep advisory+blocking, guard, tests), with 0 semgrep findings and 0 guard blocking findings in `src/commands/**`.
