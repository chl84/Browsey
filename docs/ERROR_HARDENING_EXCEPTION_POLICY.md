# Backend Error Hardening Exception Policy

Created: 2026-03-03  
Scope: backend typed-error hardening guards and semgrep rules

## Why exceptions are rare

Typed errors are required to keep stable API `code` values and prevent
string-based regressions. Exceptions are allowed only at hard integration
boundaries where typed propagation is not technically possible yet.

## Requirements for any new exception

1. Keep scope minimal: one exact pattern (or single boundary), never a broad module wildcard.
2. Add explicit rationale: why typed propagation is not possible right now.
3. Add reference: link to TODO/issue/design note for planned removal.
4. Add/keep regression test: preserve expected `code_str()` mapping at boundary.
5. Define removal trigger: what change allows removing the exception.

## Guard allowlist format

`scripts/maintenance/check-backend-error-hardening-guard.sh` requires:

`pattern|reason|reference`

Any entry missing one of these parts is treated as invalid.

## Review rule

PRs that introduce new allowlist entries must include:

1. test proof,
2. reason,
3. reference,
4. explicit follow-up plan.

## Test-only policy

`tests.rs` files are checked as advisory-only in the backend hardening guard.
They should still prefer typed error codes when asserting runtime behavior, but
they do not block the suite at the same severity as production/runtime seams.

Runtime files remain strict even when they contain inline `#[cfg(test)]` blocks.
If a test-only seam would otherwise trip a blocking runtime guard, prefer moving
that helper/assertion into a dedicated `tests.rs` file instead of weakening the
runtime rule.
