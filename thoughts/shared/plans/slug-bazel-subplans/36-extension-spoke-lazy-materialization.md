# Plan 36: Module-Extension Spoke-Repo Lazy Materialization

> Parent: [2026-01-21-slug-bazel-compatible-build-tool.md](../2026-01-21-slug-bazel-compatible-build-tool.md)
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

## Status: PARTIAL

Phases 1-3 landed and zeromatter now reaches deep analysis with lazily
materialized crate spokes. This plan remains high-priority because the
remaining follow-ups are extension correctness gaps, not cleanup:

1. Phase 3b: audit `repository_ctx.path(Label)` /
   `repository_ctx.read(Label)` for the same materialization guarantee.
2. Phase 4: backfill `repository_rule_attr` accessors surfaced by real
   rules_python/rules_rs extensions (`auth_patterns`,
   `_rules_python_workspace`, `vcs`, plus anything discovered during the
   audit).
3. Phase 5: replace confusing downstream `No such file` errors from
   stubbed sub-extension spokes with a direct extension-failure error.

2026-05-09 follow-up: `module_ctx.execute([Label(...), ...])` and
`repository_ctx.execute([Label(...), ...])` now use the same resolved
Label materialization path as `ctx.path(Label)`. The shared Label
filesystem resolver also resolves dynamic extension apparent aliases
(notably `use_repo_rule` repos such as `@toml2json_linux_amd64`) to
their canonical `bazel-external/<canonical>` directories, and
precomputed `use_repo_rule` RepoSpecs are seeded into the lazy
materialization registry. This removes the rules_rs
`toml2json.bzl:6` `No such file or directory` failure in the bounded
zeromatter run. Phase 3b remains partially open for the broader
repository_ctx method audit/stub-failure cleanup, but Label execute tool
materialization is covered.

2026-05-09 follow-up 2: the next zeromatter smoke reached
`aspect_tools_telemetry+telemetry+aspect_tools_telemetry_report` lazy
materialization. That repository rule exercises Bazel's
`repository_ctx.workspace_root` to probe/read the root workspace
`MODULE.bazel(.lock)`. Slug was exposing `workspace_root` as the
generated repository working directory, causing the telemetry rule to
look for root files inside the repo it was creating and then stub.
Bazel source of truth:
`StarlarkRepositoryContext.getWorkspaceRoot()` returns
`directories.getWorkspace()`, while `StarlarkBaseExternalContext.path`
keeps string-relative paths anchored at the repository working
directory. Slug now passes the invocation project root into
`RepositoryContext` and keeps `repository_ctx.path("relative")`
relative to the repo dir. This is a Phase 3b repository_ctx parity
fix; Phase 5 loud-fail/stub cleanup remains open.

2026-05-09 follow-up 3: after rebuilding `slug`, the bounded zeromatter
smoke in `/tmp/plan55-workspace-root-1.log` advanced past the previous
telemetry point into later extension/repository-rule work. A second run
after deleting the stale generated telemetry stub
(`/tmp/plan55-workspace-root-2.log`) did not recreate that repo before
failing later, so it did not directly re-exercise the telemetry
repository rule. The next systemic blocker observed in that second log
is Starlark provider field presence semantics: rules_kotlin defines a
provider with optional field `strip_prefix_template`, then checks
`hasattr(version, "strip_prefix_template")`; Slug reports true for a
field whose value is absent/`None`, causing `.format(...)` on `None`.
Verify Bazel's provider-field/`hasattr` behavior before changing it,
then fix that as a Starlark-provider parity issue rather than a
rules_kotlin special case.

2026-05-09 follow-up 4: Bazel 9 provider-field presence was verified
against both source and a focused Bazel 9.1.0 repro. In
`StarlarkInfoWithSchema`, `getFieldNames()` only returns schema fields
whose table slot is non-null, and `Starlark.hasattr` only reports
structure fields where `getValue(name) != null`; the repro confirmed
`P(a="x")` for `provider(fields = ["a", "b"])` has `dir(p) == ["a"]`,
`hasattr(p, "b") == False`, and `getattr(p, "b", "fallback") ==
"fallback"`, while explicit `b = None` remains present. Slug provider
instances now track field presence separately from stored values, so
missing optional provider fields are absent but explicit `None` remains
present. The bounded zeromatter smoke in
`/tmp/plan56-provider-presence-1.log` advanced past the previous
rules_kotlin `strip_prefix_template` failure. It timed out later while
waiting on `crates__github.com_ZeroMatter_diplomat.git_99406ff1//runtime`
package file tree loading, with Gazelle `go_repository` cache/stub
failures and non-host JDK download timeouts as concurrent later noise.

