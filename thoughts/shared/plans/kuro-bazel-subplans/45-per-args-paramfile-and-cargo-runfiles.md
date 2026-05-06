# Plan 45: Per-Args paramfile materialization & cargo_build_script runfiles

> Parent: [2026-01-21-kuro-bazel-compatible-build-tool.md](../2026-01-21-kuro-bazel-compatible-build-tool.md)
>
> Siblings:
> - [15-bazel-9-parity.md](./15-bazel-9-parity.md) §15.5.23 (LANDED 2026-04-21) —
>   runfiles tree synthesis for `DefaultInfo(executable=..., default_runfiles=...)`.
>   This plan covers a different runfiles flavour: explicit
>   `ctx.actions.declare_directory("foo.cargo_runfiles")` outputs that the
>   action's own runner is supposed to populate.
> - [44-workspace-layout-parity.md](./44-workspace-layout-parity.md) Phase 3 —
>   introduces an execroot. The `execroot/<basename> -> ..` self-symlink
>   landed today (commit `d29a4ff4`) is a tactical interim; Phase 4 below
>   removes it once Plan 44 Phase 3 lands.

## Status: PROPOSED

## Context

`zeromatter//sdk:sdk_contents` advances through analysis, ≈550 crate spokes
lazy-materialize (Plan 36), `${exec_root}` substitution resolves
(d29a4ff4), and execution reaches `cargo_build_script_run`. Then:

```
thread 'main' panicked at .../cargo_build_script_runner/lib.rs:150:33:
Unable to start command:
  program: ".../glue-build-script-",
  cwd:     ".../glue-build-script.cargo_runfiles/_main/lib/glue",
  ...
Os { code: 2, kind: NotFound }
```

The runner panics because its working directory
(`<bs>.cargo_runfiles/_main/<pkg>`) does not exist on disk. The runner
is supposed to create it itself — `bin.rs:54-56`:

```rust
if let Some(cargo_manifest_maker) = &cargo_manifest_maker {
    cargo_manifest_maker.create_runfiles_dir().unwrap()
}
```

`cargo_manifest_maker` is parsed from `--cargo_manifest_args=@<paramfile>`
(`bin.rs:366-370`). On the kuro invocation it is `None`. The runner gets
`<runfiles_dir>` and `<retain_list>` as bare positional args instead, never
recognises them, and `create_runfiles_dir` never fires.

### Why the runner sees positional args

`rules_rust+0.69.0/cargo/private/cargo_build_script.bzl:334-338` builds the
runfiles args object:

```python
args = ctx.actions.args()
args.use_param_file("--cargo_manifest_args=@%s", use_always = True)
args.add(runfiles_dir.path)
args.add(",".join(retain_list))
args.add_all(runfiles.files, map_each = _runfiles_map, allow_closure = True)
```

and passes it as `arguments = [main_args] + extra_args` to
`ctx.actions.run` (`:638`). `extra_args = [runfiles_args]`, so `runfiles_args`
is a *nested* cmd_args inside the action's argument list, with its own
`use_param_file(... use_always=True)` config.

Kuro currently honours `param_file` only on the top-level cmd_args:
`app/kuro_action_impl/src/actions/impls/run.rs:929-933`:

```rust
let param_file = self
    .starlark_values
    .args
    .param_file()              // top-level only
    .map(frozen_param_file_to_spec);
```

`FrozenStarlarkCmdArgs::param_file_or_nested()` (`cmd_args/typ.rs:599-612`)
*does* exist and walks one level of nesting, but `run.rs` doesn't call it.
Even if it did, the design isn't right: with multiple nested cmd_args
each carrying a different `param_file_arg`, the action needs *one
paramfile per Args slot*, not a single global paramfile holding the
union of every arg.

### The hack that's there today

`app/kuro_execute_impl/src/executors/local.rs:836-870` detects this exact
shape post-hoc in the local executor: "if there's a positional arg
ending in `.cargo_runfiles` followed by ≥1 mapping (`a=b`) entry, write
the trailing positional args to a tmp file and replace them with
`--cargo_manifest_args=@<tmp>`." The detection requires `positional.len()
>= 3` (`local.rs:857`); for `glue-build-script` the script has zero
runfile mappings so positional length is 2, the heuristic doesn't fire,
and the runner sees raw positional args.

## Three subgoals, three sizes

### Phase 1 (medium): per-Args paramfile materialization

Lift `param_file` from a single global `Option<ParamFileSpec>` on the
`PreparedRunAction` to a `Vec<ParamFileSlot>`, where each slot carries:

- `cmd_args_index`: which item in the action's expanded arg list this
  paramfile replaces (the paramfile takes the place of every arg
  contributed by that nested cmd_args).
- `param_file_arg`: the format string (`"--cargo_manifest_args=@%s"`).
- `format`: `Multiline | FlagPerLine | Shell`.
- `use_always`: forced or threshold-driven.

Action prep walks `starlark_args.items`. For each item:
- If it's a `FrozenStarlarkCmdArgs` with `param_file.is_some()`, emit a
  slot bracketing exactly that item's expanded args.
- Otherwise pass through inline.

The local executor (and any future remote executor's local fall-back)
takes the slot list, writes one paramfile per slot, and substitutes the
slot's arg range with `param_file_arg.replace("%s", path)`.

**Files**:
- `app/kuro_build_api/src/interpreter/rule_defs/cmd_args/typ.rs` — keep
  `param_file_or_nested`, but additionally expose
  `nested_param_files() -> Vec<(item_idx, &FrozenParamFileData)>` so
  the consumer can locate item boundaries.
- `app/kuro_action_impl/src/actions/impls/run.rs` — rebuild
  `param_file` plumbing from `Option<ParamFileSpec>` to
  `Vec<ParamFileSlot>`. Walk `self.starlark_values.args.items` to
  build the slot list. The `expanded` arg list already records which
  arg came from which nested cmd_args (via the visitor); preserve
  that boundary information through to the executor.
- `app/kuro_execute/src/execute/request.rs` — rename
  `with_param_file(Option<ParamFileSpec>)` → `with_param_files(Vec<...>)`.
- `app/kuro_execute_impl/src/executors/local.rs` — replace lines
  778-832 (single-paramfile materializer) with a loop over slots.
  **Delete the cargo_runfiles heuristic at lines 836-870** — Phase 1
  obsoletes it.
- Any RE / hybrid executor that consumes `param_file` (grep
  `param_file()`).

**Caveats**:
- Bazel writes paramfiles to a per-action scratch dir
  (`bazel-bin/.../<action>.params`), not `/tmp`. Use the action's
  scratch path (already routed through `BUCK_SCRATCH_PATH`).
- Paramfile content for concat'd cmd_args needs the same expansion
  rules as inline (artifact short_paths, `${pwd}` etc.). Reuse the
  existing expand-once pipeline; the slot just changes *where* the
  expanded strings land.
- Bazel honours `use_always=True` even at length 0. Match that —
  unconditional materialization for forced slots.

**Effort**: ~2 days, mostly mechanical refactor. The trickiest bit is
preserving "which item produced which arg" through the
`CommandLineArgLike::add_to_command_line` walk.

### Phase 2 (small): validate end-to-end

After Phase 1, `kuro build //sdk:sdk_contents` from
`zeromatter/` should:

