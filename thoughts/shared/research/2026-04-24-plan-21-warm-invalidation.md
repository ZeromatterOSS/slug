# Plan 21.2 — CellResolver self-invalidates on every warm cquery

**Date:** 2026-04-24
**Workload:** `slug cquery @llvm-project//llvm:Demangle` on
`/var/mnt/dev/llvm-project/utils/bazel`
**Baseline:** warm cquery 1.85 s (matched Plan 21 headline).

## Summary

Warm cquery paid 1.3 s inside `load_package_futs.next()` because every
DICE key reachable from `CellResolverKey` was recomputed every
invocation. The cause was not the file watcher (it reported zero events
per warm sync, and `unstable_take` never fired). It was the
`CellResolver` InjectedKey itself: `changed_to` was being handed a new
`CellResolver` value each transaction whose `PartialEq` returned false
versus the previously-stored value, bumping the DICE version and
force-dirtying every dependent.

Three independent sources of non-determinism fed into the
`CellResolver` PartialEq mismatch. Each one alone was enough to defeat
caching; they had to all be fixed to get warm hits.

## Instrumentation

Added in 21.1 (committed 6c25ea29) — `SLUG_LOG_DICE=<path>` writes a
CSV row per `DiceTaskWorker::do_work` with outcome ∈ {`hit`, `reused`,
`miss_deps`, `miss_fresh`}. The split between `miss_deps` and
`miss_fresh` is the forensic clue: if 4151 of 4300 warm keys are
`miss_deps` (DICE had a prior value, re-ran because a dep was dirty),
something is being dirtied each transaction. If they were
`miss_fresh` (no prior entry), we'd be looking at cache eviction
instead.

## Disproved hypotheses

- **File-watcher reporting changes.** Added an eprintln in
  `NotifyFileWatcher::sync2`. Every warm run reported
  `events=0 ignored=0 missed=false`. `unstable_take` never fires.
- **Buck-out self-write flap.** Plan 17.4's filter in
  `slug_file_watcher::install_filtered_watches` is working as
  designed; no buck-out paths surface.
- **`ConcurrentTargetLabelInterner` fresh per transaction** — the
  CLAUDE.md comment at `ctx.rs:645` made this suspect, but its
  `PartialEq` returns `true` unconditionally, so the new Arc still
  compares equal to the old value through `BuildInterpreterConfiguror`.
  `BuildContextKey` shows up as `upd_noop` in the
  `update_state` accounting.

## The actual invalidator

Eight `changed_to` calls fire per transaction (one per InjectedKey).
Added per-key instrumentation in
`dice/dice/src/impls/transaction.rs::commit_to_state` to log the
`DiceKey` index and type name of each. On every warm run, seven
compared equal and one — always **`CellResolverKey`** — returned
false from its `equality` check.

Added a diagnostic PartialEq on `CellResolver` that printed which of
the three compared fields mismatched (`cells`, `root_cell`,
`root_cell_alias_resolver`) and which specific `CellInstance` entries
in the `cells` HashMap differed. Three categories of diff surfaced:

1. **`ExtensionRepoCellSetup.repo_spec_json` differed by JSON key
   order.** The value is produced by
   `serde_json::to_string(&RepoSpec)`; `RepoSpec.attributes` is a
   `HashMap<String, AttrValue>` whose iteration order leaked into the
   serialized JSON. Same content, different textual form, textual
   equality fails.
2. **`ExtensionRepoCellSetup.extension_id` differed by label form** —
   `"//cc:extensions.bzl%cc_configure_extension"` vs
   `"@rules_cc//cc:extensions.bzl%cc_configure_extension"` for the
   same extension. Two modules register the same extension via
   different labels (apparent vs canonical module name); whichever
   label is captured depends on iteration order upstream.
3. **`CellInstance.path` differed by apparent-vs-canonical module
   prefix.** Specifically,
   `bazel-external/rules_go+go_sdk+go_host_compatible_sdk_label` vs
   `bazel-external/io_bazel_rules_go+go_sdk+go_host_compatible_sdk_label`
   for the same `CellName`. Same root cause as (2) — the rules_go
   module uses `repo_name = "io_bazel_rules_go"`, so both prefixes are
   valid and `pre_compute_extension_repo_cells` generates a
   `PendingRepoCell` for each. The downstream dedup in
   `legacy_configs/cells.rs:463` keeps the first `PendingRepoCell`
   that wins for a given `CellName` — and "first" depends on
   iteration order.

The iteration orders were seeded by two HashMaps:

- `resolved_graph.modules: HashMap<String, ResolvedModuleInfo>` in
  `slug_bzlmod::resolution` — iterated at
  `legacy_configs/cells.rs:814` to build the `cells` Vec which seeds
  `parsed_modules` and ultimately `pre_computed_cells`.
- `extension_results: HashMap<String, (ModuleExtensionResult,
  Vec<UseRepo>)>` in `pending_repo_cells::build_all_extension_cells`.
- `ModuleExtensionResult.generated_repo_specs: HashMap<String,
  RepoSpec>` in `pending_repo_cells::build_extension_cells`.

## The fix (21.3)

Two parts, both minimal:

**Sort the HashMap iteration at every point where downstream dedup
picks a winner.** Three call sites, each keyed on the string name:

- `legacy_configs/cells.rs` — sort `resolved_graph.modules` by module
  name before converting to the `cells` Vec.
- `pending_repo_cells::build_all_extension_cells` — sort
  `extension_results` by extension id.
- `pending_repo_cells::build_extension_cells` — sort
  `result.generated_repo_specs` by internal name.

After this fix alone, `CellInstance` diffs for 243 cells dropped from
~12 per warm to 0. First-wins dedup now picks the same winner on
every invocation.

**Stop comparing textually-noisy `ExtensionRepoCellSetup` fields in
`CellInstance` equality.** The `canonical_name`, `extension_id`, and
`repo_spec_json` fields describe *how to re-fetch* the cell's content,
not what's currently on disk. Two instances with the same CellName,
path, and nested cells represent the same materialized repo; they
should compare equal even if the provenance metadata was captured in
different textual form. Added a `external_origin_eq` helper that
collapses `ExtensionRepo` comparison down to `internal_name +
materialized`; other variants use derived equality.

Without the sort fix, the `external_origin_eq` relaxation alone was
not enough — path differences still fired CellInstance mismatches and
flipped the root cell's `nested_cells` Vec order.

## Measurement (on the llvm-project workload)

Warm cquery wall (3 runs, same daemon):

| Target           | Before   | After            |
|------------------|---------:|-----------------:|
| llvm:Demangle    | 1.86 s   | **0.20 s**       |
| clang:clang      | 1.93 s   | **0.21 s**       |

Warm build wall (Demangle, 2 runs): 0.21–0.26 s (target <0.6 s).
Cold cquery unchanged at ~2.9 s.

## Alternative considered: swap `HashMap` → `FxHashMap` everywhere

A follow-up experiment tested the theory "just pick a deterministic
hasher and drop the sort." Reasoning: Rust's `HashMap` iteration
reveals bucket order, which is a pure function of `(hash(keys),
capacity)`; with a fixed-seed hasher those are stable across
invocations, so iteration order would be stable too.

**The theory is wrong for `hashbrown`.** Rust's `HashMap` uses
hashbrown (SwissTable — SIMD linear probing within 16-slot groups),
not Robin Hood hashing. Within-group collisions mean insertion order
affects which slot a key lands in, so iteration order still depends
on insertion order even with a deterministic hasher. Verified
empirically: same 40 string keys inserted into `HashMap<String, u32,
BuildHasherDefault<FxHasher>>` in forward vs reverse vs rotated
orders gives different iteration orders every time (`/tmp/slug-map-
bench/src/bin/order_test.rs`).

Swapping ~8 struct fields (`ResolvedGraph.modules`,
`ExtensionExecutionOutput.generated_repo_specs`,
`ModuleExtensionResult.generated_repo_specs + canonical_names`,
`RepoSpec.attributes`, `RepositoryInvocation.attrs`,
`AttrValue::Dict`, `RepoRuleInvocation.attrs`,
`RepoSpecRegistry.specs`) to `FxHashMap` did **not** fix the
invalidation — `parsed_modules` iteration still varied per warm
invocation because `selected: HashMap<String, Version>` upstream
still had random-state iteration, and the downstream FxHashMap
inherited that via insertion order.

Any fix has to inject a sort somewhere along the data-flow — either
at the consumer (sort-on-read, what 21.3 does) or at the producer
(sort before inserting into the final map). The choice of hasher is
orthogonal.

## References

- DICE hit/miss instrumentation: commit 6c25ea29 (21.1)
- Plan doc: `thoughts/shared/plans/slug-bazel-subplans/21-warm-invocation-overhead.md`
- CellInstance fix: `app/slug_core/src/cells/instance.rs`
- Sort fixes: `app/slug_bzlmod/src/pending_repo_cells.rs`,
  `app/slug_common/src/legacy_configs/cells.rs`
- Hashbrown order-invariance test: `/tmp/slug-map-bench/src/bin/order_test.rs`