2026-05-09 follow-up 5: the diplomat package-file-tree wait was not a
Plan 36 spoke-materialization failure. The missing package triggered
`extended_ignore_error`, whose cross-cell suggestion scan probed every
registered cell; probing extension cells can lazily materialize unrelated
repos. Slug now skips external cells for that diagnostic suggestion path.
The bounded smoke in `/tmp/plan57-missing-dir-suggestion-1.log` advanced
past the diplomat wait into `zeromatter//sdk:sdk_contents` analysis and
failed later on `platforms+1.1.0//:BUILD` because `module_version()` is
missing as a BUILD-file global. That next blocker belongs to Bazel 9
BUILD/prelude parity, not Plan 36.

2026-05-09 follow-up 6: the `platforms+1.1.0//:BUILD`
`module_version()` blocker was Bazel 9 BUILD-global parity, not a spoke
materialization issue. Bazel 9 exposes all non-rule `native` module methods
as direct BUILD-file globals via `StarlarkNativeModule.BINDINGS_FOR_BUILD_FILES`
and `StarlarkGlobalsImpl.getFixedBuildFileToplevelsSharedWithNative()`. Slug
now exposes direct `module_name()` and `module_version()` BUILD globals backed
by the same module metadata as `native.module_name()` /
`native.module_version()`. The bounded smoke in
`/tmp/plan58-module-build-globals-1.log` advanced past the previous
`platforms+1.1.0//:BUILD` missing-symbol error and analyzed platforms targets.
It failed later with a missing package in another generated crate repo:
`crates__github.com_Aleph-Alpha_ts-rs.git_a6bbbd18//ts-rs` does not exist.
That is the next narrow blocker.

2026-05-09 follow-up 7: the missing `ts-rs` package was not a
crate-spoke registration or lockfile RepoSpec gap. The lockfile had a
`git_cargo_workspace_repository` RepoSpec for
`crates__github.com_Aleph-Alpha_ts-rs.git_a6bbbd18`, but an earlier failed
lazy materialization left a stub in the shared `bazel-external/` tree. Slug
now discards any prior stub marker when a non-empty RepoSpec is available and
retries real repository-rule execution. The actual repo-rule failure was
repository_ctx parity in Bazel's git worker: `ctx.path(".")` stringifies as an
absolute repository path, `ctx.delete(ctx.path("."))` deletes the repository
root when present, and a following `ctx.execute(..., working_directory=root)`
recreates that missing directory. Slug now stringifies `RepositoryPath` as an
absolute normalized path, creates missing `repository_ctx.execute`
working directories, unsets `environment` entries whose value is `None`, returns
Bazel-style booleans from `repository_ctx.delete`, and normalizes delete paths
so `repo/.` does not hit Linux `EINVAL`. The bounded smoke in
`/tmp/plan64-repoctx-delete-root-2.log` advanced past the previous missing
`crates__github.com_Aleph-Alpha_ts-rs.git_a6bbbd18//ts-rs` package: the git repo
materialized, including `ts-rs/BUILD.bazel`. The next blocker is load-label
canonicalization for generated BUILD files:
`@crates__ts-rs-12.0.1//:crate.bzl` must be accepted or rewritten as the
canonical `@rules_rs+crate+crates__ts-rs-12.0.1//:crate.bzl`.

2026-05-09 follow-up 8: the load-label canonicalization blocker was a
Bazel repo-mapping parity issue, not a ts-rs special case. Bazel's
`Label.parseWithPackageContext` applies the current package's
`RepositoryMapping` to single-`@` labels, `BzlLoadFunction` fetches
the mapping for the repository containing the loaded file, and
`ModuleExtensionRepoMappingEntriesFunction` gives extension-generated
repos visibility to all repos generated by the same extension by
internal name. Slug now accepts those apparent names during load
resolution and immediately rewrites the resolved `CellPath` to the
canonical reformed path so `.bzl` module identity stays canonical.
Focused tests cover registered dynamic aliases and same-extension
internal repo names. The bounded smoke in
`/tmp/plan65-load-repo-mapping-1.log` advanced past the previous
`@crates__ts-rs-12.0.1//:crate.bzl` failure. The next blocker is later
analysis of zstd's generated crate dependency:
`zstd+1.5.7//:zstd+1.5.7` is requested, but the materialized package
only exposes `:zstd`.

