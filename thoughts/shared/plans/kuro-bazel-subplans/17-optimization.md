# Plan 17: Optimizations driven by Plan 16 measurements

> Parent: [2026-01-21-kuro-bazel-compatible-build-tool.md](../2026-01-21-kuro-bazel-compatible-build-tool.md)
>
> Depends on Plan 16 (benchmark telemetry). Every phase here lands with a
> before/after delta measured by the Plan 16 harness and committed under
> `benchmarks/<date>-<sha>/`. Speculative optimizations without a
> measurement are explicitly out of scope.

## Scope

Close the performance gap vs Bazel 9 on llvm-scale builds, and undo
poor-performance patterns introduced by prior AI agents during the
bootstrap phase ("make it work" code that was never revisited).

Baseline from plan-1 head-to-head on `@llvm-project//clang:clang`:

|                              | Bazel 9.0.2 | Kuro       | Ratio  |
|------------------------------|-------------|------------|--------|
| Cold full build (wall)       | 1131s       | 1346s      | 1.19×  |
| Warm analysis (no-op)        | 0.75s       | ~2.1s      | 2.8×   |
| Actions executed             | 6352        | 4278       | 0.67×  |
| act/sec throughput           | 5.6         | 3.2        | 0.57×  |
| Critical path                | 71s         | not emitted| —      |

Kuro runs ~1/3 fewer actions but takes 19% longer wall. The throughput gap
is real and load-bearing.

## Current State Analysis

**Concrete footguns found in the plan-1 research pass.** Each is scoped
enough to measure independently:

1. **Scheduler parallelism.** 5.6 → 3.2 act/sec is the biggest single
   number. Investigate `app/kuro_execute/src/scheduler*.rs` and
   `app/kuro_build_api/src/execute/*`. Specific symptoms observed:
   - The LLVM `td_generate` phase was visibly sequential (one pending
     action at a time for minutes). Either a false serial dep chain or
     per-category concurrency cap.
   - 2400+ action queue depth observed mid-build, 700+ during late
     compile phase. Suggests scheduler isn't saturating cores when the
     queue is deep.

2. **`NodeKey::from_dyn_key()` downcasts on every edge.**
   `app/kuro_build_signals_impl/src/lib.rs:123-143`. O(edges) work in the
   signal pipeline. 100k+ edges on a clang build.

3. **`.to_owned()` on compile-time-known variant names.**
   `app/kuro_build_signals_impl/src/lib.rs:229-237`. Variant names are
   `&'static str`; proto accepts `String` so they get cloned. Visible in
   flame graphs of critical-path computation.

4. **Panicking `EventSink`.** `app/kuro_events/src/sink/*.rs:254-259`.
   Correctness smell (already flagged in 16.5.5) but also means a single
   slow sink blocks the dispatcher thread; a `Result`-returning design
   permits async/batched implementations.

5. **File watcher catch-up.** Every `kuro cquery` / `kuro build` warm
   invocation logs "26068 additional file change events". Either the
   watcher isn't persisting last-scan state, or it's re-enumerating the
   world. `app/kuro_file_watcher/*`. Suspect AI-written shortcut.

6. **Prelude cleanup aftermath.** Memory note: `prelude/rules_impl.bzl`
   was hand-reduced 576→40 lines in phase 7d. Dead loads, removed
   language dirs (android, apple, cxx, erlang, go, haskell, java, kotlin,
   python, rust, csharp, ocaml, julia, js, lua, aosp — 732 files
   deleted). Verify no orphaned code paths are still running.

7. **Starlark re-compilation on daemon cold start.** Bazel persists
   compiled bzl across invocations; Kuro's bundled-cell cache has a
   `touch app/kuro_external_cells_bundled/build.rs` escape hatch which
   implies it's real, but we haven't measured effectiveness.

8. **Unbounded MPSC + non-evicting span tracker.** Already scoped in
   16.5.2 and 16.5.3 as "benchmark noise"; reflagged here because they
   also cap how long a daemon can run benchmarks.

## Phases

### 17.1 AI-agent-pattern sweep (OPEN)

**Parity source.** CLAUDE.md coding conventions + dupe trait semantics.

Single agent-dispatched scan over the whole repo. Ranked output, no
automatic edits — each hit reviewed manually. Patterns to flag:

