# Plan 21: Warm-invocation overhead

> Parent: [2026-01-21-kuro-bazel-compatible-build-tool.md](../2026-01-21-kuro-bazel-compatible-build-tool.md)
>
> Prerequisites: Plan 17.4 cold+self-invalidation fixes (landed
> 2026-04-23: commits `5a6c346a`, `0c326250`). Plan 20 complete
> (cold wall beats Bazel on this workload).

## Scope

Close the fixed per-invocation overhead kuro pays on *warm* queries
and builds. Every kuro command — even a no-op cquery on a trivially
small target — currently spends ~1.85 s in server-side work before
any user computation begins. Bazel's equivalent is ≈170 ms. The gap
is constant regardless of target size.

**Concrete goal.** Warm cquery of `@llvm-project//llvm:Demangle` drops
from 1.85 s to <0.5 s on the same machine. Warm build of the same
target drops proportionally. No regression in cold wall or correctness.

## Current State Analysis

### What the measurements show (post-Plan-17.4 fixes)

With the file-watcher notifying correctly and the
`ensure_external_symlink` flap suppressed, back-to-back warm cqueries
still consistently take 1.80–1.95 s:

    cquery llvm:Demangle warm (3 runs): 1.86 / 1.86 / 1.81 s
    cquery clang:clang  warm (3 runs): 1.89 / 1.98 / 1.95 s

The cost is constant per-invocation, not proportional to the
dependency graph size — a tiny target and the full clang graph
both cost the same.

### Server-side profile of one warm cquery

    total wall                          1.84 s
    ├─ client <→ daemon round trip       ~5 ms
    ├─ DiceUpdater::update
    │   ├─ existing_state                 0.00 s
    │   ├─ load_new_configs               0.05 s
    │   ├─ setup_interpreter              0.00 s
    │   └─ file_watcher.sync              0.00 s (post-17.4)
    ├─ cquery_command
    │   ├─ get_cell_resolver              0.00 s
    │   ├─ output_configuration           0.00 s
    │   ├─ global_cfg_options             0.00 s
    │   └─ eval_cquery                    1.60-1.70 s ← here
    │       └─ build_cquery_universe_from_literals
    │           └─ eval_literals<Configured>
    │               └─ load_compatible_patterns
    │                   ├─ load_patterns_with_modifiers
    │                   │   └─ load_package_futs.next()   **1.32 s**
    │                   └─ get_compatible_targets         0.23 s
    └─ result serialization + send       ~5 ms

`load_package_futs.next()` drives `ctx.get_interpreter_results(package)`
through DICE. On a warm daemon the result should come from the cache;
the fact that it takes 1.3 s every time points at either:

1. **DICE cache miss.** A dep (cell config / file fingerprint / something
   touched by `load_new_configs`) invalidates the `InterpreterResults`
   node on every command.
2. **DICE cache hit + slow traversal.** The cached result is a big
   structure (~7 kloc of BUILD.bazel → many `TargetNode`s) and the
   per-call plumbing that returns it has a fixed cost.

Hypothesis (1) is more likely given the 1.3 s scale. Repeat-commands
on Bazel take 10–100× less because Bazel's per-invocation work is
tight — a hit in the Skyframe cache returns almost for free.

### What 17.4's cold fix *did* remove

Pre-17.4: `external/*` symlinks got rewritten on every invocation,
the file watcher saw those as changes, DICE invalidated package loads,
load_package_futs re-parsed. Post-17.4: mtimes are stable (verified by
stat before/after a warm cquery), *no* "File changed" events surface
in the client output, but load_package_futs is still 1.3 s.

Meaning: some invalidation source remains that isn't the symlink flap.

### Suspects worth checking first

- **`load_new_configs`.** Runs at the start of every command. If it
  re-reads buckconfigs and signals "config changed" to DICE (even when
  content hash is identical), every dependent node invalidates.
- **`setup_interpreter`.** Creates a fresh `BuildInterpreterConfiguror`
  per transaction (`app/kuro_server/src/ctx.rs:636`). If the
  configuror's identity is part of any cache key (including the
  interpreter calculation), the fresh instance = fresh cache.
- **`ConcurrentTargetLabelInterner::default()`** at the same site.
  A fresh interner per transaction means target-label identities don't
  reuse across commands; downstream caches keyed on those interned
  labels can't hit.
- **File-watcher sync reporting zero-change but still touching DICE.**
  Even a "no changes" sync may call `unstable_take()` or similar on
  missed-event paths.

## Desired End State

- Warm cquery on any target ≤ 0.5 s (currently 1.85 s).
- Warm build on a small target (Demangle) ≤ 0.6 s including executor
  dispatch (currently 1.9 s).
- Cold cquery unchanged or better (currently 3.0 s).
- No regression on cold build wall, action count, or cache-hit %.

## Phases

### 21.1 DICE-hit vs DICE-miss visibility (OPEN, instrumentation)

Add one-shot instrumentation that tells us, on a warm command, which
nodes are cache hits vs misses:

1. Instrument `DiceComputations::compute` (or the next layer up) to
   log each key computed, with a flag whether the result came from the
   cache or ran the `compute` path.
