# Plan 16: Benchmark-grade telemetry

> Parent: [2026-01-21-kuro-bazel-compatible-build-tool.md](../2026-01-21-kuro-bazel-compatible-build-tool.md)
>
> Follows Plan 15 (Bazel 9 parity). Foundation for Plans 17 (optimization)
> and 18 (BEP parity).

## Scope

Make it possible to answer "did this change help or hurt build performance,
and where?" without guesswork. Phase-level, action-level, and per-mnemonic
rollups from both live builds and saved event logs, plus a harness for
repeatable before/after comparisons.

Premise: Kuro already captures the data. The event stream
(`app/kuro_data/data.proto`, ~2400 lines) emits timed spans for every DICE
key — analysis, load, action, listing, materialization — with async poll-
time metrics per span. The observer layer (`kuro_event_observer`) aggregates
`ActionStats` live. Critical-path and slowest-path are computed at build end
and persisted in `BuildGraphExecutionInfo`. What's missing is the rollup +
reporting step and a handful of correctness/noise fixes that would otherwise
pollute benchmark signal.

## Current State Analysis

**What's captured today (confirmed by plan-1 research pass):**
- Per-span wall time + async poll time (`SpanStats` in `data.proto:152-155`).
- DICE activation data carries `TimeSpan` on every key completion; used by
  `app/kuro_build_signals_impl/src/lib.rs:415-467`.
- Action stats (cache-hit %, local/remote/cached/fallback counts, excess
  cache misses): `app/kuro_event_observer/src/action_stats.rs:30-106`.
- Critical path + slowest path with per-node potential-improvement:
  `app/kuro_build_signals_impl/src/lib.rs:534-765`, backends at
  `app/kuro_build_signals_impl/src/backend/`.
- Event log format: `.pb.zst` under `buck-out/v2/log/`, queryable via
  `kuro log {what-ran,critical-path,slowest-path,summary,diff,replay}`.
- Superconsole renders live cache-hit %, command counts, latency percentiles,
  RE queue depth.

**What's missing:**
- No end-of-build summary other than the bare
  `Cache hits: N%. Commands: M (cached, remote, local).` line.
- `kuro log summary` (`app/kuro_cmd_log_client/src/summary.rs`) is minimal.
- `kuro log diff` (`app/kuro_cmd_log_client/src/diff.rs`) exists but does
  event-level diff, not rollup-vs-rollup.
- Analysis is a single opaque span per target. No `configure` / `eval-attrs`
  / `run-impl` breakdown.
- Load + listing entries in `DetailedCriticalPath` (variants exist in
  `data.proto:495-635`) may not be populated — verify.
- No repeatable benchmark harness. Every timing measurement has been ad-hoc
  via `/usr/bin/time`.

**Noise sources that would corrupt benchmarks:**
- `output_file_target: value=... registry.len=42` ERROR fires on every
  llvm build — `app/kuro_interpreter_for_build/src/attrs/coerce/ctx.rs`.
  Non-fatal but cluttering and possibly indicative of an attr-coercion gap.
- Unbounded MPSC on build signals
  (`app/kuro_build_signals_impl/src/lib.rs:942`) → OOM under long runs.
- Span tracker never evicts (`app/kuro_event_observer/src/span_tracker.rs`)
  → daemon memory drifts across invocations.
- Event log reader decompresses entire file into memory
  (`app/kuro_event_log/src/read.rs`) → 5-10s tax per `kuro log …` call on
  llvm-sized logs.
- `EventSink::send` panics on failure (`app/kuro_events/src/sink/*.rs:254-259`)
  → a wedged sink aborts the benchmark run.

## Phases

### 16.1 Rollup engine (OPEN)

**Parity source.** Bazel's build summary line + `bazel analyze-profile`
output schema — per-phase, per-mnemonic, top-N.

New module `app/kuro_event_observer/src/build_summary.rs`. Pure aggregation
over the existing event stream. Output shape (stable, used by 16.2 and 16.3):

