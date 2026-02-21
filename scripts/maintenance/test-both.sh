#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

echo "== Running backend suite =="
"${SCRIPT_DIR}/test-backend.sh"

echo "== Running frontend suite =="
"${SCRIPT_DIR}/test-frontend.sh"

echo "All suites completed."