2026-05-09 follow-up 9: corrected the follow-up-8 smoke reading. The
bounded `/tmp/plan66-label-shorthand-zstd-1.log` did not stop at the
temporary `aspect_tools_telemetry` idle; it eventually failed in
`crates__zstd-sys-2.0.16-zstd.1.5.7` attr coercion because the already
materialized generated BUILD still contained `deps = ["@@zstd//:"]`.
The source include file
`external_cells/bzlmod/rules_rs/override/3rd_party/zstd-sys/include.MODULE.bazel`
has the Bazel-valid shorthand `deps = ["@zstd"]`, and the zeromatter
`MODULE.bazel.lock` cache now has the correct regenerated value
`"@@zstd//:zstd"`. Bazel 9.1.0 repros confirm bare `@zstd` resolves to
the root target named `zstd` (`@@zstd+//:zstd` in Bazel's canonical
form), while `@zstd//:` is rejected as an empty target.

Slug changes in this slice:

- repository-rule attr label capture has focused coverage that bare
  `@zstd` canonicalizes to `@@zstd//:zstd`;
- extension repo successful materialization writes
  `.slug_repo_complete` as `complete:<RepoSpec hash>` for future
  invalidation;
- existing legacy `complete` markers are still accepted so Slug does
  not re-run every old crate repository;
- legacy generated BUILD files containing invalid empty-target label
  strings are repaired from the current RepoSpec label attrs and then
  stamped with the current spec hash. This fixes stale `@@zstd//:` style
  output without re-executing the crate repository rule.

Verification:

```sh
cargo fmt
cargo test -p slug_external_cells extension_repo::tests:: --lib
cargo test -p slug_bzlmod test_extension_repo_complete_marker_includes_spec_hash --lib
cargo test -p slug_interpreter_for_build repository_rule::tests::repository_attr_bare_repo_label_uses_repo_name_as_target --lib
cargo check -p slug_external_cells
cargo build -p slug
git diff --check
```

Bounded smoke:

```sh
cd /var/mnt/dev/zeromatter
timeout 180s env SLUG_MEMORY_CHECKPOINTS=1 \
  /var/mnt/dev/slug/target/debug/slug \
    --isolation-dir plan67-zstd-spec-hash-2 \
    build //sdk:sdk_contents \
  2>&1 | tee /tmp/plan67-zstd-spec-hash-2.log
```

The smoke did not reach zstd because the previous
`plan67-zstd-spec-hash-1` attempt used an overly broad invalidation rule
and contaminated the shared `bazel-external` tree: several crate repos
were re-run, failed at `ctx.execute([Label("@toml2json_...")])` with
`No such file or directory`, and were replaced by stubs. The second
smoke now fails earlier at `crates__clap-4.5.60//:clap` because that
repo's generated BUILD is a stub with zero targets. The next narrow
frontier is repository_ctx Label-tool materialization for
use_repo_rule-generated tools such as `@toml2json_linux_amd64`, plus
cleaning/restoring the stale stubbed crate repos in the shared external
tree before using zeromatter `//sdk:sdk_contents` as a zstd signal again.

2026-05-09 follow-up 10: completed the repository_ctx Label-tool
materialization gap for `use_repo_rule()` repos. Bazel source
`StarlarkBaseExternalContext.execute` converts Label argv entries via
`getPathFromLabel(label).toString()`, and `getPathFromLabel` obtains a
`RootedPath` from Skyframe before returning. Slug already converted Label
argv values through `resolve_label_to_filesystem_path()` and called the
lazy spoke materializer, but precomputed `use_repo_rule()` repos were only
present in the static `CellResolver`. They were not mirrored into the
dynamic extension-cell registry that Label filesystem resolution consults.

Slug now registers every precomputed extension repo with
`register_dynamic_extension_cell_with_setup()` while building bzlmod cells,
so apparent aliases such as `@toml2json_linux_amd64` resolve to the
canonical `bazel-external/rules_rs+http_file+toml2json_linux_amd64` path
before `repository_ctx.execute` runs. The existing stale-stub invalidation
then retries real crate repository execution from the valid RepoSpec; no
manual broad cleanup of `zeromatter/bazel-external` was needed. A
focused `use_repo_rule()` precompute test now asserts that the serialized
RepoSpec is present for lazy materialization.

