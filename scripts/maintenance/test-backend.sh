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

echo "== Backend: tests =="
cargo test --all-targets --all-features

echo "Backend suite completed."