```rust
pub struct BuildSummary {
    // Phase durations — derived from SpanStart/End events.
    pub load_wall_us: u64,
    pub analyze_wall_us: u64,
    pub execute_wall_us: u64,
    pub materialize_wall_us: u64,
    pub total_wall_us: u64,

    // Action rollup, keyed by mnemonic (category).
    pub by_mnemonic: Vec<MnemonicRow>,

    // Top-N slowest leaves on the critical path.
    pub slowest_actions: Vec<ActionRow>,        // N=10 default
    pub slowest_analyses: Vec<AnalysisRow>,     // N=10 default

    // Cache breakdown.
    pub cache_hit_pct: f64,
    pub cache_hit_pct_by_mnemonic: Vec<(String, f64)>,

    // Parallelism.
    pub peak_in_flight_actions: u32,
    pub peak_re_queue_depth: u32,
    pub total_action_count: u64,

    // Graph.
    pub num_dice_nodes: u64,
    pub num_dice_edges: u64,
    pub action_graph_size: u64,

    // Critical + slowest path totals.
    pub critical_path_wall_us: u64,
    pub slowest_path_wall_us: u64,
}

pub struct MnemonicRow {
    pub category: String,
    pub count: u64,
    pub cached: u64,
    pub total_wall_us: u64,
    pub critical_wall_us: u64,
    pub p50_us: u64,
    pub p95_us: u64,
    pub p99_us: u64,
}
```

Implementation notes:
- Stream the event log once; produce the struct. No double-read.
- Reuse existing `ActionStats::update` logic for cache-hit classification.
- `MnemonicRow` populated from `ActionExecutionEnd.kind` + `.name.category`.
- Critical-path data lifted from the terminal `BuildGraphExecutionInfo`
  instant event, not recomputed.

Dependencies: 16.5, 16.6 (noise fixes) — but rollup engine can be built and
tested in isolation first.

---

### 16.2 Live end-of-build summary (OPEN)

When stderr is a TTY and `--quiet` is not set, print the BuildSummary after
the existing `Cache hits: …` line. Add flag `--build-summary={off,short,full}`
(default `short` on TTY, `off` otherwise). `full` dumps the whole table.

Wiring in `app/kuro_server_commands/src/build/result_report.rs` (or
equivalent — confirm call site during implementation). Uses the in-memory
aggregator, not the event log — no re-reading cost.

---

### 16.3 `kuro log summary` — rewrite (OPEN)

Replace the existing bare-bones summary with a call to the 16.1 rollup
engine over the event log file. `app/kuro_cmd_log_client/src/summary.rs`.

Flags:
- `--format={table,json,csv}` (default `table`).
- `--top-n=10` (controls slowest-action + slowest-analysis cutoffs).
- `--by-mnemonic` / `--no-by-mnemonic` (default on).

Regenerates the same `BuildSummary` struct the live path produces.

---

### 16.4 `kuro log diff <before> <after>` — rollup diff (OPEN)

Parity source: no exact bazel equivalent; inspired by
`bazel analyze-profile --dump=text` + hand-rolled scripts CI teams write.

Extend `app/kuro_cmd_log_client/src/diff.rs` (or add `--summary` mode if the
existing diff does event-level). Produces a side-by-side table:

```
metric                  before          after           Δ         Δ%
total_wall_us           1_346_210_000   1_180_045_000   -166M     -12.3%
analyze_wall_us            9_250_000       8_980_000     -270k      -2.9%
execute_wall_us        1_320_410_000   1_155_340_000   -165M     -12.5%
cache_hit_pct                  0.00            0.00        0       0.0%
...
by_mnemonic cxx_compile
  count                        2_841           2_841          0      0.0%
  total_wall_us            812_430_000     701_880_000   -110M    -13.6%
  p95_us                     1_204_500       1_088_000    -116k    -9.7%
```

Regression gate: flag rows where `|Δ%| > threshold` (default 5%) with a
leading `!`. Optional `--fail-on-regression` for CI use.

---

### 16.5 Noise fixes (OPEN, prerequisite for 16.7)

Individually small; grouped because they all distort benchmark signal.

**16.5.1** `output_file_target: ... registry.len=42` log spam —
`app/kuro_interpreter_for_build/src/attrs/coerce/ctx.rs`. Investigate,
either fix the underlying registry-lookup miss or demote to `debug!` if
the case is legitimately unreachable in practice.

**16.5.2** Bounded MPSC on build signals —
`app/kuro_build_signals_impl/src/lib.rs:942`. Switch to
`tokio::sync::mpsc::channel(capacity)` with a capacity informed by
graph-size heuristic (e.g. `max(1024, num_dice_nodes / 4)`). Back-pressure
path = block the sender with a `send().await`. Measure overhead on the
plan-1 harness before/after.

**16.5.3** Span-tracker eviction —
`app/kuro_event_observer/src/span_tracker.rs`. Evict on matching
`SpanEnd`; optionally cap total entries with an LRU for long-running
daemons.

