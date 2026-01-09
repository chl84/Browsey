#!/usr/bin/env bash
set -euo pipefail

# Tauri config already starts/stops the Vite dev server via beforeDevCommand.
# This wrapper just runs Tauri dev without spawning an extra server.

exec cargo tauri dev --no-dev-server
