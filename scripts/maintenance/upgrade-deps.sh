#!/usr/bin/env bash
set -euo pipefail

# Compatible Rust lockfile upgrade helper.
# Keep manifest constraints, coordinated Tauri/GTK versions and native ABI pins.
# Breaking changes and source/ABI refreshes are reviewed separately.

usage() {
  cat <<'EOF'
Usage: bash scripts/maintenance/upgrade-deps.sh [--allow-dirty] [--quick]

Updates Cargo.lock within the current manifest constraints, never widens pins.
Use the documented staged process for major/0.x-breaking or native updates.

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

echo "Verifying native sources and coordinated dependency pins..."
node scripts/maintenance/check-pdfium.mjs
node scripts/maintenance/check-vendor.mjs
node scripts/maintenance/check-dependency-policy.mjs

echo "Previewing compatible lockfile updates..."
cargo update --dry-run

echo "Updating lockfile..."
cargo update

echo "Checking Rust security advisories..."
cargo audit

if [[ "$QUICK" -eq 1 ]]; then
  echo "Running quick verification (cargo check --all-targets --all-features)..."
  cargo check --locked --all-targets --all-features
else
  echo "Running full backend verification suite..."
  bash scripts/maintenance/test-backend.sh
fi

echo "Done. Review changes and open a maintenance PR."
