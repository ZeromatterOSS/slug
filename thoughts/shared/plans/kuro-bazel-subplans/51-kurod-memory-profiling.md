# Plan 51: Kurod Memory Profiling

> Parent: [2026-01-21-kuro-bazel-compatible-build-tool.md](../2026-01-21-kuro-bazel-compatible-build-tool.md)
>
> Related: [Plan 16](./16-benchmark-telemetry.md),
> [Plan 17](./17-optimization.md),
> [Plan 21](./21-warm-invocation-overhead.md),
> [Plan 31](./31-bazel-perf-parity.md),
> [Plan 50](./50-canonical-label-architecture.md)

## Status: IN PROGRESS

## Goal

Profile and reduce `kurod` memory usage during large bzlmod/zeromatter
builds. Recent local runs showed `kurod` consuming more than 20 GB of
memory. Before optimizing broadly, identify whether memory is being spent
on expected retained graph state or on avoidable behavior such as giant
diagnostic strings, duplicated alias maps, retained event payloads, or
repeated cloned repository metadata.

The goal is not merely to lower one repro's peak RSS. The goal is to make
memory growth explainable, bounded where possible, and covered by enough
instrumentation that future regressions are easy to diagnose.

## Recent Trigger

While verifying Plan 50 from `/var/mnt/dev/zeromatter`:

```sh
/var/mnt/dev/kuro/target/debug/kuro \
  --isolation-dir verify-canonical-label-architecture \
  build //sdk:sdk_contents
```

the build failed during analysis after loading many external repositories.
The output included a very large diagnostic for an unknown cell alias:

- `Toolchain package '@rules_python+python+pythons_hub//:all' load failed`
- `unknown cell alias: '@rules_python'`
- the diagnostic rendered a huge "known aliases are" list

That diagnostic path is a priority suspect because large rendered strings
can be allocated repeatedly and retained in errors, event logs, or analysis
state.

## Investigation Questions

- Does RSS grow steadily, spike in one phase, or stay high after a failed
  build returns?
- Is memory mostly retained graph state, temporary allocations, daemon
  caches, diagnostics/events, or duplicated repository/cell data?
- Are large cell alias maps cloned per package, configuration, target, or
  error?
- Are rendered error strings retained when structured errors would suffice?
- Does event logging buffer huge diagnostics before flushing?
- Does `bazel-external` or package tree scanning retain full path lists for
  many repositories?
- Does the daemon release memory between builds, or does warm daemon state
  accumulate unexpectedly?

## Phase 1: Fast Triage

Capture process-level memory and daemon state for the running or next
repro:

```sh
pgrep -af kurod
ps -o pid,ppid,rss,vsz,etime,nlwp,cmd -p <pid>
pmap -x <pid> | tail -n 20
lsof -p <pid> | wc -l
```

Track RSS over time while reproducing:

```sh
while true; do
  date
  ps -o pid,rss,vsz,nlwp,etime,cmd -p <pid>
  sleep 5
done
```

Record whether memory climbs during:

- daemon startup;
- bzlmod module/lockfile loading;
- external repository materialization;
- package loading;
- configured target analysis;
- diagnostic rendering;
- event log flushing;
- idle time after a failed build.

## Phase 2: Narrow Reproductions

Use the full zeromatter repro as the baseline, then reduce it:

1. Full repro:

   ```sh
   /var/mnt/dev/kuro/target/debug/kuro \
     --isolation-dir verify-canonical-label-architecture \
     build //sdk:sdk_contents
   ```

2. A smaller target that loads bzlmod and external repositories but avoids
   full SDK analysis.

3. A target or synthetic fixture that triggers the unknown-cell-alias
   diagnostic with a large alias set.

4. A warm-daemon second run to distinguish one-time loading from retained
   daemon growth.

For each repro, record:

- peak RSS;
- RSS after failure or success;
- elapsed time;
- last high-level build phase before the spike;
- whether the large alias diagnostic was emitted.

## Phase 3: In-Process Memory Checkpoints

Add temporary low-overhead tracing around high-cardinality phases. Each
checkpoint should log current RSS plus relevant object counts.

Candidate checkpoint sites:

- bzlmod lockfile decode;
- pending repo/cell registration;
- dynamic extension-cell registration;
- package loading;
- configured target analysis;
- cell alias map creation;
- diagnostic construction and rendering;
- event emission and event log buffering/flushing.

Useful counts:

- registered cells;
- aliases per cell;
- total alias entries;
- loaded packages;
- configured targets;
- configured target graph nodes;
- repository specs;
- extension repos;
- pending materialization entries;
- retained diagnostics/events;
- total bytes in rendered diagnostic strings, if available.

Prefer a helper that can be left behind under a debug flag or tracing
target instead of one-off prints.

## Phase 4: Heap Profiling

Run at least one reproduction under an allocation profiler.

Preferred tools, in order:

1. `heaptrack` for actionable allocation call stacks.
2. `valgrind --tool=massif` if `heaptrack` is unavailable, accepting the
   slower run.
3. allocator-native profiling if the daemon already uses, or can easily
   use, a profiling allocator.

Example:

```sh
heaptrack /var/mnt/dev/kuro/target/debug/kuro \
  --isolation-dir verify-canonical-label-architecture \
  build //sdk:sdk_contents
```

The output should identify:

- top allocation sites;
- top retained allocation sites;
- repeated allocation hot loops;
- large strings, vectors, maps, or graph nodes;
- whether diagnostic/event rendering dominates the spike.

## Phase 5: Specific Suspects

### Giant Diagnostics

The unknown-cell-alias diagnostic currently can render an enormous alias
list. Audit:

- where the alias list is built;
- whether the list is sorted/cloned for every error;
- whether the full rendered string is stored in multiple places;
- whether the event log retains it.

Likely guardrail:

- cap displayed aliases to a small number, for example 50 or 100;
- include total alias count;
- include a short hint for how to inspect the full alias set when needed.

### Alias Map Cloning

Audit cell alias data structures for accidental full clones across:

- cells;
- packages;
- configurations;
- target nodes;
- toolchain contexts;
- diagnostics.

Likely guardrails:

- share alias maps behind `Arc`;
- intern common strings;
- store canonical identifiers instead of repeated rendered labels where
  possible.

### Event and Error Retention

Audit whether structured errors are rendered early and then retained as
large strings.

Likely guardrails:

- render diagnostics late;
- cap event payload sizes;
- avoid storing both structured and rendered forms unless needed;
- truncate repeated contextual lists in event payloads.

### Package and Directory State

Audit external package loading and `bazel-external` scanning for retained
path lists or per-repo directory snapshots.

Likely guardrails:

- stream scans instead of retaining full lists;
- avoid duplicate absolute and project-relative path storage;
- keep fallback scan results scoped to the operation unless cached
  intentionally.

## Phase 6: Fixes and Guardrails

Apply fixes in order of confidence:

1. Cap huge diagnostics and event payloads.
2. Replace obvious full-map clones with shared data.
3. Avoid retaining rendered error strings where structured errors exist.
4. Add focused memory checkpoints behind tracing/debug flags.
5. Add a smoke benchmark or script that records peak RSS for a
   representative bzlmod load.

Any guardrail should preserve enough information to debug real failures.
Truncation should say exactly how many entries were omitted.

## Acceptance Criteria

- We can reproduce and report peak RSS for the baseline zeromatter command.
- We know which phase accounts for the largest memory spike.
- Heap profiling identifies the top retained allocation categories.
- Unknown-cell-alias diagnostics no longer render or retain unbounded alias
  lists.
- Alias/cell/repo metadata is not cloned per package or configured target
  without a clear reason.
- Event logging cannot retain arbitrarily large rendered diagnostics.
- A repeatable memory smoke command exists, even if it is initially manual.

## Verification

Minimum verification for the first implementation pass:

- `cargo fmt` on touched Rust files.
- `cargo check` for touched crates.
- Re-run the baseline zeromatter command and record peak RSS.
- Capture before/after examples for the unknown-cell-alias diagnostic.
- Confirm truncated diagnostics include total counts.

Optional but preferred:

- `heaptrack` or Massif report attached or summarized in the plan progress.
- Warm-daemon second run showing whether RSS stabilizes or grows.

## Progress

- First implementation pass:
  - capped unknown-cell-alias diagnostics to 50 displayed aliases and a
    total/omitted count, so errors no longer clone/render unbounded alias
    lists;
  - interned `CellAlias` and `NonEmptyCellAlias` using the existing
    `static_interner` pattern already used for `CellName`, reducing repeated
    alias string clones in maps and diagnostics;
  - added `KURO_MEMORY_CHECKPOINTS`-gated tracing checkpoints for lockfile
    reads/cache inserts, bzlmod extension repo precompute, lockfile spoke
    seeding, extension cell aggregation, and cell resolver construction;
  - added `scripts/memory_smoke.sh` for repeatable RSS sampling by PID, pgrep
    pattern, or wrapped command.