2. Gate with `KURO_LOG_DICE=<path>` (mirror of
   `KURO_LOG_ADMISSION`). Zero cost when unset.
3. Run a single back-to-back `kuro cquery @llvm-project//llvm:Demangle`
   warm; diff the logs of the two runs. Any key that was a miss on the
   second is evidence of spurious invalidation.

**Success criteria:** a CSV per invocation with
`(key_type, key_string, hit_or_miss, us_inside_compute)` rows.

**Deferrals:** don't touch DICE internals deeper than the outermost
instrumentation point; that would risk invalidation-correctness bugs.

### 21.2 Identify the invalidation source (OPEN)

Using the DICE CSV from 21.1:

1. List keys that were misses on the warm run. Likely candidates:
   `LegacyBuckConfigKey`, `CellResolverKey`, `FileChangeNotifyKey`,
   `InterpreterResultsKey`.
2. For each miss, find what invalidated it. DICE tracks invalidations
   via version numbers; we need to trace which input key bumped
   versions between the two runs.
3. Root-cause: some process or path is reporting "this changed" every
   invocation. Identify the producer.

Most likely outcomes (in order):

a. `load_new_configs` signals config change because buckconfig file
   reads use mtime comparisons and our daemon re-reads on every
   command. Fix: cache the parsed `LegacyBuckConfig` keyed on
   `(path, mtime, size)` in-memory; return the same `Arc` when
   unchanged, so its DICE identity is stable.

b. A non-file DICE key (e.g., a `HostInfoKey` or `StartupContextKey`)
   has a non-deterministic hash. Fix: replace the changing part with
   a stable derivative.

c. `file_watcher.sync` is reporting "changed since last sync" on
   directories whose mtime the kernel bumps for access (unlikely with
   noatime, but worth ruling out).

### 21.3 Fix + validate (OPEN)

Apply the minimal change 21.2 points at. Validate with:

- Warm cquery on Demangle ≤ 0.5 s (3 runs, tight confidence interval).
- Warm cquery on clang:clang ≤ 0.7 s.
- Cold cquery unchanged (≤ 3.5 s).
- Action count unchanged on a cold clang:clang build.
- `kuro log summary` reports 100% DICE hits on the second warm
  invocation of the same query (target metric).

Commit per fix. If the root cause bisects into two separate sources,
commit separately.

### 21.4 Daemon startup residual (OPEN, secondary)

Cold cquery is 3.0 s on this machine; Bazel's is 4.9 s. We already
beat Bazel here, but ~2.5 s of kuro's cold is daemon-process startup
cost (Rust binary launch + basic runtime init) that doesn't benefit
from the 17.4 file-watcher fix. Not urgent, but worth a brief look
once 21.1-21.3 land:

- `kuro_daemon::daemon::exec` timeline: what takes the seconds between
  process exec and "Listening."? Goal: narrow the ~2 s gap between
  process start and first listening socket.

## Dependencies and ordering

```
21.1 DICE hit/miss instrumentation
    └─► 21.2 Identify invalidation source
            └─► 21.3 Fix + validate
                    └─► 21.4 Daemon startup residual (if budget remains)
```

Phases 21.1–21.3 are the critical path. 21.4 is exploratory.

## Open questions

- Does kuro's file watcher track buck-out/ writes? Our own log writes
  happen under buck-out/v2/log. If buck-out is excluded from
  ignore_specs, self-writes could trigger invalidations.
- Is the `ConcurrentTargetLabelInterner` new-per-transaction deliberate?
  The comment at `ctx.rs:645` says "New interner for each transaction",
  which smells like a correctness choice that also hurts cache reuse
  across commands.

## Success criteria

- Warm cquery on Demangle ≤ 0.5 s (currently 1.85 s). Target: 4× faster.
- Warm cquery on clang:clang ≤ 0.7 s (currently 1.9 s).
- Warm build on Demangle ≤ 0.6 s.
- All measurements repeated 3× with ≤ 10 % variance.
- No regression on cold wall, action count, critical path, or cache-hit %
  (compare to `benchmarks/post-plan-20-final-41ce00a5/summary.json`).

## References

- Warm-cquery profile: `thoughts/shared/research/plan-17-2-measure-findings.md`
- Post-Plan-20 baseline: `benchmarks/post-plan-20-final-41ce00a5/FINDINGS.md`
- File-watcher cold fix: commit `5a6c346a`
- Self-invalidation fix: commit `0c326250`
- Relevant source:
  - `app/kuro_server/src/ctx.rs:624-686` (`DiceUpdater::update`)
  - `app/kuro_node/src/load_patterns.rs:350-390`
    (`load_patterns_with_modifiers`)
  - `app/kuro_build_api/src/configure_targets.rs:226-260`
    (`load_compatible_patterns_with_modifiers`)
  - `app/kuro_query_impls/src/cquery/evaluator.rs:38-160` (`eval_cquery`)
  - `app/kuro_query_impls/src/dice.rs:351-370`
    (`eval_literals<ConfiguredTargetNode>`)
