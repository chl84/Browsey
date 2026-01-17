#!/usr/bin/env bash
set -euo pipefail

# All-in-one Rust dependency upgrade helper.
# - Ensures cargo-edit and cargo-outdated are installed.
# - Shows outdated crates.
# - Upgrades to newest compatible versions.
# - Updates lockfile and runs cargo check.

ROOT="$(cd -- "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

need_tool() {
  local bin="$1"
  local crate="$2"
  if ! command -v "$bin" >/dev/null 2>&1; then
    echo "Installing $crate..."
    cargo install "$crate"
  fi
}

need_tool cargo-outdated cargo-outdated
need_tool cargo-upgrade cargo-edit

echo "Checking for outdated crates..."
cargo outdated --depth 1 || true

echo "Upgrading Cargo.toml dependencies..."
cargo upgrade --workspace

echo "Updating lockfile..."
cargo update

echo "Running cargo check..."
cargo check

echo "Done. Review changes and run full test/build as needed."
