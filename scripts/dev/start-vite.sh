#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -- "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
FRONTEND_DIR="${ROOT_DIR}/frontend"
PORT="${BROWSEY_VITE_PORT:-5173}"
READY_TIMEOUT_SECONDS="${BROWSEY_VITE_READY_TIMEOUT_SECONDS:-30}"

log() {
  printf '[start-vite] %s\n' "$*"
}

require_cmd() {
  if ! command -v "$1" >/dev/null 2>&1; then
    log "ERROR: required command not found: $1"
    exit 1
  fi
}

require_cmd npm
require_cmd curl

if [[ ! -d "$FRONTEND_DIR" ]]; then
  log "ERROR: frontend directory not found: $FRONTEND_DIR"
  exit 1
fi

# Start Vite dev server
log "starting Vite on port ${PORT}"
npm --prefix "${FRONTEND_DIR}" run dev -- --host --port "${PORT}" --strictPort --clearScreen false &
SERVER_PID=$!

cleanup() {
  kill "${SERVER_PID}" 2>/dev/null || true
}
trap cleanup EXIT

# Wait for server to respond
start_ts="$(date +%s)"
until curl -fsS "http://localhost:${PORT}/" >/dev/null 2>&1; do
  if ! kill -0 "${SERVER_PID}" 2>/dev/null; then
    set +e
    wait "${SERVER_PID}"
    status=$?
    set -e
    log "ERROR: Vite exited before becoming ready (exit=${status})"
    exit "${status}"
  fi
  now_ts="$(date +%s)"
  if (( now_ts - start_ts >= READY_TIMEOUT_SECONDS )); then
    log "ERROR: timed out waiting for Vite readiness after ${READY_TIMEOUT_SECONDS}s"
    exit 1
  fi
  sleep 0.2
done

# Prewarm Svelte compile to avoid transient virtual CSS errors
curl -fsS "http://localhost:${PORT}/src/App.svelte" >/dev/null 2>&1 || true

log "Vite is ready"
wait "${SERVER_PID}"
