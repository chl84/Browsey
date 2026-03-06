#!/usr/bin/env bash
set -euo pipefail

# Build Linux release bundles with ccache configured to use a writable temp
# directory.

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
CCACHE_DIR="${CCACHE_DIR:-$ROOT/tmp/ccache}"
CCACHE_TEMPDIR="${CCACHE_TEMPDIR:-$CCACHE_DIR/tmp}"

if ! command -v cargo >/dev/null 2>&1; then
  echo "[build-release] ERROR: cargo not found in PATH" >&2
  exit 1
fi

if command -v ccache >/dev/null 2>&1; then
  mkdir -p "$CCACHE_DIR" "$CCACHE_TEMPDIR"
  export CCACHE_DIR CCACHE_TEMPDIR
  export RUSTC_WRAPPER="${RUSTC_WRAPPER:-ccache}"
  echo "[build-release] ccache enabled (dir=$CCACHE_DIR)"
fi

cd "$ROOT"
echo "[build-release] Building RPM + DEB bundles from $ROOT"
cargo tauri build --bundles rpm,deb "$@"
