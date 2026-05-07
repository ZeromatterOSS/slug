# Plan 44: Workspace layout parity with Bazel

> Parent: [2026-01-21-kuro-bazel-compatible-build-tool.md](../2026-01-21-kuro-bazel-compatible-build-tool.md)
>
> Sibling: [15-bazel-9-parity.md](./15-bazel-9-parity.md) Phase 3 (lockfile
> round-trip) — same theme, different scope.

## Status: PARTIAL

Phase 2.5's shared synthesized execroot stopgap has landed. Phase 2.6
(per-action execroot narrowing) and Phase 3 (real Bazel-shaped execroot
and external-repo layout) remain proposed.

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

## Action Layout Invariants

Plan 44 owns the action-cwd and workspace-layout contract. Other plans
may consume or enforce this contract, but should not redefine it.

### Current stopgap state (Phase 2.5)

- Source tree authority: the real checkout remains the authoritative
  source tree.
- Action cwd: local actions run under
  `<workspace>/execroot/<workspace_basename>/`.
- Workspace view: the execroot contains directory symlinks for selected
  top-level workspace directories plus `buck-out` / external access as
  required by declared inputs.
- Outputs: writes still flow through `buck-out` in the real workspace;
  there is no copy-back step.
- External repos: materialized extension/BCR repos still live under the
  current kuro external location until Phase 2 / Phase 3 moves them.
- Hermeticity: this is not a sandbox. It narrows `read_dir(cwd)` for
  tools like rules_rust's cargo runner, but symlink targets can still
  expose undeclared files.

### Target state (Phase 2.6 + Phase 3)

- Phase 2.6 narrows the execroot per action or per input-prefix digest.
  The cwd exposes only top-level prefixes derived from declared inputs
  and tools. This removes the frozen collision-name filter.
- Phase 3 moves toward Bazel's shape:
  `<output_base>/execroot/<workspace_name>` for action cwd and
  `<output_base>/external/<canonical>` for materialized repos.
- Plan 34 then enforces the declared-input contract through sandboxing.
  It must use this plan's cwd/input/output model as input rather than
  inventing a different action layout.
- Plan 45 may land before Phase 3. Its per-Args paramfile work should
  work against the current Phase 2.5 cwd and remain valid after Phase
  2.6/3 because the action sees the same declared input prefixes.

## Generated Output Hygiene

`bazel-bin`, `bazel-out`, `bazel-testlogs`, `bazel-*`, `execroot/`,
`external/`, and `bazel-external/` are generated workspace/output
artifacts. They must remain ignored and must not be committed. Phase 44
owns the layout policy for these names; a future CI hygiene check should
fail if any of them appear in `git status --short` outside a deliberately
tracked fixture.

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

### Phase 2.5 (medium): shared synthesized execroot — STOPGAP, FROZEN

