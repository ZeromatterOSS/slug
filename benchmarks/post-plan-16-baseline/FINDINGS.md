# Post-Plan-16 baseline

> **Addendum 2026-04-22 (after commits e86daf6 + fe96e88):** The
> `summary.json` in this directory was regenerated from the same
> `build.pb.zst` with the fixed aggregator. Wall time, action count
> and critical_path figures are unchanged (those came from the live
> build), but per-mnemonic numbers are corrected and queue wait is
> now explicit. Re-aggregate anytime with:
>
>     /var/mnt/dev/kuro/kuro log summary --format json \
>         cold-01/build.pb.zst > cold-01/summary.json
>
> The pre-fix narrative below is preserved for bisect context; the
> corrected numbers at the end are the ones to trust.


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

## Corrected numbers (re-aggregated 2026-04-22)

After landing e86daf6 (per-action exec vs queue split) and fe96e88
(critical_path uses `user` not `total`), re-reading the same event
log tells a much clearer story:

    total_wall     =  1982.9 s
    execute_wall   =  1975.8 s   (elapsed first→last)
    critical_path  = [still 1972s in the saved json]
    actions        = 5367
    peak in-flight = 2471

> Critical path in the saved summary.json is still 1972s — the
> critical-path *algorithm* was fixed post-build, but the event log
> was emitted pre-fix. The value from the next baseline run will be
> correct. Support smoke test (post-fix) shows critical_path=3.6s vs
> wall=11.2s — that's what the fix should produce.

### Per-mnemonic (exec-only vs queue wait)

    mnemonic     count  exec-only  queue wait  ratio     p50     p95
    c_compile    3664     20,749s  2,529,155s  121.9×   4.9s   13.5s
    td_generate   446      9,766s     51,459s    5.3×   7.9s   81.1s
    cpp_archive   176         39s     85,964s 2210.6×   0.1s    0.8s
    cpp_link        4         32s          5s    0.2×   1.9s   28.0s

### The real story

**Kuro is queue-bound, not CPU-bound.** Pre-fix this was hidden — every
per-action and critical-path number included queue wait, making actions
look impossibly slow. The corrected numbers reveal:

- Real compile wall (c_compile) averages 5.7s with p95 of 13.5s. That's
  reasonable for clang source compiled in debug mode.
- Queue wait on c_compile alone is 2.5M seconds. Every compile waited
  122× its exec time before a permit opened up.
- cpp_archive is 2210× queue-bound — 100ms of real work waiting 10min
  on average.

Total exec-time = 30,589s across 5367 actions. Build took 1983s wall.
Average parallelism = 30,589 / 1983 ≈ 15.4 concurrent actions. That's
consistent with CPU throughput being the bottleneck *given* the
HostSharingBroker permit count — but far less than the 2471-deep
queue would suggest.

### Plan 17.2 brief, revised

Old framing: "why does execution throughput plateau at 3 act/sec?"

New framing: **why does scheduler admission accept 2471 concurrent
actions when only 15-ish can actually execute at any given moment?**

This is usually either:
1. DICE dispatching all action futures eagerly instead of pulling them
   when a worker is free.
2. HostSharingBroker permit count far too high (or missing).
3. Fine-grained permits (category, memory) not configured, so every
   action grabs a "core" permit regardless of actual resource use.

The scheduler investigation at
`thoughts/shared/research/scheduler-parallelism-findings.md` noted the
permit machinery exists. The question now is its parameters and
back-pressure behavior.
