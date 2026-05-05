# Plan 38: Spoke registration without lockfile dependency

## Status: COMPLETE (2026-05-05)

Landed in commit `Plans 37+38: spoke registration without lockfile dependency`.

## Context

Plan 36 introduced lazy spoke materialization on `mctx.path(Label)`. After
landing, zeromatter's `//sdk:sdk_contents` build hung on
`Waiting on crates__rstar-0.12.2-zm//lib/wirebuf -- loading package file
tree`. Plan 37 fixed the immediate hang (`@@//pkg` routing). After that
fix, the build hard-erred with `unknown cell name: crates__strsim-0.11.1`,
even though the spoke directory was on disk in `bazel-external/`.

## Root cause

Spoke cells were only registered as a side-effect of
`extension_repo::get_file_ops_delegate` running its post-extension-eval
loop (`extension_repo.rs:488` pre-fix). That loop was gated on TWO
conditions: hub `.kuro_repo_complete` marker absent **and**
`setup.repo_spec_json` empty. On warm builds — where the hub had already
been materialized in a prior session — both gates short-circuited the
registration loop, leaving `crates__strsim-0.11.1` unknown to the cell
resolver.

The lockfile (`MODULE.bazel.lock`) already records every spoke under
`moduleExtensions[<id>].general.generatedRepoSpecs` (1246 entries for
zeromatter's `crates` extension), but no startup pass was reading them into
the cell resolver.

## Fix

Three converging changes so spokes are always registered before BUILD
file evaluation needs them:

1. **Lockfile pre-seed at startup**
   - New `pre_compute_extension_repo_cells_from_lockfile` in
     `pending_repo_cells.rs`: walks every extension's
     `generatedRepoSpecs`, synthesises a `PendingRepoCell` per repo with
     `repo_spec_json` populated from the lockfile entry.
   - `cells.rs::parse_with_config_args` calls it after the existing
     `pre_compute_extension_repo_cells` pass; entries already in the
     use_repo()-driven set are skipped via `existing_canonicals`.
   - Each seeded extension is marked via
     `kuro_bzlmod::mark_extension_spokes_seeded(ext_id)`.

2. **Runtime fallback for the no-lockfile case**
   - New `extension_repo::ensure_extension_spokes_registered` runs at
     the top of `get_file_ops_delegate`, before any marker check.
   - Bails immediately when the extension is already seeded.
   - Otherwise calls `ctx.compute(&ext_key)` and registers all sibling
     spokes via `register_dynamic_extension_cell_with_setup` and
     `kuro_bzlmod::register_spoke`.
   - DICE compute is cheap when the lockfile is present (the
     extension-execution-dice cache lookup short-circuits on a
     digest-matched lockfile entry); when the lockfile is absent, the
     extension actually runs once, after which DICE caches the result.
   - Old inline registration loop in the marker-gated block deleted.

3. **Single lockfile parse**
   - New `kuro_bzlmod::cached_lockfile(workspace_root)` accessor backed
     by a process-wide `Mutex<HashMap<PathBuf, Option<Arc<Lockfile>>>>`.
     Negative results (file absent / unparseable) are cached too.
   - `cells.rs` (startup seed) and `extension_execution_dice.rs` (cache
     lookup at line 414) both go through it. The zeromatter lockfile (~160
     KB) parses once per daemon instead of twice.
   - `update_lockfile_extension_cache` calls
     `invalidate_cached_lockfile` after writing so the next read sees
     the new contents.

### Plus collateral fixes uncovered by re-materialising stuck stubs

4. **Stale-stub recovery** in `extension_repo.rs`: detect
   `.kuro_repo_complete` content of `"stub"` and re-materialise when a
   valid `repo_spec_json` is now available (the prior failure was a
   side-effect of the bug being fixed in this plan).
5. **`ctx.patch(file, strip)` accepts `strip` positionally** — Bazel
   signature allows either positional or keyword.
6. **`ctx.patch` resolves Label / string-form labels** (e.g.
   `"@@//:foo.patch"`) via `resolve_label_to_path`, anchoring root-cell-
   relative results at the project root so `patch(1)` can open them.

## Behaviour matrix

| Scenario | Path |
|---|---|
| Lockfile present, warm marker | Startup seed registers spokes; runtime helper finds extension already seeded, no-ops. |
| Lockfile present, cold marker | Startup seed registers spokes; existing cached-spec materialization path materializes the repo. |
| Lockfile absent, cold | Runtime helper runs `ctx.compute(&ext_key)` for hub's first access; extension actually evaluates; spokes registered; subsequent accesses short-circuit. |
| Lockfile absent, warm marker (the original bug shape) | Runtime helper runs DICE compute regardless of marker; registration succeeds. |
| `use_repo_rule()` (single-repo extension) | Helper marks seeded immediately, no DICE compute. |

## Verification

- `cargo test -p kuro_bzlmod --lib` → 163/163
- `cargo test -p kuro_external_cells --lib` → 4/4
- `cargo test -p kuro_common --lib` → 83/83
- `cargo test -p kuro_interpreter_for_build --lib` → 50/50
- `examples/multi_package :gen_version_header` builds cleanly
- zeromatter `//sdk:sdk_contents` advances past the unknown-cell errors;
  next blocker is `crate_git_repository` for git-source crates (Plan
  39).

## Files

- `app/kuro_bzlmod/src/lib.rs` — exports
- `app/kuro_bzlmod/src/lockfile.rs` — `cached_lockfile`,
  `invalidate_cached_lockfile`
- `app/kuro_bzlmod/src/extension_execution_dice.rs` — share cache,
  invalidate after write
- `app/kuro_bzlmod/src/pending_repo_cells.rs` —
  `pre_compute_extension_repo_cells_from_lockfile`
- `app/kuro_bzlmod/src/spoke_materialization.rs` —
  `mark_extension_spokes_seeded` / `extension_spokes_seeded`
- `app/kuro_external_cells/src/extension_repo.rs` —
  `ensure_extension_spokes_registered`, stub detection
- `app/kuro_common/src/legacy_configs/cells.rs` — wire lockfile pre-seed
- `app/kuro_interpreter_for_build/src/repository_ctx.rs` — `ctx.patch`
  signature + label resolution
