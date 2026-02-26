#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
state_root="$script_dir/state"
log_file="$script_dir/fake-rclone.log"
mkdir -p "$state_root"

printf '%s\n' "$*" >> "$log_file"

args=("$@")
idx=0
while [[ $idx -lt ${#args[@]} ]]; do
  case "${args[$idx]}" in
    --retries|--low-level-retries|--stats)
      idx=$((idx + 2))
      ;;
    *)
      break
      ;;
  esac
done

if [[ $idx -ge ${#args[@]} ]]; then
  echo "missing subcommand" >&2
  exit 2
fi

subcmd="${args[$idx]}"
idx=$((idx + 1))

map_spec_path() {
  local spec="$1"
  if [[ "$spec" != *:* ]]; then
    printf '%s' "$spec"
    return
  fi
  local remote="${spec%%:*}"
  local rel=""
  rel="${spec#*:}"
  if [[ -z "$remote" ]]; then
    echo "invalid remote spec" >&2
    exit 2
  fi
  local path="$state_root/$remote"
  if [[ -n "$rel" ]]; then
    path="$path/$rel"
  fi
  printf '%s' "$path"
}

json_escape() {
  local s="$1"
  s="${s//\\/\\\\}"
  s="${s//\"/\\\"}"
  s="${s//$'\n'/\\n}"
  printf '%s' "$s"
}

emit_item_json() {
  local path="$1"
  local name
  name="$(basename -- "$path")"
  if [[ -d "$path" ]]; then
    printf '{"Name":"%s","IsDir":true,"Size":0}' "$(json_escape "$name")"
  else
    local size
    size="$(wc -c < "$path" | tr -d '[:space:]')"
    printf '{"Name":"%s","IsDir":false,"Size":%s}' "$(json_escape "$name")" "$size"
  fi
}

case "$subcmd" in
  version)
    echo "rclone v1.69.1"
    echo "- os/version: fake"
    ;;
  listremotes)
    if [[ -d "$state_root" ]]; then
      shopt -s nullglob
      for d in "$state_root"/*; do
        [[ -d "$d" ]] || continue
        printf '%s:\n' "$(basename -- "$d")"
      done
    fi
    ;;
  config)
    if [[ $idx -ge ${#args[@]} || "${args[$idx]}" != "dump" ]]; then
      echo "unsupported config command" >&2
      exit 2
    fi
    idx=$((idx + 1))
    printf '{'
    first=1
    shopt -s nullglob
    for d in "$state_root"/*; do
      [[ -d "$d" ]] || continue
      remote_name="$(basename -- "$d")"
      if [[ $first -eq 0 ]]; then
        printf ','
      fi
      first=0
      printf '"%s":{"type":"onedrive"}' "$(json_escape "$remote_name")"
    done
    printf '}\n'
    ;;
  lsjson)
    want_stat=0
    if [[ $idx -lt ${#args[@]} && "${args[$idx]}" == "--stat" ]]; then
      want_stat=1
      idx=$((idx + 1))
    fi
    if [[ $idx -ge ${#args[@]} ]]; then
      echo "missing path for lsjson" >&2
      exit 2
    fi
    target="$(map_spec_path "${args[$idx]}")"
    if [[ $want_stat -eq 1 ]]; then
      if [[ ! -e "$target" ]]; then
        echo "object not found" >&2
        exit 3
      fi
      emit_item_json "$target"
      printf '\n'
      exit 0
    fi
    if [[ ! -d "$target" ]]; then
      echo "directory not found" >&2
      exit 3
    fi
    shopt -s nullglob dotglob
    printf '['
    first=1
    for child in "$target"/* "$target"/.*; do
      base="$(basename -- "$child")"
      [[ "$base" == "." || "$base" == ".." ]] && continue
      if [[ $first -eq 0 ]]; then
        printf ','
      fi
      first=0
      emit_item_json "$child"
    done
    printf ']\n'
    ;;
  mkdir)
    if [[ $idx -ge ${#args[@]} ]]; then
      echo "missing path for mkdir" >&2
      exit 2
    fi
    target="$(map_spec_path "${args[$idx]}")"
    mkdir -p -- "$target"
    ;;
  deletefile)
    if [[ $idx -ge ${#args[@]} ]]; then
      echo "missing path for deletefile" >&2
      exit 2
    fi
    target="$(map_spec_path "${args[$idx]}")"
    if [[ ! -f "$target" ]]; then
      echo "file not found" >&2
      exit 3
    fi
    rm -f -- "$target"
    ;;
  purge)
    if [[ $idx -ge ${#args[@]} ]]; then
      echo "missing path for purge" >&2
      exit 2
    fi
    target="$(map_spec_path "${args[$idx]}")"
    if [[ ! -e "$target" ]]; then
      echo "directory not found" >&2
      exit 3
    fi
    rm -rf -- "$target"
    ;;
  rmdir)
    if [[ $idx -ge ${#args[@]} ]]; then
      echo "missing path for rmdir" >&2
      exit 2
    fi
    target="$(map_spec_path "${args[$idx]}")"
    rmdir -- "$target"
    ;;
  copyto|moveto)
    if (( idx + 1 >= ${#args[@]} )); then
      echo "missing src/dst for $subcmd" >&2
      exit 2
    fi
    src="$(map_spec_path "${args[$idx]}")"
    dst="$(map_spec_path "${args[$idx + 1]}")"
    if [[ ! -e "$src" ]]; then
      echo "object not found" >&2
      exit 3
    fi
    mkdir -p -- "$(dirname -- "$dst")"
    if [[ "$subcmd" == "copyto" ]]; then
      if [[ -d "$src" ]]; then
        rm -rf -- "$dst"
        cp -R -- "$src" "$dst"
      else
        cp -f -- "$src" "$dst"
      fi
    else
      rm -rf -- "$dst"
      mv -- "$src" "$dst"
    fi
    ;;
  *)
    echo "unsupported fake-rclone subcommand: $subcmd" >&2
    exit 2
    ;;
esac
