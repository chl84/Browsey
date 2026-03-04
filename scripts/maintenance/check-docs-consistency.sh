#!/usr/bin/env bash
set -euo pipefail

STRICT=0
case "${1:-}" in
  "")
    ;;
  --strict)
    STRICT=1
    ;;
  -h|--help)
    cat <<'EOF'
Usage: bash scripts/maintenance/check-docs-consistency.sh [--strict]

Checks consistency across README, CHANGELOG, and docs-site release notes.
Default mode is advisory (WARN, exit 0).
Use --strict to return non-zero when checks fail.
EOF
    exit 0
    ;;
  *)
    echo "Unknown argument: $1" >&2
    exit 2
    ;;
esac

SCRIPT_DIR="$(cd -- "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd -- "${SCRIPT_DIR}/../.." && pwd)"
cd "${REPO_ROOT}"

pass_count=0
warn_count=0
fail_count=0

report_pass() {
  local id="$1"
  local message="$2"
  printf 'PASS [%s] %s\n' "${id}" "${message}"
  pass_count=$((pass_count + 1))
}

report_issue() {
  local id="$1"
  local message="$2"
  if [[ "${STRICT}" -eq 1 ]]; then
    printf 'FAIL [%s] %s\n' "${id}" "${message}"
    fail_count=$((fail_count + 1))
  else
    printf 'WARN [%s] %s\n' "${id}" "${message}"
    warn_count=$((warn_count + 1))
  fi
}

check_contains_regex() {
  local id="$1"
  local file="$2"
  local regex="$3"
  local ok_message="$4"
  local missing_message="$5"
  if grep -Eq "${regex}" "${file}"; then
    report_pass "${id}" "${ok_message}"
  else
    report_issue "${id}" "${missing_message}"
  fi
}

check_contains_fixed() {
  local id="$1"
  local file="$2"
  local text="$3"
  local ok_message="$4"
  local missing_message="$5"
  if grep -Fq "${text}" "${file}"; then
    report_pass "${id}" "${ok_message}"
  else
    report_issue "${id}" "${missing_message}"
  fi
}

VERSION="$(grep -m1 '^version = "' Cargo.toml | sed -E 's/^version = "([^"]+)".*$/\1/')"
if [[ -z "${VERSION}" ]]; then
  report_issue "version-extract" "Unable to read app version from Cargo.toml"
else
  report_pass "version-extract" "Resolved app version from Cargo.toml: ${VERSION}"
fi

if [[ -n "${VERSION}" ]]; then
  check_contains_regex \
    "changelog-version" \
    "CHANGELOG.md" \
    "^## v${VERSION}[[:space:]]+" \
    "CHANGELOG includes version heading for v${VERSION}" \
    "CHANGELOG is missing expected heading for v${VERSION}"

  check_contains_fixed \
    "docs-release-notes-version" \
    "docs-site/src/content/pages.ts" \
    "v${VERSION} (" \
    "docs-site release notes include v${VERSION}" \
    "docs-site release notes are missing v${VERSION}"
fi

if grep -Eq 'duplicate scan, thumbnails, or direct open-with for cloud files' README.md; then
  report_issue \
    "readme-cloud-thumbs-outdated" \
    "README still claims cloud thumbnails are unsupported"
else
  report_pass \
    "readme-cloud-thumbs-outdated" \
    "README no longer claims cloud thumbnails are unsupported"
fi

check_contains_fixed \
  "readme-cloud-thumbs-opt-in" \
  "README.md" \
  "Cloud thumbs" \
  "README mentions Cloud thumbs opt-in behavior" \
  "README is missing Cloud thumbs opt-in wording"

check_contains_fixed \
  "readme-cloud-thumbs-scope-grid" \
  "README.md" \
  "Grid view" \
  "README mentions Grid-only cloud thumbnail scope" \
  "README is missing Grid-only cloud thumbnail scope"

check_contains_fixed \
  "readme-cloud-thumbs-scope-formats" \
  "README.md" \
  "image/pdf/svg" \
  "README mentions cloud thumbnail format scope (image/pdf/svg)" \
  "README is missing cloud thumbnail format scope (image/pdf/svg)"

check_contains_fixed \
  "readme-rename-shortcut" \
  "README.md" \
  '`Ctrl+R` rename' \
  "README uses Ctrl+R as default rename shortcut" \
  "README is missing Ctrl+R rename default"

if grep -Fq '`F2` rename' README.md; then
  report_issue \
    "readme-rename-shortcut-legacy" \
    "README still contains legacy F2 rename default"
else
  report_pass \
    "readme-rename-shortcut-legacy" \
    "README does not contain legacy F2 rename default"
fi

check_contains_fixed \
  "readme-cloud-cache-maintenance" \
  "README.md" \
  "cloud file cache" \
  "README includes cloud file cache in maintenance actions" \
  "README is missing cloud file cache in maintenance actions"

mode_label="advisory"
if [[ "${STRICT}" -eq 1 ]]; then
  mode_label="strict"
fi
printf 'SUMMARY mode=%s PASS=%d WARN=%d FAIL=%d\n' \
  "${mode_label}" "${pass_count}" "${warn_count}" "${fail_count}"

if [[ "${STRICT}" -eq 1 && "${fail_count}" -gt 0 ]]; then
  exit 1
fi
