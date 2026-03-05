#!/usr/bin/env bash
set -euo pipefail

if command -v rg >/dev/null 2>&1; then
  SEARCH_TOOL="rg"
elif command -v grep >/dev/null 2>&1; then
  SEARCH_TOOL="grep"
  echo "warning: ripgrep (rg) not found; falling back to grep" >&2
else
  echo "error: neither ripgrep (rg) nor grep is available" >&2
  exit 1
fi

status=0

TYPED_ERROR_HARDENED_DIRS=(
  src/commands
  src/undo
  src/clipboard
  src/fs_utils
  src/path_guard
  src/tasks
)

STRING_ROUNDTRIP_HARDENED_DIRS=(
  src/commands/fs
  src/commands/rename
  src/commands/decompress
  src/commands/transfer
  src/commands/cloud
  src/commands/network
  src/commands/permissions
  src/commands/open_with
  src/commands/system_clipboard
  src/undo
  src/clipboard
)

CORE_TYPED_ERROR_FILES=(
  src/commands/fs/delete_ops.rs
  src/commands/fs/trash/mod.rs
  src/commands/rename/mod.rs
  src/commands/rename/ops.rs
  src/commands/rename/preview.rs
  src/clipboard/mod.rs
  src/clipboard/ops.rs
  src/commands/cloud/write.rs
  src/commands/cloud/providers/rclone/write.rs
  src/commands/transfer/execute.rs
  src/commands/transfer/execute/flow.rs
  src/commands/transfer/execute/progress.rs
  src/commands/transfer/route.rs
  src/commands/network/mounts.rs
  src/commands/permissions/set_permissions.rs
  src/commands/permissions/ownership.rs
  src/commands/permissions/ownership/unix.rs
  src/commands/permissions/windows_acl.rs
)

ADVISORY_TO_STRING_DIRS=(
  src/commands/cloud
  src/commands/transfer
  src/commands/network
  src/commands/permissions
)

# Keep empty by default. Any new entry must use:
#   'pattern|reason|reference'
# where reference points to design/TODO context.
# Policy: docs/ERROR_HARDENING_EXCEPTION_POLICY.md
ADVISORY_TO_STRING_ALLOWLIST_ENTRIES=()

ADVISORY_TO_STRING_ALLOWED_PATTERNS=()

search_hits() {
  local pattern="$1"
  shift
  if [[ "${SEARCH_TOOL}" == "rg" ]]; then
    rg -n "${pattern}" "$@" || true
  else
    grep -R -n -E -- "${pattern}" "$@" 2>/dev/null || true
  fi
}

filter_hits_allowlist() {
  local hits="$1"
  shift
  local filtered="$hits"
  local pattern
  for pattern in "$@"; do
    filtered="$(printf '%s\n' "${filtered}" | grep -E -v -- "${pattern}" || true)"
  done
  printf '%s' "${filtered}"
}

build_allowlist_patterns() {
  local entry pattern reason reference
  for entry in "${ADVISORY_TO_STRING_ALLOWLIST_ENTRIES[@]}"; do
    IFS='|' read -r pattern reason reference <<<"${entry}"
    if [[ -z "${pattern}" || -z "${reason}" || -z "${reference}" ]]; then
      echo "error: malformed advisory allowlist entry; expected pattern|reason|reference:" >&2
      echo "  ${entry}" >&2
      status=1
      continue
    fi
    ADVISORY_TO_STRING_ALLOWED_PATTERNS+=("${pattern}")
  done
}

echo "Checking for disallowed typed-error -> String conversions in hardened modules..."
from_to_string_hits="$(
  search_hits 'impl From<.*> for String' \
    "${TYPED_ERROR_HARDENED_DIRS[@]}" || true
)"
if [[ -n "${from_to_string_hits}" ]]; then
  echo "error: found disallowed 'impl From<...> for String' in hardened modules:" >&2
  echo "${from_to_string_hits}" >&2
  status=1
fi

echo "Checking for disallowed from_external_message(error.to_string()) seams..."
string_roundtrip_hits="$(
  search_hits 'from_external_message\([^)]*to_string\(\)\)' \
    "${STRING_ROUNDTRIP_HARDENED_DIRS[@]}" || true
)"
if [[ -n "${string_roundtrip_hits}" ]]; then
  echo "error: found disallowed string round-trip classification seams:" >&2
  echo "${string_roundtrip_hits}" >&2
  status=1
fi

echo "Checking strict typed-error regime in core operations modules..."
core_ops_stringly_hits="$(
  search_hits 'from_external_message\(' \
    "${CORE_TYPED_ERROR_FILES[@]}" || true
)"
if [[ -n "${core_ops_stringly_hits}" ]]; then
  echo "error: stringly from_external_message() usage is forbidden in core operations modules:" >&2
  echo "${core_ops_stringly_hits}" >&2
  status=1
fi

core_ops_literal_err_hits="$(
  search_hits 'Err\(".*"\.into\(\)\)|Err\(".*"\)' \
    "${CORE_TYPED_ERROR_FILES[@]}" || true
)"
if [[ -n "${core_ops_literal_err_hits}" ]]; then
  echo "error: string-literal Err(...) is forbidden in core operations modules; use typed domain errors:" >&2
  echo "${core_ops_literal_err_hits}" >&2
  status=1
fi

echo "Advisory: checking map_err(...to_string()) in expanded backend seams..."
build_allowlist_patterns
raw_advisory_to_string_hits="$(
  search_hits 'map_err\(\|[^)]*\|\s*[^)]*to_string\(\)\)' \
    "${ADVISORY_TO_STRING_DIRS[@]}" || true
)"
advisory_to_string_hits="$(
  filter_hits_allowlist \
    "${raw_advisory_to_string_hits}" \
    "${ADVISORY_TO_STRING_ALLOWED_PATTERNS[@]}"
)"
if [[ -n "${advisory_to_string_hits}" ]]; then
  echo "warning: found map_err(...to_string()) occurrences (advisory; non-blocking):" >&2
  echo "${advisory_to_string_hits}" >&2
fi

if [[ "${status}" -ne 0 ]]; then
  echo "backend error hardening guard failed" >&2
  exit "${status}"
fi

echo "backend error hardening guard passed"
