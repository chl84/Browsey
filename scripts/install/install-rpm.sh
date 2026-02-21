#!/usr/bin/env bash
set -euo pipefail
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "$ROOT"
version=$(grep '^version' Cargo.toml | head -n1 | cut -d '"' -f2)
rpm_candidates=(
  "Browsey-${version}.x86_64.rpm"
  "Browsey-${version}-1.x86_64.rpm"
)
pkg=""
for rpm_file in "${rpm_candidates[@]}"; do
  bundle_path="target/release/bundle/rpm/$rpm_file"
  if [[ -f "$rpm_file" ]]; then
    pkg="$rpm_file"
    break
  fi
  if [[ -f "$bundle_path" ]]; then
    pkg="$bundle_path"
    break
  fi
done
if [[ -z "$pkg" ]]; then
  echo "RPM not found. Checked: ${rpm_candidates[*]} (repo root and target/release/bundle/rpm/)" >&2
  exit 1
fi
exec sudo rpm -Uvh --replacepkgs "$pkg"
