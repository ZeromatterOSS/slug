# `@llvm-project//llvm:Support` overhead profile

Date: 2026-04-30

Target: `@llvm-project//llvm:Support`

Workspace: `/var/mnt/dev/llvm-project/utils/bazel`

## Method

Kuro was run from `/var/mnt/dev/kuro/target/debug/kuro`.

Bazel was run through Bazelisk using Bazel 9.0.2. The Bazel comparison used
`--ignore_all_rc_files`, local GCC/C++17 flags, `--spawn_strategy=local`, and
`--jobs=16` to avoid the workspace `.bazelrc` BuildBuddy remote execution setup.

For cold builds, external compiler/archive/linker work was discounted by using
the measured wall-clock union of external action execution:

`exposed_non_external = server_or_daemon_wall - external_action_union_wall`

This is not a perfect no-op compiler experiment, but it removes the wall time
covered by running external processes and keeps analysis, traversal, scheduling,
cache checking, materialization, and launch overhead visible.

## Cold Build

| Metric | Kuro | Bazel |
|---|---:|---:|
| CLI wall | 13.86 s | 19.02 s |
| daemon/server wall | 12.91 s | 16.68 s |
| external action union wall | 9.58 s | 11.04 s |
| exposed non-external wall | 3.33 s | 5.65 s |
| first subprocess/action start | 3.25 s | 5.51 s |
| external action total CPU-ish wall sum | 129.75 s | 159.27 s |
| peak action parallelism | 16 | 16 |
| average action parallelism while actions ran | 13.54 | 14.43 |

Kuro's daemon-side exposed non-external wall was about 2.32 s lower than Bazel's
server-side wall for this run. Bazel kept the action executor slightly fuller
once actions started, but Kuro reached external action launch earlier.

## Warm No-Op Rebuild

| Metric | Kuro | Bazel |
|---|---:|---:|
| CLI wall | 0.87 s | 0.46 s |
| daemon/server wall | 0.300 s | 0.412 s |
| analysis | 0.060 s | 0.081 s |
| execution/cache-check phase | 0.029 s | 0.015 s |
| critical path | 0.271 s | 0.010 s |

Kuro's daemon-side no-op path was in the same range and slightly lower than
Bazel's server wall, but Kuro had more client/outer overhead in the measured CLI
wall.

## Starlark Interpreter

Kuro embeds `starlark-rust`. Build-file and `.bzl` loading parse source to
`AstModule`, resolve imports, evaluate transitive loaded modules through DICE,
then evaluate the module with `Evaluator::eval_module`. Rule analysis invokes
the loaded rule implementation with `Evaluator::eval_function`.

The interpreter is bytecode based, not a native-code JIT. `starlark-rust` lowers
top-level statements and function bodies to bytecode (`Bc`) and dispatches
opcodes in a Rust loop. Generated native machine code only comes from compiling
the Rust implementation itself.

## Low-Hanging Optimization Candidates

The cold run does not point at Starlark bytecode execution as the obvious first
target for this workload. Kuro's measured Starlark-heavy load/analyze phases were
roughly 3.0 s plus 0.12-0.18 s; the discounted cold overhead delta versus Bazel
is already favorable daemon-side.

The more obvious candidates from this pass are:

- Reduce Kuro CLI/client/event-log/BES overhead on no-op builds. The daemon
  reported 0.300 s while the CLI measured 0.87 s.
- Separate local executor slot wait from true per-action setup in profiles. The
  aggregate per-action "overhead" in the cold Kuro action events is mostly
  pre-exec waiting behind the 16 local slots, not post-action bookkeeping.
- Improve local process saturation after the initial wave. Bazel averaged 14.43
  parallel actions during action execution versus Kuro's 13.54 in this run.
- Run a controlled fake-compiler/no-op toolchain experiment to preserve cold
  graph/action creation while making external action duration near zero. That
  would measure overhead more directly than discounting real compiler spans.

## Artifacts

- Kuro cold discounted overhead: `cold-daemon-default-01/discounted-overhead.json`
- Kuro cold summary: `cold-daemon-default-01/summary.json`
- Kuro warm summary: `kuro-local-warm-01/summary.json`
- Bazel cold BEP metrics: `bazel-local-cold-01/metrics-summary.json`
- Bazel cold profile action rollup: `bazel-local-cold-01/profile-action-execute-rollup.json`
- Bazel warm BEP metrics: `bazel-local-warm-01/metrics-summary.json`
- Consolidated comparison: `comparison-summary.json`
