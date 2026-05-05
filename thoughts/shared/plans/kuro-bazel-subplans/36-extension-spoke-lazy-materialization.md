# Plan 36: Module-Extension Spoke-Repo Lazy Materialization

> Parent: [2026-01-21-kuro-bazel-compatible-build-tool.md](../2026-01-21-kuro-bazel-compatible-build-tool.md)
>
> Discovered while verifying Plan 13 Phase 3 against
> `zeromatter//sdk:sdk_contents` on 2026-05-05. Plan 13 reduced
> `bazel-external/` from 1120 → 120 repos as designed, but the build
> still fails because the rules_rs `crate` extension calls
> `mctx.path(Label("@cargo_linux_x86_64_1_95_0//:bin/cargo"))` and gets
> back a path inside an unmaterialized spoke repo.
>
> Adjacent to Plan 10 (which marked `module_ctx.path(Label)` "complete"
> after teaching it to resolve strings) and Plan 23 (which targets the
> `toolchains_buildbuddy` extension shape). This plan covers the
> structural gap those two left: extensions that reference *internal*
> spoke repos by `Label` and depend on those spokes being on disk
> before the next call.

## Scope

When a module extension calls `mctx.path(Label("@spoke_repo//pkg:file"))`
or `mctx.read(Label("@spoke_repo//pkg:file"))`, kuro must guarantee the
spoke repo is materialized on disk before returning. Today kuro returns
a synthesized path that points into an empty (or absent) directory;
subsequent `mctx.execute([cargo_path, ...])` then fails with `No such
file or directory (os error 2)` because the spoke was never
materialized.

**In scope:**

1. Trigger lazy materialization of the spoke repo on
   `module_ctx.path(Label)` and `module_ctx.read(Label)` access.
2. Same trigger for `repository_ctx.path(Label)` /
   `repository_ctx.read(Label)` if it's missing there too (audit).
3. Backfill the few `repository_rule_attr` accessors surfaced once the
   `@rules_python` python extension started generating spokes
   (`auth_patterns`, `_rules_python_workspace`).
4. Stub-repo materialization fallbacks for sub-repos created by
   extensions that themselves stubbed (e.g. `aspect_tools_telemetry`'s
   `tel_repository` calling `rctx` on a stub MODULE.bazel) — surface
   the underlying extension failure rather than producing a confusing
   `No such file` further down the chain.

**Out of scope:**

- Apple-only platform extensions (e.g. `apple_cc_configure_extension`).
  Tracked separately under platform support, not host-build blocker.
- Full Skyframe-equivalent demand-driven repo loading. We continue to
  use kuro's existing `ExtensionRepoExecutionKey` DICE machinery; this
  plan just plumbs `mctx`-side Label access into it.
- Plan 13's eager-vs-deferred toolchain split — already complete.

## Current State Analysis

### Symptom (zeromatter `//sdk:sdk_contents`, 2026-05-05)

```
Extension 'crate' implementation failed:
  bazel-external/rules_rs+override/rs/extensions.bzl:1185, in _crate_impl
    cargo_path = mctx.path(RS_HOST_CARGO_LABEL)   # @cargo_linux_x86_64_1_95_0//:bin/cargo
  bazel-external/rules_rs+override/rs/extensions.bzl:284, in _generate_hub_and_spokes
    result = mctx.execute(
      [cargo_path, "metadata", "--no-deps", "--format-version=1", "--quiet"],
      ...
    )
error: Failed to execute command: No such file or directory (os error 2)
```

`bazel-external/` after the run shows
`rules_rs+toolchains+default_rust_toolchains` and
`rules_rs+toolchains+rs_rust_host_tools` materialized but **no**
`rules_rs+toolchains+cargo_linux_x86_64_1_95_0`. The `toolchains`
extension *did* declare a `cargo_repository(name=...)` for the host
triple, but kuro never materialized the resulting spoke spec.

Two further loud failures from the same run, same shape:
- `aspect_tools_telemetry+telemetry+aspect_tools_telemetry_report`:
  `tel_repository` rctx tries to read its own `MODULE.bazel` which
  doesn't exist (parent extension stubbed).
- `apple_cc_configure_extension`: tries to read
  `build_bazel_apple_support/crosstool/BUILD.toolchains` from an
  unmaterialized apple_support spoke. Expected on Linux hosts; goal
  here is "fail loudly with a real error" not "silently work."

### What works today

`app/kuro_external_cells/src/extension_repo.rs:488-511` already
registers all spokes returned by an extension's `ext_result` as
dynamic extension cells, and `ExtensionRepoExecutionKey` will lazily
materialize a spoke on first cell-resolver access. Plan 10 Phase 2
made `module_ctx.path(Label)` resolve the Label to a filesystem path
via `resolve_label_to_filesystem_path`.

