# Plan 28.1 Feasibility Spike: Bundled Builtins Loader

> **Plan**:
> [Plan 28: Bazel Builtins Module Architecture](../plans/kuro-bazel-subplans/28-builtins-module-architecture.md)
>
> **Status**: spike landed 2026-04-30. Gap empirically confirmed; design
> below to be implemented under Phase 28.2.

## Goal

Per Plan 28.1, prove that Starlark-defined builtins can be made visible
in every required context (BUILD files, `.bzl` files, both root and
external cells) without relying on the existing prelude-only injection
path.

## Findings

### Environment matrix (current state, kuro main)

| Context | Visibility today |
|---------|------------------|
| Root BUILD (cell with prelude) | Rust globals (via `register_load_natives`) + prelude's public symbols (via `import_public_symbols`) + prelude's `native` struct members (via `extra_globals_from_prelude_for_buck_files`). |
| External BUILD (cell with prelude) | Same as root BUILD — `prelude_import` returns `Some` for any `BuildFile` path. |
| Root `.bzl` (cell with prelude) | Rust globals + prelude's public symbols. |
| External `.bzl` (cell with prelude, file outside the prelude itself) | Rust globals + prelude's public symbols. `prelude_import` returns `Some` for non-prelude `LoadFile` paths (`interpreter_for_dir.rs:467-471`). |
| BUILD in Bazel-mode workspace (no prelude) | Rust globals + autoloaded `@rules_cc//cc:defs.bzl` symbols (`interpreter_for_dir.rs:317-340, 422-432`). |
| External `.bzl` in Bazel-mode workspace | **Rust globals only.** No prelude, no rules_cc autoload (autoload runs only for `BuildFile`). |

The interesting cell is the last one: external `.bzl` files in
Bazel-mode workspaces (the realistic target audience for kuro) see no
Starlark-defined symbols at all.

### Empirical confirmation of the gap

Spike test: defined `kuro_28_1_probe = "spike-ok"` at the top level of
`tests/e2e_util/nano_prelude/prelude.bzl`. Referenced it without a
`load()` from `tests/core/analysis/test_native_rules_data/defs.bzl`
(a Bazel-mode fixture — its `MODULE.bazel` is `module(name = "native_rules_test")`,
no `prelude` cell registered).

Result:

```
Error evaluating module: `native_rules_test//defs.bzl`
error: Variable `kuro_28_1_probe` not found
   --> defs.bzl:12:23
    |
 12 | _KURO_28_1_OBSERVED = kuro_28_1_probe
```

Confirms: in Bazel-mode workspaces, `import_public_symbols` is never
invoked because `prelude_import()` returns `None` (no prelude
configured). The only Starlark-defined-symbol injection that exists
today is `rules_cc_autoload`, and it gates on `BuildFile` paths only.

### Insertion point identified

`app/kuro_interpreter_for_build/src/interpreter/interpreter_for_dir.rs`:

- `Self::new` (~line 320): builds `rules_cc_autoload`. A new
  `bazel_builtins_autoload: Option<OwnedStarlarkModulePath>` field
  goes here, resolved unconditionally to a kuro-bundled path.
- `Self::parse` (~line 514-528): appends implicit imports. The new
  builtins path joins both BUILD and `.bzl` paths (vs. rules_cc which
  is BUILD-only).
- `Self::create_env` (~line 354-373): new
  `import_public_symbols(builtins_env)` call placed alongside the
  existing prelude one, gated on `bazel_builtins_autoload.is_some()`
  rather than `prelude_import().is_some()`.

### Representation decision

Three candidates considered:

1. **Dynamic insert into each `Module` before eval.** Rejected: requires
   threading mutable globals through every interpreter call site;
   doesn't compose with the existing `import_public_symbols` flow.
2. **Frozen builtins module loaded once and queried by the loader.**
   ✅ Chosen. Mirrors the existing `prelude_env` lifecycle: bundled
   `.bzl` is loaded once via the normal load resolver, ends up in
   `loaded_modules.map`, and `import_public_symbols` copies its public
   symbols into each consuming env. No new starlark-rust hooks needed.
3. **Rust forwarding callables.** Rejected for general use: requires
   a per-symbol Rust shim, which defeats the "author builtins in
   Starlark" goal of Plan 28. May still be the right answer for a
   handful of compatibility shims that need to call into Rust-only
   APIs, but not for the general loader.

### Bundled-cell mechanism

The builtins file needs to be loadable from *any* user workspace
without the user having to register it in `MODULE.bazel`. Two options:

- **A.** Auto-register a synthetic `@kuro_builtins` cell (similar to
  `@local_config_platform` per `memory/MEMORY.md` "Host Platform
  Auto-Detection (Phase 17)"). Files live under `app/kuro_external_cells_bundled/`.
  The loader resolves `@kuro_builtins//:exports.bzl` at startup.
