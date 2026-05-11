# Plan 51: Kurod Memory Profiling

> Parent: [2026-01-21-kuro-bazel-compatible-build-tool.md](../2026-01-21-kuro-bazel-compatible-build-tool.md)
>
> Related: [Plan 16](./16-benchmark-telemetry.md),
> [Plan 17](./17-optimization.md),
> [Plan 21](./21-warm-invocation-overhead.md),
> [Plan 31](./31-bazel-perf-parity.md),
> [Plan 50](./50-canonical-label-architecture.md)

## Status: IN PROGRESS

## Current Status 2026-05-10

Follow-up smoke after sampled depset/cc checkpoints used isolation
`sdk-parity-20260510-sampled-checkpoints-1` and log
`/tmp/sdk-parity-20260510-sampled-checkpoints-1.log`. The depset/cc sampling
fix worked initially: the log stayed around 2.5 MiB through configured target
setup instead of hundreds of MiB. The run then paused in repository download
rather than analysis CPU. A gdb capture at
`/tmp/sdk-parity-20260510-sampled-checkpoints-1.gdb.txt` showed one worker in
`module_ctx.download -> repository_ctx::download_url -> Command::output`,
waiting on `curl -fsSL --max-time 300 https://index.crates.io/re/gi/region`.
After that bounded curl wait cleared, Kuro resumed Rust crate analysis.

The same smoke then exposed the next measurement artifact: the expanded
`analysis_starlark_call_sample` instrumentation flooded the log during Rust
crate analysis, growing it to about 541 MiB around
`crates__rand_distr-0.5.1//:rand_distr`. This was logging overhead, not a new
semantic Bazel mismatch. An intermediate sampler changed this to powers-of-two
sample counts plus first sightings of distinct call sites, but that still
needed another retry before drawing performance conclusions from the slice.

Fresh retry `sdk-parity-20260510-sampled-checkpoints-2` confirmed the first
distinct call-site clause was still too broad for `rules_rust`; after a bounded
crate-index curl wait cleared, the log grew to about 737 MiB around
`crates__actix-http-3.12.0//:actix-http`. The daemon was making analysis
progress and using CPU, so this again classified as instrumentation overhead.
The call sampler is now stricter: only powers-of-two interesting Starlark call
counts are logged per target.

Fresh retry `sdk-parity-20260510-sampled-checkpoints-3` reached the SDK
analysis tail quickly after another bounded crate-index curl wait
(`https://index.crates.io/fi/le/file-id`). It was making progress at
`completed=13044`, `active=371`, with daemon RSS around 720 MiB, and no
semantic Bazel-parity error was visible. The Starlark sampler was no longer
the dominant source, but general configured/analysis checkpoints still produced
about 504 MiB of log in roughly six minutes. The current slice now samples
high-volume configured-node, configured-gather, analysis-dependency,
analysis-phase, and toolchain-resolution checkpoints by powers of two plus
every 1024th event.

Fresh retry `sdk-parity-20260510-sampled-checkpoints-4` ran for the full
1200s timeout and emitted
`memory_smoke_summary elapsed_s=1202 peak_rss_kib=879768 final_rss_kib=704568`.
No terminal semantic Bazel-parity error was visible in
`/tmp/sdk-parity-20260510-sampled-checkpoints-4.log`; the only matched errors
were non-fatal toolchain/repository probing warnings. The run still showed
bounded memory, but the analysis tail remained slow: progress reached
`completed=13101`, `active=314`, with user-facing waits around
`zeromatter//lib/viz_tool:viz_tool` and other Rust rules. Log volume fell to
about 134 MiB, but the dominant remaining instrumentation stream was
`analysis_dep_request_start`/`analysis_dep_request_complete` with more than
230k checkpoint lines. The current slice adds those dep-request checkpoints to
the high-volume sampler before the next SDK smoke.

Fresh retry `sdk-parity-20260510-sampled-checkpoints-5` verified that the
dep-request sampling fix works. It ran for the full 1200s timeout and emitted
`memory_smoke_summary elapsed_s=1204 peak_rss_kib=898424 final_rss_kib=733548`.
The log stayed about 19 MiB; `analysis_dep_request_*` checkpoints dropped to
hundreds of lines instead of hundreds of thousands. No terminal semantic
Bazel-parity error was visible. The build still timed out in the same
slow-tail shape: daemon CPU stayed high, RSS stayed bounded, Starlark eval
heartbeats continued from `rules_rust`/`rules_cc`, and progress reached
`completed=13104`, `active=314`. The oldest active keys were still root SDK
aggregators waiting on leaves, while the visible leaf frontier moved through
`aws-smithy-http-client`, `resources`, `configs`, and `viz_tool`.

Current hypothesis: this does not look like a classic circular dependency or a
globally parked DICE future. The active root keys are old because they wait on
leaf analysis, and the daemon remains CPU-bound with Starlark heartbeats. The
frontier looks more like a small remaining Rust/`CcInfo` Starlark analysis tail
with expensive per-target rule implementation work, possibly multiplied across
multiple configurations. To test the DICE angle directly, the next useful
instrumentation should add per-active-key phase/current-child tracking or take
a gdb/task snapshot during the tail, looking for duplicate/non-converging
analysis keys rather than a whole-daemon hang.

Follow-up SDK smoke with active-analysis snapshots:

```sh
env KURO_MEMORY_CHECKPOINTS=1 \
  /var/mnt/dev/kuro/scripts/memory_smoke.sh \
    --interval 5 \
    --include-pgrep "kurod\\[zeromatter\\].*sdk-parity-20260510-active-snapshot-1" \
    -- \
    timeout 1200s \
      /var/mnt/dev/kuro/target/debug/kuro \
        --isolation-dir sdk-parity-20260510-active-snapshot-1 \
        build //sdk:sdk_contents \
  > /tmp/sdk-parity-20260510-active-snapshot-1.log 2>&1
```

The run was intentionally stopped after about 8.5 minutes with status `143`
because it had already produced the needed evidence and was still making slow
progress. No semantic Bazel-parity error was visible. RSS stayed bounded:
daemon RSS was about 708 MiB and sampled total RSS about 870 MiB near stop.
The active snapshot worked. It showed the oldest active key was consistently
the root `zeromatter//sdk:sdk_contents`, followed by SDK wrapper/aggregation
targets such as `sdk_with_configs`, `sdk_staged`, `zeromatter_ffi`, and CLI
targets. Those are long-lived because they wait on the leaf tail, not because
they are stuck in their own rule implementation. The visible leaf tail was
still Rust analysis, including waits such as
`crates__aws-sigv4-1.4.2//:aws-sigv4` and samples in
`rules_rust+0.69.0/rust/private/rustc.bzl`.

This smoke also exposed a measurement artifact: `KURO_MEMORY_CHECKPOINTS=1`
was producing hundreds of thousands of per-depset log lines. At stop,
`depset_create_live create_count` had exceeded `250k`, with many repeated
small shapes such as `transitive_len=16/17`, and `depset_to_list_*` checkpoints
were emitted for every Starlark `depset.to_list()`. The per-event tracing I/O
distorts the performance smoke. The current slice samples `depset_create_live`,
`depset_to_list_live`/`depset_to_list_frozen`, and `cc_internal_freeze` by
powers of two plus genuinely large payload shapes, preserving diagnostic maxes
while avoiding log-volume backpressure.

Loop iteration follow-up on `/tmp/sdk-parity-20260510-rust-sampler-1.log`:
classification remains Plan 51 slow-tail/performance, not a Plan 15 semantic
parity failure. The run timed out after steady progress with
`analysis_key_complete active=310 completed=13108 max_active=3682` and no
terminal Bazel-mismatch error. The final visible wait was
`zeromatter//lib/viz_tool:viz_tool` with 15 other analysis actions, while the
offline reconstructed active set still contained 309 analysis keys.

The expanded Starlark sampler narrowed the hot slow-tail to repeated
rules_rust per-crate analysis. Top sampled locations were
`rules_rust+0.69.0/rust/private/utils.bzl:386` (crate-name invalid-character
scan), `utils.bzl:227` (output-hash computation), `utils.bzl:529` (dependency
filtering), and `rust.bzl:146-155` (common Rust library setup). This suggests
many expensive Rust rule analyses rather than one stuck target. Add a gated
active-analysis snapshot so the next long smoke logs the oldest active analysis
keys directly instead of requiring post-run reconstruction from
`analysis_key_start`/`analysis_key_complete` pairs.

Latest bounded SDK parity smoke after expanding the Rust Starlark sampler:

```sh
env KURO_MEMORY_CHECKPOINTS=1 \
  /var/mnt/dev/kuro/scripts/memory_smoke.sh \
    --interval 5 \
    --include-pgrep "kurod\\[zeromatter\\].*sdk-parity-20260510-rust-sampler-1" \
    -- \
    timeout 1200s \
      /var/mnt/dev/kuro/target/debug/kuro \
        --isolation-dir sdk-parity-20260510-rust-sampler-1 \
        build //sdk:sdk_contents \
  > /tmp/sdk-parity-20260510-rust-sampler-1.log 2>&1
```

Result: timeout status `124` after 1201s, with no semantic analysis error
visible in `/tmp/sdk-parity-20260510-rust-sampler-1.log`.
`memory_smoke_summary elapsed_s=1201 peak_rss_kib=902552 final_rss_kib=726096`;
the largest daemon sample was about 737 MiB RSS. Memory remains bounded below
1 GiB, so this is a performance/slow-tail frontier rather than an unbounded-RSS
blocker.

The expanded sampler in `app/kuro_analysis/src/analysis/env.rs` worked: the log
now includes `analysis_starlark_call_sample` events for
`rules_rust+0.69.0/rust/private/rust.bzl`,
`rules_rust+0.69.0/rust/private/rustc.bzl`, and
`rules_rust+0.69.0/rust/private/utils.bzl`. The build continued making
analysis progress through the full timeout. It completed the previous
`tonic`/`arrow`/`aws-smithy-http-client` frontier, reached
`completed=13106` / `active=312` before timeout, then continued into targets
such as `crates__http-cache-0.19.0//:http-cache`. The last visible user-facing
waits were around `zeromatter//lib/viz_tool:viz_tool` while Rust rule evaluation
was still active.

