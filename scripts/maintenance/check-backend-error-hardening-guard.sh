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

search_hits() {
  local pattern="$1"
  shift
  if [[ "${SEARCH_TOOL}" == "rg" ]]; then
    rg -n "${pattern}" "$@" || true
  else
    grep -R -n -E -- "${pattern}" "$@" 2>/dev/null || true
  fi
}

echo "Checking for disallowed typed-error -> String conversions in hardened modules..."
from_to_string_hits="$(
  search_hits 'impl From<.*> for String' \
    src/commands \
    src/undo \
    src/clipboard \
    src/fs_utils \
    src/path_guard \
    src/tasks || true
)"
if [[ -n "${from_to_string_hits}" ]]; then
  echo "error: found disallowed 'impl From<...> for String' in hardened modules:" >&2
  echo "${from_to_string_hits}" >&2
  status=1
fi

echo "Checking for disallowed from_external_message(error.to_string()) seams..."
string_roundtrip_hits="$(
  search_hits 'from_external_message\([^)]*to_string\(\)\)' \
    src/commands/fs \
    src/commands/rename \
    src/commands/decompress \
    src/commands/transfer \
    src/commands/cloud \
    src/undo \
    src/clipboard || true
)"
if [[ -n "${string_roundtrip_hits}" ]]; then
  echo "error: found disallowed string round-trip classification seams:" >&2
  echo "${string_roundtrip_hits}" >&2
  status=1
fi

echo "Checking strict typed-error regime in core operations modules..."
core_ops_stringly_hits="$(
  search_hits 'from_external_message\(' \
    src/commands/fs/delete_ops.rs \
    src/commands/fs/trash/mod.rs \
    src/commands/rename/mod.rs \
    src/clipboard/mod.rs \
    src/clipboard/ops.rs || true
)"
if [[ -n "${core_ops_stringly_hits}" ]]; then
  echo "error: stringly from_external_message() usage is forbidden in core operations modules:" >&2
  echo "${core_ops_stringly_hits}" >&2
  status=1
fi

core_ops_literal_err_hits="$(
  search_hits 'Err\(".*"\.into\(\)\)|Err\(".*"\)' \
    src/commands/fs/delete_ops.rs \
    src/commands/fs/trash/mod.rs \
    src/commands/rename/mod.rs \
    src/clipboard/mod.rs \
    src/clipboard/ops.rs || true
)"
if [[ -n "${core_ops_literal_err_hits}" ]]; then
  echo "error: string-literal Err(...) is forbidden in core operations modules; use typed domain errors:" >&2
  echo "${core_ops_literal_err_hits}" >&2
  status=1
fi

if [[ "${status}" -ne 0 ]]; then
  echo "backend error hardening guard failed" >&2
  exit "${status}"
fi

echo "backend error hardening guard passed"
