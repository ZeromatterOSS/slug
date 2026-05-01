# Plan 32: Local cold-overhead and launch parallelism

> Parent: [2026-01-21-kuro-bazel-compatible-build-tool.md](../2026-01-21-kuro-bazel-compatible-build-tool.md)
>
> Follows Plans 16, 17, 21, and 31. Plan 31 owns warm-RE parity on
> `@llvm-project//llvm:llvm`; this plan owns local-build overhead where the
> compiler/linker work is discounted.

## Goal

Make Kuro's local build overhead measurable, keep the daemon-side advantage over
Bazel, and close the remaining gaps in no-op client wall and action-launch
parallelism.

Primary workload:

```
@llvm-project//llvm:Support
```

Measured from `/var/mnt/dev/llvm-project/utils/bazel/`.

## Baseline

Artifacts:

```
benchmarks/2026-04-30-support-profile/llvm-project_llvm_Support/
```

Kuro artifact highlights:

- `cold-daemon-default-01/discounted-overhead.json`
- `cold-daemon-default-01/summary.json`
- `kuro-local-warm-01/summary.json`
- `comparison-summary.json`

Bazel artifact highlights:

- `bazel-local-cold-01/metrics-summary.json`
- `bazel-local-cold-01/profile-action-execute-rollup.json`
- `bazel-local-warm-01/metrics-summary.json`

### Cold local build, external action wall discounted

Method:

```
exposed_non_external_wall = daemon_or_server_wall - external_action_union_wall
```

This discounts the wall interval covered by real compiler/archive/linker
processes, leaving load, analysis, graph traversal, scheduling, cache checks,
materialization, event handling, and process launch overhead visible.

| Metric | Kuro | Bazel 9.0.2 |
|---|---:|---:|
| CLI wall | 13.86 s | 19.02 s |
| daemon/server wall | 12.91 s | 16.68 s |
| external action union wall | 9.58 s | 11.04 s |
| exposed non-external wall | 3.33 s | 5.65 s |
| first subprocess/action start | 3.25 s | 5.51 s |
| external action wall sum | 129.75 s | 159.27 s |
| peak external action parallelism | 16 | 16 |
| average external action parallelism | 13.54 | 14.43 |

Interpretation:

- Kuro's exposed daemon-side overhead is about **2.32 s lower** than Bazel's
  server-side overhead on this run.
- Kuro reaches the first external action earlier.
- Bazel keeps the local executor slightly fuller once actions are running.

### Warm no-op rebuild

| Metric | Kuro | Bazel 9.0.2 |
|---|---:|---:|
| CLI wall | 0.87 s | 0.46 s |
| daemon/server wall | 0.300 s | 0.412 s |
| analysis | 0.060 s | 0.081 s |
| execution/cache-check phase | 0.029 s | 0.015 s |
| critical path | 0.271 s | 0.010 s |

Interpretation:

- Kuro's daemon-side no-op path is competitive and slightly lower than Bazel's
  server wall.
- Kuro pays too much outside the daemon: about **570 ms** of CLI/client/event
  overhead versus about **48 ms** for Bazel.

### Starlark interpreter finding

Kuro embeds `starlark-rust`. Build-file and `.bzl` loading parse source to
`AstModule`, resolve imports through DICE, and evaluate with
`Evaluator::eval_module`; rule analysis invokes implementation functions with
`Evaluator::eval_function`.

`starlark-rust` is a bytecode interpreter, not a native-code JIT. It lowers
top-level statements and function bodies to `Bc` bytecode and dispatches
opcodes in Rust. The 2026-04-30 profile does not make Starlark bytecode
execution the obvious first optimization target.

## Scope

In:

- Local builds and no-op rebuilds of `@llvm-project//llvm:Support`.
- Kuro-vs-Bazel overhead after discounting external process execution.
- Process launch timing, local executor saturation, client wall, event-log/BES
  overhead, and benchmark harness improvements.

Out:

- Optimizing the compiler, linker, or archive tools themselves.
- Warm-RE action-cache persistence and BuildBuddy-specific behavior. See
  Plan 31.
- A Starlark native-code JIT. Revisit only if later profiling shows Starlark
  bytecode dispatch is a dominant cost.

## Phases

### 32.1 Controlled no-op external-action harness (OPEN)

The current metric discounts real external action spans after the fact. That is
useful, but it still lets real compiler duration perturb queueing, phase
overlap, memory pressure, and CPU availability.

Build a controlled harness where the action graph stays close to the real
`Support` graph while the external C/C++ tools do near-zero work.

Deliverables:

- A wrapper toolchain or fake compiler/archive/linker setup for both Kuro and
  Bazel.
- A repeatable script under `thoughts/shared/` or `benchmarks/` that records:
  - Kuro event log + `kuro log summary --format=json`
  - Bazel BEP + JSON profile
  - `/usr/bin/time` CLI wall for both
  - derived exposed overhead and action parallelism rollups
- A validation row proving the fake workload preserves action count and graph
  shape close enough to compare:
  - Kuro: target/action graph nodes, actions by mnemonic
  - Bazel: actions created/executed, packages loaded, Skyframe nodes

Success criteria:

- External action union wall is below 5% of daemon/server wall.
- The benchmark reports cold, warm, and no-op numbers in one JSON file.
- Metrics can be compared run-to-run without hand parsing profiles.

### 32.2 Split action queue wait from true setup overhead (OPEN)

The 2026-04-30 Kuro action-event rollup shows large aggregate "pre-exec"
overhead, but most of it is local-slot waiting behind the 16-process cap. That
number is useful for critical-path analysis and misleading for per-action setup
cost.

Add or derive these buckets:

- ready-to-admitted queue wait
- admitted-to-spawn setup
- spawn-to-first-process-start
- process execution
- process-end-to-action-complete
- materialization and output digest bookkeeping

Deliverables:

- Updated `kuro log summary --format=json` schema or a sidecar post-processor
  that exposes those buckets.
- p50/p95/p99 per bucket and per mnemonic.
- The same union/peak/average parallelism metrics already computed in
  `discounted-overhead.json`.

Success criteria:

- A future report can state "slot wait" separately from "executor overhead".
- p95 true post-process bookkeeping stays under 10 ms on this workload.

### 32.3 Reduce warm no-op CLI/client overhead (OPEN)

Kuro daemon wall is 0.300 s while CLI wall is 0.87 s. Bazel's server wall is
0.412 s while CLI wall is 0.46 s. The Kuro gap is too large for a no-op build.

Investigation order:

1. Add client-side timeline spans around startup, daemon connect, request send,
   event stream receive, event-log finalization, BES setup/finalization, terminal
   rendering, and process exit.
2. Run a matrix:
   - event log on/off
   - BES on/off
   - TTY summary on/off
   - `.bazelrc` BuildBuddy config on/off
3. Decide whether the dominant cost belongs in this plan, Plan 30, or Plan 31.

Likely implementation areas:

- `app/kuro_client_ctx/src/events_ctx.rs`
- `app/kuro_client_ctx/src/subscribers/`
- `app/kuro_cmd_log_client/`
- daemon command finalization and event-stream shutdown

Success criteria:

- Warm no-op CLI wall <= 0.55 s for `@llvm-project//llvm:Support`.
- Difference between CLI wall and daemon wall <= 150 ms when BES is disabled.
- If BES is enabled, the remaining client-visible cost is explicitly attributed
  and tied to Plan 31.3 if daemon-resident BES is the right fix.

### 32.4 Improve local executor saturation (OPEN)

Bazel averaged 14.43 external actions in flight while action processes ran;
Kuro averaged 13.54 with the same peak of 16. The gap is not huge, but it is
visible and should be measurable with the no-op harness.

Questions to answer:

- Is Kuro's lower average caused by real graph dependencies or scheduler/local
  executor admission latency?
- Are input materialization and process setup serializing the start of later
  waves?
- Does the scheduler hold ready work behind per-category or global bookkeeping
  when there are available local slots?

Deliverables:

- Ready-queue depth timeline, local slot occupancy timeline, and action start
  histogram for the `Support` build.
- Compare real-compiler and fake-compiler runs. If fake actions expose the same
  average-parallelism gap, it is scheduler/admission shaped; if not, it is
  caused by compiler-duration variance and graph structure.
- Patch only if the evidence shows avoidable idle slots.

Success criteria:

- Average local process parallelism while actions run >= 14.2 on the real
  `Support` workload, with peak still 16.
- No increase in exposed non-external cold overhead.

### 32.5 Keep Starlark work measured, but do not prioritize a JIT (OPEN)

The cold Kuro run reports load around 3.0 s and analysis around 0.12-0.18 s on
this target, with daemon-side exposed overhead already below Bazel. A Starlark
JIT would be high complexity and poorly targeted for this profile.

Instead:

- Add a Starlark parse/compile/eval breakdown to the benchmark report when
  profiler support is cheap to enable.
- Reuse Plan 17.6 for Starlark compilation persistence if daemon-cold
  re-parsing of bundled or external `.bzl` modules becomes visible.
- Gate any interpreter-level optimization on evidence that Starlark bytecode
  dispatch is at least 20% of exposed non-external wall on a representative
  workload.

Success criteria:

- The plan has a clear "do nothing yet" result for Starlark JIT unless later
  measurements overturn the current finding.

## Dependencies and ordering

```
32.1 no-op harness
    ├─► 32.2 overhead bucket split
    ├─► 32.3 no-op CLI/client overhead
    └─► 32.4 executor saturation
            └─► 32.5 Starlark follow-up only if profiles justify it
```

32.1 and 32.2 are measurement prerequisites. 32.3 and 32.4 can proceed in
parallel once the harness exists.

## Success Criteria

- Cold exposed non-external Kuro overhead remains below Bazel by at least 20%
  on `@llvm-project//llvm:Support`.
- Warm no-op Kuro CLI wall <= 0.55 s.
- Warm no-op Kuro daemon wall <= 0.300 s or any regression is explained by a
  correctness/parity change.
- Average local process parallelism during action execution >= 14.2.
- Benchmark artifacts include enough data to reproduce:
  - CLI wall
  - daemon/server wall
  - external action union wall
  - exposed non-external wall
  - first action start
  - peak and average action parallelism
  - graph/action counts

## References

- Current benchmark report:
  `benchmarks/2026-04-30-support-profile/llvm-project_llvm_Support/report.md`
- Consolidated comparison:
  `benchmarks/2026-04-30-support-profile/llvm-project_llvm_Support/comparison-summary.json`
- Kuro cold discounted overhead:
  `benchmarks/2026-04-30-support-profile/llvm-project_llvm_Support/cold-daemon-default-01/discounted-overhead.json`
- Bazel cold BEP metrics:
  `benchmarks/2026-04-30-support-profile/llvm-project_llvm_Support/bazel-local-cold-01/metrics-summary.json`
- Bazel cold profile action rollup:
  `benchmarks/2026-04-30-support-profile/llvm-project_llvm_Support/bazel-local-cold-01/profile-action-execute-rollup.json`
- Plan 16: benchmark-grade telemetry.
- Plan 17: broader optimization backlog.
- Plan 21: warm-invocation overhead.
- Plan 31: warm-RE performance parity.