Current handoff: keep Plan 51 active for the SDK slow-tail/performance
investigation, and do not classify this as a Plan 15 semantic parity blocker
unless a later smoke returns a concrete Bazel mismatch. Plan 17/31 performance
work is likely the next relevant track if repeated long smokes keep showing
bounded memory plus steady analysis progress.

## Previous Status 2026-05-10

Earlier bounded SDK parity smoke after the provider/`ctx.attr` parity slice:

```sh
timeout 900s env KURO_MEMORY_CHECKPOINTS=1 \
  /var/mnt/dev/kuro/scripts/memory_smoke.sh \
    --interval 5 \
    --include-pgrep "kurod\\[zeromatter\\].*sdk-parity-20260510-003745" \
    -- \
    /var/mnt/dev/kuro/target/debug/kuro \
      --isolation-dir sdk-parity-20260510-003745 \
      build //sdk:sdk_contents \
  > /tmp/sdk-parity-20260510-003745.log 2>&1
```

Result: outer timeout status `124` after 900s, with no semantic analysis error
visible in `/tmp/sdk-parity-20260510-003745.log`. Because the outer timeout
killed the wrapper, `memory_smoke_summary` was not emitted. Sampled process RSS
peaked around `858236` KiB and `kurod` reached about 699 MiB, so this is still
not an unbounded-RSS conclusion. The build continued making analysis progress:
it passed the prior `aws-sigv4` and `arrow` frontiers, completed roughly 13k
analysis keys, and was still evaluating a few hundred Rust external targets
near timeout, including `crates__tonic-prost-0.14.5//:tonic-prost`,
`crates__aws-smithy-runtime-1.10.3//:aws-smithy-runtime`, and
`crates__aws-smithy-http-client-1.1.12//:aws-smithy-http-client`.

The hottest Starlark locations in the smoke included
`rules_cc+0.2.17/cc/private/cc_info.bzl` and
`rules_rust+0.69.0/rust/private/rustc.bzl` / `rust/private/rust.bzl`, but the
existing call sampler only treated two `rules_rust` helper files as
interesting. The next smoke should use the expanded sampler in
`app/kuro_analysis/src/analysis/env.rs` so long-running Rust rule evaluation
emits useful `analysis_starlark_call_sample` events. Keep Plan 51 active for
this performance/frontier investigation unless the next run returns a concrete
Bazel parity error.

## Previous Status 2026-05-09

Latest bounded smoke after the Plan 44/BazelOutput declared-path slice:

```sh
bash -o pipefail -c 'timeout 220s env KURO_MEMORY_CHECKPOINTS=1 \
  /var/mnt/dev/kuro/scripts/memory_smoke.sh \
    --interval 5 \
    --include-pgrep '\''kurod\[zeromatter\].*plan44-bazel-output-path-1'\'' \
    -- \
    /var/mnt/dev/kuro/target/debug/kuro \
      --isolation-dir plan44-bazel-output-path-1 \
      build //sdk:sdk_contents \
  2>&1 | tee /tmp/plan44-bazel-output-path-1.log'
```

Result: Kuro status `3` after 158s,
`memory_smoke_summary elapsed_s=158 peak_rss_kib=802136 final_rss_kib=644308`.
This is still not a Plan 51 memory-retention conclusion: RSS stayed below 1 GiB
and the build failed with a concrete analysis error. The previous
`bazel_skylib` `select_file`/`__generate_glibc_stubs__` output-path blocker did
not recur. The current blocker belongs to Plan 15/rules_cc Starlark parity:
`rules_cc+0.2.17/cc/private/link/cpp_link_action.bzl:127` rejects
`object_files + additional_object_files` because Kuro does not support
`tuple + list`.

Keep Plan 51 parked unless a later Plan 15/54 smoke returns to unbounded RSS or
an unexplained daemon wait. The memory checkpoints remain useful for locating
the next analysis frontier.

### Previous status

Latest bounded smoke after the Plan 15 `cmd_args.add_all(map_each=...)`
sequence-return fix:

```sh
bash -o pipefail -c 'timeout 220s env KURO_MEMORY_CHECKPOINTS=1 \
  /var/mnt/dev/kuro/scripts/memory_smoke.sh \
    --interval 5 \
    --include-pgrep '\''kurod\[zeromatter\].*plan15-map-each-seq-1'\'' \
    -- \
    /var/mnt/dev/kuro/target/debug/kuro \
      --isolation-dir plan15-map-each-seq-1 \
      build //sdk:sdk_contents \
  2>&1 | tee /tmp/plan15-map-each-seq-1.log'
```

Result: Kuro status `3` after 190s,
`memory_smoke_summary elapsed_s=190 peak_rss_kib=891896 final_rss_kib=668632`.
This is still not a Plan 51 memory-retention conclusion: RSS stayed below 1 GiB
and the build failed with a concrete analysis error. The previous
`rules_cc` `cmd_args.add_all(..., map_each = ...)` `tuple (repr: ())` blocker
did not recur. The current blocker belongs to Plan 15:
`bazel_skylib+1.9.0/rules/select_file.bzl:36` cannot find the requested
`llvm+0.7.0//runtimes/glibc:libc.s` file among the
`generate_glibc_stubs` generated outputs.

Keep Plan 51 parked unless a later Plan 15/54 smoke returns to unbounded RSS or
an unexplained daemon wait. The memory checkpoints remain useful for locating
the next analysis frontier.

### Previous status

Latest bounded smoke after the Plan 15 provider-callable key fix:

```sh
bash -o pipefail -c 'timeout 220s env KURO_MEMORY_CHECKPOINTS=1 \
  /var/mnt/dev/kuro/scripts/memory_smoke.sh \
    --interval 5 \
    --include-pgrep '\''kurod\[zeromatter\].*plan15-provider-callable-1'\'' \
    -- \
    /var/mnt/dev/kuro/target/debug/kuro \
      --isolation-dir plan15-provider-callable-1 \
      build //sdk:sdk_contents \
  2>&1 | tee /tmp/plan15-provider-callable-1.log'
```

Result: Kuro status `3` after 184s,
`memory_smoke_summary elapsed_s=184 peak_rss_kib=902404 final_rss_kib=710988`.
This is still not a Plan 51 memory-retention conclusion: RSS stayed below 1 GiB
and the build failed with a concrete analysis error. The previous
`with_cfg/private/transitioning_alias.bzl:55 if provider in target` /
`AnalysisTestResultInfo ... got function` blocker did not recur. The current
blocker belongs to Plan 15: rules_cc calls
`actions.args().add_all(linker_inputs, map_each = map_each)` in
`rules_cc+0.2.17/cc/private/rules_impl/cc_static_library.bzl:174`; Kuro rejects
the map_each result `tuple (repr: ())` as a command-line item.

Keep Plan 51 parked unless a later Plan 15/54 smoke returns to unbounded RSS or
an unexplained daemon wait. The memory checkpoints remain useful for locating
the next analysis frontier.

## Previous Status 2026-05-09

Latest bounded smoke after the Plan 15 `ctx.toolchains` lookup normalization:

```sh
bash -o pipefail -c 'timeout 220s env KURO_MEMORY_CHECKPOINTS=1 \
  /var/mnt/dev/kuro/scripts/memory_smoke.sh \
    --interval 5 \
    --include-pgrep '\''kurod\[zeromatter\].*plan15-toolchain-label-canon-1'\'' \
    -- \
    /var/mnt/dev/kuro/target/debug/kuro \
      --isolation-dir plan15-toolchain-label-canon-1 \
      build //sdk:sdk_contents \
  2>&1 | tee /tmp/plan15-toolchain-label-canon-1.log'
```

Result: Kuro status `3` after 164s,
`memory_smoke_summary elapsed_s=164 peak_rss_kib=785084 final_rss_kib=605488`.
This is still not a Plan 51 memory-retention conclusion: RSS stayed below 1 GiB
and the build failed with a concrete analysis error. The previous
`@@rules_rust+0.69.0//rust:toolchain_type` unresolved lookup did not recur.
The current blocker belongs to Plan 15: C++ toolchain analysis reaches
`with_cfg.bzl+0.12.0/with_cfg/private/transitioning_alias.bzl:51` and fails on
`ctx.attr.exports[0]` with
`provider collection operation [] parameter type must be a provider type ... got int`.

Keep Plan 51 parked unless a later Plan 15/54 smoke returns to unbounded RSS or
an unexplained daemon wait. The memory checkpoints remain useful for locating
the next analysis frontier.

### Previous status

The latest Plan 54 smoke did not reproduce the prior indefinite low-RSS daemon
wait. With configured-node and analysis-dep checkpoints enabled, Kuro exited
with a real analysis error:

```sh
bash -o pipefail -c 'timeout 180s env KURO_MEMORY_CHECKPOINTS=1 \
  /var/mnt/dev/kuro/scripts/memory_smoke.sh \
    --interval 5 \
    --include-pgrep '\''kurod\[zeromatter\].*plan54-configured-gather-probe-1'\'' \
    -- \
    /var/mnt/dev/kuro/target/debug/kuro \
      --isolation-dir plan54-configured-gather-probe-1 \
      build //sdk:sdk_contents \
  2>&1 | tee /tmp/plan54-configured-gather-probe-1.log'
```

Result: Kuro status `3` after 158s,
`memory_smoke_summary elapsed_s=158 peak_rss_kib=653752 final_rss_kib=578572`.
This is not a Plan 51 memory-retention conclusion. The current blocker belongs
to Plan 15: `rules_rust//ffi/rs:empty_allocator_libraries` reaches
`ctx.toolchains` and reports that
`@@rules_rust+0.69.0//rust:toolchain_type` was not resolved, even though the
provider checkpoint used `@@rules_rust//rust:toolchain_type`.

Keep Plan 51 parked unless a later Plan 15/54 smoke returns to unbounded RSS or
an unexplained daemon wait. The added checkpoints in
`app/kuro_configured/src/nodes.rs` and
`app/kuro_analysis/src/analysis/calculation.rs` should remain useful for the
next bounded zeromatter smoke.

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

## Progress 2026-05-08: depset flattening checkpoints

