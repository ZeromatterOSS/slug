# Post-17.2-aggregator-fix baseline

**Date:** 2026-04-22
**HEAD:** 6e452ff + e86daf6 + fe96e88 (see git log)
**Target:** `@llvm-project//clang:clang`
**Mode:** cold, single run, no --drop-caches
**Workspace:** `/var/mnt/dev/llvm-project/utils/bazel`

## Headline

| Metric               | Bazel 9.0.2 | Prior baseline (same workspace) | This run       |
|----------------------|-------------|---------------------------------|----------------|
| Cold wall            | 1131 s      | 1983 s (noisy) / 1434 s (user)  | **1435.7 s**   |
| Actions              | 6352        | 5367                            | 5367           |
| act/sec              | 5.6         | 2.71                            | **3.74**       |
| Critical path        | 71.4 s      | 1972 s (bogus, pre-fix)         | **367.6 s**    |
| Slowest path         | —           | 1971 s (bogus, pre-fix)         | 132.7 s        |
| Peak in-flight       | —           | 2471                            | 2445           |
| Avg parallelism      | —           | 15.4×                           | 15.4×          |

Same run, same config, two fixes flipped between them (`e86daf6`,
`fe96e88`). Wall-clock dropped from 1983s → 1436s — the 1983s was
environmental noise, not a Plan 16 regression. Per-action numbers
and critical path are now meaningful.

## Phase breakdown

    total_wall       = 1435.7 s
    load_wall        =    1.8 s     (elapsed first Load → last Load)
    analyze_wall     =    1.2 s
    execute_wall     = 1432.4 s
    materialize_wall = 1430.0 s

Load + analyze = 3s. Everything else is execute.

## Critical path breakdown

    td_generate    290.7 s  (79.1% of critical path)
    c_compile       42.6 s  (11.6%)
    cpp_link        31.2 s  ( 8.5%)
    cpp_archive      1.4 s  ( 0.4%)
    -----------------------
    sum            365.9 s
    reported       367.6 s  (1.7 s gaps / waits)

**79% of the critical path is `td_generate`.** Nothing else comes
close. Slowest 5 actions are all AMDGPU tablegen, 207–280 s each.

## Per-mnemonic detail (exec-only vs queue wait)

    mnemonic     count  exec-only  queue wait   ratio   p50    p95     crit
    c_compile    3664    16,784 s  1,573,765 s  93.8×  3.6 s  11.4 s  42.6 s
    td_generate   446     5,248 s     23,834 s   4.5×  4.1 s  43.7 s 290.7 s
    cpp_archive   176         39 s    60,100 s 1539.2×  0.1 s   0.9 s   1.4 s
    cpp_link        4         32 s         3 s   0.1×  0.8 s  30.0 s  31.2 s
    genrule         1        0.7 s       0.1 s   0.1×  0.7 s   0.7 s   0.0 s
    run_binary      1        0.2 s       1.3 s   8.4×  0.2 s   0.2 s   0.0 s

## Two separate problems, ranked

### (1) td_generate critical chain — **296 s of the 305 s wall gap vs Bazel**

Bazel's critical path = 71 s. Kuro's = 367 s. The gap is **296 s**,
and the total wall gap vs Bazel is **305 s**. These are almost
identical numbers. If kuro's critical path matched Bazel's, the wall
would be ~1,140 s — within a few % of Bazel.

Of that 296 s critical-path gap, 290 s is inside `td_generate`. The
top five tablegen actions sit on the serial chain and take 207–280 s
*each*. Either:

- They genuinely serialize because `AMDGPUGenRegisterInfo.inc` ->
  (depends on) -> `AMDGPUGenRegisterBank.inc` -> (depends on) -> …
  by BUILD-file wiring. In that case it's a rule-design question, not
  a scheduler one.
- Or some kuro-specific dep edge (e.g. a shared genrule toolchain
  dependency pulling everything serial) that Bazel doesn't have.

`kuro log critical-path benchmarks/post-plan-17-fixed-aggregator/
llvm-project_clang_clang/cold-01/build.pb.zst` will dump the exact
chain, including the edges between entries.

### (2) c_compile queue ratio 93.8× — real but not on the wall

c_compile p50 wall = 3.6 s, mean wall = 4.6 s. But each compile
queued ~425 s on average (total queue 1.57M s / 3664 actions). This
is the "scheduler admission vs dispatch" mismatch the earlier fixes
surfaced.

Yet the overall wall clock is close to what the critical path
allows. So queue depth ≠ wall-clock cost — the scheduler piles up
work, but the work that DOES dispatch stays on the critical chain.

Why does this matter? It's a signal that the scheduler is not
actively managing concurrency — it's just piling up and waiting.
Under ideal pressure-managed scheduling, in-flight would hover near
CPU count instead of 2,445. The penalty of having a deep queue
shows up as:

- Large per-action latency jitter.
- Wasted RAM on in-flight future state.
- Harder-to-read traces (action-level waiting_data is noise).

This is still worth fixing but it won't move the wall clock much
on this particular target.

## Plan 17.2 direction, revised again

1. **(1)** is the headline. Run `kuro log critical-path` to dump
   the exact td_generate chain. If the chain is rule-design-induced,
   note it for the LLVM repo owners and move on. If it's a kuro
   scheduling bug, that's 17.2's real ticket.
2. **(2)** is background — dive in only after the critical path is
   sorted. Fine-tune `HostSharingBroker` semaphore sizing, or
   introduce per-category permits (tablegen vs compile) that match
   actual resource use.

## Artifacts

- `cold-01/wall.txt` — 1435.698 s
- `cold-01/summary.json` — full BuildSummary, post-fix semantics
- `cold-01/build.pb.zst` — event log for diffing (3.5 MB, gitignored)
- `cold-01/build.log` — captured stderr (gitignored)
