#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
version=$(grep '^version' Cargo.toml | head -n1 | cut -d '"' -f2)
rpm_file="Browsey-${version}-1.x86_64.rpm"
if [[ ! -f "$rpm_file" ]]; then
  echo "RPM not found: $rpm_file" >&2
  exit 1
fi
exec sudo rpm -Uvh --replacepkgs "$rpm_file"
