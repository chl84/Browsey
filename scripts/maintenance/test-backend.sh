#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd -- "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

echo "== Backend: bundled PDFium integrity =="
node scripts/maintenance/check-pdfium.mjs

echo "== Backend: vendored source integrity =="
node scripts/maintenance/check-vendor.mjs
node --test scripts/maintenance/check-dependency-policy.test.mjs
node scripts/maintenance/check-dependency-policy.mjs

echo "== Backend: rustfmt check =="
cargo fmt --all -- --check

echo "== Backend: cargo check =="
cargo check --all-targets --all-features

echo "== Backend: clippy (deny warnings) =="
cargo clippy --all-targets --all-features -- -D warnings

echo "== Backend: semgrep typed-error seams (advisory) =="
if command -v semgrep >/dev/null 2>&1; then
  node --test scripts/maintenance/check-semgrep.test.mjs
  node scripts/maintenance/check-semgrep.mjs advisory || true

  echo "== Backend: semgrep typed-error seams (blocking: commands-first) =="
  node scripts/maintenance/check-semgrep.mjs blocking
else
  echo "warning: semgrep not installed; skipping semgrep advisory/blocking runs" >&2
fi

echo "== Backend: typed-error hardening guard =="
bash scripts/maintenance/check-backend-error-hardening-guard.sh

echo "== Backend: tests =="
cargo test --all-targets --all-features

echo "Backend suite completed."