- **B.** Special-case load resolution: a hardcoded path (e.g.
  `__kuro_builtins__//:exports.bzl`) bypassed via the load resolver
  and served from kuro's compiled bundle.

(A) is cleaner because it reuses the existing bundled-cell plumbing.
The cell registration cost is a one-time addition to the cell resolver
seed list. Pick (A) for Phase 28.2 implementation.

## Recommended Phase 28.2 work

1. Add `app/kuro_external_cells_bundled/cells/kuro_builtins/` (or
   reuse `prelude/bazel_builtins/` as the source path with bundled-cell
   wrapping).
2. Add `exports.bzl` per the Plan 28 export contract — start with a
   single probe symbol.
3. Auto-register the cell in cell resolution (mirror
   `app/kuro_external_cells_bundled/build.rs` for `local_config_platform`).
4. In `InterpreterForDir`:
   - Add `bazel_builtins_autoload: Option<OwnedStarlarkModulePath>`
     resolved at `new()` time, unconditional (not gated on prelude
     absence).
   - Append it to `implicit_imports` for both BUILD and `.bzl` paths
     in `parse()`.
   - Call `import_public_symbols(builtins_env)` from `create_env()`
     when the autoload is set.
5. Acceptance test: a fixture without a prelude and without registering
   `@kuro_builtins` (autoload) sees a probe symbol from `exports.bzl`
   at the top of an external `.bzl` file.

## Risks / open questions

- **Provider identity.** Starlark-defined providers live on the bundled
  module's heap. Identity-comparison consumers (e.g.
  `target[CcInfo]` resolution) must agree on which provider object is
  canonical. Plan 28.3 says "Avoid provider migrations until identity
  semantics are designed" — keep providers Rust-side until then.
- **DICE invalidation.** Bundled builtins are static, so the loader
  should freeze once per daemon run. A digest-of-file-contents
  invalidation key is simpler than tracking individual file mtimes.
- **Stack traces.** When a symbol resolves through the builtins
  autoload, error backtraces should still attribute the call site to
  the user's `.bzl`, not to kuro internals. The existing
  `import_public_symbols` flow already gets this right for prelude.
- **Order of injection.** Builtins should *not* override user
  `load()` bindings or Rust globals when names collide. Today
  `import_public_symbols` fires before user load bindings are
  established, so user `load()` wins by re-binding — verify the same
  ordering applies to the builtins autoload before depending on it.
- **External `.bzl` files inside the bundled builtins cell.** The
  loader must reject loads from outside the bundled package, mirroring
  the Plan 28.2 acceptance bullet "rejects loads outside the builtins
  package unless explicitly allowlisted." `prelude_import()`'s
  `is_prelude_path` check is a usable template.

## What I did not change

- No new Rust code; this is a research-only deliverable.
- The probe symbol added to `nano_prelude/prelude.bzl` and the
  matching test were reverted after the spike confirmed the gap.
- Plan 28's plan doc already captures the design intent — this
  document records the *empirical* check and the specific kuro
  insertion points, not the high-level plan.
