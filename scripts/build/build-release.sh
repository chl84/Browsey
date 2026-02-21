#!/usr/bin/env bash
set -euo pipefail

# Build release RPM with ccache configured to use a writable temp directory.

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
CCACHE_DIR="${CCACHE_DIR:-$ROOT/tmp/ccache}"
CCACHE_TEMPDIR="${CCACHE_TEMPDIR:-$CCACHE_DIR/tmp}"

if command -v ccache >/dev/null 2>&1; then
  mkdir -p "$CCACHE_DIR" "$CCACHE_TEMPDIR"
  export CCACHE_DIR CCACHE_TEMPDIR
  export RUSTC_WRAPPER="${RUSTC_WRAPPER:-ccache}"
fi

cd "$ROOT"
cargo tauri build --bundles rpm "$@"
