#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"
version=$(grep '^version' Cargo.toml | head -n1 | cut -d '"' -f2)
rpm_file="Browsey-${version}-1.x86_64.rpm"
bundle_path="target/release/bundle/rpm/$rpm_file"
if [[ -f "$rpm_file" ]]; then
  pkg="$rpm_file"
elif [[ -f "$bundle_path" ]]; then
  pkg="$bundle_path"
else
  echo "RPM not found: $rpm_file (expected in repo root or $bundle_path)" >&2
  exit 1
fi
exec sudo rpm -Uvh --replacepkgs "$pkg"
