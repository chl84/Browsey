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

/cloud command timing/ {
  op = ""
  ms = ""
  if (match($0, /op=([A-Za-z0-9_]+)/, m)) op = m[1]
  if (match($0, /elapsed_ms=([0-9]+)/, t)) ms = t[1] + 0
  if (op != "" && ms != "") {
    op_count[op] += 1
    op_sum_ms[op] += ms
    if (ms > op_max_ms[op]) op_max_ms[op] = ms
  }
}

/rclone command succeeded/ && /command=lsjson/ {
  if ($0 ~ /stat=true/) {
    lsjson_stat += 1
  } else if ($0 ~ /stat=false/) {
    lsjson_list += 1
  } else {
    lsjson_unknown += 1
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
}
' "$LOG_PATH"
