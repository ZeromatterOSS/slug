# Plan 17.2-measure — admission latency is not the bottleneck

**Date:** 2026-04-23
**HEAD:** c9e2de7f (instrumentation landed)
**Build:** cold `@llvm-project//clang:clang` with
`SLUG_LOG_ADMISSION=/tmp/slug-admission-clang.csv`
**CSV:** 5558 rows covering c_compile (3874), td_generate (450),
copy (1054), cpp_archive (146), cpp_link (3), and minor other
categories.

## Admission latency: fine

The `delta_us` between "inputs resolved" and "handed to executor"
is uniformly small:

- **p50:** ~12 µs
- **p95:** ~15 ms
- **worst:** 19 ms (one clang:sema c_compile)

No action waits more than 20 ms between ready and executor handoff.
This rules out hypothesis (a) from the post-Plan-20 critical-path
note — scheduler admission is not starving td_generate or any
other category.

## What the critical-path data actually shows

`slug log critical-path` after this build breaks down as:

| Slice                                   | Wall (s) |
|-----------------------------------------|----------|
| llvm-tblgen c_compiles (Record.cpp etc.) on CP | 40      |
| llvm-tblgen cpp_link                    | 0.3      |
| td_generate AMDGPUGenRegisterInfo.inc   | 26.4     |
| **waiting for_deps (one block)**        | **446.7**|
| **c_compile clang:sema SemaConcept.cpp.pic** | **279.3**|
| **waiting for_deps (second block)**     | **247.3**|
| cpp_archive libsema.a                   | 1.0      |
| cpp_link clang                          | 30.2     |
| **CP wall (sum)**                       | **≈1071**|

Total wall clock was 1108 s, so CP is ≈ 97 % of wall — nearly
fully serial at the tail.

### Two headline anomalies

**1. One c_compile that takes 279 s.** `SemaConcept.cpp.pic`
(clang:sema) is a target-cfg fastbuild compile (`-g0`, no `-O2`)
yet takes 4 minutes 39 seconds. `c_compile p95 = 10.4 s` from
summary.json, so this is a p99+ outlier — one source eating a
quarter of the wall clock. Bazel likely compiles this in similar
wall time but pipelines it earlier so it doesn't land on CP.

**2. 694 s of "waiting for_deps" on CP.** Two blocks — 446 s and
247 s — where the critical path is blocked on dep resolution with
no action executing. These are huge serial dep chains.

## Revised hypothesis

Plan 17.2's original premise (scheduler admission is under-
dispatching ready actions) is **wrong** post-Plan-20. The
instrumentation disproved it directly.

The remaining CP gap is two different bugs:

a. **Action-duration outlier.** SemaConcept.cpp.pic 279 s on
   fastbuild is either pathological source (unavoidable) or a
   slug-specific slowdown (e.g. PCH/pre-compiled-header not
   reused, or include-path fanout making the preprocessor slower).
   Compare wall time vs Bazel on the same file.

b. **Serial dep chains.** 694 s of `waiting for_deps` on CP means
   actions downstream of SemaConcept + tablegen wait hundreds of
   seconds on single-threaded dep resolution. Could be:
   - A DICE reduction key that gates many consumers on one producer.
   - Missing parallelism in analysis-phase dep traversal.
   - Or just a real dep chain that Bazel's scheduler handles
     differently.

## Cost/benefit

- Fixing (a) could recover 50-100+ s of wall, depending on whether
  the 279 s is slug-specific or inherent.
- Fixing (b) could unblock big parallel regions but the ROI depends
  on how much of the wait is "real dep chain" vs "serialisation bug".
- Plan 17.2's original act/sec >= 5.5 target (post-20 is 5.05)
  would be reached by either fix, because both cut total wall.

## Deliverables

- Instrumentation landed in commit c9e2de7f
  (`app/slug_build_api/src/actions/admission_log.rs`), env-gated
  by `SLUG_LOG_ADMISSION`.
- Admission CSV for clang:clang archived at
  `/tmp/slug-admission-clang.csv` (not committed — regenerable).

## cquery / aquery comparison

To isolate pure analysis cost (no compile noise), compared slug and
Bazel on the same `@llvm-project//clang:clang` graph:

| Command           | Bazel cold | Slug cold | Bazel warm | Slug warm |
|-------------------|-----------:|----------:|-----------:|----------:|
| cquery            | 4.87 s     | 46.1 s    | 0.17 s     | 2.04 s    |
| aquery deps(...)  | 9.92 s     | 50.3 s    | —          | —         |

Ratios: cold 9.5× slower, warm 12× slower.

**Where the 51 s of cold time goes.** Client log shows:

    14:06:27 Could not connect to slug daemon, killing daemon
    14:06:27 Starting new slug daemon
    14:07:18 Connected to new slug daemon     ← 51 s later
    14:07:18 File change events (<100 ms)
    14:07:22 Toolchain warnings (4 s after connected)
    14:07:23 cquery result emitted

So ≈ 51 s is daemon startup + bzlmod cold load, ≈ 4 s is
module-load / cell-resolver, and ≈ 2 s is the cquery itself.
Warm run (daemon already up) confirms analysis-only cost is 2 s.

**This is Plan 17.5 territory** (external-cell fetch caching):
> How to measure: `tools/bench/run.sh --cold --target @llvm-project//clang:clang`
> on a fresh tree — look at the `load_wall_us` slice of the
> resulting `summary.json`. If load wall >> analyze wall on first
> daemon start, module hashing is likely dominant.

On the post-Plan-20 summary.json: `load_wall_us = 2.4 s`,
`analyze_wall_us = 1.1 s`. That doesn't match the 51 s cold cost,
so whatever the 51 s is, it's happening outside the build's
load/analyze phase boundary — likely in daemon-level init before
the build phases even start.

The cold clang:clang wall-clock breakdown then looks like:

    Daemon startup + module resolution:   ~50 s
    Load + analyze phase:                   ~4 s
    Execute phase:                       ~1055 s
    Total:                               ~1108 s

Bazel's equivalent:

    Cold analysis (≈ full startup):       ~5 s
    Execute phase:                       ~1126 s
    Total:                               ~1131 s

So **slug beats Bazel on action execution by ~70 s** but loses
~45 s on cold startup. Net: slug 23 s faster overall, but for the
ideal "match Bazel on every axis" narrative, the startup gap is
the single biggest measurable item left.

## Next steps (supersedes 17.2-fix)

Re-scope 17.2-fix or split into two follow-ups:

**17.2-compile-outlier.** Time-bound SemaConcept.cpp on Bazel and
on slug in isolation (`slug build @llvm-project//clang:sema`).
If slug is materially slower than Bazel on the same file, trace
the compile (preprocess+compile split, -ftime-report, include
fanout). If slug matches Bazel wall but Bazel doesn't put it on
CP, the fix is deeper scheduling.

**17.2-dep-chain.** Use `slug log critical-path` to identify what
dep chain fills the 446 s and 247 s `waiting for_deps` blocks.
Specifically: what action became ready at the end of each wait
block, and what was blocking it? DICE trace needed.

The admission-latency fix path from the original 17.2 scope is
dead — no fix to apply there.

## References

- Instrumentation: commit c9e2de7f
- Investigation of post-Plan-20 CP: `post-plan-20-critical-path.md`
- Original 17.2 hypothesis doc: `scheduler-parallelism-findings.md`
- Remaining parked 17.x phases: `plan-17-remaining-phases.md`
