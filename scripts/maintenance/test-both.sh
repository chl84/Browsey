#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd -- "$(dirname "${BASH_SOURCE[0]}")" && pwd)"

DOCS_MODE="advisory"
case "${1:-}" in
  "")
    ;;
  --strict-docs)
    DOCS_MODE="strict"
    ;;
  -h|--help)
    cat <<'EOF'
Usage: bash scripts/maintenance/test-both.sh [--strict-docs]

Runs backend + frontend maintenance suites.
Docs consistency runs in advisory mode by default.
Use --strict-docs to make docs consistency blocking.
EOF
    exit 0
    ;;
  *)
    echo "Unknown argument: $1" >&2
    exit 2
    ;;
esac

echo "== Running backend suite =="
"${SCRIPT_DIR}/test-backend.sh"

echo "== Running frontend suite =="
"${SCRIPT_DIR}/test-frontend.sh"

if [[ "${DOCS_MODE}" == "strict" ]]; then
  echo "== Running docs consistency check (blocking) =="
  "${SCRIPT_DIR}/check-docs-consistency.sh" --strict
else
  echo "== Running docs consistency check (advisory) =="
  if ! "${SCRIPT_DIR}/check-docs-consistency.sh"; then
    echo "Docs consistency check reported a non-zero status in advisory mode; continuing."
  fi
fi

echo "All suites completed."
