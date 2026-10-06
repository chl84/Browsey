#!/usr/bin/env bash
set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
state_root="$script_dir/state"
provider_types_root="$script_dir/provider-types"
log_file="$script_dir/fake-rclone.log"
mkdir_destination_exists_once_file="$script_dir/mkdir-destination-exists-once"
mkdir_destination_exists_always_file="$script_dir/mkdir-destination-exists-always"
config_dump_fail_file="$script_dir/config-dump-fail"
mkdir -p "$state_root" "$provider_types_root"

maybe_delay_subcommand() {
  local subcmd="$1"
  local counter_file="$script_dir/${subcmd}-count"
  local delay_ms_file="$script_dir/${subcmd}-delay-ms"
  local delay_invocation_file="$script_dir/${subcmd}-delay-invocation"
  local notify_file="$script_dir/${subcmd}-delay-notify"
  local invocation=1
  if [[ -f "$counter_file" ]]; then
    invocation="$(( $(cat "$counter_file") + 1 ))"
  fi
  printf '%s\n' "$invocation" > "$counter_file"
  if [[ ! -f "$delay_ms_file" || ! -f "$delay_invocation_file" ]]; then
    return
  fi
  local target_invocation
  target_invocation="$(tr -d '[:space:]' < "$delay_invocation_file")"
  if [[ "$invocation" != "$target_invocation" ]]; then
    return
  fi
  local delay_ms
  delay_ms="$(tr -d '[:space:]' < "$delay_ms_file")"
  if [[ -f "$notify_file" ]]; then
    rm -f -- "$notify_file"
  fi
  : > "$notify_file"
  python3 - "$delay_ms" <<'PY'
import sys
import time

time.sleep(int(sys.argv[1]) / 1000.0)
PY
}

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
    if [[ -f "$config_dump_fail_file" ]]; then
      echo "forced config dump failure" >&2
      exit 3
    fi
    printf '{'
    first=1
    shopt -s nullglob
    for d in "$state_root"/*; do
      [[ -d "$d" ]] || continue
      remote_name="$(basename -- "$d")"
      remote_type="onedrive"
      remote_type_file="$provider_types_root/$remote_name"
      if [[ -f "$remote_type_file" ]]; then
        remote_type="$(tr -d '\r\n' < "$remote_type_file")"
        if [[ -z "$remote_type" ]]; then
          remote_type="onedrive"
        fi
      fi
      if [[ $first -eq 0 ]]; then
        printf ','
      fi
      first=0
      printf '"%s":{"type":"%s"}' "$(json_escape "$remote_name")" "$(json_escape "$remote_type")"
    done
    printf '}\n'
    ;;
  lsjson)
    maybe_delay_subcommand "$subcmd"
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
      # Opt-in alias lookup models OneDrive's case-insensitive metadata without
      # changing the case-sensitive fixture storage or transfer destination.
      if [[ ! -e "$target" && -f "$script_dir/stat-case-insensitive" ]]; then
        target="$(python3 - "$target" <<'PY_ALIAS'
import pathlib
import sys
path = pathlib.Path(sys.argv[1])
if path.parent.is_dir():
    matches = [entry for entry in path.parent.iterdir() if entry.name.casefold() == path.name.casefold()]
    if len(matches) == 1:
        path = matches[0]
print(path)
PY_ALIAS
)"
      fi
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
    while [[ $idx -lt ${#args[@]} ]]; do
      case "${args[$idx]}" in
        --onedrive-hard-delete|--drive-use-trash=false)
          idx=$((idx + 1))
          ;;
        *)
          break
          ;;
      esac
    done
    if [[ $idx -ge ${#args[@]} ]]; then
      echo "missing path for mkdir" >&2
      exit 2
    fi
    if [[ -f "$mkdir_destination_exists_always_file" ]]; then
      echo "destination exists" >&2
      exit 3
    fi
    if [[ -f "$mkdir_destination_exists_once_file" ]]; then
      rm -f -- "$mkdir_destination_exists_once_file"
      echo "destination exists" >&2
      exit 3
    fi
    target="$(map_spec_path "${args[$idx]}")"
    mkdir -p -- "$target"
    ;;
  deletefile)
    trash_mode=0
    while [[ $idx -lt ${#args[@]} ]]; do
      case "${args[$idx]}" in
        --onedrive-hard-delete=false|--drive-use-trash=true)
          trash_mode=1
          idx=$((idx + 1))
          ;;
        --onedrive-hard-delete|--drive-use-trash=false)
          idx=$((idx + 1))
          ;;
        *)
          break
          ;;
      esac
    done
    if [[ $idx -ge ${#args[@]} ]]; then
      echo "missing path for deletefile" >&2
      exit 2
    fi
    target="$(map_spec_path "${args[$idx]}")"
    if [[ ! -f "$target" ]]; then
      echo "file not found" >&2
      exit 3
    fi
    if [[ "$trash_mode" -eq 1 ]]; then
      mkdir -p -- "$state_root/.trash"
      mv -- "$target" "$state_root/.trash/$(basename -- "$target")"
    else
      rm -f -- "$target"
    fi
    ;;
  purge)
    trash_mode=0
    while [[ $idx -lt ${#args[@]} ]]; do
      case "${args[$idx]}" in
        --onedrive-hard-delete=false|--drive-use-trash=true)
          trash_mode=1
          idx=$((idx + 1))
          ;;
        --onedrive-hard-delete|--drive-use-trash=false)
          idx=$((idx + 1))
          ;;
        *)
          break
          ;;
      esac
    done
    if [[ $idx -ge ${#args[@]} ]]; then
      echo "missing path for purge" >&2
      exit 2
    fi
    target="$(map_spec_path "${args[$idx]}")"
    if [[ ! -e "$target" ]]; then
      echo "directory not found" >&2
      exit 3
    fi
    if [[ "$trash_mode" -eq 1 ]]; then
      mkdir -p -- "$state_root/.trash"
      mv -- "$target" "$state_root/.trash/$(basename -- "$target")"
    else
      rm -rf -- "$target"
    fi
    ;;
  rmdir)
    while [[ $idx -lt ${#args[@]} ]]; do
      case "${args[$idx]}" in
        --retries|--low-level-retries)
          idx=$((idx + 2))
          ;;
        --onedrive-hard-delete|--drive-use-trash=false)
          idx=$((idx + 1))
          ;;
        *)
          break
          ;;
      esac
    done
    if [[ $idx -ge ${#args[@]} ]]; then
      echo "missing path for rmdir" >&2
      exit 2
    fi
    target="$(map_spec_path "${args[$idx]}")"
    rmdir -- "$target"
    ;;
  copy|copyto|move|moveto)
    immutable=0
    ignore_times=0
    ignore_existing=0
    error_on_no_transfer=0
    create_empty_dirs=0
    delete_empty_dirs=0
    transfer_paths=()
    while [[ $idx -lt ${#args[@]} ]]; do
      case "${args[$idx]}" in
        --immutable) immutable=1; idx=$((idx + 1)) ;;
        --ignore-times) ignore_times=1; idx=$((idx + 1)) ;;
        --checksum) idx=$((idx + 1)) ;;
        --retries|--low-level-retries) idx=$((idx + 2)) ;;
        --ignore-existing) ignore_existing=1; idx=$((idx + 1)) ;;
        --error-on-no-transfer) error_on_no_transfer=1; idx=$((idx + 1)) ;;
        --create-empty-src-dirs)
          if [[ "$subcmd" != copy && "$subcmd" != move ]]; then
            echo "Error: unknown flag: --create-empty-src-dirs" >&2
            exit 2
          fi
          create_empty_dirs=1
          idx=$((idx + 1))
          ;;
        --delete-empty-src-dirs)
          if [[ "$subcmd" != move ]]; then
            echo "Error: unknown flag: --delete-empty-src-dirs" >&2
            exit 2
          fi
          delete_empty_dirs=1
          idx=$((idx + 1))
          ;;
        -*)
          echo "Error: unknown flag: ${args[$idx]}" >&2
          exit 2
          ;;
        *) transfer_paths+=("${args[$idx]}"); idx=$((idx + 1)) ;;
      esac
    done
    if [[ ${#transfer_paths[@]} -ne 2 ]]; then
      echo "expected exactly two src/dst paths for $subcmd" >&2
      exit 2
    fi
    maybe_delay_subcommand "$subcmd"
    if [[ "$subcmd" == move && -f "$script_dir/move-fail-invocation" && "$(cat "$script_dir/move-count")" == "$(cat "$script_dir/move-fail-invocation")" ]]; then
      echo "forced move failure" >&2
      exit 3
    fi
    if [[ "$subcmd" == moveto && -f "$script_dir/moveto-fail-invocation" && "$(cat "$script_dir/moveto-count")" == "$(cat "$script_dir/moveto-fail-invocation")" ]]; then
      echo "forced moveto failure" >&2
      exit 3
    fi
    if [[ -f "$script_dir/transfer-failure" ]]; then
      head -c 512 -- "$script_dir/transfer-failure" >&2
      exit 3
    fi
    src="$(map_spec_path "${transfer_paths[0]}")"
    dst="$(map_spec_path "${transfer_paths[1]}")"
    if [[ "$subcmd" == moveto && -d "$src" && -f "$script_dir/stat-case-insensitive" && "$src" != "$dst" && "${src,,}" == "${dst,,}" ]]; then
      echo "can't sync or move files on overlapping remotes" >&2
      exit 3
    fi
    if [[ "$ignore_existing" -eq 1 && -e "$dst" ]]; then
      if [[ "$error_on_no_transfer" -eq 1 ]]; then
        echo "No files transferred" >&2
        exit 9
      fi
      exit 0
    fi
    # The real single-file CopyFile path does not enforce --immutable alone.
    if [[ "$subcmd" == copy && "$immutable" -eq 1 && -e "$dst" ]]; then
      echo "destination exists (immutable)" >&2
      exit 3
    fi
    if [[ ! -e "$src" ]]; then
      echo "object not found" >&2
      exit 3
    fi
    # Opt-in regression model of rclone's size/mtime quick check, not a fake content check.
    if [[ -f "$script_dir/metadata-fast-check" && "$ignore_times" -eq 0 && -f "$src" && -f "$dst" && ( "$subcmd" == copy || "$subcmd" == copyto ) ]]; then
      if [[ "$(stat -c '%s:%Y' -- "$src")" == "$(stat -c '%s:%Y' -- "$dst")" ]]; then
        exit 0
      fi
    fi
    mkdir -p -- "$(dirname -- "$dst")"
    if [[ "$subcmd" == copy || "$subcmd" == copyto ]]; then
      if [[ -d "$src" ]]; then
        # Like real rclone, an entirely empty source has no contents to copy;
        # --create-empty-src-dirs handles descendants, not the source root.
        if [[ -z "$(find "$src" -mindepth 1 -print -quit)" ]]; then
          exit 0
        fi
        mkdir -p -- "$dst"
        if [[ "$create_empty_dirs" -eq 1 ]]; then
          cp -R -- "$src/." "$dst/"
        else
          # Ignore empty source directories, never delete existing destination
          # directories: copy/copyto are not sync.
          while IFS= read -r -d '' source_file; do
            relative_file="${source_file#"$src/"}"
            mkdir -p -- "$(dirname -- "$dst/$relative_file")"
            cp -f -- "$source_file" "$dst/$relative_file"
          done < <(find "$src" -type f -print0)
        fi
      elif [[ "$subcmd" == copy ]]; then
        mkdir -p -- "$dst"
        cp -f -- "$src" "$dst/"
      else
        cp -f -- "$src" "$dst"
      fi
    elif [[ -d "$src" && ( "$subcmd" == move || "${transfer_paths[0]}" != *:* || "${transfer_paths[1]}" != *:* ) ]]; then
      # A cross-backend move transfers files rather than renaming the root.
      # moveto lacks empty-directory flags and retains emptied source dirs.
      if [[ "$create_empty_dirs" -eq 1 ]]; then
        mkdir -p -- "$dst"
        while IFS= read -r -d '' source_dir; do
          mkdir -p -- "$dst/${source_dir#"$src/"}"
        done < <(find "$src" -mindepth 1 -type d -print0)
      fi
      while IFS= read -r -d '' source_file; do
        relative_file="${source_file#"$src/"}"
        mkdir -p -- "$(dirname -- "$dst/$relative_file")"
        mv -f -- "$source_file" "$dst/$relative_file"
      done < <(find "$src" -type f -print0)
      if [[ "$delete_empty_dirs" -eq 1 ]]; then
        # Like rclone, clean descendants but leave its filesystem root.
        find "$src" -mindepth 1 -depth -type d -empty -delete
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