Added `KURO_MEMORY_CHECKPOINTS`-gated checkpoints around Bazel depset
flattening so the next zeromatter repro can confirm or rule out repeated large
`depset.to_list()` expansion during analysis. The new checkpoint names are:

- `depset_to_list_frozen`
- `depset_to_list_live`

Each checkpoint records:

- direct element count at the root depset;
- transitive child count at the root depset;
- collected element count before value dedupe;
- deduped element count after `to_list()` value dedupe;
- duplicate element count.

This is intentionally diagnostic-only: when `KURO_MEMORY_CHECKPOINTS` is not
set, the live depset path avoids the extra direct/transitive extraction and the
existing `to_list()` behavior is unchanged.

- [x] Added depset flattening memory checkpoints
- [x] Ran `cargo fmt -- app/kuro_build_api/src/interpreter/rule_defs/depset.rs`
- [x] Ran `cargo check -p kuro_build_api`

## Progress 2026-05-09: post-Plan-54 memory follow-up

After completing Plan 54's depset/shared traversal/action-input work, reran
the zeromatter memory repro from `/var/mnt/dev/zeromatter`:

```sh
KURO_MEMORY_CHECKPOINTS=1 /var/mnt/dev/kuro/scripts/memory_smoke.sh \
  --interval 5 \
  --include-pgrep 'kurod\[zeromatter\].*plan54-followup' \
  -- /var/mnt/dev/kuro/target/debug/kuro \
     --isolation-dir plan54-followup \
     build //sdk:sdk_contents
```

The run was manually stopped before OOM risk. Evidence captured in
`/tmp/plan54-followup-memory.log`:

- sampled client+daemon RSS reached `14893728 KiB` at `09:22:52-07:00`;
- max checkpoint RSS reached at least `15334957056` bytes before shutdown;
- `depset_to_list_live` appeared 73 times;
- `depset_to_list_frozen` did not appear;
- `cell_alias_resolver_shared` appeared 541 times;
- status output was still in package-file-tree loading, for example
  `crates__windows-sys-0.61.2// -- loading package file tree`.

The important conclusion is that Plan 54 did not by itself close the zeromatter
RSS issue. The remaining blocker is still owned by this Plan 51 memory work.
The observed checkpoints suggest many repeated root-alias resolver creations
or lookups during external package loading, with only small live depset
flattening checkpoints (`direct_len=4`, `transitive_len=0`) at the sampled
peak. The next implementation slice should investigate why
`cell_alias_resolver_shared` is emitted hundreds of times during package tree
loading and whether resolver/package-loading state is being retained or rebuilt
per external package.

## Progress 2026-05-09: bzlmod alias resolver fast path

Investigated the repeated `cell_alias_resolver_shared` checkpoints during
zeromatter package-file-tree loading. The checkpoint is emitted when Kuro builds a
per-cell `CellAliasResolver` for an external repo cell. In bzlmod mode this is
not a root alias map clone: non-root resolvers use the root alias map through
the shared `Arc`, and the per-cell object is needed because `resolve("")`
depends on the current cell. The high RSS visible at that checkpoint is
therefore mostly ambient package-loading state, not the resolver allocation
itself.

The clear avoidable work was adjacent to those resolver creations:

- `CellAliasResolverKey::compute` fetched `LegacyBuckConfigForCellKey` before
  checking `is_bzlmod`, even though Bazel 9/Bzlmod ignores per-repository
  `.buckconfig` alias sections.
- `ImportPathsKey::compute` fetched an opaque legacy config for every cell only
  to pass a config view into `ImplicitImportPaths::parse`, whose Q1=B path does
  not read the config.

Implemented bzlmod fast paths that skip per-cell legacy buckconfig DICE nodes
in both places while preserving the per-cell resolver and existing memory
checkpoints.

Focused verification:

- `cargo fmt -- app/kuro_common/src/dice/cells.rs app/kuro_interpreter/src/import_paths.rs`
- `cargo check -p kuro_common -p kuro_interpreter`
- `cargo test -p kuro_common dice::cells --lib` (compiled; 0 matching tests)
- `cargo test -p kuro_interpreter import_paths --lib` (compiled; 0 matching
  tests)
- `cargo check -p kuro`
- `cargo build -p kuro`
- `git diff --check`

Bounded zeromatter rerun from `/var/mnt/dev/zeromatter`:

```sh
KURO_MEMORY_CHECKPOINTS=1 /var/mnt/dev/kuro/scripts/memory_smoke.sh \
  --interval 5 \
  --include-pgrep 'kurod\[zeromatter\].*plan51-bzlmod-fast-path' \
  -- timeout --signal=INT --kill-after=15s 180s \
     /var/mnt/dev/kuro/target/debug/kuro \
       --isolation-dir plan51-bzlmod-fast-path \
       build //sdk:sdk_contents
```

The run was stopped manually before OOM risk. Evidence is in
`/tmp/plan51-bzlmod-fast-path-memory.log`:

- sampled RSS reached `7585552 KiB` after `80s`;
- max checkpoint RSS reached `6858244096` bytes;
- `cell_alias_resolver_shared` appeared `417` times;
- `depset_to_list_live` appeared `45` times;
- `depset_to_list_frozen` did not appear;
- live depset checkpoints were still tiny (`direct_len=4`,
  `transitive_len=0`, `deduped_len=4` near the sampled peak);
- status was still package-file-tree loading, ending at
  `crates__windows-sys-0.61.2// -- loading package file tree`.

Conclusion: this slice removes unnecessary retained legacy-config state from
the bzlmod external-cell path, but `//sdk:sdk_contents` still does not build.
The next blocker is not root alias map reconstruction. It is the package-file
tree loading phase retaining or allocating several GiB while many external
repo packages are in flight. The next investigation should instrument or
profile package-listing/package-loading retained values directly (for example
`PackageListingKey`, directory reads, package listings by files/dirs/subpackage
counts, and DICE retained value sizes) instead of treating
`cell_alias_resolver_shared` as the allocation source.

## Progress 2026-05-09: package loading retained-memory instrumentation

Added `KURO_MEMORY_CHECKPOINTS`-gated instrumentation for the package-file-tree
path without deleting any existing checkpoints:

- `PackageListingKey` active/completed/max-active counters plus listing file,
  dir, subpackage, and approximate path-text sizes.
- Package listing builder counters for directories read, total entries read,
  and largest single directory.
- `ReadDirKey` active/completed/max-active counters plus entry/file/dir/symlink
  counts, name-byte totals, path length, and ignore-check mode. To keep output
  bounded, this checkpoint emits for large directories and each 1000th completed
  read.
- `InterpreterResultsKey` active/completed/max-active counters plus target
  count, import count, target-name bytes, and package-path length. This measures
  the package/build-file evaluation layer above package-file-tree listing.

Bounded package-listing/read-dir run from `/var/mnt/dev/zeromatter`:

```sh
KURO_MEMORY_CHECKPOINTS=1 /var/mnt/dev/kuro/scripts/memory_smoke.sh \
  --interval 5 \
  --include-pgrep 'kurod\[zeromatter\].*plan51-package-loading-instrumented' \
  -- timeout --signal=INT --kill-after=15s 95s \
     /var/mnt/dev/kuro/target/debug/kuro \
       --isolation-dir plan51-package-loading-instrumented \
       build //sdk:sdk_contents
```

Evidence in `/tmp/plan51-package-loading-instrumented-memory.log`:

- `memory_smoke_summary elapsed_s=86 peak_rss_kib=8884144 final_rss_kib=8884144`
- `package_listing_key_count=501`
- `package_completed=501`
- `package_max_active=91`
- `package_max_files=1532`
- `package_max_dirs=178`
- `package_max_path_bytes=96400`
- `package_max_dirs_read=179`
- `package_max_dir_entries_total=1586`
- `package_max_single_dir_entries=1127`
- `read_dir_checkpoint_count=5`
- `read_dir_completed=4000`
- `read_dir_max_active=135`
- `read_dir_max_entries=1127`
- `read_dir_max_name_bytes=22134`
- `depset_live_count=46`
- `cell_alias_resolver_shared_count=419`

The daemon was manually stopped after the run to avoid OOM risk. This ruled out
raw package listings and directory-entry vectors as the direct multi-GiB
retained value: the largest observed listing had under 100 KiB of path text,
and only 501 package listings plus 4000 directory reads had completed while RSS
was already near 8.9 GiB.

Bounded rerun with `InterpreterResultsKey` instrumentation:

```sh
KURO_MEMORY_CHECKPOINTS=1 /var/mnt/dev/kuro/scripts/memory_smoke.sh \
  --interval 5 \
  --include-pgrep 'kurod\[zeromatter\].*plan51-interpreter-results-instrumented' \
  -- timeout --signal=INT --kill-after=15s 60s \
     /var/mnt/dev/kuro/target/debug/kuro \
       --isolation-dir plan51-interpreter-results-instrumented \
       build //sdk:sdk_contents
```

Evidence in `/tmp/plan51-interpreter-results-instrumented-memory.log`:

- `memory_smoke_summary elapsed_s=65 peak_rss_kib=6803056 final_rss_kib=6688300`
- `package_listing_key_count=486`
- `package_completed=486`
- `package_max_active=90`
- `package_max_files=1532`
- `package_max_dirs=153`
- `package_max_subpackages=15`
- `package_max_path_bytes=96400`
- `package_max_dirs_read=154`
- `package_max_dir_entries_total=1586`
- `package_max_single_dir_entries=1127`
- `read_dir_checkpoint_count=4`
- `read_dir_completed=3000`
- `read_dir_max_active=105`
- `read_dir_max_entries=1127`
- `read_dir_max_name_bytes=22134`
- `interpreter_results_key_count=267`
- `interpreter_completed=267`
- `interpreter_max_active=251`
- `interpreter_max_targets=1536`
- `interpreter_max_imports=9`
- `interpreter_max_target_name_bytes=59699`
- `depset_live_count=44`
- `cell_alias_resolver_shared_count=431`

