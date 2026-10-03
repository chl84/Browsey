#!/usr/bin/env sh
# Resolve the adjacent installation, without baking a personal home path into it.
set -eu
BROWSEY_PREFIX=$(CDPATH= cd -- "$(dirname -- "$0")/../opt/browsey/usr" && pwd)
exec "$BROWSEY_PREFIX/bin/browsey" "$@"