- Short zeromatter repro capture from `/var/mnt/dev/zeromatter`:
  - command:
    `KURO_MEMORY_CHECKPOINTS=1 scripts/memory_smoke.sh --interval 5 --include-pgrep 'kurod\[zeromatter\].*verify-canonical-label-architecture-memory-profile-3' -- target/debug/kuro --isolation-dir verify-canonical-label-architecture-memory-profile-3 build //sdk:sdk_contents`;
  - failed early on `unknown cell alias: bazel_lib`;
  - diagnostic was bounded to 50 displayed aliases:
    `showing 50 of 5518; 5468 omitted`;
  - peak sampled client+daemon RSS: 302204 KiB;
  - final sampled daemon RSS after failure: 225944 KiB;
  - checkpoint highlights:
    - `bzlmod_pre_compute_extension_repo_cells`: 71 parsed modules, 380 cells,
      401 aliases, RSS 196427776 bytes in daemon;
    - `bzlmod_pre_compute_extension_repo_cells_from_lockfile`: 380 existing
      cells, 4708 new lockfile-seeded cells, RSS 207536128 bytes;
    - `legacy_cells_bzlmod_precomputed_repos`: 5088 precomputed cells,
      401 aliases, RSS 207945728 bytes;
    - `cell_alias_resolver_new`: 5518 aliases, RSS 214892544 bytes;
  - `cell_resolver_new`: 5147 cells and 5518 root aliases, RSS 215416832

## Progress 2026-05-08: package-listing preallocation guard

After root bzlmod aliases and native platform-label fixes, the zeromatter
`//sdk:sdk_contents` build reached package loading and failed while waiting on
`crates__aws-smithy-http-0.63.6// -- loading package file tree` with:

```text
memory allocation of 1715238139729024 bytes failed
```

The failure shape points at `Directory::flatten()` in
`app/kuro_common/src/package_listing/interpreter.rs`, which preallocated three
vectors from recursively accumulated directory counters before collecting the
actual entries. That makes package listing vulnerable to a bad/corrupt
bookkeeping count or unexpected external-repo directory shape turning into a
huge allocation before any real data is pushed.

Implemented a systemic guard: `flatten()` now builds the output vectors with
`Vec::new()` and lets actual collection drive allocation growth. This preserves
the same listing semantics and removes the unbounded trust in recursive
preallocation counts.

- [x] Removed recursive-counter-based preallocation from package listing
- [x] Re-ran zeromatter `//sdk:sdk_contents`: the petabyte allocation is gone

The follow-up run progressed past `aws-smithy-http` and then the daemon
segfaulted while many package-file-tree loads were still active, with RSS
above 10 GiB. `build -j 2` did not materially reduce loading concurrency.

Second guard implemented in the same package-listing subsystem:
`Directory::gather_subdirs()` now walks subdirectories sequentially within a
single package instead of spawning a DICE `compute_join` for every child
directory. Kuro still has package-level parallelism, but a large external repo
no longer multiplies that with intra-package directory fan-out and many
partially retained directory trees.

- [x] Removed intra-package package-listing fan-out
- [x] Re-ran zeromatter `//sdk:sdk_contents` after sequential traversal

The clean rerun still failed in package loading, this time while waiting on
`crates__diplomat-runtime-0.15.1// -- loading package file tree`, with:

```text
memory allocation of 4497183668736512 bytes failed
```

Third package-listing guard: replace the recursive `Directory` tree with a
streaming DFS that writes files, directories, and subpackages directly into the
final listing vectors. This removes the recursive bookkeeping counters entirely
and avoids retaining every intermediate directory node plus its child vectors
until a later flatten pass. The traversal remains deterministic and still
stops at subpackage boundaries, but package listing now has one representation
of discovered paths instead of both a tree and the final sorted listing.

- [x] Replaced recursive package-listing tree/flatten with streaming DFS
- [x] Re-ran zeromatter `//sdk:sdk_contents` after streaming DFS

The OOM persisted with the same failure phase, so the next step is to identify
the exact allocation site rather than continuing to guess from the status line.
Added a daemon allocation-error hook that prints the failed layout and a forced
backtrace before aborting. This is general memory diagnostic infrastructure and
should make future oversized allocation failures actionable.

- [x] Added daemon allocation-error backtrace hook
- [x] Re-ran zeromatter `//sdk:sdk_contents` with allocation backtraces