1. Materialize `<bs>.cargo_runfiles/_main/<pkg>/...` for every
   `cargo_build_script` action via the runner's own
   `create_runfiles_dir()`.
2. Spawn the build script with cwd inside that tree.
3. Build script writes `${OUT_DIR}/macros.rs` (e.g. for `target-triple`).
4. Dependent rustc compile reads `OUT_DIR=${exec_root}/buck-out/.../out_dir`,
   `${exec_root}` resolves through the `execroot/<basename> -> ..`
   self-symlink to the project root, and `include!(...)` finds
   `macros.rs`.

Acceptance:
- `kuro build //sdk:sdk_contents` advances past `target-triple` and
  `glue-build-script` and either succeeds or fails at a *distinct*
  layer.
- No regressions on `examples/multi_package` and the existing rules_rust
  tests in `tests/`.

### Phase 3 (small): coverage scan

`use_param_file(use_always=True)` shows up beyond rules_rust. Grep BCR
mirrors to enumerate consumers and confirm Phase 1 covers them:

```bash
grep -rn 'use_param_file' bazel-external/ \
  | grep -v rules_rust \
  | grep 'use_always = True'
```

Known/likely hits:
- `rules_proto` (proto compiler invocations with thousands of `--proto_path`).
- `rules_java` (javac with long classpaths).
- `rules_cc` (objc-style genrule wrappers occasionally).

For each, write a one-line note: "Phase 1 covers this" or "needs an
additional gap captured here". Add findings to this plan as Phase 3
notes; spin out new plans only if the gap is structural.

### Phase 4 (deferred): retire the execroot self-symlink

Once Plan 44 Phase 3 introduces a real execroot
(`<output_base>/execroot/<workspace_name>`), the
`<project_root>/execroot/<basename> -> ..` self-symlink installed by
`set_dynamic_project_root` (`app/kuro_core/src/cells.rs`) becomes
redundant and should be removed; actions running with
`cwd = <execroot>` will see `${exec_root}` resolve naturally.

The `execroot` entry in `RESERVED_OUTPUT_COMPONENTS`
(`app/kuro_file_watcher/src/notify.rs`) stays — Plan 44 Phase 3 may
move the location but the component name stays reserved.

**Triggered by**: Plan 44 Phase 3 status flip to "in progress".

## Why now

`//sdk:sdk_contents` is one of two real-world milestones tracked in
the parent plan (the other is `@llvm-project//clang:clang`).
Currently 4 layered execution-time gaps block it:

1. ✅ `${exec_root}` substitution — landed in `d29a4ff4`.
2. ⏳ Per-Args paramfile materialization — this plan, Phase 1.
3. ❓ `cargo_build_script_runner` runfiles_dir creation —
   should fall out of Phase 1; Phase 2 confirms.
4. ❓ Other runfiles-shaped gaps for non-`*_binary` rules — Phase 3
   coverage scan determines whether they exist.

Phase 1 is the smallest useful unit that unsticks the whole layer.

## Open questions

- Does the Bazel paramfile location matter to consumers? rules_rust's
  process_wrapper resolves `@<path>` itself — agnostic to location.
  Other consumers may stat the paramfile relative to `${pwd}`. If the
  scratch path isn't reachable from the action cwd (because it's an
  abs path or under buck-out), no issue; if it's a relative path,
  ensure consistency.
- Bazel allows `use_param_file` on the *top-level* args object too,
  applying to the whole command line. Phase 1 should still honour
  that case (it's strictly simpler — single slot covering all args).
- `args.set_param_file_format("multiline" | "flag_per_line" | "shell")`
  applies per Args. The slot must carry the format from its source
  cmd_args, not a global default.

## Out of scope

- Runfiles tree synthesis for non-executable rules' `default_runfiles`.
  §15.5.23 covers `DefaultInfo(executable=..., default_runfiles=...)`;
  this plan covers explicit `declare_directory` runfiles outputs. If a
  third flavour (e.g. data-only runfiles flowing into a non-executable
  rule's action inputs) shows up, capture it as a follow-up.
- Real sandboxed execution. Plan 34 covers that.
- Remote execution param-file delivery. Phase 1 ships the local
  executor; RE adoption happens once the slot vector lands in the
  request type.
