#!/usr/bin/env bash
set -euo pipefail

LOG_PATH="${1:-$HOME/.local/share/browsey/logs/browsey.log}"

if [[ ! -f "$LOG_PATH" ]]; then
  echo "Log file not found: $LOG_PATH" >&2
  exit 1
fi

echo "Browsey cloud/rclone perf summary"
echo "Log: $LOG_PATH"
echo

awk '
function print_op_stats(op,   count, sum, max, avg) {
  count = op_count[op]
  if (count == 0) return
  sum = op_sum_ms[op]
  max = op_max_ms[op]
  avg = sum / count
  printf("  %-32s count=%-4d avg_ms=%-8.1f max_ms=%d\n", op, count, avg, max)
}

function print_rc_stats(method,   count, sum, max, avg, ok, fail) {
  count = rc_count[method]
  if (count == 0) return
  sum = rc_sum_ms[method]
  max = rc_max_ms[method]
  avg = sum / count
  ok = rc_ok_count[method] + 0
  fail = rc_fail_count[method] + 0
  printf("  %-24s count=%-4d avg_ms=%-8.1f max_ms=%-6d ok=%-4d fail=%d\n", method, count, avg, max, ok, fail)
}

function print_backend_stats(key,   split_pos, op, backend, count, fallback, pct_fallback) {
  count = backend_count[key]
  if (count == 0) return
  split_pos = index(key, SUBSEP)
  op = substr(key, 1, split_pos - 1)
  backend = substr(key, split_pos + 1)
  fallback = backend_fallback_count[key] + 0
  pct_fallback = (count > 0) ? (fallback * 100.0 / count) : 0
  printf("  %-24s backend=%-4s count=%-4d fallback=%-4d (%.1f%%)\n", op, backend, count, fallback, pct_fallback)
}

/cloud command timing/ {
  op = ""
  ms = ""
  if (match($0, /op="?([A-Za-z0-9_]+)"?/, m)) op = m[1]
  if (match($0, /elapsed_ms=([0-9]+)/, t)) ms = t[1] + 0
  if (op != "" && ms != "") {
    op_count[op] += 1
    op_sum_ms[op] += ms
    if (ms > op_max_ms[op]) op_max_ms[op] = ms
  }
}

/rclone command succeeded/ && /command="?lsjson"?/ {
  if ($0 ~ /stat=true/) {
    lsjson_stat += 1
  } else if ($0 ~ /stat=false/) {
    lsjson_list += 1
  } else {
    lsjson_unknown += 1
  }
}

/rclone rc method completed/ {
  method = ""
  ms = ""
  ok = ""
  if (match($0, /method="?([^" ]+)"?/, m)) method = m[1]
  if (match($0, /elapsed_ms=([0-9]+)/, t)) ms = t[1] + 0
  if (match($0, /success=(true|false)/, s)) ok = s[1]
  if (method != "" && ms != "") {
    rc_count[method] += 1
    rc_sum_ms[method] += ms
    if (ms > rc_max_ms[method]) rc_max_ms[method] = ms
    if (ok == "true") {
      rc_ok_count[method] += 1
    } else if (ok == "false") {
      rc_fail_count[method] += 1
    }
  }
}

/cloud provider backend selected/ {
  op = ""
  backend = ""
  fallback = ""
  if (match($0, /op="?([A-Za-z0-9_]+)"?/, m)) op = m[1]
  if (match($0, /backend="?([A-Za-z0-9_]+)"?/, b)) backend = b[1]
  if (match($0, /fallback_from_rc=(true|false)/, f)) fallback = f[1]
  if (op != "" && backend != "") {
    key = op SUBSEP backend
    backend_count[key] += 1
    if (fallback == "true") backend_fallback_count[key] += 1
  }
}

END {
  print "Cloud command timings (from backend info logs):"
  if (length(op_count) == 0) {
    print "  (none found)"
  } else {
    for (op in op_count) {
      print_op_stats(op)
    }
  }
  print ""
  print "rclone lsjson success calls (from rclone_cli logs):"
  printf("  lsjson --stat       count=%d\n", lsjson_stat + 0)
  printf("  lsjson dir listing  count=%d\n", lsjson_list + 0)
  if ((lsjson_unknown + 0) > 0) {
    printf("  lsjson (unknown)    count=%d\n", lsjson_unknown + 0)
  }
  print ""
  print "rclone rc method timings (from rclone_rc logs):"
  if (length(rc_count) == 0) {
    print "  (none found)"
  } else {
    for (method in rc_count) {
      print_rc_stats(method)
    }
  }
  print ""
  print "Cloud provider backend usage (from provider logs):"
  if (length(backend_count) == 0) {
    print "  (none found)"
  } else {
    for (key in backend_count) {
      print_backend_stats(key)
    }
  }
}
' "$LOG_PATH"
