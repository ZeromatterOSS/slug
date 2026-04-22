# Plan 17 remaining phases — deferred pending harness baseline

**Date:** 2026-04-22

Plan 17 phases that are *gated on Plan 16 harness measurements* and
not actionable without a cold-clang:clang baseline captured on the
current tree. Shipping a "fix" here without the before/after delta
violates the plan's own discipline.

## Phases in this state

### 17.5 External-cell fetch caching

Question: does daemon cold start re-hash every bzlmod module's source
tree? If yes, add a persistent content-addressed cache keyed by
`(module, version, url, sha)`.

How to measure: `tools/bench/run.sh --cold --target @llvm-project//clang:clang`
on a fresh tree — look at the `load_wall_us` slice of the resulting
`summary.json`. If load wall >> analyze wall on first daemon start,
module hashing is likely dominant.

Code sites: `app/kuro_external_cells/src/*.rs`,
`app/kuro_bzlmod/src/resolution.rs` (registry + source fetch),
`app/kuro_bzlmod/src/source_fetcher.rs`.

### 17.8 DICE key granularity review

Exploratory. Questions:
- Is there a per-target AnalysisKey or a per-package one?
- Are ListingKey / LoadKey split or merged?
- Does every rule's implementation get its own DICE key?

Fix surface is in the shape of DICE keys + their computation
boundaries. Invalidation scope is the signal to watch: a single-file
touch that rebuilds all targets in the package points at over-coarse
keys.

How to measure: extend Plan 16.8 harness with an `--incremental`
mode that touches one `.cc` and measures the rebuild scope. Plan 17
open question already flagged this.

### 17.9 kuro_error allocation sites

Low priority until a flame graph from the Plan 16 harness surfaces
`kuro_error::Error::new` / backtrace capture as a hot-path frame.
Documented in the plan as "Low priority unless plan-16 flame graphs
surface it." Leaving parked.

## What to do instead

Run the Plan 16 harness on the current tree to establish a baseline:

```bash
KURO=$(pwd)/target/debug/kuro tools/bench/run.sh \
    --target '@llvm-project//clang:clang' --runs 3 --cold \
    --workspace /var/mnt/dev/llvm-project/utils/bazel \
    --out benchmarks/baseline-post-plan-16
```

Then inspect `summary.json` to see which phase dominates. Promote the
matching Plan 17 phase above to active work and iterate.

## Non-deferred Plan 17 work already landed

- 17.1 pattern sweep (report at `thoughts/shared/research/ai-pattern-sweep.md`)
- 17.2 scheduler investigation (report at
  `thoughts/shared/research/scheduler-parallelism-findings.md`)
- 17.3 signal pipeline micro-opts (scoped — no code changes safe without
  measurement, MPSC fix already landed via 16.5.2)
- 17.4 file-watcher absolute-vs-relative symlink equality (landed)
- 17.6 Starlark persistence audit (report at
  `thoughts/shared/research/starlark-compilation-persistence.md`)
- 17.7 prelude-cleanup aftermath — 4 orphan files removed