### What's broken

`module_ctx.path(Label)` and `module_ctx.read(Label)`
(`app/kuro_interpreter_for_build/src/module_ctx/methods.rs:453,66`)
both compute a path via `resolve_label_to_filesystem_path` but **never
trigger DICE materialization** of the referenced cell. The path
returned is therefore valid-looking but points into a directory that
doesn't exist on disk. The next `mctx.execute()` / `mctx.read()` then
fails with the kernel-level "No such file or directory."

Bazel's `StarlarkBaseExternalContext.getPathFromLabel()` resolves the
`Label` *and* enqueues a Skyframe dependency on the owning repository,
which forces it to be fetched before the function returns. We need
the equivalent.

### Why Plan 10 thought it was done

Plan 10 Phase 2's success criteria checked only "doesn't crash" and
"string case still works." The case where the resolved Label points
into a not-yet-materialized spoke wasn't part of the test, and the
extensions Plan 10 verified against happened to reference Labels in
already-eagerly-materialized cells. The zeromatter `crate` extension
is the first real consumer of an internal spoke from inside another
extension's `mctx.path(Label)` call.

## Desired End State

After this plan:

1. `kuro build //sdk:sdk_contents` (zeromatter) gets past the
   `crate` extension. The cargo binary is on disk by the time
   `mctx.execute(cargo_path, ...)` runs.
2. Extensions that legitimately depend on host-only spokes (apple,
   windows-only) fail with a single clear error tracing back to the
   missing spoke, not a derivative `No such file` from a downstream
   `read`/`execute`.
3. `repository_rule_attr` exposes the named attributes rules_python
   passes to its repository rules (`auth_patterns`,
   `_rules_python_workspace`, plus whatever else surfaces during
   verification).
4. No regression on `bazel-external/` count from Plan 13 — only
   Label-referenced spokes get pulled in, not the full transitive
   closure.

### Verification criteria

- [ ] `kuro build //sdk:sdk_contents` reaches an analysis or build
      action error, not a `module_ctx`/`repository_ctx` extension
      error.
- [ ] `bazel-external/` after the build contains
      `rules_rs+toolchains+cargo_linux_x86_64_1_95_0` (the spoke we
      forced via Label access) but NOT every spoke the toolchains
      extension declared.
- [ ] Existing `examples/lazy_toolchain` smoke test still completes
      under the Plan 13 budget.
- [ ] `cargo test -p kuro_interpreter_for_build` passes.
- [ ] `pytest tests/core/bzlmod tests/core/analysis -q` no
      regressions.

## Phase 1: Audit and Spec

Before touching code, enumerate the surface that needs the trigger:

- `module_ctx`: `path`, `read`, `watch_tree`, `template`,
  `extract`, `download`, `download_and_extract` — any method that
  *consumes* a Label.
- `repository_ctx`: same surface; verify whether the existing impl
  already triggers materialization (it accesses cell paths via
  `kuro_core::cells::get_dynamic_extension_cell` which is registered
  but doesn't drive DICE — same gap, just observed less because
  `repository_ctx` callers tend to be inside repo rules whose cell is
  already being materialized).

Decide: do we trigger materialization eagerly inside the method (fast
path, blocks the extension's Starlark thread on a DICE compute), or
do we pre-warm during cell registration (reuses Plan 10's spoke
registration path)?

**Recommendation**: trigger inside the method. Pre-warming would
break Plan 13's lazy-loading invariant (we'd materialize all spokes
just because they were *registered*, not because they were *used*).

## Phase 2: Wire `mctx.path(Label)` to DICE Materialization

### Touchpoints

- `app/kuro_interpreter_for_build/src/module_ctx/methods.rs::path`
  and `::read`: when the resolved path's containing cell is registered
  as a dynamic extension cell, run a synchronous
  `ExtensionRepoExecutionKey::compute()` (or equivalent) before
  returning.
- `app/kuro_interpreter_for_build/src/module_ctx/context.rs`: thread
  a DICE handle (or a callback) onto `ModuleContext` so methods can
  drive computation. Today `ModuleContext` is a pure data struct — the
  executor in `module_extension_executor_impl.rs:324` has DICE
  access; we need to pass it down.
- `app/kuro_external_cells/src/extension_repo.rs`: expose a single
  entry point `materialize_spoke_repo(canonical_name) -> Result<...>`
  that wraps the existing per-spoke DICE compute. The extension
  executor and the new module_ctx path/read both call this.

### Expected refactor cost

Threading DICE through `ModuleContext` is the hard bit. Existing
methods are `fn (this: &ModuleContext, ...)` with no `Evaluator`/
`DiceComputations` access. Two viable shapes:

- **Add `&mut Evaluator` to method signatures**, then stash a DICE
  handle in `Evaluator::extra` during extension execution. Mirrors how
  several `repository_ctx` methods already access services. Surgical.
