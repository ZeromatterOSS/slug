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

### Phase 2.5 (medium): minimal per-action execroot for sandbox-free input isolation

**Status: PARTIAL (2026-05-07)** — directory-only execroot landed; covers
runfiles trees with no top-level dir collisions. Per-action input
narrowing (next step) needed to cover trees with subdirs that match
workspace top-level names (e.g. `ci/`).

**Landed:**
- `app/kuro_core/src/cells.rs::ensure_execroot_layout` builds
  `<project_root>/execroot/<basename>/` as a real directory with
  directory-only symlinks of each top-level workspace dir. Replaces
  the prior `<basename> -> ..` self-symlink behaviour.
- `app/kuro_execute_impl/src/executors/local.rs::exec` routes action
  cwd through that execroot.
- Tests: `kuro_execute_impl test_exec_cmd_environment` updated to
  observe the new `cwd = execroot` semantics.

**Verified:**
- `examples/multi_package :gen_version_header` — builds clean.
- `crates__typenum-1.19.0//:_bs` cold build — succeeds; no
  `drain_runfiles_dir` panic (typenum runfiles tree has no `ci/`,
  `docs/`, etc. that would collide with zeromatter's first-party
  workspace dirs).
- `kuro_execute_impl --lib` — 44/44 tests pass.

**Outstanding:**
- `crates__zerocopy-0.8.42//:_bs` and similar crates whose runfiles
  tree contains a directory named like a zeromatter top-level dir
  (`ci/`, plausibly `docs/`, `examples/`) still fail. Failure mode:
  `create_runfiles_dir` creates `manifest_dir/ci/<runfiles>` as a
  real subdirectory; `should_symlink_exec_root` then iterates
  exec_root, finds our `ci` symlink, calls
  `symlink_if_not_exists(<exec_root>/ci, manifest_dir/ci)` — swallows
  AlreadyExists, pushes `manifest_dir/ci` into `exec_root_links`,
  and the cleanup loop's `remove_symlink(manifest_dir/ci)` panics
  with `IsADirectory` since the path is a real dir, not a symlink.
