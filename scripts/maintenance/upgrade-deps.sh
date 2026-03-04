#!/usr/bin/env bash
set -euo pipefail

# All-in-one Rust dependency upgrade helper.
# - Ensures cargo-edit and cargo-outdated are installed.
# - Shows outdated crates.
# - Upgrades to newest compatible versions.
# - Updates lockfile and runs backend quality verification.

usage() {
  cat <<'EOF'
Usage: bash scripts/maintenance/upgrade-deps.sh [--allow-dirty] [--quick]

Options:
  --allow-dirty  Allow running when git worktree is not clean.
  --quick        Run only cargo check after upgrades (skip full backend suite).
  -h, --help     Show this help.
EOF
}

ALLOW_DIRTY=0
QUICK=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --allow-dirty)
      ALLOW_DIRTY=1
      ;;
    --quick)
      QUICK=1
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "Unknown argument: $1" >&2
      usage >&2
      exit 2
      ;;
  esac
  shift
done

ROOT="$(cd -- "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

if [[ "$ALLOW_DIRTY" -ne 1 ]]; then
  if [[ -n "$(git status --porcelain)" ]]; then
    echo "error: git worktree is not clean." >&2
    echo "       Commit/stash changes first, or rerun with --allow-dirty." >&2
    exit 1
  fi
fi

need_tool() {
  local bin="$1"
  local crate="$2"
  if ! command -v "$bin" >/dev/null 2>&1; then
    echo "Installing $crate..."
    cargo install --locked "$crate"
  fi
}

need_tool cargo-outdated cargo-outdated
need_tool cargo-upgrade cargo-edit

echo "Checking for outdated crates..."
if ! cargo outdated --depth 1; then
  echo "error: 'cargo outdated' failed. Resolve this before continuing." >&2
  exit 1
fi

echo "Upgrading Cargo.toml dependencies..."
cargo upgrade

echo "Updating lockfile..."
cargo update

if [[ "$QUICK" -eq 1 ]]; then
  echo "Running quick verification (cargo check --all-targets --all-features)..."
  cargo check --all-targets --all-features
else
  echo "Running full backend verification suite..."
  bash scripts/maintenance/test-backend.sh
fi

echo "Done. Review changes and open a maintenance PR."
