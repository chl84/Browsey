#!/usr/bin/env bash
# Build the current checkout and install it for this user, without restarting it.
set -euo pipefail

repo_dir=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/../.." && pwd)
local_dir=${BROWSEY_LOCAL_DIR:-"$HOME/.local"}
dry_run=false
case "${1:-}" in
  '') ;;
  --dry-run) dry_run=true ;;
  -h|--help)
    printf '%s\n' 'Usage: bash scripts/install/install-local.sh [--dry-run]' \
      'Builds this checkout and installs under ~/.local/opt/browsey.' \
      'Keeps a rollback copy; does not commit, push, change defaults or restart Browsey.' \
      'BROWSEY_LOCAL_DIR overrides ~/.local (for isolated installations/tests).'
    exit 0
    ;;
  *) printf 'Unknown argument: %s\n' "$1" >&2; exit 2 ;;
esac
if [[ $# -gt 1 ]]; then
  printf '%s\n' 'Only one option is supported.' >&2
  exit 2
fi
if [[ $(uname -s) != Linux || $(uname -m) != x86_64 ]]; then
  printf '%s\n' 'This installer requires Linux x86_64 (the bundled PDFium target).' >&2
  exit 1
fi
if [[ $local_dir != /* || $local_dir == / || $local_dir == "$HOME" || $local_dir == "$repo_dir" ]]; then
  printf '%s\n' 'BROWSEY_LOCAL_DIR must be an absolute, dedicated local installation directory.' >&2
  exit 1
fi
# T3 terminals may put a mise cargo shim with no configured Rust ahead of rustup.
# Prefer the user's existing rustup installation without changing global settings.
if [[ -x $HOME/.cargo/bin/cargo ]]; then
  export PATH="$HOME/.cargo/bin:$PATH"
fi
for required_command in cargo node npm ldd flock tar install mktemp realpath; do
  if ! command -v "$required_command" >/dev/null 2>&1; then
    printf 'Missing command: %s\n' "$required_command" >&2
    exit 1
  fi
done
local_dir=$(realpath -m -- "$local_dir")
if [[ $local_dir == / || $local_dir == "$HOME" || $local_dir == "$repo_dir" ]]; then
  printf '%s\n' 'The resolved installation directory is too broad.' >&2
  exit 1
fi

install_dir="$local_dir/opt/browsey"
launcher="$local_dir/bin/browsey"
if [[ -L $install_dir || ( -e $install_dir && ! -d $install_dir ) ]]; then
  printf 'Refusing to replace a symlink or non-directory: %s\n' "$install_dir" >&2
  exit 1
fi
printf 'Source: %s\nInstallation: %s\n' "$repo_dir" "$install_dir"
if $dry_run; then
  printf '%s\n' 'Dry run: build with Tauri --no-bundle -- --locked; stage resources; back up and install.' \
    'No files changed. Running Browsey processes are never stopped.'
  exit 0
fi

mkdir -p -- "$local_dir/opt" "$local_dir/bin"
exec 9>"$local_dir/opt/.browsey-install.lock"
if ! flock -n 9; then
  printf '%s\n' 'Another Browsey installation is already running.' >&2
  exit 1
fi
cd -- "$repo_dir"
if [[ ! -x frontend/node_modules/.bin/tauri ]]; then
  npm --prefix frontend ci
fi
printf '%s\n' 'Building Browsey with the production frontend embedded…'
frontend/node_modules/.bin/tauri build --no-bundle -- --locked
binary="$repo_dir/target/release/browsey"
[[ -x $binary ]] || { printf '%s\n' 'Build did not produce an executable.' >&2; exit 1; }
linked_libraries=$(ldd "$binary")
if [[ $linked_libraries == *'not found'* ]]; then
  printf 'Missing runtime libraries:\n%s\n' "$linked_libraries" >&2
  exit 1
fi

staging_dir=''
launcher_temp=''
backup_dir=''
cleanup() {
  # Restore the old installation if interrupted between the two directory moves.
  if [[ -n $backup_dir && -d $backup_dir/app && ! -e $install_dir ]]; then
    mv -- "$backup_dir/app" "$install_dir" || printf 'Restore manually from: %s/app\n' "$backup_dir" >&2
  fi
  [[ -z $staging_dir || ! -d $staging_dir ]] || rm -rf -- "$staging_dir"
  [[ -z $launcher_temp || ! -f $launcher_temp ]] || rm -f -- "$launcher_temp"
}
trap cleanup EXIT
trap 'exit 130' INT
trap 'exit 143' TERM
staging_dir=$(mktemp -d "$local_dir/opt/.browsey-install.XXXXXX")
mkdir -p -- "$staging_dir/usr/bin" "$staging_dir/usr/lib/Browsey"
install -m 755 "$binary" "$staging_dir/usr/bin/browsey"
install -m 755 resources/pdfium-linux-x64/lib/libpdfium.so "$staging_dir/usr/bin/libpdfium.so"
tar --exclude='*.orig' -cf - resources THIRD_PARTY_NOTICES | tar -C "$staging_dir/usr/lib/Browsey" -xf -
# Keep existing desktop integration; this action does not change file associations.
if [[ -d $install_dir/usr/share ]]; then
  cp -a -- "$install_dir/usr/share" "$staging_dir/usr/"
fi
launcher_temp=$(mktemp "$local_dir/bin/.browsey-launcher.XXXXXX")
install -m 755 scripts/install/browsey-launcher.sh "$launcher_temp"
cmp -- "$binary" "$staging_dir/usr/bin/browsey"

if [[ -d $install_dir ]]; then
  backup_dir=$(mktemp -d "$local_dir/opt/.browsey-backup.XXXXXX")
  mv -- "$install_dir" "$backup_dir/app"
fi
mv -- "$staging_dir" "$install_dir"
staging_dir=''
mv -fT -- "$launcher_temp" "$launcher"
launcher_temp=''
cmp -- "$binary" "$install_dir/usr/bin/browsey"
printf 'Installed: %s\n' "$launcher"
[[ -z $backup_dir ]] || printf 'Previous installation retained: %s/app\n' "$backup_dir"
printf '%s\n' 'Open Browsey normally. If it is already running, finish active operations before restarting.'
