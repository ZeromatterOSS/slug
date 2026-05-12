#!/usr/bin/env bash
# Plan 16.8: benchmark harness.
#
# Drive a build target N times (cold, warm, or both), emit a JSON
# BuildSummary rollup per run, and save the raw event-log alongside so
# `slug log diff summary` can compare across runs.
#
# Output layout:
#   benchmarks/<YYYY-MM-DD>-<git-sha>/<slug>/
#     cold-01/{summary.json,build.pb.zst,wall.txt}
#     cold-02/...
#     warm-01/...
#
# Cold runs invoke `slug kill` and (optionally) drop OS caches between
# iterations. Warm runs reuse the daemon and source tree caches.
#
# Usage:
#   run.sh --target '@llvm-project//clang:clang' [--runs 3] \
#          [--cold|--warm|--both] [--drop-caches] [--out DIR] \
#          [--slug BIN] [--workspace DIR]
#
# All options can be repeated for --target (multi-target batch run).

set -euo pipefail

RUNS=3
MODE=both
DROP_CACHES=0
OUT_ROOT=
SLUG_BIN="${SLUG:-$(command -v slug || true)}"
WORKSPACE=
TARGETS=()

usage() {
  sed -n '1,40p' "$0" >&2
  exit 1
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --target) TARGETS+=("$2"); shift 2;;
    --runs) RUNS="$2"; shift 2;;
    --cold) MODE=cold; shift;;
    --warm) MODE=warm; shift;;
    --both) MODE=both; shift;;
    --drop-caches) DROP_CACHES=1; shift;;
    --out) OUT_ROOT="$2"; shift 2;;
    --slug) SLUG_BIN="$2"; shift 2;;
    --workspace) WORKSPACE="$2"; shift 2;;
    -h|--help) usage;;
    *) echo "unknown arg: $1" >&2; usage;;
  esac
done

if [[ ${#TARGETS[@]} -eq 0 ]]; then
  echo "error: at least one --target required" >&2
  usage
fi
if [[ -z "$SLUG_BIN" ]]; then
  echo "error: pass --slug or set SLUG env var" >&2
  exit 1
fi
if [[ ! -x "$SLUG_BIN" ]]; then
  echo "error: slug binary not executable: $SLUG_BIN" >&2
  exit 1
fi

if [[ -z "$OUT_ROOT" ]]; then
  sha=$(git -C "$(dirname "$(realpath "$SLUG_BIN")")" rev-parse --short HEAD 2>/dev/null || echo unknown)
  date=$(date +%Y-%m-%d)
  OUT_ROOT="benchmarks/${date}-${sha}"
fi
mkdir -p "$OUT_ROOT"

drop_caches() {
  if [[ "$DROP_CACHES" != 1 ]]; then
    return 0
  fi
  # Root-only operation; don't silently skip if the user asked for it.
  if [[ $EUID -ne 0 ]] && ! command -v sudo >/dev/null; then
    echo "warn: --drop-caches requires root or sudo; skipping" >&2
    return 0
  fi
  echo "  dropping OS caches (requires sudo)..." >&2
  sync
  if [[ $EUID -eq 0 ]]; then
    echo 3 >/proc/sys/vm/drop_caches
  else
    echo 3 | sudo tee /proc/sys/vm/drop_caches >/dev/null
  fi
}

slug_kill() {
  # slug kill may fail if no daemon is running. That's fine.
  "$SLUG_BIN" kill >/dev/null 2>&1 || true
}

slugify() {
  # //foo:bar → foo_bar ; @ext//a/b:c → ext_a_b_c
  echo "$1" | sed -E 's@^[@]+@@; s@//@_@g; s@/@_@g; s@:@_@g; s@[^A-Za-z0-9_.+-]@_@g'
}

workspace_cwd() {
  if [[ -n "$WORKSPACE" ]]; then
    printf '%s' "$WORKSPACE"
  else
    pwd
  fi
}

run_one() {
  local target="$1" label="$2" rundir="$3"
  mkdir -p "$rundir"

  local ws
  ws=$(workspace_cwd)

  local start_ns end_ns wall_s
  start_ns=$(date +%s%N)
  # Capture both stdout/stderr into build.log; let the user peek when
  # numbers look weird.
  if ! (cd "$ws" && "$SLUG_BIN" build "$target") >"$rundir/build.log" 2>&1; then
    echo "  [$label] build failed — see $rundir/build.log" >&2
    tail -20 "$rundir/build.log" >&2 || true
    return 1
  fi
  end_ns=$(date +%s%N)
  wall_s=$(awk -v s="$start_ns" -v e="$end_ns" 'BEGIN{printf "%.3f", (e-s)/1e9}')
  printf '%s\n' "$wall_s" >"$rundir/wall.txt"

  # Copy the most recent event log to a stable name for later diffing.
  local log_src
  log_src=$(ls -t "$ws/buck-out/v2/log/"*.pb.zst 2>/dev/null | head -n1 || true)
  if [[ -n "$log_src" ]]; then
    cp "$log_src" "$rundir/build.pb.zst"
    "$SLUG_BIN" log summary --format json "$rundir/build.pb.zst" \
      >"$rundir/summary.json" 2>"$rundir/summary.err" || {
      echo "  [$label] slug log summary failed — see $rundir/summary.err" >&2
    }
  else
    echo "  [$label] no event log found under $ws/buck-out/v2/log" >&2
  fi
  echo "  [$label] wall=${wall_s}s"
}

for target in "${TARGETS[@]}"; do
  slug=$(slugify "$target")
  tdir="$OUT_ROOT/$slug"
  mkdir -p "$tdir"
  echo "target: $target" | tee "$tdir/target.txt"

  if [[ "$MODE" == cold || "$MODE" == both ]]; then
    for i in $(seq 1 "$RUNS"); do
      label=$(printf 'cold-%02d' "$i")
      echo "  $label"
      slug_kill
      drop_caches
      run_one "$target" "$label" "$tdir/$label"
    done
  fi

  if [[ "$MODE" == warm || "$MODE" == both ]]; then
    # Prime the daemon + cache once before warm measurements (unless we
    # just did a cold pass, in which case the state is already warm).
    if [[ "$MODE" == warm ]]; then
      (cd "$(workspace_cwd)" && "$SLUG_BIN" build "$target") >/dev/null 2>&1 || true
    fi
    for i in $(seq 1 "$RUNS"); do
      label=$(printf 'warm-%02d' "$i")
      echo "  $label"
      run_one "$target" "$label" "$tdir/$label"
    done
  fi
done

echo "done. output under: $OUT_ROOT"