| Pattern | File:line hint | Why |
|---|---|---|
| `.to_string()` on `&str` | grep `\.to_string\(\)` | CLAUDE.md says `.to_owned()`. |
| `.clone()` on `Dupe` types | grep, filter by type | `Dupe` exists to avoid allocation. |
| `std::fs::*` inside `async fn` | syntax pattern | Blocks the tokio runtime thread. |
| `.unwrap()` / `.expect()` outside tests | grep + file filter | Should be `internal_error!`. |
| `HashMap<K, V>` where tiny N dominates | context-dependent | `SmallMap` / `FxHashMap` / `Vec<(K,V)>` wins. |
| Long-lived `String` / `HashMap<String, ...>` in Bazel-compat code | grep `HashMap<String`, `FxHashMap<String`, `Vec<String>` | Check [Plan 26](./26-string-interning.md): stable graph identifiers should usually be typed + interned or explicitly justified. |
| `Arc::new(x.clone())` | grep `Arc::new\(.*\.clone` | Double-alloc. |
| String concat in hot loops | context-dependent | Use `write!` into a buffer. |
| Regex compilation inside `fn` called per-event | grep `Regex::new` | Compile once. |
| `.into_iter().collect::<Vec<_>>().iter()` round-trips | syntax | Unnecessary alloc. |
| `Arc<Mutex<T>>` for read-dominant data | type-scan | `ArcSwap` or `OnceCell`. |

Output: `thoughts/shared/research/ai-pattern-sweep.md` with a ranked list.
Concrete fixes land as follow-up phases 17.1.x, each with a benchmark
delta.

---

### 17.2 Scheduler parallelism (INVESTIGATED + DISPROVED 2026-04-23)

**Status.** Hypothesis instrumented and disproved. `act/sec` on the
post-Plan-20 cold clang:clang is 5.05 vs 5.6 Bazel; the remaining gap
is not admission-shaped.

Phases landed:
- **17.2-instrument** (commit `c9e2de7f`): side-channel CSV logger in
  `app/kuro_build_api/src/actions/admission_log.rs`, gated by
  `KURO_LOG_ADMISSION=<path>`. Captures ready_us / start_us / delta_us
  per action.
- **17.2-measure** (commit `3ea26d37`, findings
  `thoughts/shared/research/plan-17-2-measure-findings.md`):
  admission delta p95 = 15 ms, worst = 19 ms. Scheduler admission is
  not the bottleneck. Real CP anomalies are:
  1. One c_compile (`SemaConcept.cpp.pic`) taking 279 s on fastbuild.
  2. 694 s of "waiting for_deps" on the critical path.

Deferred splits (NOT YET SCHEDULED — pick up when prioritised):
- **17.2-compile-outlier.** Is kuro's 279 s on SemaConcept kuro-specific
  or intrinsic? Time the single file isolated in both Bazel and kuro,
  compare.
- **17.2-dep-chain.** DICE-trace the 446 s and 247 s `waiting for_deps`
  blocks on CP. Identify what dep chain is forcing serial progress and
  whether a ReductionKey is too coarse.

Out-of-scope for 17.2: the warm-invocation overhead (Plan 21).

---

### 17.3 Build-signal pipeline micro-opts (OPEN)

**Parity source.** None — kuro-specific hot path in
`kuro_build_signals_impl`.

17.3.1 `NodeKey::from_dyn_key` — cache by pointer-identity or batch via
       type-id. Measure flame-graph delta in
       `BuildSignalReceiver::run`.