Verification:

```sh
cargo fmt
cargo test -p slug_bzlmod pending_repo_cells::tests::test_precompute_use_repo_rule_uses_canonical_name_and_apparent_alias --lib
cargo test -p slug_interpreter_for_build label_filesystem::tests::resolves_dynamic_extension_apparent_alias_to_canonical_path --lib
cargo check -p slug_common
cargo build -p slug
git diff --check
```

Bounded smoke:

```sh
cd /var/mnt/dev/zeromatter
timeout 240s env SLUG_MEMORY_CHECKPOINTS=1 \
  /var/mnt/dev/slug/target/debug/slug \
    --isolation-dir plan68-label-tool-2 \
    build //sdk:sdk_contents \
  2>&1 | tee /tmp/plan68-label-tool-2.log
```

The smoke advanced past both the previous `toml2json.bzl:6`
`No such file or directory` failures and the stale
`crates__clap-4.5.60//:clap` zero-target failure. It materialized
`rules_rs+http_file+toml2json_linux_amd64/file/downloaded`, analyzed
`crates__clap-4.5.60//:clap`, reached `zeromatter//sdk:sdk_info_json`, and
timed out later waiting on the already-tracked
`rules_rust//ffi/rs:empty_allocator_libraries` analysis/toolchain cycle.
That next blocker belongs to Plan 15 / Plan 51 toolchain-analysis work, not
Plan 36 spoke materialization.

## Scope

When a module extension calls `mctx.path(Label("@spoke_repo//pkg:file"))`
or `mctx.read(Label("@spoke_repo//pkg:file"))`, slug must guarantee the
spoke repo is materialized on disk before returning. Today slug returns
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
  use slug's existing `ExtensionRepoExecutionKey` DICE machinery; this
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
triple, but slug never materialized the resulting spoke spec.

Two further loud failures from the same run, same shape:
- `aspect_tools_telemetry+telemetry+aspect_tools_telemetry_report`:
  `tel_repository` rctx tries to read its own `MODULE.bazel` which
  doesn't exist (parent extension stubbed).
- `apple_cc_configure_extension`: tries to read
  `build_bazel_apple_support/crosstool/BUILD.toolchains` from an
  unmaterialized apple_support spoke. Expected on Linux hosts; goal
  here is "fail loudly with a real error" not "silently work."

### What works today

`app/slug_external_cells/src/extension_repo.rs:488-511` already
registers all spokes returned by an extension's `ext_result` as
dynamic extension cells, and `ExtensionRepoExecutionKey` will lazily
materialize a spoke on first cell-resolver access. Plan 10 Phase 2
made `module_ctx.path(Label)` resolve the Label to a filesystem path
via `resolve_label_to_filesystem_path`.

### What's broken

`module_ctx.path(Label)` and `module_ctx.read(Label)`
(`app/slug_interpreter_for_build/src/module_ctx/methods.rs:453,66`)
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

1. `slug build //sdk:sdk_contents` (zeromatter) gets past the
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

- [x] `slug build //sdk:sdk_contents` reaches an analysis or build
      action error, not a `module_ctx`/`repository_ctx` extension
      error (2026-05-05). New failure: target compatibility
      (`crates__clap-4.5.60` incompatible with host platform) — out
      of Plan 36 scope, belongs to Plan 11/24.