This run stopped at the time cap and the daemon was interrupted afterward. It
did not build `//sdk:sdk_contents`. The important new signal is that package
evaluation, not package listing payloads, is massively in flight:
`InterpreterResultsKey` reached 251 active computations while each completed
package result still reported modest target/import/name-byte counts. The next
slice should inspect why external package loading allows hundreds of concurrent
Starlark package evaluations and whether DICE retains evaluator/heaps/import
modules or transient package-evaluation state until the whole burst drains.
If the package-evaluation values themselves remain small under heap profiling,
the systemic fix is likely a bounded package-loading/evaluation concurrency
gate that preserves Bazel 9/Bzlmod semantics while preventing unbounded
external-repo fanout.

Focused verification for this slice:

- `cargo fmt -- app/kuro_common/src/file_ops/dice.rs app/kuro_common/src/package_listing/dice.rs app/kuro_common/src/package_listing/interpreter.rs app/kuro_common/src/package_listing/listing.rs app/kuro_interpreter_for_build/src/interpreter/calculation.rs`
- `cargo check -p kuro_common`
- `cargo check -p kuro_interpreter_for_build -p kuro_common`
- `cargo check -p kuro`
- `cargo build -p kuro`
- `cargo test -p kuro_common package_listing --lib`
- `cargo test -p kuro_common file_ops --lib`
- `cargo test -p kuro_interpreter_for_build interpreter::calculation --lib`
- `git diff --check`

## Progress 2026-05-09: package-evaluation concurrency gate and load-signal retention

Followed the previous `InterpreterResultsKey max_active=251` signal into
package/build-file evaluation. Bazel 9's source of truth for the relevant
shape is `src/main/java/com/google/devtools/build/lib/runtime/LoadingPhaseThreadsOption.java`
and `QuiescingExecutorsImpl.java`: `--loading_phase_threads=auto` is
host-resource based and feeds loading/analysis parallelism. Kuro currently
accepts `--loading_phase_threads` for Bazel CLI compatibility but marks it
ignored in `app/kuro_client_ctx/src/common.rs`, so this slice implemented only
the Bazel-style auto shape, not explicit flag plumbing.

Changes:

- Added a process-global package/build-file evaluation semaphore in
  `InterpreterResultsKey::compute`, sized to
  `kuro_util::threads::available_parallelism().max(1)`.
- Extended `interpreter_results_key` memory checkpoints with `queued`,
  `max_queued`, `concurrency_limit`, `wait_us`, and `dep_packages`.
- Removed one avoidable retained copy of successful package evaluation
  results from build-signal/critical-path metadata. `InterpreterResultsKey`
  activation data now stores only the compact cross-package dependency package
  list needed by `BuildSignalReceiver::enrich_load`; it no longer dupes and
  stores `Arc<EvaluationResult>` in `NodeExtraData::Load`.

The intermediate gated-only smoke,
`/tmp/plan51-package-eval-gated-memory.log`, confirmed the gate worked but did
not solve memory:

- `memory_smoke_summary elapsed_s=118 peak_rss_kib=16851400 final_rss_kib=16851400`
- `interpreter_max_active=16`
- `interpreter_max_queued=235`
- `interpreter_completed=593`
- `package_max_active=8`
- `package_completed=603`
- `read_dir_max_active=14`
- `read_dir_completed=5000`

The next compact-load run used:

```sh
KURO_MEMORY_CHECKPOINTS=1 /var/mnt/dev/kuro/scripts/memory_smoke.sh \
  --interval 5 \
  --include-pgrep 'kurod\[zeromatter\].*plan51-package-eval-gated-compact-load' \
  -- timeout --signal=INT --kill-after=15s 180s \
     /var/mnt/dev/kuro/target/debug/kuro \
       --isolation-dir plan51-package-eval-gated-compact-load \
       build //sdk:sdk_contents
```

It was manually interrupted at about 81s to avoid OOM risk; because the shell
pipeline was interrupted, `memory_smoke_summary` was not emitted. Evidence in
`/tmp/plan51-package-eval-gated-compact-load-memory.log`:

- sampled peak total RSS before interrupt: `12806228 KiB`;
- `interpreter_completed=469`;
- `interpreter_max_active=16`;
- `interpreter_max_queued=232`;
- `interpreter_max_targets=1536`;
- `interpreter_max_imports=9`;
- `interpreter_max_dep_packages=1246`;
- `package_completed=480`;
- `package_max_active=9`;
- `package_max_path_bytes=96400`;
- `package_max_files=1532`;
- `read_dir_completed=4000`;
- `read_dir_max_active=14`;
- `depset_max_collected=4`;
- `depset_max_deduped=4`.

Interpretation:

- The unbounded package/build-file evaluation fanout was real. The concurrency
  gate reliably caps active `InterpreterResultsKey` work at 16 on this host,
  with queued work making the fanout visible instead of creating hundreds of
  simultaneous evaluators/heaps.
- The build-signal `Arc<EvaluationResult>` retention was also real and worth
  removing: after compacting it, the run reached 469 completed package
  evaluations at ~12.8 GiB, compared with the gated-only run's 593 completed
  evaluations at ~16.9 GiB. This is a material reduction but not enough.
- Package-listing payloads, directory reads, and depset expansion remain ruled
  out as the direct multi-GiB payload source. Listings are still tiny, and
  depset checkpoints remain at length 4.
- The remaining memory growth tracks completed package evaluations even when
  active evaluation is capped and build signals no longer retain full
  `EvaluationResult`s. The next likely source is DICE-retained completed
  package values, Starlark modules/heaps/import module retention associated
  with those values, or DICE activation/dep graph state for the external
  package burst.

Focused verification for this slice:

- `cargo fmt -- app/kuro_interpreter_for_build/src/interpreter/calculation.rs app/kuro_build_signals_impl/src/lib.rs`
- `cargo check -p kuro_interpreter_for_build -p kuro_build_signals_impl`
- `cargo test -p kuro_build_signals_impl -p kuro_interpreter_for_build interpreter::calculation --lib`
- `cargo build -p kuro`
- `git diff --check`

## Progress 2026-05-09: loaded-module compaction and import-fanout signal

Followed the package-value retention signal into completed Starlark load
values and their import graph. `LoadedModule` used to retain a full
`LoadedModules` map for its direct imports. Because each imported module is
itself a DICE-cached `LoadedModule`, that made every completed module value
hold handles into a recursively expanded loaded-module graph. This slice
compacted `LoadedModule` so cached values retain only the frozen environment
and direct import paths. The audit server's `kuro audit starlark package-deps`
walk now reloads direct imports through DICE when it needs the postorder list,
instead of relying on cached recursive handles.

Added `EvalImportKey` memory checkpoints to distinguish package BUILD-file
evaluation from `.bzl` module loading. The first compact-module smoke,
`/tmp/plan51-loaded-module-compact-memory.log`, still timed out:

- `memory_smoke_summary elapsed_s=123 peak_rss_kib=16246220 final_rss_kib=16246220`
- `eval_import_key max_active=1091`
- `eval_import_key completed` reached about `69590`
- `interpreter_results_key max_active=16`

The new signal is that `.bzl` import evaluation has a much larger in-flight
burst than package BUILD-file evaluation. The compacted `LoadedModule` removes
one real completed-value retention edge, but the run still grew to about
15.5 GiB RSS before the timeout.

The next attempt reduced transient parse retention while modules wait for
their imports. `prepare_eval` now parses once to discover imports, drops that
AST before awaiting loaded deps, and reparses from the same DICE file content
only when the module has imports. This avoids retaining parsed `AstModule`
trees in parent load tasks while child modules evaluate.
`/tmp/plan51-ast-drop-compact-load-memory.log` still timed out:

- `memory_smoke_summary elapsed_s=102 peak_rss_kib=15767964 final_rss_kib=15767964`
- `eval_import_key max_active=1079`
- `eval_import_key completed=67000` at about `16023232512` RSS bytes
- `interpreter_results_key max_active=16`, `completed=560`, `max_queued=236`
- `package_listing_key max_active=7`, with tiny listing payloads
- depset expansion remained tiny (`direct_len=4`, `deduped_len=4`)

Interpretation:

- Recursive `LoadedModule` handle retention was real and is now removed from
  cached loaded-module values, but it was not the whole multi-GiB source.
- Retaining parsed ASTs while waiting on import deps was a plausible transient
  source and is now reduced, but it also did not materially change peak RSS in
  the bounded smoke.
- The strongest remaining signal is the unbounded `.bzl` import load burst:
  over one thousand active `EvalImportKey` computations and about 67k completed
  loaded modules by the 100s cap, while package evaluation remains capped at
  16 active computations and package/listing/depset payloads stay small.
- The next systemic fix should target DICE-retained loaded `.bzl`
  `FrozenModule` heaps and the activation/import graph. A naive semaphore held
  across `EvalImportKey::compute` would risk deadlocking recursive imports,
  because parent modules can wait for child modules while holding permits. Any
  loading-phase gate needs to be deadlock-safe: release before awaiting import
  deps, gate only the actual non-recursive parse/eval subphase, or introduce a
  scheduler that respects import ordering.

Focused verification for this slice:

- `cargo fmt -- app/kuro_interpreter/src/file_loader.rs app/kuro_interpreter_for_build/src/interpreter/calculation.rs`
- `cargo check -p kuro_interpreter -p kuro_interpreter_for_build`
- `cargo test -p kuro_interpreter file_loader --lib`
- `cargo test -p kuro_interpreter_for_build interpreter::calculation --lib`
- `cargo build -p kuro`
- `cargo fmt -- app/kuro_interpreter/src/file_loader.rs app/kuro_cmd_audit_server/src/starlark/package_deps.rs`
- `cargo check -p kuro_cmd_audit_server -p kuro_interpreter`
- `cargo fmt -- app/kuro_interpreter_for_build/src/interpreter/dice_calculation_delegate.rs app/kuro_interpreter_for_build/src/interpreter/calculation.rs`
- `cargo check -p kuro_interpreter_for_build`
- `cargo build -p kuro`

## Progress 2026-05-09: deadlock-safe module-evaluation gate

Added a loading-phase gate at the ready-to-evaluate module subphase, after
`prepare_eval` has loaded recursive imports. This avoids the deadlock shape
called out above: parent modules do not hold permits while waiting for child
`EvalImportKey` computations. The gate covers `StarlarkEvaluatorProvider`
construction plus `InterpreterForDir::eval_module`/freeze, and emits a new
`module_evaluation_phase` checkpoint with active/completed/max-active,
queued/max-queued, wait time, direct import count, and module path length.

