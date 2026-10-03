#!/usr/bin/env bash
set -euo pipefail

ROOT="$(cd -- "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
PROFILE=release
PARENT="${TMPDIR:-/tmp}"
DRY_RUN=0
while (($#)); do
  case "$1" in
    --debug) PROFILE=debug; shift ;;
    --directory) [[ $# -ge 2 ]] || exit 2; PARENT="$2"; shift 2 ;;
    --dry-run) DRY_RUN=1; shift ;;
    -h|--help)
      printf '%s\n' 'Usage: bash scripts/dev/performance-workloads.sh [--debug] [--directory EXISTING_PARENT] [--dry-run]' \
        'Creates disposable local workloads only. Does not reset OS caches, use cloud accounts, install or restart Browsey.'
      exit 0 ;;
    *) printf 'Unknown argument: %s\n' "$1" >&2; exit 2 ;;
  esac
done
[[ -d "$PARENT" && -w "$PARENT" ]] || { printf 'Fixture parent must be an existing writable directory.\n' >&2; exit 2; }
PARENT="$(cd -- "$PARENT" && pwd -P)"
if [[ -x "${HOME}/.cargo/bin/cargo" ]]; then
  export PATH="${HOME}/.cargo/bin:${PATH}"
fi
TESTS=(
  undo::tests::measurements::recovery_storage_workloads
  commands::listing::local::measurements::large_directory_workloads
  commands::search::worker::measurements::recursive_search_workloads
  commands::thumbnails::measurements::mixed_thumbnail_workloads
  clipboard::tests::measurements::controlled_slow_copy_cancellation
)
ARGS=(--locked --offline)
[[ "$PROFILE" == debug ]] || ARGS+=(--release)
cd "$ROOT"
printf 'Source: %s; profile: %s; filesystem: %s\n' "$(git rev-parse HEAD)" "$PROFILE" "$(stat -f -c %T "$PARENT")"
[[ -z "$(git status --porcelain)" ]] || printf '%s\n' 'Working tree has changes; record them with the results.'
if ((DRY_RUN)); then
  printf 'Would run exactly: %s\n' "${TESTS[@]}"
  exit 0
fi
AVAILABLE_KIB="$(df -Pk -- "$PARENT" | awk 'NR == 2 {print $4}')"
[[ "$AVAILABLE_KIB" =~ ^[0-9]+$ && "$AVAILABLE_KIB" -ge 1048576 ]] || {
  printf 'At least 1 GiB available fixture space is required.\n' >&2; exit 1;
}
WORK="$(mktemp -d "$PARENT/browsey-performance.XXXXXX")"
export TMPDIR="$WORK"
export XDG_DATA_HOME="$WORK/data" XDG_CONFIG_HOME="$WORK/config"
export XDG_CACHE_HOME="$WORK/cache" XDG_STATE_HOME="$WORK/state"
trap 'if ! rmdir -- "$WORK" 2>/dev/null; then printf "Fixture retained for inspection: %s\n" "$WORK" >&2; fi' EXIT
for TEST in "${TESTS[@]}"; do
  cargo test "${ARGS[@]}" "$TEST" -- --ignored --exact "$TEST" --nocapture --test-threads=1
done
