#!/usr/bin/env bash
set -euo pipefail

NATIVE_REPO="$(cd -- "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$NATIVE_REPO"

if [[ "${1:---help}" == "--build" ]]; then
  if [[ $# -ne 1 ]]; then
    printf '%s\n' 'Build takes no target/credential arguments.' >&2
    exit 2
  fi
  if [[ -n "${BROWSEY_CARGO:-}" ]]; then
    # Optional explicit working Rust toolchain, e.g. when a mise shim is unset.
    export PATH="$(dirname -- "$BROWSEY_CARGO"):$PATH"
  fi
  node frontend/e2e-native/stage.mjs --prepare
  frontend/node_modules/.bin/tauri build --debug --no-bundle --features native-test -- --locked
  node frontend/e2e-native/stage.mjs
else
  exec node frontend/e2e-native/run.mjs "$@"
fi
