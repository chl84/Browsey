#!/usr/bin/env bash
set -euo pipefail

# Tauri config already starts/stops the Vite dev server via beforeDevCommand.
# This wrapper just runs Tauri dev without spawning an extra server.
# Disable GTK overlay scrollbars so custom scrollbar styles apply in WebKitGTK.
export GTK_OVERLAY_SCROLLING=0

exec cargo tauri dev --no-dev-server