- [x] `bazel-external/` after the build contains
      `rules_rs+toolchains+cargo_linux_x86_64_1_95_0` (2026-05-05).
      The crate extension successfully drove its `mctx.path(Label)`
      through the new lazy-materialization path. Total bazel-external
      count grew from 120 (Plan 13 baseline) to 1915 — reflects the
      crate extension legitimately materializing per-crate spoke
      repos for the workspace's transitive Cargo deps. This is the
      correct Bazel-shaped behavior; Plan 13's Phase 3 win still
      applies (we did not eagerly load thousands of toolchain repos
      we don't need).
- [ ] `examples/lazy_toolchain` smoke test still completes under the
      Plan 13 budget — blocked by a pre-existing gazelle MODULE.bazel
      `print()` builtin parsing issue, unrelated to Plan 36.
- [x] `cargo test -p slug_interpreter_for_build` passes (50 passed
      2026-05-05).
- [x] `cargo test -p slug_bzlmod --lib` passes (163 passed 2026-05-05).
- [x] `pytest tests/core/analysis -q` — no regressions (18 pre-existing
      failures, 300 passed; identical pass/fail set before/after Plan 36).

## Implementation status (2026-05-05)

Phase 1 (audit) and Phase 2 (mctx.path/read → DICE materialization)
landed on `main`. Touchpoints:

- `app/slug_bzlmod/src/spoke_materialization.rs` (new) — global
  `SPOKE_REGISTRY: RwLock<HashMap<canonical_name, SpokeRegistration>>`
  + `EXTENSION_DICE_PTR` thread-local + `materialize_spoke_sync()`
  sync→async bridge using `tokio::task::block_in_place` +
  `Handle::block_on`. The bridge is the only `unsafe` in this plan;
  it is sound because the pointer's lifetime is strictly bounded by
  the `with_extension_dice` scope, which is a synchronous closure
  inside a single async DICE compute.
- `app/slug_external_cells/src/extension_repo.rs` — when the spoke
  registration loop registers dynamic cells, it now also calls
  `slug_bzlmod::register_spoke()` so the canonical name maps to its
  `RepoSpec` for later lazy materialization.
- `app/slug_interpreter_for_build/src/module_extension_executor_impl.rs`
  — wraps the Starlark eval in `slug_bzlmod::with_extension_dice(ctx,
  || { ... })` so that nested sync code can drive DICE work.
- `app/slug_interpreter_for_build/src/module_ctx/methods.rs` — `read()`
  and `path()` resolve a `Label` to a path, then call a new helper
  `ensure_spoke_materialized(&path)` that extracts the canonical
  spoke name from the `bazel-external/<canonical>/...` shape and
  drives `materialize_spoke_sync`.

Phase 3 (analysis-time spoke materialization, partially landed
2026-05-05 in commit `01ce01f5`): the analysis-time complement to
Phase 2's `mctx.path/read` sync bridge. When the cell resolver
synthesizes a `CellInstance` for a dynamic extension spoke
(`get_or_create_dynamic_cell`), it now attaches
`ExternalCellOrigin::ExtensionRepo(setup)` if the spoke was
registered with a `RepoSpec` via the new
`register_dynamic_extension_cell_with_setup` API. With the origin
set, file-ops accesses route through
`extension_repo::get_file_ops_delegate`'s lazy materialization path
— same code path used by `use_repo`'d cells at startup.

Without this, target analysis that reaches deep into crate-spoke
dependencies (e.g. `crates__clap-4.5.60//:clap` from a `rust_binary`)
hit raw `read_dir` on an unmaterialized `bazel-external/.../`
directory and aborted before triggering materialization. With it,
~550 crate spokes lazily materialize during zeromatter's
`//sdk:sdk_contents` analysis as their cells are first accessed.

`repository_ctx` audit (the original Phase 3 scope) is still open —
it's a sister concern but not the current blocker. Logged as Phase
3b for follow-up.

Phase 4 (`repository_rule_attr` backfill): not started — surfaced as
the new top blocker once Phase 2 unblocked the python extension's
spoke generation. Tracked here, work pending. Specific symbols:
`auth_patterns`, `_rules_python_workspace`, `vcs`.

Phase 5 (loud-fail for stubbed sub-extensions): not started.

Phase 6 (zeromatter walkthrough): in progress. New blocker is target
compatibility, not extension execution.

## Phase 1: Audit and Spec

Before touching code, enumerate the surface that needs the trigger:

- `module_ctx`: `path`, `read`, `watch_tree`, `template`,
  `extract`, `download`, `download_and_extract` — any method that
  *consumes* a Label.
- `repository_ctx`: same surface; verify whether the existing impl
  already triggers materialization (it accesses cell paths via
  `slug_core::cells::get_dynamic_extension_cell` which is registered
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

- `app/slug_interpreter_for_build/src/module_ctx/methods.rs::path`
  and `::read`: when the resolved path's containing cell is registered
  as a dynamic extension cell, run a synchronous
  `ExtensionRepoExecutionKey::compute()` (or equivalent) before
  returning.
- `app/slug_interpreter_for_build/src/module_ctx/context.rs`: thread
  a DICE handle (or a callback) onto `ModuleContext` so methods can
  drive computation. Today `ModuleContext` is a pure data struct — the
  executor in `module_extension_executor_impl.rs:324` has DICE
  access; we need to pass it down.
- `app/slug_external_cells/src/extension_repo.rs`: expose a single
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

- Mark the parent's stub-status on registered sub-cells so slug skips
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
