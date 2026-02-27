#!/usr/bin/env bash
set -euo pipefail

# Tauri config already starts/stops the Vite dev server via beforeDevCommand.
# This wrapper just runs Tauri dev without spawning an extra server.

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

# Enable verbose thumbnail logging for debugging.
# export BROWSEY_DEBUG_THUMBS=1

export BROWSEY_RCLONE_RC=1

exec cargo tauri dev --no-dev-server
