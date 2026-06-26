# Plan 45: Per-Args paramfile materialization & cargo_build_script runfiles

> Parent: [2026-01-21-slug-bazel-compatible-build-tool.md](../2026-01-21-slug-bazel-compatible-build-tool.md)
>
> Siblings:
> - [15-bazel-9-parity.md](./15-bazel-9-parity.md) §15.5.23 (LANDED 2026-04-21) —
>   runfiles tree synthesis for `DefaultInfo(executable=..., default_runfiles=...)`.
>   This plan covers explicit `ctx.actions.declare_directory("foo.cargo_runfiles")`
>   outputs that the action's own runner is supposed to populate.
> - [34-sandboxed-execution-strategy.md](./34-sandboxed-execution-strategy.md) —
>   owns REAPI executor-boundary proof. Direct-local paramfile success does not
>   count as Plan 34 evidence.
> - [44-workspace-layout-parity.md](./44-workspace-layout-parity.md) —
>   Phase 2.5 provides the current shared synthesized execroot stopgap; Phase 2.6
>   narrows that execroot per action; Phase 3 later replaces it with a real
>   Bazel-shaped execroot.

## Status: PARTIAL

Local and REAPI per-`Args` paramfile slots are implemented and covered by fast
repo-owned regressions, including a cargo-runfiles-shaped REAPI directory-output
handoff. Public cargo-build-script validation and BCR coverage scan remain open.

## Bazel source anchors

- `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/starlarkbuildapi/CommandLineArgsApi.java:469-500`:
  `Args.use_param_file(param_file_arg, use_always)` is an `Args` API that
  replaces that object's args with a formatted paramfile pointer.
- `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/analysis/starlark/Args.java:555-568`:
  Bazel validates `param_file_arg` as a single-`%s` format and stores
  `alwaysUseParamFile` on the `Args` object.
- `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/starlarkbuildapi/CommandLineArgsApi.java:502-516`:
  `set_param_file_format` is also an `Args` API; the chosen format must travel
  with the same slot.
- `/var/mnt/dev/bazel/src/test/java/com/google/devtools/build/lib/starlark/StarlarkRuleImplementationFunctionsTest.java:2580-2608`:
  Bazel's lazy-args test asserts `args.use_param_file(..., use_always=True)`
  spills only that `Args` object's content into the paramfile.

## Current state

- Slug carries paramfiles as `Vec<ParamFileSlot>` on
  `CommandExecutionRequest`. Each slot records an `args[start..end]` range plus
  its own `param_file_arg`, `use_always`, and format.
- `ctx.actions.run(arguments=[...])` rendering preserves top-level item
  boundaries and creates a slot for each nested `FrozenStarlarkCmdArgs` with
  `param_file()`. A top-level `Args.use_param_file(...)` still becomes one slot
  covering the whole rendered arg vector.
- The local executor materializes each slot into the action scratch directory
  and splices the slot range with `param_file_arg.replace("%s", path)`. The old
  post-hoc `.cargo_runfiles` positional-argument heuristic is no longer present.
- Repo-owned fixture `//:args_nested_param_file` verifies a nested
  `Args.use_param_file("--cargo_manifest_args=@%s", use_always=True)` reaches
  the action as one pointer arg while the nested content lands in the paramfile.
- Action request construction now lowers spillable `ParamFileSlot`s into
  generated `ActionMetadata` inputs with inline blob bytes before RE action
  preparation. `RE::Command.arguments` receives the formatted paramfile pointer,
  and the Action/CAS upload path adds the generated paramfile bytes to the RE
  input tree. The local executor writes the same inline metadata bytes when it
  runs the request directly.
- Plan 34's NativeLink fixture `//:nested_param_file` verifies a nested
  `Args.use_param_file("--cargo_manifest_args=@%s", use_always=True)` crosses
  REAPI with `executor_boundary="reapi"`, `direct_local_actions=0`, and the
  remote action reading the uploaded paramfile input.
- Plan 34's `//:cargo_runfiles_param_file` fixture declares a
  `_bs.cargo_runfiles` directory, populates it from a nested
  `--cargo_manifest_args=@...` paramfile, then consumes that directory in a
  downstream RE action. The uploader now materializes and re-uploads recent
  RE-produced file inputs when the remote CAS reports them missing, avoiding a
  direct-local shortcut for generated directory handoffs.

## Accepted evidence

- `target/debug/slug killall || true`
- `TEST_EXECUTABLE=/var/mnt/dev/slug/target/debug/slug python -m pytest -q tests/core/analysis/test_cmd_args.py::test_nested_args_use_param_file_materializes_slot -s --tb=short`
  - Passed: `1 passed in 0.40s`
- `TEST_EXECUTABLE=/var/mnt/dev/slug/target/debug/slug python -m pytest -q tests/core/analysis/test_cmd_args.py -s --tb=short`
  - Passed: `17 passed in 5.15s`
- `cargo test -p slug_execute_impl paramfile --lib -- --nocapture`
  - Passed: `2 passed`
- `TEST_EXECUTABLE=/var/mnt/dev/slug/target/debug/slug python -m pytest -q tests/plan34/test_reapi_local_executor_smoke.py::test_native_link_nested_paramfile_reaches_reapi_input_tree -s --tb=short`
  - Passed: `1 passed in 0.76s`
- `TEST_EXECUTABLE=/var/mnt/dev/slug/target/debug/slug python -m pytest -q tests/plan34/test_reapi_local_executor_smoke.py::test_native_link_cargo_runfiles_paramfile_advances_reapi_layer -s --tb=short`
  - Passed: `1 passed in 1.67s`
- `TMPDIR=/var/mnt/dev/slug/.tmp SLUG_PLAN34_EVIDENCE_JSONL=/var/mnt/dev/slug/.tmp/plan34-reapi-evidence.jsonl TEST_EXECUTABLE=/var/mnt/dev/slug/target/debug/slug python -m pytest -q tests/plan34/ -s --tb=short`
  - Passed: `17 passed in 20.84s`; evidence summary:
    `reapi_actions=12`, `direct_local_actions=0`, `upload_records=12`.

## Remaining gaps

- Run a fast public cargo-build-script smoke that proves a real ruleset runner
  receives `--cargo_manifest_args=@...`, creates the declared
  `.cargo_runfiles` tree, and advances to a distinct layer.
- Complete the public BCR `use_param_file(use_always=True)` scan and record
  whether any consumer needs more than per-`Args` slot materialization.
- Revisit the Plan 44 Phase 3 cleanup hook after the real execroot lands; the
  temporary execroot self-symlink should not become permanent architecture.

## Next owner

1. Use the smallest public cargo-build-script smoke to prove a real ruleset
   runner receives `--cargo_manifest_args=@...`, creates the declared
   `.cargo_runfiles` tree, and advances to a distinct layer.
2. Complete the public BCR `use_param_file(use_always=True)` scan, then update
   this file with only new accepted evidence and remaining gaps.

## Out of scope

- Bazel versions before 9.0.
- Runfiles tree synthesis for non-executable rules' `default_runfiles` (owned by
  Plan 15).
- Real sandboxed execution and undeclared input enforcement (owned by Plan 34).