This slice also removed a transient `LoadedModules` map clone from
`eval_starlark_module_uncached`: direct import paths are extracted before
evaluation and `LoadedModule::new_with_direct_imports` stores those paths in
the cached result after the evaluator's file-loader map has been consumed and
dropped.

Bounded zeromatter rerun from `/var/mnt/dev/zeromatter`:

```sh
KURO_MEMORY_CHECKPOINTS=1 /var/mnt/dev/kuro/scripts/memory_smoke.sh \
  --interval 5 \
  --include-pgrep 'kurod\[zeromatter\].*plan51-module-eval-gate' \
  -- timeout --signal=INT --kill-after=15s 180s \
     /var/mnt/dev/kuro/target/debug/kuro \
       --isolation-dir plan51-module-eval-gate \
       build //sdk:sdk_contents
```

Evidence is in `/tmp/plan51-module-eval-gate-memory.log`:

- `memory_smoke_summary elapsed_s=183 peak_rss_kib=23021976 final_rss_kib=23021976`
- `module_evaluation_phase max_active=16`, `max_queued=5`,
  `completed=100000`
- `eval_import_key max_active=1059`, `completed=100000`
- `interpreter_results_key max_active=16`, `max_queued=242`,
  `completed=818`
- `package_listing_key max_active=8`, `completed=833`; listings remained
  small
- `depset_to_list_live`/`depset_to_list_frozen` appeared near the end but with
  zero collected elements in the sampled checkpoints

Interpretation:

- The gate is deadlock-safe and correctly bounds the ready module
  parse/eval/freeze subphase.
- It does not solve the main RSS growth. Completed DICE-cached `.bzl`
  modules still reached 100k by the 180s timeout, and RSS climbed to about
  22 GiB. The strongest remaining signal is no longer simultaneous evaluator
  heap pressure; it is retained completed module state and/or duplicate module
  keying across generated external repos.
- The next slice should inspect why zeromatter reaches 100k completed
  `EvalImportKey`/`module_evaluation_phase` values. In particular, identify
  whether keys are semantically duplicate loads under different generated
  repo/cell paths, and whether Starlark loads that are only needed for package
  evaluation can avoid DICE-retaining full `FrozenModule` heaps after their
  dependent BUILD evaluation completes.

Focused verification for this slice:

- `cargo fmt -- app/kuro_interpreter/src/file_loader.rs app/kuro_interpreter_for_build/src/interpreter/dice_calculation_delegate.rs`
- `cargo check -p kuro_interpreter -p kuro_interpreter_for_build`
- `cargo test -p kuro_interpreter file_loader --lib`
- `cargo test -p kuro_interpreter_for_build interpreter::calculation --lib`
- `cargo build -p kuro`

## Progress 2026-05-09: bzlmod canonical loaded-module keys

Investigated the 100k completed `EvalImportKey`/`module_evaluation_phase`
smoke result. The root cause was that loaded `.bzl` module identity still
included Buck's top-level `BuildFileCell` dimension. In bzlmod mode the module
identity must be the canonical file label. Carrying the consuming BUILD file's
cell caused the same external `.bzl` file to be evaluated and DICE-retained
once per consuming generated repo/cell.

Implemented bzlmod canonicalization for loaded-module keys:

- `InterpreterCalculationImpl::get_loaded_module` now canonicalizes
  `LoadFile`/`JsonFile`/`TomlFile` import paths to
  `BuildFileCell::new(path.path().cell())` when `ctx.is_bzlmod()` is true.
  Legacy non-bzlmod behavior is unchanged.
- `InterpreterLoadResolver::resolve_load` creates canonical import paths in
  bzlmod mode, instead of threading the consuming package's
  `build_file_cell` into every load.
- `get_interpreter_calculator` normalizes the interpreter config key's
  `BuildFileCell` in bzlmod mode so package and `.bzl` evaluation agree on the
  same canonical module identity.
- `rules_cc` and `@kuro_builtins` autoloads are bzlmod-only and use canonical
  import paths. Legacy tests explicitly inject `is_bzlmod=false`.
- `eval_import_key` memory checkpoints now include `cross_cell`, which is 1
  when the loaded file cell differs from the build-file cell.

Focused verification for this slice:

- `cargo fmt -- app/kuro_interpreter_for_build/src/interpreter/calculation.rs app/kuro_interpreter_for_build/src/interpreter/dice_calculation_delegate.rs app/kuro_interpreter_for_build/src/interpreter/interpreter_for_dir.rs app/kuro_interpreter_for_build/src/interpreter/testing.rs app/kuro_interpreter_for_build_tests/src/tests.rs`
- `cargo check -p kuro_interpreter_for_build`
- `cargo test -p kuro_interpreter_for_build_tests test_eval_import -- --nocapture`
- `cargo test -p kuro_interpreter_for_build_tests test_load -- --nocapture`
- `cargo build -p kuro`

Bounded zeromatter rerun from `/var/mnt/dev/zeromatter`:

```sh
timeout 180s env KURO_MEMORY_CHECKPOINTS=1 \
  /var/mnt/dev/kuro/scripts/memory_smoke.sh \
    --interval 5 \
    --include-pgrep 'kurod\[zeromatter\].*plan51-bzlmod-import-key-2' \
    -- /var/mnt/dev/kuro/target/debug/kuro \
         --isolation-dir plan51-bzlmod-import-key-2 \
         build //sdk:sdk_contents \
  2>&1 | tee /tmp/plan51-bzlmod-import-key-2-memory.log
```

Evidence is in `/tmp/plan51-bzlmod-import-key-2-memory.log`:

- `memory_smoke_summary elapsed_s=142 peak_rss_kib=30713916 final_rss_kib=30206784`
- No `completed=100000` checkpoint was emitted.
- No `cross_cell=1` checkpoint was emitted; sampled `eval_import_key`
  checkpoints all had `cross_cell=0`.
- `eval_import_key` reached only about `completed=805`,
  `max_active=104`, down from the prior 100k completed / 1059 max-active
  shape.
- `module_evaluation_phase` stayed at the earlier bounded ready-evaluation
  shape instead of racing to 100k completed modules.
- The RSS spike now starts after the build reports:
  `Waiting on zeromatter//sdk:sdk_info_json (...) -- running analysis [evaluate_rule], and 15 other actions`.

Interpretation:

- The duplicate loaded-module retention problem for bzlmod cross-cell loads is
  fixed systemically by canonical keying, without holding the
  module-evaluation semaphore across recursive `eval_deps`.
- `//sdk:sdk_contents` still does not build under Kuro. The bounded smoke was
  stopped before OOM and peaked at about 30.7 GiB RSS during analysis, after
  module loading had largely settled.
- The next Plan 51 slice should move from loaded `.bzl` module retention to
  analysis retention/concurrency: provider values, configured target/action
  graph state, depset/list materialization, and any DICE keys retained by
  `evaluate_rule` for the `zeromatter//sdk:sdk_info_json` analysis frontier.
- The smoke also still shows bzlmod/module-extension semantic warnings such
  as missing extension aggregation entries and a `bazel_gazelle`/`gazelle`
  alias self-check failure. Those are covered by the existing module-extension
  ownership area (Plans 10/23/36/38), while the memory evidence in this slice
  points at analysis retention as the current Plan 51 blocker.

## Progress 2026-05-09: analysis live-heap and result-size checkpoints

Added analysis checkpoints around `AnalysisKey::compute`,
`get_dep_analysis`, and Starlark `evaluate_rule` completion. The checkpoints
report active/completed/max-active analysis keys plus result sizes:
retained/profile bytes, provider count, recorded action/action-data counts,
transitive-set count, and declared action/artifact counts. The underlying
helpers now expose recorded action counts and analysis result counts without
changing analysis semantics.

Also added a conservative immediate-dependency fanout batch in
`get_dep_analysis` (`128` deps per join-all chunk). This is intentionally not a
global semaphore and does not hold any permit across recursive dependency
analysis; it only prevents a single configured target from scheduling an
unbounded immediate dep vector at once. Query resolution now runs after direct
dependency analysis rather than racing in the same `try_compute2` call.

Focused verification for this slice:

- `cargo fmt -- app/kuro_analysis/src/analysis/calculation.rs app/kuro_build_api/src/analysis.rs app/kuro_build_api/src/analysis/registry.rs app/kuro_build_api/src/actions/registry.rs`
- `cargo check -p kuro_analysis -p kuro_build_api`
- `cargo check -p kuro_analysis`
- `cargo build -p kuro`

Bounded zeromatter smoke with checkpoints before the fanout batch:

```sh
timeout 180s env KURO_MEMORY_CHECKPOINTS=1 \
  /var/mnt/dev/kuro/scripts/memory_smoke.sh \
    --interval 5 \
    --include-pgrep 'kurod\[zeromatter\].*plan51-analysis-retention-1' \
    -- /var/mnt/dev/kuro/target/debug/kuro \
         --isolation-dir plan51-analysis-retention-1 \
         build //sdk:sdk_contents \
  2>&1 | tee /tmp/plan51-analysis-retention-1-memory.log
```

Evidence from `/tmp/plan51-analysis-retention-1-memory.log`:

- The bounded run timed out before a clean build.
- Peak sampled RSS was about `36444952` KiB.
- The waiting frontier moved to
  `rules_rust+0.69.0//ffi/rs:empty_allocator_libraries (...) -- running analysis [evaluate_rule], and 15 other actions`.
- `analysis_key_start` reached about `max_active=2214` and
  `completed=5840` before the large spike.
- Completed `analysis_evaluate_rule_result`/`analysis_key_complete` samples
  near the spike were small: mostly `retained_bytes` under a few KiB, tiny
  provider counts, and usually no recorded actions. The completed-result
  checkpoints do not explain the 30+ GiB RSS.
- There were no large depset/list materialization checkpoints during the
  rising part of the spike.

Bounded zeromatter smoke after the immediate-dependency batch:

```sh
timeout 180s env KURO_MEMORY_CHECKPOINTS=1 \
  /var/mnt/dev/kuro/scripts/memory_smoke.sh \
    --interval 5 \
    --include-pgrep 'kurod\[zeromatter\].*plan51-analysis-batched-deps-1' \
    -- /var/mnt/dev/kuro/target/debug/kuro \
         --isolation-dir plan51-analysis-batched-deps-1 \
         build //sdk:sdk_contents \
  2>&1 | tee /tmp/plan51-analysis-batched-deps-1-memory.log
```

Evidence from `/tmp/plan51-analysis-batched-deps-1-memory.log`:

- The bounded run timed out before a clean build.
- Peak sampled RSS was `38574352` KiB at `2026-05-09T10:50:32-07:00`.
  The `kuro_memory` max RSS checkpoint recorded `max_rss_bytes=39520804864`.
- After the live spike unwound, RSS fell quickly to about `197160` KiB and
  then hovered around `250000-300000` KiB while still waiting on the same
  target. This shows the 38 GiB shape is a live allocation burst, not retained
  completed analysis results or retained DICE module state.
- The waiting frontier remained
  `rules_rust+0.69.0//ffi/rs:empty_allocator_libraries (...) -- running analysis [evaluate_rule], and 15 other actions`.
- `analysis_key_start` reached `max_active=2216`.
- No `analysis_dep_batch_complete` checkpoint was emitted, so no immediate dep
  list on this path exceeded the `128`-dep batch threshold.
- Completed rule results after the live spike were still tiny. Representative
  samples include `retained_bytes=928`, `providers=2`, `actions=0`, and the
  largest repeated toolchain sample was `retained_bytes=4800`,
  `providers=2`, `actions=27`.

Interpretation:

- The bzlmod loaded-module duplicate retention problem remains fixed; this
  slice did not reintroduce `EvalImportKey` blowup.
- The current Plan 51 blocker is not completed analysis result retention and
  not a single huge immediate dependency fanout. It is a live heap burst while
  evaluating `rules_rust+0.69.0//ffi/rs:empty_allocator_libraries` and 15
  sibling analysis actions.
- The next slice should instrument inside the rule-evaluation live heap,
  especially `cc_common` and depset construction used by
  `rules_rust` allocator/toolchain helpers:
  `cc_common.create_library_to_link`, `create_linker_input`,
  `create_linking_context`, `merge_cc_infos`, and depset creation/union paths
  before `to_list`.
- The likely source Starlark path is
  `bazel-external/rules_rust+0.69.0/rust/private/rust_allocator_libraries.bzl`,
  where `_rust_allocator_libraries_impl` calls
  `toolchain.make_libstd_and_allocator_ccinfo(...)`; that helper builds
  `library_to_link` values, nested depsets, linking contexts, and merged
  `CcInfo` providers. A fix should be systemic in Kuro's provider/depset/
  `cc_common` representation, not a one-off `rules_rust` or zeromatter special
  case.

## Progress 2026-05-09: cc_common/depset live-heap checkpoints

Added `KURO_MEMORY_CHECKPOINTS`-gated live-heap checkpoints around the native
`cc_common` entry points that were suspected in the allocator/toolchain path:

- `cc_common_create_library_to_link`
- `cc_common_create_linker_input_start`
- `cc_common_create_linker_input_result`
- `cc_common_create_linking_context`
- `cc_common_create_linking_context_from_outputs`
- `cc_common_merge_linking_contexts`
- `cc_common_merge_cc_infos_collected`
- `cc_common_merge_cc_infos_result`

Also added depset creation metadata through `depset_create_live`, including
total create count, direct length before/after direct-element dedupe,
transitive child count, order id, element-type presence, emptiness, depth, and
max observed direct/transitive/depth values. The checkpoint is sampled at
powers of two and forced for depsets with at least 16 direct+transitive entries
or depth at least 16, so it stays lower volume than logging every tiny depset.

Because the `rules_rust` path loads `@rules_cc//cc/common:cc_common.bzl`,
which in `rules_cc+0.2.17` re-exports pure Starlark wrappers from
`cc/private/cc_common.bzl`, the native `CcCommonModule` methods above are not
the hot path for this target. Added `cc_internal_freeze` around
`_cc_internal.freeze(...)`, which those Starlark wrappers use after
`depset.to_list()` for pure-Starlark provider fields.

Focused verification for this slice:

- `cargo fmt -- app/kuro_build_api/src/interpreter/rule_defs/depset.rs app/kuro_build_api/src/interpreter/rule_defs/cc_common/actions.rs`
- `cargo check -p kuro_build_api`
- `cargo check -p kuro_analysis`
- `cargo build -p kuro`

Bounded zeromatter smoke with native cc/depset checkpoints:

```sh
timeout 180s env KURO_MEMORY_CHECKPOINTS=1 \
  /var/mnt/dev/kuro/scripts/memory_smoke.sh \
    --interval 5 \
    --include-pgrep 'kurod\[zeromatter\].*plan51-cc-liveheap-1' \
    -- /var/mnt/dev/kuro/target/debug/kuro \
         --isolation-dir plan51-cc-liveheap-1 \
         build //sdk:sdk_contents \
  2>&1 | tee /tmp/plan51-cc-liveheap-1-memory.log
```

Evidence from `/tmp/plan51-cc-liveheap-1-memory.log`:

- The bounded run timed out before a clean build.
- Sampled total RSS peaked at `772492` KiB.
- The daemon `max_rss_bytes` checkpoint peaked at `717438976`.
- The waiting frontier stayed
  `rules_rust+0.69.0//ffi/rs:empty_allocator_libraries (...) -- running analysis [evaluate_rule], and 15 other actions`.
- Native `cc_common_*` checkpoints did not fire; matches for `cc_common` in
  the log were target labels such as
  `rules_rust+0.69.0//rust/settings:experimental_use_cc_common_link`, not
  checkpoint names.
- `depset_create_live` max values were tiny:
  `create_count=230`, `direct_len=64`, `deduped_direct_len=64`,
  `max_transitive_len=2`, `max_depth=1`.
- `depset_to_list_*` remained tiny:
  `direct_len=9`, `transitive_len=1`, `collected_len=9`, `deduped_len=9`.

Bounded zeromatter smoke after adding `_cc_internal.freeze` checkpointing:

```sh
timeout 150s env KURO_MEMORY_CHECKPOINTS=1 \
  /var/mnt/dev/kuro/scripts/memory_smoke.sh \
    --interval 5 \
    --include-pgrep 'kurod\[zeromatter\].*plan51-cc-freeze-1' \
    -- /var/mnt/dev/kuro/target/debug/kuro \
         --isolation-dir plan51-cc-freeze-1 \
         build //sdk:sdk_contents \
  2>&1 | tee /tmp/plan51-cc-freeze-1-memory.log
```

Evidence from `/tmp/plan51-cc-freeze-1-memory.log`:

- The bounded run timed out before a clean build.
- Sampled total RSS peaked at `790084` KiB.
- The daemon `max_rss_bytes` checkpoint peaked at `708857856`.
- The waiting frontier stayed
  `rules_rust+0.69.0//ffi/rs:empty_allocator_libraries (...) -- running analysis [evaluate_rule], and 14 other actions`.
- `cc_internal_freeze` fired 11 times, but observed payloads were small or
  non-iterable (`iterable_len=0`, no depset payload).
- Native `cc_common_*` checkpoints still did not fire.
- `depset_create_live` max values stayed tiny:
  `create_count=228`, `direct_len=64`, `deduped_direct_len=64`,
  `max_transitive_len=2`, `max_depth=1`.
- `depset_to_list_*` stayed tiny:
  `direct_len=9`, `transitive_len=1`, `collected_len=9`, `deduped_len=9`.

Interpretation:

- The previous 38 GiB live heap burst did not reproduce under the added
  checkpointing. The instrumentation is likely perturbing enough scheduling or
  allocation timing to avoid the spike, so this is not evidence of a semantic
  fix.
- The current stuck target still does not build under Kuro. The failure shape
  in these bounded runs is now a stalled analysis frontier with low steady RSS,
  not a retained-result or depset/list blowup.
- The suspected `cc_common.create_*` calls in
  `rules_rust+0.69.0/rust/private/rust_allocator_libraries.bzl` route through
  `rules_cc+0.2.17`'s Starlarkified `cc_common` provider wrappers, not Kuro's
  native `CcCommonModule` methods. Future instrumentation should target
  Starlark evaluation of those wrapper functions and provider construction,
  especially:
  `rules_cc+0.2.17/cc/private/link/create_linker_input.bzl`,
  `create_library_to_link.bzl`,
  `create_linking_context_from_compilation_outputs.bzl`, and
  `cc/private/cc_info.bzl`.
- The next useful Plan 51 step is a lower-perturbation stack/progress
  checkpoint for live `evaluate_rule`: record the current Starlark call stack
  or function-name counters for long-running analysis tasks, plus native
  provider construction sizes for pure Starlark providers. That should explain
  whether `empty_allocator_libraries` is spinning, blocked, or repeatedly
  traversing provider/depset values before any native `cc_common` method is
  reached.

## Progress 2026-05-09: Starlark rules_cc provider/call-stack checkpoints

Implemented the next lower-perturbation checkpointing slice:

- Added an embedder-owned `CallStackCheckpoint` hook to the vendored Starlark
  evaluator. Kuro installs it only when `KURO_MEMORY_CHECKPOINTS=1`.
- Added `analysis_starlark_call_sample` in analysis evaluation. It samples
  Starlark call-stack pushes for targeted wrapper files:
  `rules_cc+0.2.17/cc/private/link/create_linker_input.bzl`,
  `create_library_to_link.bzl`,
  `create_linking_context_from_compilation_outputs.bzl`,
  `cc/private/cc_info.bzl`,
  `rules_rust+0.69.0/rust/private/rust_allocator_libraries.bzl`, and
  `rust/private/cc/cc_utils.bzl`.
- Added pure Starlark provider construction checkpoints:
  `init_provider_call_start`, `init_provider_call_result`,
  `user_provider_create`, and `user_provider_create_schemaless`, scoped to
  providers defined under `rules_cc+0.2.17/cc/private`.

Focused verification:

```sh
cargo fmt -- \
  starlark-rust/starlark/src/eval/runtime/evaluator.rs \
  starlark-rust/starlark/src/eval.rs \
  app/kuro_analysis/src/analysis/env.rs \
  app/kuro_build_api/src/interpreter/rule_defs/provider/user.rs \
  app/kuro_build_api/src/interpreter/rule_defs/provider/callable.rs

cargo check -p starlark -p kuro_analysis -p kuro_build_api
cargo build -p kuro
```

Bounded zeromatter smoke:

```sh
timeout 110s env KURO_MEMORY_CHECKPOINTS=1 \
  /var/mnt/dev/kuro/scripts/memory_smoke.sh \
    --interval 5 \
    --include-pgrep 'kurod\[zeromatter\].*plan51-starlark-progress-2' \
    -- /var/mnt/dev/kuro/target/debug/kuro \
         --isolation-dir plan51-starlark-progress-2 \
         build //sdk:sdk_contents \
  2>&1 | tee /tmp/plan51-starlark-progress-2-memory.log
```

Evidence from `/tmp/plan51-starlark-progress-2-memory.log`:

- The bounded run still did not complete `//sdk:sdk_contents`.
- Build ID: `cf5b7b40-7ace-403b-8f0f-93bd9a493381`.
- Sampled total RSS peaked at `821628` KiB.
- The daemon `max_rss_bytes` checkpoint peaked at `748875776`.
- The waiting frontier again stayed
  `rules_rust+0.69.0//ffi/rs:empty_allocator_libraries (...) -- running analysis [evaluate_rule], and 15 other actions`.
- The Starlark sampler fired: `38` structured
  `analysis_starlark_call_sample` events, plus matching trace lines.
  Sampled files were:
  - `rules_cc+0.2.17/cc/private/cc_info.bzl`: `36` trace lines.
  - `rules_rust+0.69.0/rust/private/cc/cc_utils.bzl`: `2` trace lines.
- The hot sampled source locations in `cc_info.bzl` were the `_flat_depset`
  helper and `_merge_compilation_contexts` construction path:
  lines `397`, `400`, `405`, `406`, and the `CcCompilationContextInfo(...)`
  field construction around lines `448`-`464`.
- Pure Starlark provider construction was confirmed but remained small:
  `16` structured `user_provider_create` events, `4` start/result init-provider
  pairs, and no schemaless provider construction events.
- Provider names observed in the trace were limited to `CcInfo`,
  `_UnboundValueProviderDoNotUse`, `CcCompilationContextInfo`,
  `CcLinkingContextInfo`, `CcCompilationOutputsInfo`,
  `CcNativeLibraryInfo`, `CcDebugContextInfo`,
  `ExtraLinkTimeLibrariesInfo`, and `LtoCompilationContextInfo`.
- The largest observed provider schema/value width was
  `CcCompilationContextInfo` with `26` fields. `CcInfo` init-provider results
  had `4` fields.

Interpretation:

- The Starlarkified `rules_cc` provider wrappers are definitely on the path,
  and `cc_info.bzl`'s `_flat_depset` logic is visible in the live samples.
- The run still does not reproduce the earlier 38 GiB burst. The new
  checkpointing stayed under roughly 0.8 GiB RSS, so this remains diagnostic
  evidence, not a semantic or memory fix.
- The provider construction counts and field widths are too small to explain a
  large heap burst by themselves.
- The persistent blocker is still the live `evaluate_rule` for
  `rules_rust+0.69.0//ffi/rs:empty_allocator_libraries`. The next useful slice
  is to distinguish "actively executing Starlark bytecode" from "blocked inside
  an awaited DICE dependency" for that target, preferably by adding a bounded
  per-target live-evaluator heartbeat/checkpoint that can report the current
  stack/location while the wait frontier is stuck, without relying only on
  call-stack pushes.

## Progress 2026-05-09: evaluate_rule phase heartbeat and DICE-wait boundary

Added a bounded live-evaluator heartbeat/checkpoint for Starlark bytecode:

- The vendored Starlark `CallStackCheckpoint` hook now also has an
  `on_infrequent_instr_check` callback from the evaluator's existing
  infrequent bytecode check path. It is inert unless an embedder installs the
  checkpoint.
- Analysis installs the callback only under `KURO_MEMORY_CHECKPOINTS=1`.
  `analysis_starlark_eval_heartbeat` reports target, current top
  file/function/line/column, stack depth, bytecode check count, and elapsed
  time at bounded intervals.
- Added `analysis_evaluate_rule_phase` checkpoints across the
  `evaluate_rule` body: attr eval, execution platform lookup, toolchain
  resolution, context toolchain-provider analysis, Starlark provider setup,
  rule impl invocation, promise resolution, provider collection, and freeze.

Focused verification:

```sh
cargo fmt -- app/kuro_analysis/src/analysis/env.rs starlark-rust/starlark/src/eval/runtime/evaluator.rs
cargo check -p starlark -p kuro_analysis
cargo check -p kuro_build_api
cargo build -p kuro
git diff --check
```

Bounded zeromatter smokes from `/var/mnt/dev/zeromatter`:

```sh
timeout 120s env KURO_MEMORY_CHECKPOINTS=1 \
  /var/mnt/dev/kuro/scripts/memory_smoke.sh \
    --interval 5 \
    --include-pgrep 'kurod\[zeromatter\].*plan51-eval-heartbeat-1' \
    -- /var/mnt/dev/kuro/target/debug/kuro \
         --isolation-dir plan51-eval-heartbeat-1 \
         build //sdk:sdk_contents \
  2>&1 | tee /tmp/plan51-eval-heartbeat-1-memory.log

timeout 75s env KURO_MEMORY_CHECKPOINTS=1 \
  /var/mnt/dev/kuro/scripts/memory_smoke.sh \
    --interval 5 \
    --include-pgrep 'kurod\[zeromatter\].*plan51-eval-heartbeat-2' \
    -- /var/mnt/dev/kuro/target/debug/kuro \
         --isolation-dir plan51-eval-heartbeat-2 \
         build //sdk:sdk_contents \
  > /tmp/plan51-eval-heartbeat-2-memory.log 2>&1

timeout 60s env KURO_MEMORY_CHECKPOINTS=1 \
  /var/mnt/dev/kuro/scripts/memory_smoke.sh \
    --interval 5 \
    --include-pgrep 'kurod\[zeromatter\].*plan51-eval-heartbeat-3' \
    -- /var/mnt/dev/kuro/target/debug/kuro \
         --isolation-dir plan51-eval-heartbeat-3 \
         build //sdk:sdk_contents \
  > /tmp/plan51-eval-heartbeat-3-memory.log 2>&1
```

Evidence:

- `//sdk:sdk_contents` still did not complete under Kuro.
- The latest build ID was `48b28345-cc83-4518-b6b5-835b0d93986b`.
- The waiting frontier remained
  `rules_rust+0.69.0//ffi/rs:empty_allocator_libraries (...) -- running
  analysis [evaluate_rule], and 14 other actions`.
- `analysis_starlark_eval_heartbeat` count was `0` while the target was stuck.
  That means the stuck frontier was not actively executing Starlark bytecode,
  so there is no current Starlark stack/location to report for the target.
- Both configurations of `empty_allocator_libraries` reached
  `ctx_toolchain_provider_analysis_start` immediately after
  `toolchain_resolution_complete` and never reached
  `ctx_toolchain_provider_analysis_complete`,
  `starlark_provider_start`, or any rule-impl phase before the timeout.
- Latest sampled RSS was low and stable relative to the earlier live burst:
  `/tmp/plan51-eval-heartbeat-3-memory.log` peaked at `772480 KiB` total RSS
  and ended at `729228 KiB`.

Interpretation:

- The current low-RSS stall is not a Starlark bytecode spin in
  `empty_allocator_libraries`; it is an awaited DICE dependency inside the
  context toolchain-provider analysis block that prepares `ctx.toolchains`
  values after toolchain resolution.
- The next slice should instrument that block per resolved toolchain type and
  implementation label, then inspect the dependency cycle/frontier for the
  toolchain impl analysis requested by `empty_allocator_libraries`. The likely
  immediate area is `run_analysis_with_env_underlying` in
  `app/kuro_analysis/src/analysis/env.rs`, specifically the loop that calls
  `dice.get_analysis_result(&configured).await` while constructing
  `resolved_toolchains_for_ctx`.

## Progress 2026-05-09: ctx.toolchains DICE await frontier

Instrumented the `resolved_toolchains_for_ctx` loop in
`app/kuro_analysis/src/analysis/env.rs` with
`analysis_ctx_toolchain_provider` checkpoints. Each record names the
requesting target, resolved toolchain type, implementation label, configured
implementation target, mandatory bit, self-edge bit, loop index, and whether
the awaited analysis completed.

Focused verification:

```sh
cargo fmt -- app/kuro_analysis/src/analysis/env.rs
cargo check -p kuro_analysis
cargo build -p kuro
git diff --check
```

Bounded zeromatter smoke from `/var/mnt/dev/zeromatter`:

```sh
timeout 90s env KURO_MEMORY_CHECKPOINTS=1 \
  /var/mnt/dev/kuro/scripts/memory_smoke.sh \
    --interval 5 \
    --include-pgrep 'kurod\[zeromatter\].*plan51-toolchain-await-1' \
    -- /var/mnt/dev/kuro/target/debug/kuro \
         --isolation-dir plan51-toolchain-await-1 \
         build //sdk:sdk_contents \
  2>&1 | tee /tmp/plan51-toolchain-await-1-memory.log
```

Evidence from `/tmp/plan51-toolchain-await-1-memory.log`:

- `//sdk:sdk_contents` still did not complete under Kuro.
- Build ID: `187c01d4-c3bb-494a-b348-53ba21022d67`.
- Sampled total RSS peaked at `819664 KiB`; daemon checkpoint max RSS peaked
  at `716673024` bytes. This was again a low-RSS stall, not a high-RSS burst.
- Structured provider-await status counts were `18` `analysis_start`, `3`
  `analysis_compatible`, and `1` `unresolved_optional`.
- The displayed stuck frontier remained
  `rules_rust+0.69.0//ffi/rs:empty_allocator_libraries (...) -- running
  analysis [evaluate_rule], and 14 other actions`.
