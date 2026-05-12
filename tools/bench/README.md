# Slug benchmark harness

Plan 16.8 tooling. Drives repeatable cold / warm measurements and
pairs them with `slug log diff summary` for before / after analysis.

## Quick start

```bash
# Cold + warm × 3 runs against the large canary target. Requires
# /var/mnt/dev/llvm-project/utils/bazel to be set up as a Slug workspace.
#
# BUCKD_STARTUP_TIMEOUT=180 is required on cold daemons — the 10s
# default is too tight when bundled-cell init has to run. Slug cold
# start takes 20–30s in practice.
BUCKD_STARTUP_TIMEOUT=180 SLUG=$(pwd)/target/debug/slug tools/bench/run.sh \
    --target '@llvm-project//clang:clang' --runs 3 --both \
    --workspace /var/mnt/dev/llvm-project/utils/bazel

# Output: benchmarks/<YYYY-MM-DD>-<sha>/llvm-project_clang_clang/{cold-01,...,warm-03}/

# Later, after a code change:
tools/bench/compare.sh \
    --baseline benchmarks/2026-04-22-abc123 \
    --current  benchmarks/2026-04-23-def456 \
    --threshold 3.0 --fail-on-regression
```

## Canary sizes

`tools/bench/targets.sh` exposes four size tiers:

| Size   | Target                                         | Scale       |
|--------|------------------------------------------------|-------------|
| small  | `@llvm-project//llvm:config`                   | seconds     |
| medium | `@llvm-project//clang:analysis_htmllogger_gen` | tens of s   |
| large  | `@llvm-project//clang:clang`                   | minutes     |
| xl     | `@llvm-project//llvm:llvm`                     | tens of min |

Pick the size that matches your CI budget. `large` is the canonical
Plan 17 measurement target — its numbers appear in every optimization
commit.

## Modes

- `--cold` — invoke `slug kill` before each run to force a fresh
  daemon. Combine with `--drop-caches` (requires sudo/root) to also
  flush the OS page cache. This is the most reproducible mode but
  slowest.
- `--warm` — primes the daemon once, then measures repeat rebuilds.
  Useful for iterative-dev latency measurements.
- `--both` (default) — cold runs first, then warm (since cold
  naturally warms the caches).

## Output layout

```
benchmarks/
  2026-04-22-abc1234/
    llvm-project_clang_clang/
      target.txt                 # the target label, for reference
      cold-01/
        wall.txt                 # wall time in seconds
        build.log                # captured build stdout+stderr
        build.pb.zst             # copy of the event log
        summary.json             # `slug log summary --format=json`
      cold-02/...
      warm-01/...
```

`summary.json` shape is stable — see `BuildSummary` in
`app/slug_event_observer/src/build_summary.rs`.

## Plan references

- Harness spec: `thoughts/shared/plans/slug-bazel-subplans/16-benchmark-telemetry.md` §16.8
- Optimization phases gated on this harness:
  `thoughts/shared/plans/slug-bazel-subplans/17-optimization.md`