- This is the same upstream rules_rust runner shape (push-to-cleanup
  even when symlink wasn't created) re-manifesting at directory
  granularity. Phase 2.5's directory-only filter only protected
  against leaf-file collisions.

**Next step (Phase 2.6 or revisit 2.5):** narrow exec_root contents
per-action so only the action's declared inputs appear at top level.
For cargo_build_script actions that's just `external/` and
`buck-out/`, no first-party workspace dirs. Sketch:

1. In `kuro_execute_impl`, pass the action's input set through to
   the `exec` call (already partially threaded via
   `request.working_directory()` etc.).
2. Build a per-action exec_root at
   `<buck-out>/v2/execroot/<action_digest>/` containing symlinks
   only for the inputs (or their top-level prefix). Reuse across
   actions with identical input-set hashes.
3. Continue to set cwd to that path. Outputs flow through the
   `buck-out/` symlink as before.

Alternative: patch rules_rust upstream so `exec_root_links` only
pushes paths where `symlink_if_not_exists` actually created the link
(returns a bool). One-line fix in
`bazel-external/rules_rust+0.69.0/cargo/private/cargo_build_script_runner/bin.rs:69-84`,
but the patch needs to survive bzlmod cache invalidation — likely
needs a kuro-side patch hook (see `kuro_external_cells_bundled`
build script).



Discovered while extending Plan 22.4 (cell-aware build-setting CLI flags)
into zeromatter `//sdk:sdk_contents`. The cell-flag fix lands
`RULES_RUST_SYMLINK_EXEC_ROOT=1` in `cargo_build_script_run` action
envs as intended. With the flag set, `cargo_build_script_runner/bin.rs`
does:

```rust
let exec_root = env::current_dir().unwrap();
for path in read_dir(&exec_root)? {
    let link = manifest_dir.join(file_name);
    symlink_if_not_exists(&path, &link)?;   // swallows AlreadyExists
    exec_root_links.push(link)              // pushed UNCONDITIONALLY
}
// build script runs ...
for link in exec_root_links { remove_symlink(&link)?; }
cargo_manifest_maker.drain_runfiles_dir(...)?;  // expects symlinks intact
```

In Bazel, `exec_root` is a per-build staging directory whose top-level
entries are the action's declared inputs (`external/`, `bazel-out/`,
plus the workspace's own dirs as a symlink subtree). In kuro,
`exec_root = cwd = $(kuro root --kind project)` — the user's source
tree. ZeroMatter's project root has `Cargo.toml`, `Cargo.lock`,
`CHANGELOG.md`, `README.md` at top level. Those collide name-wise with
crate runfiles (`create_runfiles_dir` had already symlinked
`manifest_dir/CHANGELOG.md → external/<crate>/CHANGELOG.md`). The
runner's exec_root_links cleanup then removes the runfiles symlink it
didn't create, and `drain_runfiles_dir` panics with
`Failed to delete symlink … CHANGELOG.md … NotFound`. Confirmed on
disk: every crate runfiles dir has exactly the entries that *don't*
collide with zeromatter top-level files.

This is not a rules_rust bug — Bazel doesn't hit it because the
exec_root layout invariant holds. It's the kuro execution-environment
gap. Phase 3 is the full fix; this phase carves out the smallest
slice that unblocks the rules_rust pattern (and any similar
read-dir-of-exec-root behaviour) without requiring sandboxing,
per-build staging, or broad path-shape changes.

**Scope**: per-action exec_root containing *only* what the action
declares.

**Target architecture**:
1. For each action, allocate `<buck-out>/v2/execroot/<action_digest>/`.
2. Materialize **only**:
   - Top-level symlink `external/` →
     `<workspace>/external/` (kuro's apparent-name alias dir; Phase 3
     replaces this with a real subtree).
   - Top-level symlink `buck-out/` → `<workspace>/buck-out/`. Outputs
     declared under buck-out flow through this symlink, so no
     post-action copy-back needed.
   - The action's declared first-party source inputs: `lib/` /
     `sdk/` etc. as **directory symlinks** to the workspace
     equivalents, **not** file-by-file.
3. Run the action with `cwd = <execroot>`. Existing
   workspace-relative paths in the action's command line resolve
   through the directory symlinks.
4. Garbage-collect `<execroot>/<action_digest>/` after the action
   completes (or leave under buck-out lifecycle — needs a small
   decision).

**Why this works for the rules_rust case**: top-level
`read_dir(exec_root)` returns `["external", "buck-out", "lib",
"sdk", …]` — all directories. None collide with leaf-file runfiles
(`CHANGELOG.md`, `Cargo.toml`, `README.md`). The runner's
`symlink_if_not_exists`/cleanup loop creates `manifest_dir/external
→ exec_root/external` and friends; those don't shadow runfiles
entries.

**Why this is "without full-on sandboxing"**: no input
hermeticity, no per-action resource limits, no cgroup work.
Symlinks point straight at the workspace, so an action that
strays outside its declared inputs still sees them — same as
today. The only invariant we add is "top-level shape matches
Bazel's exec_root", which is what `read_dir(exec_root)` callers
actually rely on.

**Touch points**:
- `app/kuro_execute_impl/` (and the local-execution path) —
  build the per-action execroot before launching, set `cwd` on the
  spawn.
- `app/kuro_action_impl/` — thread the action's declared input set
  through to the staging step. Today most actions use the workspace
  directly; the staging set is "all top-level entries of the
  workspace minus collidables" as a v0, tightened in v1.
- `set_dynamic_project_root` (per memory `execroot_self_symlink.md`)
  — already installs `execroot/<basename>` so rules_rust
  process_wrapper's `${exec_root}/buck-out/...` resolves.
  Coordinate with the new execroot.
- `tests/core/` — any test that asserts on action `cwd` or
  workspace-relative paths needs review. Cargo build script tests
  in particular.

**Risks & mitigations**:
- *Performance*: per-action staging directory creation is
  non-trivial at scale. v0 uses one symlink per workspace
  top-level entry — bounded fanout, ~1ms per action. Cache &
  reuse across actions in the same build (group by input set).
- *Path leakage*: an action that reads
  `<workspace>/some-untracked-file` via a directory symlink still
  succeeds; we accept this. Hermeticity is Phase 3 + sandbox
  territory.
- *Output collision*: `<execroot>/buck-out/` symlink means
  outputs land at the workspace's buck-out. No copy-back needed,
  but two parallel actions writing the same buck-out path
  (action graph bug) become harder to attribute. This is the
  same risk as today.
- *Tools at absolute paths* (`/usr/bin/gcc` etc.) — unaffected
  (action env already passes them as absolute).

**Verification**:
- ZeroMatter `kuro build //sdk:sdk_contents` advances past the
  `crates__typenum-1.19.0:_bs` `drain_runfiles_dir` panic.
  `RULES_RUST_SYMLINK_EXEC_ROOT=1` still flows; runfiles dir on
  disk contains all 11+ entries (CHANGELOG.md, Cargo.lock,
  Cargo.toml, README.md, build.rs, LICENSE*, src/, tests/,
  cargo_toml_env_vars.env), not just the 7 non-colliding ones.
- `cargo test -p kuro_action_impl -p kuro_execute_impl --lib`
  passes.
- `examples/multi_package :gen_version_header` still builds.
- `tests/core/...` regression sweep — review failures, expect
  some path-shape tests to need updates.

**Effort**: 3-5 days. The action-execution layer is well-isolated
in `kuro_execute_impl`; staging logic is mechanical. Test
fallout is the unknown.

**Relationship to Phase 3**: Phase 2.5 provides "exec_root has
the right top-level shape, no sandboxing". Phase 3 turns each
top-level entry into a real subtree (`external/<canonical>` as
real dir, not a symlink to workspace) and makes the execroot the
authoritative filesystem view. Phase 3 builds on 2.5 by tightening
the input set; 2.5 doesn't change paths user-facing.

---

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
