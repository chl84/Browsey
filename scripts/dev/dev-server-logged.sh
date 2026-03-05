#!/usr/bin/env bash
set -euo pipefail

# Run Tauri dev and tee output to a log file for GVFS debugging.

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

mkdir -p "$ROOT/.logs"
LOG_FILE="${1:-${BROWSEY_DEV_LOG_FILE:-$ROOT/.logs/dev-server.log}}"

if ! command -v cargo >/dev/null 2>&1; then
  echo "[dev-server-logged] ERROR: cargo not found in PATH" >&2
  exit 1
fi

echo "=== $(date -Iseconds) dev-server-logged start (cwd=$ROOT) ===" | tee -a "$LOG_FILE"

# Enable verbose thumbnail logging (same as dev-server.sh).
export BROWSEY_DEBUG_THUMBS=1

# Keep cargo's exit code even though we tee output.
set +e
cargo tauri dev --no-dev-server 2>&1 | tee -a "$LOG_FILE"
status="${PIPESTATUS[0]}"
set -e

echo "=== $(date -Iseconds) dev-server-logged stop (exit=$status) ===" | tee -a "$LOG_FILE"
exit "$status"
