#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd -- "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
FRONTEND_DIR="${ROOT_DIR}/frontend"
PORT=5173

# Start Vite dev server
npm --prefix "${FRONTEND_DIR}" run dev -- --host --port "${PORT}" --strictPort --clearScreen false &
SERVER_PID=$!

cleanup() {
  kill "${SERVER_PID}" 2>/dev/null || true
}
trap cleanup EXIT

# Wait for server to respond
until curl -fsS "http://localhost:${PORT}/" >/dev/null 2>&1; do
  sleep 0.2
done

# Prewarm Svelte compile to avoid transient virtual CSS errors
curl -fsS "http://localhost:${PORT}/src/App.svelte" >/dev/null 2>&1 || true

wait "${SERVER_PID}"
