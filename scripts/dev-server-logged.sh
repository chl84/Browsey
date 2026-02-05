#!/usr/bin/env bash
set -euo pipefail

# Run Tauri dev and tee output to a log file for GVFS debugging.

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

mkdir -p "$ROOT/.logs"
LOG_FILE="$ROOT/.logs/dev-server.log"

echo "=== $(date -Iseconds) dev-server-logged start ===" | tee -a "$LOG_FILE"

# Enable verbose thumbnail logging (same as dev-server.sh).
export BROWSEY_DEBUG_THUMBS=1

# Keep cargo's exit code even though we tee output.
cargo tauri dev --no-dev-server 2>&1 | tee -a "$LOG_FILE"
exit "${PIPESTATUS[0]}"
