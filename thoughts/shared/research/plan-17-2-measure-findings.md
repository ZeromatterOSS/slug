# Plan 17.2-measure — admission latency is not the bottleneck

**Date:** 2026-04-23
**HEAD:** c9e2de7f (instrumentation landed)
**Build:** cold `@llvm-project//clang:clang` with
`KURO_LOG_ADMISSION=/tmp/kuro-admission-clang.csv`
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

`kuro log critical-path` after this build breaks down as:

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
   kuro-specific slowdown (e.g. PCH/pre-compiled-header not
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
  the 279 s is kuro-specific or inherent.
- Fixing (b) could unblock big parallel regions but the ROI depends
  on how much of the wait is "real dep chain" vs "serialisation bug".
- Plan 17.2's original act/sec >= 5.5 target (post-20 is 5.05)
  would be reached by either fix, because both cut total wall.

## Deliverables

- Instrumentation landed in commit c9e2de7f
  (`app/kuro_build_api/src/actions/admission_log.rs`), env-gated
  by `KURO_LOG_ADMISSION`.
- Admission CSV for clang:clang archived at
  `/tmp/kuro-admission-clang.csv` (not committed — regenerable).

## Next steps (supersedes 17.2-fix)

Re-scope 17.2-fix or split into two follow-ups:

**17.2-compile-outlier.** Time-bound SemaConcept.cpp on Bazel and
on kuro in isolation (`kuro build @llvm-project//clang:sema`).
If kuro is materially slower than Bazel on the same file, trace
the compile (preprocess+compile split, -ftime-report, include
fanout). If kuro matches Bazel wall but Bazel doesn't put it on
CP, the fix is deeper scheduling.

**17.2-dep-chain.** Use `kuro log critical-path` to identify what
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