- The new await records show the immediate waits:
  - `empty_allocator_libraries` in the target configuration waits on mandatory
    Rust toolchain providers from
    `rules_rust+rust+rust_linux_x86_64__x86_64-unknown-linux-gnu__stable_tools//:rust_toolchain`;
  - `empty_allocator_libraries` in the exec/host configuration waits directly
    on optional C++ toolchain providers from
    `llvm+toolchain+llvm_toolchains//:linux_x86_64_cc_toolchain`;
  - that Rust toolchain target itself waits on optional C++ toolchain providers
    from the same configured LLVM C++ toolchain.
- The deeper frontier is the LLVM C++ toolchain analysis:
  `analysis_key_start` appears for
  `llvm+toolchain+llvm_toolchains//:linux_x86_64_cc_toolchain` in both
  configurations, but there is no `analysis_deps_ready`,
  `analysis_aspects_ready`, or `analysis_evaluate_rule_phase` for either
  configured key before timeout.
- Other in-flight targets such as `bazel_tools//tools/cpp:malloc`,
  `bazel_tools//tools/cpp:link_extra_lib`,
  `rules_cc+0.2.17//:link_extra_lib`,
  `glibc_headers_x86_64-linux-gnu.2.28//:gnu_libc_headers`, and
  `linux_kernel_headers_x86.4.19.325//:kernel_headers` also started
  `ctx.toolchains` provider awaits on the same configured LLVM C++ toolchain.

Interpretation:

- The current blocker is not a Starlark bytecode spin and not a memory
  retention problem. It is a DICE dependency cycle/frontier around C++ toolchain
  analysis:
  `empty_allocator_libraries -> rust_toolchain/cc toolchain provider ->
  llvm_toolchains//:linux_x86_64_cc_toolchain -> cc_library/toolchain support
  deps -> same llvm C++ toolchain provider`.
- This is outside Plan 51's memory scope. Existing Plan 15 owns the relevant
  Bazel 9 parity gap because Kuro still treats `cc_toolchain` as a native
  minimal stub and lacks Bazel's real C++ toolchain analysis/provider behavior.
  Plan 15 now has a note for this dependency-cycle failure shape.

## Progress 2026-05-09: Plan 15 cycle breaker result

Plan 15 took over the C++ toolchain await frontier and added a systemic
cycle-safe path for Bazel's C++ toolchain type during `ctx.toolchains` provider
construction. When the selected configured C++ toolchain implementation is
already an active analysis key, Kuro now returns a minimal synthetic
`ToolchainInfo(cc=..., cc_provider_in_toolchain=True)` / `CcToolchainInfo`
provider instead of awaiting that same analysis result.

Focused verification from `/var/mnt/dev/kuro`:

```sh
cargo fmt -- app/kuro_analysis/src/analysis/calculation.rs app/kuro_analysis/src/analysis/env.rs app/kuro_build_api/src/interpreter/rule_defs/context.rs app/kuro_build_api/src/interpreter/rule_defs/cc_common/feature_config.rs app/kuro_build_api/src/interpreter/rule_defs/platform_common.rs
cargo check -p kuro_build_api
cargo check -p kuro_analysis
cargo build -p kuro
```

Bounded zeromatter smoke from `/var/mnt/dev/zeromatter`:

```sh
timeout 180s env KURO_MEMORY_CHECKPOINTS=1 \
  /var/mnt/dev/kuro/target/debug/kuro \
    --isolation-dir plan15-cc-toolchain-cycle-1 \
    build //sdk:sdk_contents \
  2>&1 | tee /tmp/plan15-cc-toolchain-cycle-1-memory.log
```

Evidence:

- `//sdk:sdk_contents` no longer hangs at the old low-RSS LLVM C++ toolchain
  provider frontier.
- `/tmp/plan15-cc-toolchain-cycle-1-memory.log` contains
  `active_cc_toolchain_synthetic` records for the prior support-dep path:
  `bazel_tools//tools/cpp:link_extra_lib` and
  `rules_cc+0.2.17//:link_extra_lib` both request
  `@@bazel_tools//tools/cpp:toolchain_type` and receive the synthetic provider
  while the same configured
  `llvm+toolchain+llvm_toolchains//:linux_x86_64_cc_toolchain` key is active.
- The run advances into `rules_cc+0.2.17//:link_extra_lib` rule analysis and
  fails with:

  ```text
  error: depset elements must not be mutable values
    bazel-external/rules_cc+0.2.17/cc/private/rules_impl/cc_library.bzl:221
  ```

Interpretation:

- Plan 51's memory work is still not the owner for the current blocker. The
  former DICE await frontier is addressed enough to expose the next semantic
  failure.
- The current failure is covered by Plan 54's depset/provider immutability and
  `cc_common.create_linking_context` work. Continue there before returning to
  a full Plan 51 smoke.

## Progress 2026-05-09: current smoke after lockfile pre-seed guard

A fresh bounded smoke from `/var/mnt/dev/zeromatter` using
`--isolation-dir plan15-lockfile-preseed-zstd-1` and log
`/tmp/plan15-lockfile-preseed-zstd-1.log` did not reproduce the earlier
`rules_rust//ffi/rs:empty_allocator_libraries` low-RSS timeout. It ran to
the 180s bound and exited with Kuro status `3` after analysis produced a
real error:

```text
Error running analysis for `zstd//:zstd`
error: depset elements must not be mutable values
  bazel-external/rules_cc+0.2.17/cc/private/link/create_linking_context_from_compilation_outputs.bzl:127
```

The old `@@zstd//:` package-load failure is gone; the materialized
`crates__zstd-sys` BUILD now references `@@zstd//:zstd`. Peak RSS was about
599 MiB and final RSS about 516 MiB. This is still not a Plan 51 memory
retention issue; the active blocker remains Plan 54 provider/depset
immutability, specifically the `LibraryToLink` value returned from
`cc_common.create_linking_context_from_compilation_outputs`.

## Progress 2026-05-09: post-Plan 54 LibraryToLink freeze smoke

Plan 54 added recursive dict normalization to `_cc_internal.freeze`, covering
the `LibraryToLink` mutable field shape exposed at `zstd//:zstd`.

Focused verification from `/var/mnt/dev/kuro` passed:

```sh
cargo fmt
cargo check -p kuro_build_api
cargo test -p kuro_build_api_tests depset -- --nocapture
cargo build -p kuro
pytest -q tests/core/cc_common/test_link.py
git diff --check
```

Fresh bounded zeromatter smoke from `/var/mnt/dev/zeromatter`:

```sh
timeout 180s env KURO_MEMORY_CHECKPOINTS=1 \
  /var/mnt/dev/kuro/target/debug/kuro \
    --isolation-dir plan54-library-dict-freeze-1 \
    build //sdk:sdk_contents \
  2>&1 | tee /tmp/plan54-library-dict-freeze-1.log
```

Result: the old
`rules_cc+0.2.17/cc/private/link/create_linking_context_from_compilation_outputs.bzl`
`depset([cc_linking_outputs.library_to_link])` mutable-value error is gone.
The run reached `zeromatter//sdk:sdk_contents` analysis and timed out at the
already-tracked `rules_rust//ffi/rs:empty_allocator_libraries` analysis wait
with 5 other actions. A repository-rule side signal also appeared:
`llvm+llvm_source+llvm-raw` failed `http_bsdtar_archive` at
`rctx.execute([rctx.path(host_bsdtar)] + args)` with `No such file or
directory`, then Kuro created a stub. This still leaves Plan 51/15's
`empty_allocator_libraries` toolchain-analysis wait as the active frontier,
not a completed `//sdk:sdk_contents` build.

## Progress 2026-05-09: optional toolchain retry predicate slice

Plan 15 investigated the current
`rules_rust//ffi/rs:empty_allocator_libraries` frontier and found that
`rust_allocator_libraries` declares an optional C++ toolchain with
`mandatory = False`. Kuro's deferred toolchain retry predicate treated any
unresolved toolchain as retry-worthy, including optional misses; Bazel 9 only
blocks on missing mandatory toolchains. The predicate now preserves optional
misses without forcing a deferred-load retry.

Fresh bounded smoke log:
`/tmp/plan15-optional-toolchain-retry-1.log`. Result: the build still timed
out at `empty_allocator_libraries`, and observed instances still stopped before
`toolchain_resolution_complete`; no Starlark heartbeat or call samples appeared
for that rule, and the earlier LLVM `http_bsdtar_archive` side signal did not
recur. This remains a toolchain-resolution await frontier, not a Plan 51 memory
retention conclusion. The next owner should add checkpoints inside
`resolve_toolchain_types()` before pursuing another semantic fix.

## Progress 2026-05-09: hashable dict freeze smoke exposes allocator LTL mutability

Plan 54's hashable dict-shaped `_cc_internal.freeze` smoke was run from
`/var/mnt/dev/zeromatter` with fresh isolation
`plan54-hashable-dict-freeze-1`:

```sh
timeout 220s env KURO_MEMORY_CHECKPOINTS=1 \
  /var/mnt/dev/kuro/scripts/memory_smoke.sh \
    --interval 5 \
    --include-pgrep 'kurod\[zeromatter\].*plan54-hashable-dict-freeze-1' \
    -- \
    /var/mnt/dev/kuro/target/debug/kuro \
      --isolation-dir plan54-hashable-dict-freeze-1 \
      build //sdk:sdk_contents \
  2>&1 | tee /tmp/plan54-hashable-dict-freeze-1.log
```

Result: Kuro exited status `3` after 179s, with
`memory_smoke_summary elapsed_s=179 peak_rss_kib=771808 final_rss_kib=611132`.
This was not a Plan 51 memory-retention conclusion. The old
`create_library_to_link.bzl:106 Object of type tuple has no attribute keys`
failure is cleared. The run advanced to a new semantic blocker:
`rules_rust+0.69.0/rust/private/rust_allocator_libraries.bzl:118` creates
depsets of `_ltl(...)` values, where `_ltl` returns
`cc_common.create_library_to_link(static_library = library,
pic_static_library = library)`. Kuro rejects those direct `LibraryToLink`
provider elements as mutable. Keep Plan 51 parked until Plan 54 identifies and
fixes the remaining non-hashable provider field.
