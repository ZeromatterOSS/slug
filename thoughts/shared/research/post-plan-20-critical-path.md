# Post-Plan-20 critical-path investigation

**Date:** 2026-04-23
**HEAD:** 8a286bb6
**Baseline:** `benchmarks/post-plan-20-final-41ce00a5/`

## Context

Plan 20 closed Plan 19's td_generate win: cold
`@llvm-project//clang:clang` wall is now **1107.6 s** vs Bazel's 1131 s
(2.4 % faster). The remaining gap is critical-path shaped: kuro 109.7 s
vs Bazel 71.4 s (1.54×).

## Per-mnemonic CP vs Bazel

| Mnemonic       | kuro CP | Bazel CP | Delta  |
|----------------|---------|----------|--------|
| c_compile      | 47.5 s  | 42.6 s   | +4.9 s |
| td_generate    | 33.1 s  | ~0 s     | +33 s  |
| cpp_link       | 26.1 s  | 31.2 s   | −5 s   |
| cpp_archive    | 1.3 s   | 1.4 s    | flat   |
| **sum (CP)**   | 110 s   | 71 s     | +38 s  |

td_generate on kuro's CP is ~87 % of the gap. Bazel's critical path
doesn't include tablegen because its scheduler admits tablegen early
and parallel with independent library compiles.

## Which td_generate is on CP

`kuro log critical-path | grep td_generate` on the post-20 run:

    34956142  2198388  RISCVTargetParserDefGen ... td_generate ...   (exec cfg)
    57412624 32543586 AMDGPUCommonTableGen / AMDGPUGenRegisterBank ... td_generate ...  (target cfg ffb6fe5c7480b5e7)

Two entries. The **AMDGPU GenRegisterBank** action is 32.5 s by itself
— a single heavy tablegen invocation. This is ≈97 % of the td_generate
CP slice.

Ownership: `llvm-project//llvm:AMDGPUCommonTableGen_filegroup___gen_register_bank_...`
is a genrule that invokes `llvm-tblgen` with a large `.td` input. The
genrule itself is target-cfg; the tblgen tool it invokes was built in
exec cfg (Plan 20.1) with opt flags.

## Why it lands on kuro's CP

Bazel's tablegen invocations are the same ~32 s absolute, but Bazel
**starts** them earlier in the build so they finish while
lib-compile work runs in parallel. Kuro starts them later.

Hypothesis (d) from `scheduler-parallelism-findings.md`: a
false-serial dependency — the AMDGPU tablegen is forced to wait for
something that Bazel doesn't wait on. Candidates:
1. Analysis of the genrule's consumer (some AMDGPU cc_library)
   blocks on an earlier, unrelated action that delays ready-to-run.
2. All td_generate actions share a DICE reduction key, forcing one
   to complete before the next becomes ready.
3. Action queue admission prioritises lib compiles over tablegen
   once the ready set includes both.

## Next step: measure ready-time vs start-time

To distinguish the three candidates, instrument each action with
two timestamps:
- `ready_us`: when all inputs resolve (action becomes eligible).
- `start_us`: when the scheduler actually spawns execution.

Delta `start_us − ready_us` = admission latency. Large delta on
AMDGPU tablegen with no blocking dep means admission is the bug
(candidate 3); small delta but late `ready_us` means dep (candidates
1 or 2).

The action registry already records `user_us` and `potential_us` in
`kuro log critical-path` output (visible in the log above). A
quick analysis: take the 32.5 s AMDGPU action's `start_offset` =
57.4 s and compare to the build's t=0 — if td_generate has the
inputs ready well before 57 s, it's admission latency; if inputs
resolve at ~57 s, it's dep chain.

Looking at the CP log, AMDGPU's start_offset is 57.4 s while the
first td_generate (RISCV, 35.0 s start) also ran. Between 35 and
57 s nothing else on the CP appears to block AMDGPU directly —
suggesting admission, not dep.

## Recommended plan

Split Plan 17.2 into a concrete action:

**21.1** Instrument action admission. Log `(action_key, ready_us,
start_us, inputs_ready_count)` per action. One-off event log flag
(e.g. `--log-admission`) is fine — this is debugging not production.

**21.2** Run the instrumented build and plot admission latency per
category. Expected outcome: td_generate actions show >10 s
ready-to-start latency when lib compiles are saturating the local
executor.

**21.3** Fix whichever the data implicates:
- If admission: reduce host-sharing pressure for tablegen (give
  td_generate a small dedicated permit pool that lib compiles can't
  exhaust), or raise tablegen priority in the scheduler.
- If dep: identify the bogus serial edge in DICE and fix the
  ReductionKey or analysis output.

## Cost/benefit

Closing 33 s of CP on a 1107 s build is a 3 % overall-wall win.
Modest. The bigger practical value: bringing CP in line with Bazel
(~71 s) gives a clean "we match Bazel on every axis" story.

If prioritising by measured-win-per-effort:
- 21.x (CP scheduling): 3 % wall, 1-3 days
- 17.5 (external-cell fetch caching): unknown, mostly affects
  warm-cache scenarios and daemon-restart behaviour
- 17.8 (DICE key granularity for incremental): unknown, affects
  warm rebuilds

21.x has the tightest measurement signal (the 33 s CP gap is visible
in every benchmark run).

## References

- `benchmarks/post-plan-20-final-41ce00a5/FINDINGS.md`
- `thoughts/shared/research/scheduler-parallelism-findings.md`
  (hypothesis d)
- `thoughts/shared/research/plan-17-remaining-phases.md`
