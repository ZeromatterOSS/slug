# Plan 44: Workspace layout parity with Bazel

> Parent: [2026-01-21-kuro-bazel-compatible-build-tool.md](../2026-01-21-kuro-bazel-compatible-build-tool.md)
>
> Sibling: [15-bazel-9-parity.md](./15-bazel-9-parity.md) Phase 3 (lockfile
> round-trip) — same theme, different scope.

## Status: PROPOSED

## Context

Discovered while investigating MODULE.bazel.lock churn (commit
`f9c5a705`): kuro and bazel disagree on what files appear at the
workspace root after a build, and *where* materialized external
repositories live on disk.

Verified by running `bazel 9.1.0 build :gen_version_header` cleanly in
`examples/multi_package` and listing the workspace root.

| Path | Bazel 9.1.0 | Kuro |
|---|---|---|
| `bazel-bin`, `bazel-out`, `bazel-testlogs`, `bazel-<ws>` | symlinks → `<output_base>/execroot/_main/...` | not created |
| `<workspace>/external/<apparent>` | **not created** | symlinks → `../bazel-external/<canonical>` (or sibling local dir) |
| `<workspace>/bazel-external/` | **not created** | real directory containing materialized externals |
| Materialized external repos | `<output_base>/external/<canonical>` (under `~/.cache/bazel/...`) | `<workspace>/bazel-external/<canonical>` |

Side effects of the divergence:
- 178 kuro-generated `external/<apparent>` symlinks were committed to
  the repo across `examples/hello_world/` and
  `examples/multi_package/` before commit `e0c38d0b` untracked them.
- File watcher rewrote symlink targets on every invocation, causing
  spurious DICE invalidations and git diff churn (this is what
  triggered the investigation in the first place — the
  `_main+toolchains+...` ↔ `rules_java+toolchains+...` flip-flop).
- Per-user materialization (`bazel-external/`) lives inside the
  workspace, so two checkouts on the same machine can't share fetch
  state, every clone re-fetches everything, and CI containers must
  treat the workspace as ephemeral.
- New users running `bazel build` and `kuro build` in the same
  workspace see two independent state trees and unfamiliar paths.

## Three subgoals, three sizes

### Phase 1 (small): bazel convenience symlinks at workspace root

Write `bazel-bin`, `bazel-out`, `bazel-testlogs`, `bazel-<ws>` (or the
kuro-prefix equivalents) at workspace root pointing into kuro's
buck-out tree, after a successful build.

**Targets**:
- `bazel-bin` → `buck-out/v2/gen/<cell>/<cfg_hash>` (closest analogue;
  needs a stable per-build path)
- `bazel-out` → `buck-out/v2/gen` (or `buck-out/v2`, depending on
  user expectation)
- `bazel-testlogs` → `buck-out/v2/test`
- `bazel-<ws>` → `<workspace>` itself (matches bazel's
  `bazel-<ws> → <output_base>/execroot/_main` convention; the `_main`
  side equals the workspace contents on a clean tree)

**Caveats**:
- Bazel's symlinks point into `<output_base>` because actions
  *execute* there (execroot). Kuro runs actions from the workspace
  root, so `bazel-<ws>` becomes a self-pointing symlink. Either drop
  `bazel-<ws>` from kuro's set or make it a no-op until Phase 3
  introduces an execroot.
- `bazel-bin` requires a stable target. Bazel uses `<config>/bin`
  with `<config>` being the host or target config. Kuro's
  `<cfg_hash>` rotates per configuration — pick one (host) and
  document.

**Files**: one new module under `kuro_server` or `kuro_core`,
called from the post-build hook.

**Effort**: ~50 LoC, ~1 day. Independent of Phase 2/3.

### Phase 2 (medium): move `bazel-external/` off workspace root

Bazel keeps materialized externals under
`<output_base>/external/<canonical>`. Kuro currently writes
`<workspace>/bazel-external/<canonical>`. Move kuro's location to the
equivalent of bazel's output_base.

**Target location** (proposal): `<workspace>/buck-out/v2/external/<canonical>`,
or a process-wide cache like `~/.cache/kuro/external/<canonical>`
keyed by content hash for sharing across checkouts.

**Touch points** (13 files, 80 references — `grep -rln '"bazel-external"\|/bazel-external/'`):
- `app/kuro_bzlmod/src/repository_execution.rs` — materialization target path
- `app/kuro_bzlmod/src/repository_executor.rs` — repo-rule output base
- `app/kuro_bzlmod/src/spoke_materialization.rs` — spoke creation target
- `app/kuro_client/src/commands/clean.rs` — clean command knows the path
- `app/kuro_client/src/commands/run.rs` — runfile resolution
- `app/kuro_common/src/legacy_configs/cells.rs` — symlink writer's target side
- `app/kuro_core/src/cells.rs` — `ensure_external_symlink` target side
- `app/kuro_external_cells/src/extension_repo.rs` — extension repo path
- `app/kuro_file_watcher/src/notify.rs` — watch-or-skip logic
- `app/kuro_interpreter_for_build/src/module_ctx/context.rs` — module_ctx working dir
- `app/kuro_interpreter_for_build/src/module_ctx/methods.rs` — `mctx.path()` resolution
- `app/kuro_interpreter_for_build/src/module_extension_executor_impl.rs` — extension exec path
- `app/kuro_interpreter_for_build/src/repository_ctx.rs` — `rctx.path()` resolution