**Status: DONE-AS-FAR-AS-IT-GOES (2026-05-07)** — directory-only
shared execroot landed plus a coarse name-based collision filter.
The filter is **frozen** — no new entries. Any future collision
("crate `foo` has a `bar/` subdir whose name matches a workspace
top-level dir") triggers Phase 2.6 (per-action narrowing) instead
of growing the list.

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
  `drain_runfiles_dir` panic.
- `kuro_core --lib execroot` — 4/4 unit tests pass
  (`execroot_path_returns_basename_subdir`,
  `execroot_path_returns_none_for_empty_basename`,
  `ensure_execroot_layout_creates_dir_only_symlinks`,
  `ensure_execroot_layout_replaces_legacy_self_symlink`).
- `kuro_execute_impl --lib` — 44/44 tests pass.

**Collision-name filter** (`is_likely_runfiles_collision` in
`app/kuro_core/src/cells.rs`): excludes `ci`, `docs`, `examples`,
`tests`, `src`, `benches`, `bench`, `doc`, `assets`, `data`,
`fixtures` from the layout. Workspace dirs by these names won't
appear at exec_root top level; first-party actions that reference
them via `cwd-relative/<name>/...` will break — accept and adjust
the list as collisions surface, since per-action narrowing (below)
is the proper fix and supersedes this.

**Why the filter is frozen**: each new collision (`scripts/` from
`crates__cookie-0.18.1` was the most recent) just adds a name that
*also* breaks first-party workspace paths by that name. The set of
"crate runfile subdir names that match arbitrary user workspace
dirs" is unbounded; growing the allowlist won't converge. Phase 2.6
removes the need for the list.

---

### Phase 2.6 (medium): per-action execroot narrowing — supersedes the allowlist

**Status: PROPOSED**

**Goal**: replace the shared allowlist-filtered execroot from Phase
2.5 with a per-action execroot that contains only the symlinks the
action's inputs actually require. Action-level isolation by input
declaration, no sandbox.

#### Why this is the right fix

Phase 2.5's collision-name filter is whack-a-mole because the
runner's invariant is "`read_dir(exec_root)` is the action's
declared inputs". Bazel achieves that via per-action sandbox
staging. Kuro's shared synthesized execroot mirrors the workspace's
directory tree, so any workspace top-level dir whose name happens
to match a *runfile subdir* of *any* action's tree is a latent
collision.

The proper fix scopes the workspace-mirror to one action's worth of
input prefixes. For `cargo_build_script` that's always just
`external/` (its source crate path) and `buck-out/` (its tools and
declared outputs). For first-party actions it's whichever workspace
directories they list as inputs.

Net: zero allowlist, zero collisions by construction.

#### Target architecture

1. For each action, compute the set of distinct top-level path
   components from its declared inputs (artifacts and tools). For
   most actions this is a small set (e.g. `{external, buck-out}`
   for cargo_build_script; `{lib, external, buck-out}` for a
   first-party rust_library that pulls a crate dep).

2. Hash the sorted prefix set → `<input_set_digest>`. Group actions
   by digest; share an execroot dir per digest, not per action.
   Cache lookup in-memory + on-disk under
   `<buck-out>/v2/execroot/<input_set_digest>/`.

3. Populate the execroot lazily on first action that needs it:
   one directory symlink per prefix → workspace counterpart. Total
   work bounded by ~|inputs|; typical action <10 entries.

4. Set the action's `cwd` to the per-digest execroot. Outputs still
   flow through `<execroot>/buck-out/` → `<workspace>/buck-out/`,
   no copy-back.

5. Garbage-collect `<buck-out>/v2/execroot/` on `kuro clean`. No
   per-action GC needed — execroots are content-addressed by input
   set and small enough to leak across builds.

#### Why "narrowing", not "sandbox"

- No input hermeticity: an action that reads
  `<workspace>/some-untracked-file` through `<execroot>/external/...`
  still succeeds via the symlink target. Same risk as today.
- No process isolation: `cwd` is the only thing changing.
- No copy-back of outputs: `buck-out/` symlinked through.
- No per-action resource limits, cgroups, namespaces.

The single invariant we're enforcing: `read_dir(cwd)` returns only
prefixes the action declared. That's what rules_rust's runner (and
any similar `read_dir(exec_root)` consumer) needs.

#### Touch points

- `app/kuro_execute/src/execute/request.rs` — `CommandExecutionRequest`
  already exposes `inputs()`, `outputs()`. Add a helper that
  extracts the sorted top-level prefix set from inputs+tools.
- `app/kuro_execute_impl/src/executors/local.rs::exec_once` — call
  the helper, hash the prefix set, build/reuse the execroot dir,
  pass its absolute path to `exec`.
- `app/kuro_execute_impl/src/executors/local.rs::exec` — accept the
  per-action execroot path (replaces the
  `kuro_core::cells::execroot_path` call from Phase 2.5).
- `app/kuro_core/src/cells.rs` — keep `execroot_path` as a fallback
  for actions where the request isn't available; remove
  `is_likely_runfiles_collision` once Phase 2.6 lands and verify
  the global `ensure_execroot_layout` is no longer the cwd source
  for any action path.
- `app/kuro_action_impl/` — confirm input-set extraction has
  everything it needs (action metadata blobs, scratch paths,
  incremental remote outputs).

#### Risks & mitigations

- **Per-action setup overhead**: small dir + ~5-10 symlinks. v0
  measurement target: <2ms per execroot creation, amortized via
  digest-keyed reuse. If hot-path becomes visible, batch via
  `BlockingExecutor` queue.
- **Tests asserting on `cwd`**: a few in `kuro_execute_impl` already
  observe `cwd = execroot/<basename>`. Update to observe per-digest
  path, or accept either shape.
- **Apparent-name aliases in the action's prefix set**: for
  `external/<apparent>`, the workspace target is the bzlmod
  apparent-name symlink already. That's transitive through one
  more symlink hop — fine for filesystem semantics, slight
  performance hit on path resolution.
- **Concurrent actions writing to the same execroot dir**: shared
  by digest, multiple actions may target the same dir
  simultaneously. Use atomic `mkdir` + `symlinkat` with
  `EEXIST`-tolerance, no per-action mutation after creation.

#### Verification

- ZeroMatter `kuro build //sdk:sdk_contents` advances past
  `crates__cookie-0.18.1//:_bs` (current Phase 2.5 + frozen-filter
  blocker) without any `is_likely_runfiles_collision` entry.
- Running with `is_likely_runfiles_collision` returning `false`
  unconditionally (no allowlist) builds zerocopy, typenum, cookie
  cleanly.
- `kuro_execute_impl --lib` passes; tests updated for per-digest
  cwd shape.
- `examples/multi_package :gen_version_header` still builds.
- Cold-build wall-clock for `//sdk:sdk_contents` within ~3× bazel
  baseline (bazel 9.3s → kuro <30s, ignoring extension
  materialization which is a separate plan).

#### Effort

3-5 days. Most of the plumbing already exists from Phase 2.5
(directory symlinks, cwd routing, execroot path helper). The new
work is the input-set extraction, digest keying, and removing the
allowlist.

#### Relationship to Phase 3

Phase 2.6 keeps the workspace as the authoritative source tree
(symlinks point into it). Phase 3 flips that: the execroot becomes
the source of truth, `external/` becomes a real subtree under
buck-out, and `<workspace>/external/` goes away. Phase 2.6 is a
strict subset of Phase 3's setup work; Phase 3 builds on it.

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
