#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

echo "== Running backend suite =="
"${SCRIPT_DIR}/test-backend.sh"

echo "== Running frontend suite =="
"${SCRIPT_DIR}/test-frontend.sh"

echo "== Running docs consistency check (advisory) =="
if ! "${SCRIPT_DIR}/check-docs-consistency.sh"; then
  echo "Docs consistency check reported a non-zero status in advisory mode; continuing."
fi

echo "All suites completed."
