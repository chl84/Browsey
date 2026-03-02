#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd -- "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"

# Playwright/Node workers may set FORCE_COLOR internally.
# If NO_COLOR is present, Node emits warning noise about conflicting settings.
if [[ -n "${NO_COLOR:-}" ]]; then
  unset NO_COLOR
fi

echo "== Frontend: lint =="
npm --prefix frontend run lint

echo "== Frontend: typecheck =="
npm --prefix frontend run check

echo "== Frontend: unit/integration tests =="
npm --prefix frontend run test

if [[ "${CI:-}" == "true" ]]; then
  echo "== Frontend: playwright browser setup =="
  npx --prefix frontend playwright install --with-deps chromium
elif [[ "${FORCE_PLAYWRIGHT_INSTALL:-}" == "1" ]]; then
  echo "== Frontend: playwright browser setup (local forced) =="
  npx --prefix frontend playwright install chromium
else
  echo "== Frontend: skipping playwright install (local mode) =="
  echo "   Set FORCE_PLAYWRIGHT_INSTALL=1 to force browser install."
fi

echo "== Frontend: e2e smoke =="
npm --prefix frontend run test:e2e

echo "== Frontend: build =="
npm --prefix frontend run build

echo "Frontend suite completed."
