#!/usr/bin/env bash
set -euo pipefail

# Tauri config already starts/stops the Vite dev server via beforeDevCommand.
# This wrapper just runs Tauri dev without spawning an extra server.

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

if ! command -v cargo >/dev/null 2>&1; then
  echo "[dev-server] ERROR: cargo not found in PATH" >&2
  exit 1
fi

echo "[dev-server] Starting Tauri dev from $ROOT"

# Enable verbose thumbnail logging for debugging.
# export BROWSEY_DEBUG_THUMBS=1

exec cargo tauri dev --no-dev-server
