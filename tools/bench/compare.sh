#!/usr/bin/env bash
# Plan 16.8: compare two benchmark runs produced by run.sh.
#
# For each target slug present in both baseline and current, invoke
# `slug log diff summary` on the first matching run of each mode
# (cold-01, warm-01). Emits a consolidated report plus optional CI
# gate.
#
# Usage:
#   compare.sh --baseline benchmarks/2026-04-22-abc/ \
#              --current  benchmarks/2026-04-23-def/ \
#              [--threshold 5.0] [--fail-on-regression] \
#              [--slug BIN]

set -euo pipefail

BASELINE=
CURRENT=
THRESHOLD=5.0
FAIL_ON_REGRESSION=0
SLUG_BIN="${SLUG:-$(command -v slug || true)}"

usage() {
  sed -n '1,20p' "$0" >&2
  exit 1
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --baseline) BASELINE="$2"; shift 2;;
    --current) CURRENT="$2"; shift 2;;
    --threshold) THRESHOLD="$2"; shift 2;;
    --fail-on-regression) FAIL_ON_REGRESSION=1; shift;;
    --slug) SLUG_BIN="$2"; shift 2;;
    -h|--help) usage;;
    *) echo "unknown arg: $1" >&2; usage;;
  esac
done

if [[ -z "$BASELINE" || -z "$CURRENT" ]]; then
  echo "error: --baseline and --current required" >&2
  usage
fi
if [[ -z "$SLUG_BIN" ]]; then
  echo "error: pass --slug or set SLUG env var" >&2
  exit 1
fi

any_regression=0

for baseline_target in "$BASELINE"/*/; do
  slug=$(basename "$baseline_target")
  current_target="$CURRENT/$slug"
  [[ -d "$current_target" ]] || {
    echo "skip: $slug (missing in current)"
    continue
  }

  for mode in cold warm; do
    base_log=$(ls "$baseline_target/${mode}-"*/build.pb.zst 2>/dev/null | head -n1 || true)
    curr_log=$(ls "$current_target/${mode}-"*/build.pb.zst 2>/dev/null | head -n1 || true)
    [[ -n "$base_log" && -n "$curr_log" ]] || continue

    echo "=== $slug [$mode] ==="
    args=(log diff summary --path1 "$base_log" --path2 "$curr_log" --threshold "$THRESHOLD")
    if [[ "$FAIL_ON_REGRESSION" == 1 ]]; then
      args+=(--fail-on-regression)
    fi
    if ! "$SLUG_BIN" "${args[@]}"; then
      any_regression=1
    fi
    echo
  done
done

if [[ "$FAIL_ON_REGRESSION" == 1 && "$any_regression" == 1 ]]; then
  exit 1
fi