**16.5.4** Streaming zstd reader — `app/kuro_event_log/src/read.rs`. Use
`zstd::stream::read::Decoder` wrapping the file reader; don't allocate a
full `Vec<u8>`. Cuts 5-10s off every `kuro log` invocation on llvm-sized
logs.

**16.5.5** `EventSink::send` returns `Result` —
`app/kuro_events/src/sink/*.rs:254-259`. Existing call sites `.expect("…")`
today; switch to graceful-degrade (log + drop event) for terminal sinks
other than the event-log writer itself.

---

### 16.6 Analysis sub-spans (OPEN)

Today analysis is a single `AnalysisStart` / `AnalysisEnd` pair per target.
Split into three sub-spans so "did this commit regress configure or
analyze?" has a data-backed answer.

Proto changes in `app/kuro_data/data.proto` — new
`AnalysisConfigureStart/End`, `AnalysisAttrEvalStart/End`,
`AnalysisImplStart/End` under `SpanStartEvent.data`. Emission in
`app/kuro_analysis/src/analysis/env.rs` around the three phases (current
single `RuleImpl` invoke block at `env.rs:1340-1350` becomes three spans).

The rollup engine (16.1) picks up the new spans automatically via the
existing "phase duration = sum of spans of kind X" logic.

---

### 16.7 Critical-path load + listing coverage (OPEN)

Confirm `DetailedCriticalPath` entries for `Load`, `Listing`,
`FinalMaterialization`, `TestExecution`, `TestListing` are populated
(variants exist in `data.proto:495-635`; check
`app/kuro_build_signals_impl/src/lib.rs` emission). Fix if gaps.

Without this, the plan-1 diff masks load-phase regressions as "gaps".

---

### 16.8 Benchmark harness (OPEN)

New `tools/bench/` directory:

- `run.sh <target> [--runs=N] [--cold|--warm|--both]` — invokes
  `/var/mnt/dev/kuro/kuro kill` between cold runs, drops caches between
  cold runs (with user confirmation), emits JSON rollup per run under
  `benchmarks/<YYYY-MM-DD>-<git-sha>/<target>/<run>/summary.json`.
- `compare.sh <baseline-dir> <current-dir>` — wraps `kuro log diff` over
  every matching target, produces a consolidated table.
- Canary targets: small (a single cc_library in-repo), medium
  (`@llvm-project//clang:analysis_htmllogger_gen`), large
  (`@llvm-project//clang:clang`), xl (`@llvm-project//llvm:llvm`).

CI-friendly: accepts `--runs=1` for quick check, `--runs=5` for release
gates.

---

## Dependencies and ordering

```
16.5 (noise fixes) ────────────┐
                               │
16.1 (rollup engine) ─────────►├─► 16.2 (live summary)
                               │
                               ├─► 16.3 (log summary rewrite)
                               │
                               ├─► 16.4 (log diff)
                               │
16.6 (analysis sub-spans) ─────┘
16.7 (critical-path coverage) ─┘

16.8 (harness) depends on 16.4.
```

16.1 can be built without 16.5 done, but measurements taken before 16.5.4
(streaming reader) will have 5-10s of noise per `log` call — so defer
anything measurement-adjacent (16.8) until 16.5 lands.

Recommended order: **16.5 → 16.1 → 16.2 → 16.3 → 16.4 → 16.6/16.7 → 16.8**.

## Open questions

- Does `kuro log diff` today do event-level diff, or is it a stub? If the
  former, keep it and add `--summary` mode; if the latter, replace.
- What's the right capacity heuristic for 16.5.2 on a 4k-action build vs a
  50k-action build? Land with a flag override and tune empirically.
- Should per-attribute analysis timing (16.6 finer-grained) come here or
  in plan 17? Current call: defer finer-grained to 17 when we actually
  need it — 16.6 gives three sub-spans which is enough for most
  regression signal.

## Success criteria

- `kuro log diff baseline.pb.zst head.pb.zst` prints a ≥12-column table,
  stable across identical workspaces, `--fail-on-regression` usable in CI.
- `kuro log summary` on the clang:clang llvm log produces a full rollup
  in under 3 seconds (after 16.5.4).
- End-of-build TTY summary shows phase durations + top-3 slowest
  mnemonics by default.
- Benchmark harness produces JSON usable by plan 17's before/after
  measurements.
- No `output_file_target` ERROR log lines during a green llvm build.
