#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd -- "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

echo "== Backend: rustfmt check =="
cargo fmt --all -- --check

echo "== Backend: cargo check =="
cargo check --all-targets --all-features

echo "== Backend: clippy (deny warnings) =="
cargo clippy --all-targets --all-features -- -D warnings

echo "== Backend: semgrep typed-error seams (advisory) =="
if command -v semgrep >/dev/null 2>&1; then
  semgrep --config .semgrep/typed-errors.yml src/commands || true

  echo "== Backend: semgrep typed-error seams (blocking: commands-first) =="
  semgrep --config .semgrep/typed-errors-blocking.yml src/commands
else
  echo "warning: semgrep not installed; skipping semgrep advisory/blocking runs" >&2
fi

echo "== Backend: typed-error hardening guard =="
bash scripts/maintenance/check-backend-error-hardening-guard.sh

echo "== Backend: tests =="
cargo test --all-targets --all-features

echo "Backend suite completed."