**Approach**: introduce a single `external_repo_root(project_root)`
helper, replace all literal `"bazel-external"` references, then change
the helper. Keep the workspace symlink layer (Phase 3 covers that)
pointing into the new location for now.

**Caveats**:
- `MODULE.bazel.lock`'s `recordedInputs` entries embed file paths
  (e.g. `FILE:@@<module>+//<path> <sha>`); those are repo-relative,
  so the move is transparent.
- `.gitignore` already ignores `bazel-external` (line 4) and now
  `external/` (line 5-7, added in commit `ba35b25b`); both stay ignored.
- File watcher already filters out `bazel-external/` (component-match
  per `memory/file_watcher_buck_out_alias.md`); update the filter to
  the new location.

**Effort**: 2-3 days, mechanical. Low risk (no semantics change).
Can land before Phase 3.

### Phase 3 (large): introduce execroot, drop `<workspace>/external/`

Bazel runs actions from `<output_base>/execroot/<ws>` — a per-build
staging directory that *is* the action's filesystem view. Inside it,
`external/` exists as a real subdirectory containing the canonical
external repos. Kuro runs actions with `cwd = <workspace>` and
materializes apparent-name symlinks at `<workspace>/external/<apparent>`
so `external/X/...` paths in command lines resolve. This pollutes the
workspace and inverts bazel's invariant.

**Target architecture**: stage actions in `<buck-out>/v2/execroot/<ws>/`:
- Symlink each first-party source/package into the execroot
- Symlink each external canonical name into `<execroot>/external/<canonical>`
- Add apparent-name aliases at `<execroot>/external/<apparent>`
- Run actions with `cwd = <execroot>`

**Why this is large**:
- Action staging touches `kuro_execute_impl` materialization, the
  forkserver's working-dir setup, and every test that asserts
  on output paths.
- Path resolution in starlark (`ctx.actions.run`, `args.add_all`,
  `expand_location`) currently emits workspace-relative paths. Many
  of those will need to switch to execroot-relative.
- File watcher currently watches the workspace; under execroot
  staging the watch root may need to expand to cover both.
- BXL/audit commands that print paths (`audit cell`, `audit dep-files`)
  need to keep the user-facing form consistent.

**Approach** (sketch — needs its own design pass):
1. Build the execroot directory tree post-load, pre-execute. Symlinks
   only — no copying.
2. Switch action `cwd` to execroot. All command-line paths become
   execroot-relative automatically.
3. Drop `kuro_core::ensure_external_symlink` — execroot has the
   apparent-name aliases as its own sub-tree.
4. Remove the workspace-root `external/` directory entirely (now in
   `.gitignore`).

**Caveats**:
- Kuro's "single working tree" model has been a deliberate
  simplification; revisiting requires care around tools that hard-code
  `cwd=<project_root>` (gen scripts, custom genrules, bxl scripts).
- Tests live in `tests/core/` and `tests/e2e/`; many assert on path
  shape. Triage needed.

**Effort**: 1-2 weeks. High risk. Belongs after Phase 1+2 land and
the lockfile/spoke seeding decoupling (Plan 15 Phase 3 blocker).

## Out of scope

- Migrating `buck-out/` itself to `<output_base>/execroot/...` —
  buck-out is buck2's convention, kuro inherits it intentionally.
  Bazel users see `bazel-bin/` etc. as symlinks into buck-out via
  Phase 1.
- Sandbox staging (separate execution layer; tracked elsewhere).
- Cross-checkout shared cache (interesting follow-up to Phase 2 but
  not required for parity).

## Verification

- Phase 1: `kuro build :foo && readlink bazel-bin && readlink bazel-out`
  produce non-empty paths matching the buck-out output of the build.
- Phase 2: `git ls-files | grep bazel-external` empty; clean checkout
  + `kuro build` creates no `bazel-external/` at workspace root;
  external repos materialize at the new location.
- Phase 3: `git ls-files | grep "/external/"` empty; clean checkout
  + `kuro build` creates no `external/` at workspace root; zeromatter
  build still passes; tests/core still pass.

## Sources of truth (parity references)

- `bazel build :foo` workspace-root listing on bazel 9.1.0 (verified
  2026-05-05): `bazel-bin → ~/.cache/bazel/.../execroot/_main/bazel-out/k8-fastbuild/bin`,
  `bazel-out → .../bazel-out`, `bazel-testlogs → .../testlogs`,
  `bazel-multi_package → .../execroot/_main`. No `external/`, no
  `bazel-external/`.
- Bazel source: `src/main/java/com/google/devtools/build/lib/buildtool/OutputDirectoryLinksUtils.java`
  for the convenience-symlink set.
- Bazel source: `src/main/java/com/google/devtools/build/lib/skyframe/RepositoryDelegatorFunction.java`
  for `<output_base>/external/<canonical>` materialization.