The backtrace showed this was not package-listing memory pressure. The daemon
panicked in `Vec::clone` while cloning `NestedCells` from
`new_cell_ignores()`, with Rust's UB check reporting an invalid slice:

```text
unsafe precondition(s) violated: slice::from_raw_parts requires the pointer
to be aligned and non-null, and the total size of the slice not to exceed
isize::MAX
```

Root cause: `CellResolver::get()` returned references to dynamic
`CellInstance` values stored inside a `HashMap` after dropping the map lock.
Later dynamic-cell insertions can rehash the map and invalidate those
references while async package/file operations still hold them. That explains
the earlier impossible petabyte allocation sizes: they were symptoms of a
corrupted `Vec`, not legitimate allocation requests.

Systemic fix: dynamic cells are now stored as leaked `CellInstance`s in the
dynamic-cell map, so references returned by `CellResolver::get()` remain stable
even when more dynamic cells are inserted. This matches the previous comment's
intended lifetime guarantee without relying on invalid `HashMap` references.

- [x] Made dynamic cell instance references stable across map insertions
- [x] Re-run zeromatter `//sdk:sdk_contents` after dynamic-cell reference fix

## Progress 2026-05-08: confirmed high-RSS analysis phase

After the dynamic-cell reference fix, the impossible petabyte allocation
failures disappeared. The zeromatter build now consistently reaches package
loading and then analysis before the daemon dies from real memory pressure.

Confirmed follow-up fixes:

- root `bazel_dep(repo_name = ...)` apparent aliases are now registered in the
  bzlmod root alias map, fixing early failures such as `@bazel_lib` and
  `@com_google_protobuf`;
- root-cell native platform labels now render in Bazel form (`//pkg:tgt`)
  instead of Kuro cell form (`zeromatter//pkg:tgt`);
- non-root cells with no local alias list now share the root alias map through
  `Arc` instead of cloning thousands of aliases per `CellAliasResolver`;
- toolchain, extension, repo-rule, and extension-aggregation warning payloads
  are truncated to bounded summaries;
- extension repo stubs now record a RepoSpec hash in `.kuro_repo_complete`, so
  repeated accesses to the same failed repo rule can reuse the stub until the
  RepoSpec changes instead of deleting and re-running the same failing rule.

Measured zeromatter runs from `/var/mnt/dev/zeromatter`:

- `codex-plan51-shared-alias`: alias-map sharing fixed repeated
  `cell_alias_resolver_new` clones, but package loading still climbed to
  roughly 14 GiB with checkpoints enabled;
- `codex-plan51-no-checkpoints`: without checkpoints, package loading reached
  roughly 21 GiB before toolchain/extension analysis;
- `codex-plan51-listing-bound`: a package-listing global semaphore was tried
  and removed because it did not materially reduce peak RSS;
- `codex-plan51-diag-cap`: diagnostic caps reduced output volume, but the
  daemon still died near `44,213,512 KiB`;
- `codex-plan51-extdiag-cap`: extension aggregation warnings were bounded, but
  the daemon still died near `46,168,728 KiB`;
- `codex-plan51-stub-hash`: spec-hashed stubs reduced one repeated-failure
  source but the daemon still died near `43,652,188 KiB`;
- `codex-plan51-depset-dedupe`: an experimental hashed `depset.to_list()`
  dedupe was tried but the host OOMed before a useful conclusion; that
  experiment was removed so the next investigation can focus cleanly on a
  systemic depset/transitive_set migration.

The current reproducible failure shape is:

```text
Waiting on rules_rust+0.69.0//ffi/rs:empty_allocator_libraries
(//bazel/platforms:linux-gnu-host#...) -- running analysis [evaluate_rule],
and 15 other actions
```

The target package itself is tiny. The likely remaining systemic issue is the
analysis representation of large depsets/toolchain providers. `rules_rust`'s
allocator/toolchain path calls into large toolchain depsets (notably
`cc_toolchain.all_files.to_list()` and the libstd allocator `CcInfo` assembly),
and Kuro's current Bazel depset implementation is independent from Buck's
transitive_set machinery. The next promising line of work is to migrate or wrap
Bazel `depset` on top of transitive_set so large toolchain DAGs remain shared
instead of being repeatedly flattened or retained in Starlark heap structures.

At this point the remaining `//sdk:sdk_contents` failure is not an early alias,
label, package-listing UB, or unbounded diagnostic issue. It is a genuine
high-RSS analysis problem around large bzlmod/toolchain graphs.