- **Stash a `Box<dyn Fn(canonical_name) -> Future<Output = Result<()>>>`
  on `ModuleContext`** at construction. Avoids touching method
  signatures but requires careful lifetime/Send shenanigans because
  Starlark methods run synchronously while DICE compute is async. We'd
  need to block on a oneshot from a runtime handle — same pattern
  `repository_ctx::execute` already uses for subprocess.

Pick one during Phase 1; both are tractable.

## Phase 3: Same Trigger for `repository_ctx`

Audit `repository_ctx::path`, `read`, `template`, `symlink`,
`patch`, `watch`. Whichever methods accept `Label` should drive the
same `materialize_spoke_repo`. In practice this is rarely the failure
mode in current real-world repos because repo-rule callers' own cell
is the one being materialized — but the asymmetry is a footgun and
Plan 23's macro-wrapped patterns will hit it.

## Phase 4: `repository_rule_attr` Attribute Backfill

Once `@rules_python`'s `python` extension stops failing in Plan 13
Phase 3 verification, its repository rules surface as the next blocker:

```
error: Object of type `repository_rule_attr` has no attribute `auth_patterns`
error: Object of type `repository_rule_attr` has no attribute `_rules_python_workspace`
```

These come from `repository_rule(attrs = {"auth_patterns": attr.string_dict(), ...})`.
The attrs *should* be reflected onto `rctx.attr.<name>`. Audit
`repository_ctx::attr` (look for the field map) and confirm it returns
*all* declared attributes, not a hardcoded subset. Likely a one-line
fix in the attr-projection code.

## Phase 5: Loud-Fail for Stubbed Sub-Extension Spokes

When a parent extension stubs (e.g. `aspect_tools_telemetry`), its
declared sub-repos (`aspect_tools_telemetry_report`) currently get a
real materialization attempt that fails with a confusing
`Failed to read MODULE.bazel`. Either:

- Mark the parent's stub-status on registered sub-cells so kuro skips
  the lazy materialization attempt and returns the parent's stub error.
- Or pre-stub each declared sub-repo at the same time as the parent's
  stub.

Pick whichever yields the cleanest single-error trace.

## Phase 6: ZeroMatter `//sdk:sdk_contents` Walkthrough

Run the build, check off blockers as each one falls. Expected order
based on 2026-05-05 evidence:

1. `crate` extension reaches the `mctx.execute(cargo_path, ...)`
   call successfully.
2. `crate` extension generates `@crates//:defs.bzl` with the real
   `all_crate_deps` symbol.
3. `sdk/config_install:config_install` analysis proceeds.
4. Whatever surfaces next (likely action-execution / toolchain
   resolution issues already covered by Plans 11/24/25).

This is iterative. Each fix is minimal. New blockers that aren't in
this plan's scope get linked back to their owning subplan.

## Risks

- **DICE re-entrancy**: extensions execute *inside* a DICE compute
  already; nested compute calls must dedup correctly. The existing
  per-spoke `ExtensionRepoExecutionKey` is keyed on canonical name, so
  this should be safe, but verify on the first nested call.
- **Bazel test parity**: real Bazel allows `mctx.path(Label)` on
  cells that don't exist yet *without* failing — it returns the path
  for use with `.dirname` etc. and only triggers fetch on the next
  filesystem operation. We may need to lazy-trigger only on
  `mctx.read` and let `mctx.path` return a "promise" path. If we get
  that wrong, we'll trigger fetches for paths that were only being
  queried for `.dirname`. Mitigation: instrument the first
  implementation, count how often `path` is called *without* a
  follow-up fs op, only optimize if the metric warrants.
- **Plan 13 budget**: this plan re-introduces fetches that Plan 13
  deferred. Each fetch must be well-justified (i.e. an extension
  actually reached for it via Label), not transitive eager fetching.

## Cross-References

- Plan 10 (`10-module-extension-execution.md`) — Phase 2 marked
  module_ctx.path(Label) "complete" but only covered the
  string-resolution case. This plan finishes the job.
- Plan 13 (`13-lazy-toolchain-loading.md`) — Phase 3 verified
  bazel-external dropped 1120 → 120; this plan addresses what
  surfaced *after* that drop.
- Plan 23 (`23-module-extension-realworld.md`) — covers the
  `toolchains_buildbuddy` macro-wrapped repository_rule shape;
  separate concern but its Phase 4 (`rctx.template`/`rctx.symlink`
  Label handling) overlaps with this plan's Phase 3 audit.
- Commit `93097ab6` (2026-05-05) — `module_ctx.read(Label)` and
  `module_ctx.getenv(default=None)` fixes that surfaced this plan's
  scope. Those were prerequisites; the spoke-materialization gap is
  the current blocker.