17.3.2 Static-str proto entries — replace `.to_owned()` on variant names
       with `&'static str`-accepting proto helper (proto allows borrowed
       strings via `prost`'s `bytes::Bytes`).

17.3.3 Bounded MPSC (landed as 16.5.2 — reflagged: verify the chosen
       capacity doesn't hurt throughput on the plan-16 harness).

---

### 17.4 File-watcher catch-up cost (DONE — cold fix 2026-04-23, warm residual fixed by Plan 21)

**Cold-start sliver landed** (commit `5a6c346a`): `NotifyFileWatcher::new`
used to call `watcher.watch(root, RecursiveMode::Recursive)` which
walked 29 k directories and followed symlinks into `bazel-external/`.
Replaced with a `walkdir` pass that skips symlinks and applies the
root-cell ignore spec. Cold cquery dropped 46.1 s → 3.0 s.

**Self-invalidation loop landed** (commit `0c326250`):
`ensure_external_symlink` was replacing `external/<apparent>` symlinks
on every invocation when both the stored and desired targets failed
`canonicalize()` (target not yet materialized, two callers picking
different canonical names). Mtime flapped → notify fired → DICE
invalidated. Fix: no-op when both canonicalize fail.

**Warm residual fixed by Plan 21** (commit `8668b19e` + followups).
Root cause was `CellResolverKey` churning every transaction because
the bzlmod data flow produced a different CellInstance path/external
tuple across invocations. Fix: sort iteration at three consumer sites
plus custom `CellInstance::PartialEq` ignoring textual noise in
`ExtensionRepoCellSetup`. Warm cquery 1.85 s → 0.20 s.

Full warm-cquery breakdown:
`thoughts/shared/research/plan-17-2-measure-findings.md`.
Plan 21 write-up: `thoughts/shared/research/2026-04-24-plan-21-warm-invalidation.md`.

---

### 17.5 External-cell fetch caching (OPEN)

`kuro_external_cells/src/*.rs`. On daemon cold start, do we re-hash every
bzlmod cell source? Plan-16 cold wall measurement will show whether this
dominates. If yes: add a persistent content-addressed cache keyed by
`(module, version, url, sha)`.

---

### 17.6 Starlark compilation persistence (OPEN)

Bazel's Skylark module cache persists across invocations. Verify kuro's
equivalent. If bundled-cell bzls get re-parsed on every daemon start,
measure and fix.

`app/kuro_interpreter/src/*`, `app/kuro_external_cells_bundled/*`.

---

### 17.7 Prelude-cleanup aftermath (OPEN)

Audit `prelude/` for leftover references to deleted language directories.
Grep for `load("@prelude//{android,apple,cxx,erlang,go,haskell,java,kotlin,python,rust,csharp,ocaml,julia,js,lua,aosp}/...`
— each hit is either dead code or broken. Remove dead, fix broken.

Also: `prelude/rules_impl.bzl` is 40 lines; confirm every live rule has
an implementation path and no dead stubs are being evaluated.

---

### 17.8 DICE key granularity review (OPEN, exploratory)

DICE keys that are coarser than Bazel's equivalents cause unnecessary
recomputation under incremental builds. Spot-check common keys:

- Is there a per-target `AnalysisKey` or just a per-package one?
- Are `ListingKey` / `LoadKey` split or merged?
- Does every rule's implementation get its own DICE key, or does one
  bzl file = one key?

Fixes here would show up as improvements in incremental-build wall time,
not cold wall. Plan-16 harness should include a representative
incremental case (touch one .cc, rebuild).

---

### 17.9 `kuro_error` allocation sites (OPEN)

`kuro_error::Error` boxing + backtrace capture can dominate error paths
(irrelevant on success) but also happens on every `internal_error!(...)`
creation even when it's the happy path. Grep for `internal_error!` in
hot loops; if any are in per-action paths, switch to `Result`-returning
patterns that avoid the allocation.

Low priority unless plan-16 flame graphs surface it.

---

## Dependencies and ordering

```
Plan 16 complete ─► 17.1 (pattern sweep — identifies new targets)
                ├─► 17.2 (scheduler — highest single-ticket win)
                ├─► 17.4 (file watcher — warm-analysis win)
                ├─► 17.5 (external-cell cache — cold-start win)
                ├─► 17.6 (starlark cache — cold-start win)
                ├─► 17.7 (prelude aftermath — correctness + speed)
                │
                ├─► 17.3 (signal pipeline micro — post-17.2 measurement)
                └─► 17.8 / 17.9 (exploratory, only if measurements demand)
```

Recommended: **17.1 first** (broadest information gain for low effort),
then **17.2** (largest expected single win), then fan out.

## Open questions

- Should 17.2's scheduler investigation land in a separate subplan if it
  turns out to be a rewrite, not a fix? **Resolved (2026-04-23):**
  17.2 was disproved; real work splits into 17.2-compile-outlier,
  17.2-dep-chain, and Plan 21 (warm-invocation overhead).
- Is there an `--incremental` mode in the Plan 16 harness? If not, add
  one before 17.8.

## Success criteria

- clang:clang cold wall within 5% of Bazel on the same hardware.
- `act/sec` on the Plan-16 harness ≥5.5.
- Warm analysis wall <1s.
- No regression on cache-hit %, action count, or critical-path wall
  compared to baseline.
- All landed fixes have a committed before/after delta in
  `benchmarks/<date>-<sha>/`.
- Pattern-sweep report filed; every P0/P1 hit resolved.
