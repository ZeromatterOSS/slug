# Post-Plan-16 baseline

**Date:** 2026-04-22
**HEAD:** 7abd174 (Plan 16.8 README note)
**Target:** `@llvm-project//clang:clang`
**Mode:** cold, single run, no --drop-caches
**Workspace:** `/var/mnt/dev/llvm-project/utils/bazel`

## Headline

| Metric                 | Bazel 9.0.2 | User baseline (pre-Plan-16) | Post-Plan-16 (this run) |
|------------------------|-------------|-----------------------------|-------------------------|
| Cold wall              | 1131 s      | 1434 s                      | **1983 s**              |
| Actions                | 6352        | 4293                        | 5367                    |
| act/sec                | 5.6         | 2.99                        | **2.71**                |
| Critical path          | 71.4 s      | not emitted                 | **1972 s**              |
| Analysis-only wall     | 0.75 s      | 1.94 s                      | 2.9 s                   |

## Phase breakdown (Post-Plan-16)

    total_wall       = 1982.9 s
    load_wall        =    3.9 s     (elapsed: first Load start → last Load end)
    analyze_wall     =    2.9 s
    execute_wall     = 1975.8 s
    materialize_wall = 1971.9 s

Execute and materialize spans overlap (final materialization of early
targets happens while late targets are still compiling). Load + analyze
are negligible — this is entirely an execute-phase build.

## The critical-path surprise

**critical_path_wall ≈ total_wall.** Bazel reports 71s of critical path
for the same target. Kuro reports 1972s — **28× longer**.

Two incompatible readings on the same data:
- `peak_in_flight_actions = 2471` suggests massive parallelism.
- `critical_path ≈ total` suggests almost zero parallelism.

Most likely explanation: `CriticalPathEntry2.duration` in kuro
includes *queue wait time* on each entry, not just execution. An
action that queued for 700s behind other actions has its critical
chain contribution inflated to 700s + exec-time. Summing those
entries makes the critical path ≈ total wall by construction.

Bazel computes critical path over execution time, not wall-from-
schedule. That's why its 71s figure is so much smaller.

Confirming evidence:
- `by_mnemonic[c_compile].p50 = 783s` — way too long for a typical
  clang source file (expected 10–60s). Supports the "duration
  includes queue" hypothesis.
- `total_wall_us` for c_compile is 2.55M seconds across 3664 actions
  → mean 697s per compile. Impossible for real wall-of-CPU work on a
  reasonable core count.

## What the numbers actually say

The 2471 peak in-flight is the scheduler's queue depth, not live CPU
parallelism. Actions are scheduled faster than they execute, and
pile up. The `td_generate` phase serialization the user observed is
real — critical path has td_generate entries contributing ~911s.

Genuine per-action execute wall (excluding queue) is not reported in
the proto today. To get it, we'd need `ActionExecution.exec_start`
and `ActionExecution.exec_end` timestamps distinguishing "scheduled"
from "dispatched" from "complete".

## act/sec regression vs pre-Plan-16

Post-Plan-16 = 2.71 vs pre-Plan-16 = 2.99 (at 4293 actions). That's a
~10% throughput drop. But action counts differ (5367 vs 4293). Per-
action mean wall is closer:

- Pre-Plan-16: 1434 s / 4293 = 334 ms/action
- Post-Plan-16: 1983 s / 5367 = 370 ms/action (~11% slower)

Possible sources:
- Environmental noise (single run, cold host).
- Plan 16.6 sub-spans: 3 extra span_simple calls per analyzed target.
  ~15k targets × 3 spans = 45k extra spans.
- Plan 16.5.2 bounded MPSC try_send: should be nanoseconds.
- Plan 17.4 canonicalize-on-mismatch: only fires on symlink update, low.

Cannot confirm without a warm baseline or multi-run variance bar. One
cold run is too noisy to draw conclusions from.

## Recommendations

**Open immediately:**
1. Distinguish "scheduled" from "dispatched" timestamps in
   `ActionExecutionStart` proto. Critical for any scheduler work.
   Currently the start timestamp fires when an action is *enqueued*,
   not when it starts *executing*. All duration metrics collapse
   without this.
2. Revise `CriticalPathEntry2.duration` semantics to use exec-only
   time. Doc the change and mark the old behavior.

**Plan 17.2 scheduler work should now focus on:**
- Why queue depth reaches 2471 when the scheduler can only execute
  ~N-cores actions at once. The scheduler is accepting far more work
  than it can dispatch.
- Why `td_generate` (446 actions) serializes — its critical_wall is
  911s vs total_wall 61,225s. If all 446 ran in parallel, critical
  would be ~137s (worst individual). 911s suggests a false serial
  chain.

**Harness improvements:**
- Need ≥3 runs for variance bars.
- Need warm+incremental modes to separate scheduler-dispatch cost
  from action-execution cost.

## Artifacts

- `cold-01/wall.txt` — 1983.168 s
- `cold-01/summary.json` — full BuildSummary in JSON
- `cold-01/build.pb.zst` — event log (3.5 MB compressed), usable
  for `kuro log diff summary`
- `cold-01/build.log` — captured stderr of the build
