# Plan 61: True DICE-Owned Bzlmod

> Parent: [Slug Bazel-Compatible Build Tool](../2026-01-21-slug-bazel-compatible-build-tool.md)
>
> Created: 2026-05-18
>
> Consolidated: 2026-05-22

## Status

Open. The persisted config-load path now gets its resolved module graph,
projection facts, repo mappings, extension aggregations, registrations, and cell
graph from the clean bzlmod graph producer. It is still not a replay-complete
DICE/Skyframe implementation.

Do not mark this plan complete because `//sdk:sdk_contents` passes, because the
current guardrail file passes, or because a warm daemon smoke reuses the
transitional bridge. Those are necessary evidence, not sufficient acceptance
criteria.

Current classification:

- Slug has DICE keys for selected bzlmod inputs and extension/repository
  execution surfaces.
- The production persisted config-load path and direct no-updater
  bootstrap/completion callers now consume the clean resolved-graph producer;
  `BzlmodProjectionBridgeDiceKey` and the standalone direct cell-graph parser
  are removed.
- Replay correctness still depends on non-DICE process state, fallback scanner
  bridges, direct polling, and transitional lockfile/materialization behavior in
  places where Bazel owns explicit Skyframe keys.

The plan can only be closed when bzlmod module-file parsing, module resolution,
repo mapping, extension aggregation, extension replay inputs, repository specs,
repository materialization manifests, and lockfile policy are represented as
DICE-owned values with explicit dependencies, invalidation, and guardrails.

## Bridge Burn-Down: Source Input Tracking

**Bridge surface**: `std::fs::read_to_string` + `parse_module_with_polled_includes` in `compute_bzlmod_resolved_module_graph` (cells.rs:4472-4487) reads registry/git/archive MODULE.bazel files during clean graph computation without DICE tracking.

**Intended owner**: `NonRootModuleFilesKey` — already exists, just not wired into the clean graph producer. Collects resolved module files as `Vec<NonRootModuleFileInput>` after `fetch_sources()`, computes them through DICE, and folds `NonRootModuleFilesValue.parsed_modules` into the output.

**Bridge surface reduced**: targeted `std::fs::read_to_string` for resolved MODULE.bazel files replaced by `dice_ctx.compute(&NonRootModuleFilesKey{...})`. Validated with `cargo test -p slug_common bzlmod` (14 passed), `cargo test -p slug_bzlmod` (380 passed), `cargo build -p slug`, and `git diff --check`. Commit: `dbfeb5e1`.

**Analysis**: `scan_bzlmod_apparent_alias_from_external_dir` is too deeply interwoven with `BZLMOD_APPARENT_ALIAS_CACHE` and pre-graph cell resolver for one-slice removal. Requires multi-slice plan.

**Bridge surface reduced**: `BzlmodProjectionData` has been deleted from the
public bzlmod API. The updater surface is now `SetBzlmodDiceInputs` with
`set_bzlmod_cell_graph_data_with_inputs(...)`, accepting the
`BzlmodCellGraphValue` directly plus the separately named module-version,
lockfile-input, repo-env, registration, extension-aggregation,
resolution-fact, and repo-mapping injections. Empty non-bzlmod baselines now
use `set_empty_bzlmod_dice_inputs_for_workspace(...)`, so compatibility callers
still install explicit empty DICE inputs without preserving a monolithic
projection wrapper. Validated with `cargo test -p slug_bzlmod --lib`,
`cargo test -p slug_common bzlmod`, `cargo test -p slug_external_cells
extension_repo`, and `cargo check -p slug_server`.

**Bridge surface reduced**: lockfile spoke preseed no longer computes a hidden
fallback `.bzl` transitive digest inside
`pre_compute_extension_repo_cells_from_lockfile(...)`. The helper now requires
callers to pass an explicit digest map and skips cache preseed for extensions
whose current digest is unavailable. Production still supplies that map from
the named `FallbackScannedExtensionBzlDigestKey` bridge in `slug_common`, so
the scanner has not been eliminated yet, but it is now localized at the
remaining bridge key instead of being an implicit fallback inside
`slug_bzlmod`. Validated with `cargo test -p slug_bzlmod lockfile_preseed`
and `cargo test -p slug_common bzlmod`.

**Bridge surface reduced**: production `slug_core` no longer enables
`bazel-external` directory-scan fallback through process-global bzlmod state.
`dynamic_bzlmod_directory_scan_allowed()` is now test-only; normal builds must
resolve bzlmod repos through the resolver/runtime snapshot or explicit dynamic
registrations. This preserves existing unit coverage for the legacy scanner
shape while making the scanner unreachable in non-test production binaries.
Validated with `cargo check -p slug_core` and focused `cargo test -p slug_core
dynamic_bzlmod -- --nocapture`.

**Bridge surface reduced**: `action_external_cell_name(...)` now derives the
external execution repo name from the stored `bazel-external/<repo>` cell path
before any compatibility fallback. In production, if the path does not carry a
canonical bzlmod repo name, it returns the input cell name rather than consulting
process-global dynamic/module alias maps. The legacy symlink/cache fallback is
kept test-only. Validated with `cargo check -p slug_core` and focused `cargo
test -p slug_core action_external_cell_name -- --nocapture`.

**Bridge surface reduced**: `CellAliasResolver::resolve(...)` no longer uses
process-global scoped aliases, dynamic extension aliases/cells, or
`bazel-external` directory probing on no-runtime-snapshot misses in production.
Those compatibility fallbacks are now test-only helpers; production alias
resolution must come from declared aliases, the resolver-owned runtime snapshot,
or known bundled cells. Validated with `cargo check -p slug_core`, focused
`slug_core` resolver tests, and `pytest -q
tests/core/bzlmod/test_plan61_guardrails.py`.

**Bridge surface reduced**: `CellResolver::get(...)` no-runtime-snapshot misses
no longer auto-create production cells from process-global dynamic extension
registries or `bazel-external` suffix scans. Resolver-local dynamic cells and
runtime snapshot cells remain valid; legacy root-scoped dynamic discovery is
kept only in tests. Validated with `cargo check -p slug_core`, focused
`slug_core` cell path/resolver tests, and `pytest -q
tests/core/bzlmod/test_plan61_guardrails.py`.

**Bridge surface reduced**: load-path wrong-cell equivalence no longer accepts
process-global dynamic extension aliases on no-runtime-snapshot production
misses. `are_bzlmod_alias_equivalent(...)` now relies on literal equality,
Bazel canonical module suffix equivalence, declared/runtime resolver aliases,
and resolver-owned internal repo-name equivalence in production; the old
process-global alias equivalence is test-only. Validated with `cargo check -p
slug_interpreter_for_build`, focused `slug_interpreter_for_build` load-path
tests, and `pytest -q tests/core/bzlmod/test_plan61_guardrails.py`.

**Bridge surface reduced**: C++ toolchain metadata label canonicalization no
longer compiles process-global bzlmod alias/module canonicalization into
production metadata paths. `MetadataLabelContext` now exposes those compatibility
fallbacks only through `#[cfg(test)]` helpers; production metadata resolution
uses the active cell resolver or preserves the label's stored cell spelling.
Validated with `cargo check -p slug_analysis`, focused `slug_analysis` metadata
tests, and `pytest -q tests/core/bzlmod/test_plan61_guardrails.py`.

**Bridge surface reduced**: locked remote registry files now have real producers
instead of requiring a manually prewarmed legacy cache. `ModuleCache` owns the
`bazel_registry.json` cache path/read/write API, `RegistryClient` fetches the
top-level metadata with a lockfile-style registry hash identity, and
`RegistryFileInputsKey` hydrates missing supported non-file registry lockfile
entries (`bazel_registry.json`, `MODULE.bazel`, and `source.json`) before
enforcing checksum and metadata validation. `file:` registries and unsupported
paths remain strict tracked-file inputs. Bazel source reference:
`IndexRegistry.getBazelRegistryJson(... useChecksum = true)` fetches
`bazel_registry.json`, `ModuleFileFunction` carries registry file hashes for
fetched module files, and `RegistryFunction` constructs registries from lockfile
`registryFileHashes`. Validated with `cargo check -p slug_common`, `cargo test
-p slug_common registry_file_inputs`, and `pytest -q
tests/core/bzlmod/test_plan61_guardrails.py`.

**Bridge surface reduced**: extension-generated repositories now resolve their
owning module's self alias from the runtime snapshot path instead of depending
on process-global scoped alias fallback. For a generated repo such as
`rules_cc++compatibility_proxy+cc_compatibility_proxy`, the alias resolver maps
`@rules_cc` to the canonical owner repo `rules_cc+`, matching Bazel lockfile
recorded inputs like
`REPO_MAPPING:rules_cc++compatibility_proxy+cc_compatibility_proxy,rules_cc rules_cc+`.
Validated with focused `cargo test -p slug_core bzlmod_runtime_snapshot`.

**Bridge surface reduced**: `module_ctx.path(Label(...))` label-path resolution
now enumerates resolver-owned runtime extension cells registered after the
initial graph snapshot. The path map used by extension execution includes
graph-owned dynamic cells from the active `CellResolver`, so labels such as
`@@cargo_linux_x86_64_1_95_0//:bin/cargo` can resolve without falling back to
process-global dynamic cell maps. Validated with focused `cargo test -p
slug_core bzlmod_label_cell_paths`.

**Bridge surface reduced**: module extension execution now builds its
`module_ctx.path()` label map with the extension owner module's scoped aliases
from the active `CellResolver`. Non-root `use_repo_rule()` repos such as
`toml2json_linux_amd64`, declared by `rules_rs`, are visible to
`@rules_rs//rs:extensions.bzl%crate` without a process-global apparent-name
lookup. Validated with focused `cargo test -p slug_core
bzlmod_label_cell_paths` and `cargo check -p slug_interpreter_for_build`.

**Bridge surface reduced**: Starlark repository rule execution now builds
`repository_ctx.path(Label(...))` cell paths from the active resolver plus the
owning module's scoped aliases. Repository rules such as
`@rules_rs`-owned `toml2json_*` repos can resolve Label paths without consulting
process-global apparent-name maps. Validated with focused
`slug_interpreter_for_build` checks and SDK smoke frontier movement past
`@@toml2json_linux_amd64`.

**Bridge surface reduced**: runtime alias resolution now derives owner self
aliases, apparent/internal generated repo names, same-extension sibling repo
aliases, and bundled-tool root repo aliases from resolver-owned graph data. The
replacement covers:
`rules_cc++compatibility_proxy+cc_compatibility_proxy` loading `@rules_cc`;
`crates__clap-4.5.60` loading `@rules_rs`;
canonical module cells like `rules_license+` loading `@rules_license`;
`rules_rs++crate+crates__github...` loading sibling
`@crates__ts-rs-12.0.1`; target labels such as `zstd//:zstd` resolving to
`zstd+`; and `bazel_tools` loading `@rules_cc`. These are all backed by
`BzlmodRuntimeCellInstallSnapshot`, graph-owned dynamic cells registered on the
active `CellResolver`, or root aliases explicitly exposed only to bundled tool
cells. Focused coverage includes `cargo test -p slug_core owner_self_alias`,
`cargo test -p slug_core same_extension_internal_sibling_alias`,
`cargo test -p slug_core apparent_module_name_to_canonical_module_cell`,
`cargo test -p slug_common bzlmod_non_root_alias_resolver_preserves_runtime_snapshot`,
and `cargo test -p slug_common
bzlmod_bundled_tool_alias_resolver_can_see_root_repo_aliases`.

**Bridge surface reduced**: configured source-file attrs and native
filegroup/genrule source-file collection now canonicalize apparent bzlmod
repository package cells through the active `CellAliasResolver` runtime
snapshot before constructing `SourcePath`s. This removes a source-artifact
ownership leak where analysis/runfiles paths could preserve apparent generated
repo names such as `linux_kernel_headers_x86.4.19.325` or
`crates__zerocopy-0.8.42` even though the resolver-owned graph knew their
canonical cells. The helper has no process-global fallback; without a runtime
snapshot it preserves the stored apparent spelling. Intended owner:
`BzlmodCellGraphKey` plus `RepoMappingKey`; current producer remains the
transitional resolver-owned runtime snapshot until the cell graph is fully
DICE-derived. Validation passed with focused `cargo test -p slug_analysis
source_file_package`, `cargo check -p slug_analysis`, `cargo build -p slug`,
and `/var/mnt/dev/zeromatter-kuro //sdk:sdk_contents` reaching `BUILD
SUCCEEDED` in `/tmp/slug-plan61-sdk-smoke-20260528T0014-native-source-path-long.log`.

**Bridge surface reduced**: `BzlmodCellGraphKey` no longer returns the injected
cell graph as a standalone semantic cache hit. The key now also depends on the
named lockfile-input, repo-env, repo-mapping, resolution-fact, and
extension-aggregation DICE keys for the same workspace before exposing the graph
to repository label resolution and resolver consumers. This does not remove the
remaining injected `BzlmodCellGraphDataKey` producer, but it makes the
transitional cell graph cache hit depend on the same sibling facts that feed the
clean graph producer instead of trusting the graph-shaped payload alone.
Intended owner remains `BzlmodCellGraphKey` derived from `BzlmodWorkspaceKey`,
`BzlmodResolutionKey`, `RepoMappingKey`, extension aggregation/replay keys, and
lockfile policy keys. Validation passed with focused cell-graph tests and
`cargo test -p slug_bzlmod`.

## Current Checkpoint

Historical slice logs and detailed validation transcripts now live in
[61-true-dice-bzlmod-history.md](./61-true-dice-bzlmod-history.md). Future
workers should read this main plan first and open the history file only when
they need exact older evidence, command transcripts, or provenance for a prior
bridge-burn-down slice.

Current state to preserve:

- Plan 61 is open. The persisted config-load path now uses the clean bzlmod
  graph producer for resolved graph data and `BzlmodCellGraphValue`. Direct
  no-updater bootstrap/completion callers build the same clean graph through a
  temporary DICE instance before parsing cells. Fallback scanners, lockfile
  policy, and materialization polling remain.
- SDK frontier evidence is positive but not a closure condition: Slug and Bazel
  9.0.1 have both built `/var/mnt/dev/zeromatter-kuro //sdk:sdk_contents`, with
  matching modes and non-ELF hashes. The accepted remaining differences are ELF
  output-root strings in `bin/zm`, `bin/zerobuf`, `bin/zerosystem`, and
  `lib/libzeromatter_ffi.so`.
- The last recorded full Plan 61 Python guardrail in the archive passed after
  rebuilding `target/debug/slug`, but future workers must rerun the focused
  owner tests for their slice rather than relying on that snapshot.
- Recent cleanup clarified that fallback `.bzl` scanners are explicitly named
  fallback paths. Normal extension replay must use
  `ExtensionBzlTransitiveDigestKey` over the loaded Starlark graph; bootstrap
  and lockfile preseed paths still have a legacy fallback-scanner bridge in
  `FallbackScannedExtensionBzlDigestKey`. Lockfile preseed no longer computes a
  secondary implicit fallback when the bridge digest map is absent.
- 2026-05-28 probe: simply passing an empty digest map to lockfile spoke
  preseed is not viable. `test_valid_lockfile_replay_materializes_generated_repo_without_extension_eval`
  still passed, but `test_lockfile_replay_recorded_file_input_edit_rejects_cache`
  and `test_lockfile_replay_recorded_repo_mapping_change_rejects_cache` stopped
  observing `extension_replay_hit` during `buck audit cell`. The replacement
  must preserve recorded-input and repo-mapping lockfile replay, not just
  generated-repo materialization. A direct
  `ExtensionBzlTransitiveDigestKey` call cannot be used at this point because
  preseed runs while the bzlmod cell graph is being constructed, before
  `CellResolverKey` is injected; the loaded-module digest path currently calls
  `ctx.get_cell_resolver()`.
- 2026-05-28 preseed digest reduction: `FallbackScannedExtensionBzlDigestKey`
  no longer calls the direct `std::fs` fallback scanner for file contents. It
  resolves literal project-local `.bzl` loads without probing the filesystem,
  reads them through `DiceFileComputations::read_project_file_if_exists`, and
  registers those project paths for watcher invalidation. Present-file digests
  still match the old lockfile digest shape; missing literal loads use a
  deterministic sentinel so create/delete transitions can be tracked by DICE.
  Validated with `cargo test -p slug_common fallback_scanned_extension_bzl_digest --lib`,
  `cargo build -p slug`, and the focused Python replay set:
  `test_lockfile_replay_recorded_file_input_edit_rejects_cache`,
  `test_lockfile_replay_recorded_repo_mapping_change_rejects_cache`,
  `test_lockfile_replay_recorded_repo_mapping_from_extension_repo_source`, and
  `test_default_lockfile_mode_rejects_invalid_extension_digest`. A broader
  `cargo test -p slug_common bzlmod --lib` still fails the unrelated
  `bzlmod_cell_resolver_uses_canonical_module_cells_from_cell_graph` assertion
  that `CellResolver::get("dep")` should error.
- 2026-05-28 preseed replay validation reduction: persisted config-load
  preseed now selects lockfile extension caches with
  `select_extension_cache_for_workspace(...)`, validates recorded inputs
  through `selected_cache_recorded_inputs_current(...)` /
  `ModuleExtensionRecordedInputsKey`, records replay hits only after that DICE
  child key succeeds, and passes prevalidated repo specs to
  `pre_compute_extension_repo_cells_from_lockfile_with_prevalidated_caches(...)`.
  Bootstrap callers without a `DiceComputations` handle stay on the legacy
  synchronous validation path. This makes `FallbackScannedExtensionBzlDigestKey`
  transaction-valid (`validity = x.is_ok()`) while preserving recorded-input
  and repo-mapping lockfile replay invalidation. Validated with
  `cargo test -p slug_bzlmod lockfile_preseed --lib`,
  `cargo test -p slug_common fallback_scanned_extension_bzl_digest --lib`,
  `cargo build -p slug`, `git diff --check`, and the focused Python replay set:
  `test_lockfile_replay_recorded_file_input_edit_rejects_cache`,
  `test_lockfile_replay_recorded_repo_mapping_change_rejects_cache`,
  `test_lockfile_replay_recorded_repo_mapping_from_extension_repo_source`, and
  `test_default_lockfile_mode_rejects_invalid_extension_digest`.
- 2026-05-28 workspace recorded-file replay reduction: persisted config-load
  preseed now validates workspace-relative text `FILE`, `DIRENTS`, and
  `DIRTREE` recorded inputs through a named `PreseedRecordedInputsKey` in
  `slug_common`. The key reads project path metadata, file contents, sorted
  project directory entry names, and recursive text-file tree state through
  `DiceFileComputations`, registers recorded project paths for pre-config
  watcher invalidation, and stays non-persistent so same-daemon lockfile replay
  rechecks recorded inputs even when the surrounding cell graph is otherwise
  clean. `ENV` and `REPO_MAPPING` recorded inputs are checked in the same key
  without filesystem polling. Unsupported recorded input shapes still fall back
  to the `ModuleExtensionRecordedInputsKey` / synchronous validator path:
  binary or symlink file reads and external-repo `FILE` paths.
  Validated with `cargo test -p slug_common preseed_recorded_inputs_track_workspace_ --lib`,
  `cargo test -p slug_bzlmod lockfile_preseed --lib`,
  `cargo test -p slug_common fallback_scanned_extension_bzl_digest --lib`,
  `cargo build -p slug`, `git diff --check`, and the focused Python replay
  set covering recorded file edits, repo-mapping changes, extension-repo-source
  mappings, and invalid extension digests.
- 2026-05-28 cell-graph identity reduction: `BzlmodCellGraphDataKey` now
  carries a `BzlmodCellGraphDataValue` with the clean resolved-graph digest
  beside the graph payload. Persisted config-load installs the digest from
  `BzlmodResolvedModuleGraphValue::graph_digest`, `BzlmodCellGraphKey` rejects
  stale injected-projection keys whose digest does not match the active clean
  graph, and repository label resolution now asks for the active workspace graph
  through the digest-addressed helper. This does not finish the cell-graph
  producer move: the graph payload is still injected after the clean producer
  runs in `slug_common`. It removes the anonymous injected-projection identity
  from real persisted bzlmod graph loads and leaves the remaining bridge as the
  producer boundary itself. Validated with
  `cargo test -p slug_bzlmod bzlmod_cell_graph --lib`,
  `cargo test -p slug_bzlmod repository_label_resolution_key_projects_cell_graph_paths --lib`,
  and
  `cargo test -p slug_common persisted_cell_graph_injects_clean_root_module_version_data --lib`;
  the standard preseed/replay guardrails, `cargo build -p slug`,
  `git diff --check`, and the focused Python replay set also passed after this
  change.
- 2026-05-28 cell-graph root-name reduction: `BzlmodCellGraphKey` now derives
  the returned graph's `root_module_name` from `ModuleVersionsKey` instead of
  trusting the injected graph payload copy. This is intentionally a narrow
  payload-reduction step: `BzlmodCellGraphDataKey` still carries the cell,
  extension-cell, alias, symlink, scoped-alias, and dynamic-alias vectors, but
  the root module identity is owned by the sibling DICE module-version
  projection. Guardrail:
  `cargo test -p slug_bzlmod cell_graph_key_uses_module_data_root_name --lib`.
  Also validated with the focused bzlmod cell-graph and persisted clean-graph
  tests, `cargo build -p slug`, `git diff --check`, and the focused Python
  replay set.
- 2026-05-28 cell-graph scoped-alias reduction: `BzlmodCellGraphKey` now
  derives returned `scoped_aliases` from `BzlmodRepoMappingsKey` and root
  `repo_mapping_overrides` instead of trusting the injected graph payload copy.
  `BzlmodCellGraphDataKey` still carries the cell, extension-cell, root-alias,
  symlink, and dynamic-alias vectors. Guardrail:
  `cargo test -p slug_bzlmod cell_graph_key_uses_repo_mapping_scoped_aliases --lib`.
  Also validated with the focused bzlmod cell-graph, cell-graph-key,
  repository-label-resolution, and persisted clean-graph tests,
  `cargo build -p slug`, `git diff --check`, and the focused Python replay set.
- 2026-05-28 cell-graph root-alias reduction: `BzlmodCellGraphKey` now derives
  returned `root_aliases` from the root repo mapping in
  `BzlmodRepoMappingsKey` instead of trusting the injected graph payload copy.
  `BzlmodCellGraphDataKey` still carries the cell, extension-cell, symlink, and
  dynamic-alias vectors. Guardrail:
  `cargo test -p slug_bzlmod cell_graph_key_uses_root_repo_mapping_aliases --lib`.
  Also validated with the focused bzlmod cell-graph, cell-graph-key,
  repository-label-resolution, and persisted clean-graph tests,
  `cargo build -p slug`, `git diff --check`, and the focused Python replay set.
- 2026-05-28 cell-graph dynamic-alias reduction: `BzlmodCellGraphKey` now
  derives returned `dynamic_aliases` from canonical-looking replacement rows in
  the root repo mapping instead of trusting the injected graph payload copy.
  `BzlmodCellGraphDataKey` still carries the cell, extension-cell, and symlink
  vectors. Guardrail:
  `cargo test -p slug_bzlmod cell_graph_key_uses_root_repo_mapping_dynamic_aliases --lib`.
  Also validated with the focused bzlmod cell-graph, cell-graph-key,
  repository-label-resolution, and persisted clean-graph tests,
  `cargo build -p slug`, `git diff --check`, and the focused Python replay set.
- 2026-05-28 cell-graph module-symlink reduction: `BzlmodCellGraphKey` now
  derives module symlinks from `BzlmodCellGraphCell::module_setup.source_path`
  when that data exists, ignoring stale payload symlinks for those cells. It
  still appends non-derivable payload symlinks, currently needed for
  out-of-project local overrides whose cell setup is intentionally absent.
  `BzlmodCellGraphDataKey` still carries the cell, extension-cell, and
  local-override symlink vectors. Guardrail:
  `cargo test -p slug_bzlmod cell_graph_key_derives_module_symlinks_from_cell_setup --lib`.
  Also validated with the focused bzlmod cell-graph, cell-graph-key,
  repository-label-resolution, and persisted clean-graph tests,
  `cargo build -p slug`, `git diff --check`, and the focused Python replay set.
- `slug_core` process-global dynamic bzlmod directory scanning is now test-only;
  production binaries must use resolver/runtime graph data or explicit dynamic
  registrations instead of scanning `bazel-external` for aliases.
- Action source path external repo names now use the resolver-owned stored cell
  path first and do not consult process-global bzlmod alias maps in production.
- Production `CellAliasResolver::resolve` no-runtime-snapshot misses now ignore
  process-global bzlmod alias/cell maps and directory probing; those fallbacks
  remain only for test compatibility.
- Production `CellResolver::get` no-runtime-snapshot misses now ignore
  process-global dynamic extension registries and `bazel-external` suffix
  scans; root-scoped dynamic cells remain a test-only compatibility path.
- Production load-path wrong-cell equivalence now ignores process-global
  dynamic extension aliases on no-runtime-snapshot misses; resolver-owned
  declared/runtime aliases and structural canonical/internal-name equivalence
  remain.
- Production metadata label canonicalization no longer compiles process-global
  bzlmod alias/module fallback calls; the remaining compatibility behavior is
  test-only.
- Locked remote registry files no longer require a manually prewarmed cache:
  `RegistryFileInputsKey` can fetch missing supported non-file registry entries
  through `RegistryClient`, then still validates the exact lockfile hash and
  metadata/source shape. `file:` registries and unsupported paths remain strict.
- Extension-generated repositories resolve their owning module self-alias
  structurally from the runtime snapshot path, so generated repo loads like
  `@rules_cc` from `rules_cc++compatibility_proxy+cc_compatibility_proxy` no
  longer need process-global scoped alias fallback.
- `module_ctx.path(Label(...))` now sees resolver-owned runtime extension cells
  registered after the initial graph snapshot via `bzlmod_label_cell_paths()`,
  keeping label path resolution on the active `CellResolver` instead of
  process-global dynamic cell maps.
- Module extension execution also uses the owner module's scoped aliases when
  constructing its `module_ctx.path()` label map, so non-root
  `use_repo_rule()` repos are visible to that module's extensions without
  global apparent-name lookup.
- Repository rule execution also uses owner-scoped resolver paths for
  `repository_ctx.path(Label(...))`; labels for non-root `use_repo_rule()` repos
  are no longer dependent on global apparent-name lookup.
- Runtime alias and cell lookup for generated repos, canonical module cells,
  same-extension sibling repos, bundled tool repos, and module-name target
  labels is now resolver-owned. The SDK smoke progressed past the prior
  `rules_cc`, `rules_rs`, `rules_license`, `crates__*`, `zstd`, and
  `bazel_tools` alias/cell failures.
- Repository materialization now has a named manifest key and child state for
  marker/layout/recorded-input checks, but those child reads still poll
  filesystem state until lower-level tracked filesystem keys are available.
- The current SDK frontier is no longer the legacy resolution bridge. The
  generated kernel-header source path mismatch and the later
  `crates__zerocopy-0.8.42` build-script `Cargo.toml` runfiles miss are both
  fixed by resolver-backed source-artifact package canonicalization. The latest
  Slug SDK smoke reached `BUILD SUCCEEDED`; this is frontier evidence, not a
  Plan 61 closure condition.

What future workers should keep in this file:

- Current status, non-negotiables, target DICE/Skyframe shape, remaining work,
  strong guardrails, and validation workflow.
- Compact bridge-burn-down notes that identify the removed surface and intended
  DICE/Skyframe owner.
- Links to detailed logs in the history file instead of long command-by-command
  transcripts.

## Non-Negotiables

- Bazel 9 parity only. No Bazel 8 compatibility, no WORKSPACE support, and no
  compatibility shims unless explicitly requested.
- Bazel failure means Slug failure of the same kind. Workarounds that make
  Slug pass when Bazel 9 fails are bugs.
- Every parity decision must cite Bazel source or observed Bazel 9.0.1
  behavior.
- DICE ownership means inputs are represented as DICE keys or tracked DICE file
  dependencies. It does not mean "a legacy value was injected into DICE after
  startup."
- Do not close this plan while any bzlmod correctness path still depends on
  process-global mutable state, untracked filesystem reads, best-effort replay
  digests, or bridge cache policy.

## Target DICE/Skyframe Shape

The final implementation should mirror Bazel's conceptual Skyframe ownership,
using Rust DICE keys and values:

- `BzlmodWorkspaceKey`
  - workspace root, output base, Bazel release identity, Starlark semantics,
    bzlmod flags, repo env, nonstrict repo env, registry config, repository
    cache config, network policy, yanked-version policy, compatibility policy,
    and extension isolation mode.
- `RootModuleFileKey` and `ModuleFileKey`
  - root and dependency `MODULE.bazel` parsing, including `include()` inputs,
    parse/eval errors, module directives, deps, overrides, extension usages,
    repo rule invocations, and toolchain/platform registrations.
- `ModuleSourceKey`
  - registry module file/source metadata, local path override modules,
    git/archive override source identity, checksums, patches, overlays, and
    source fetch failures.
- `BzlmodResolutionKey`
  - selected module graph, compatibility checks, yanked-version decisions,
    override semantics, selected versions, canonical module repositories, and
    registry file hashes.
- `RepoMappingKey`
  - apparent-to-canonical mapping per module, extension implementation,
    generated repo, and innate/use_repo_rule scope.
- `ModuleExtensionAggregationKey`
  - all extension usages grouped by Bazel's extension identity, including
    `isolate`, `dev_dependency`, root/non-root behavior, tag values, lexical
    module ownership, and override rows.
- `ModuleExtensionReplayInputKey`
  - lockfile entry, actual transitive `.bzl` load graph digest, usages digest,
    recorded inputs, repo env, repo mappings, facts, and command lockfile mode.
- `ModuleExtensionExecutionKey`
  - extension eval result: generated `RepoSpec`s, canonical names, facts, and
    replay outcome. Cache hits and misses must be explainable from the key's
    inputs.
- `RepoSpecKey`
  - selected generated or innate repository rule spec after replay or extension
    execution.
- `RepositoryExecutionKey`
  - repository rule execution, watched files/trees, repo env, network policy,
    output marker identity, and failure modes.
- `RepoMaterializationManifestKey`
  - materialized tree identity and external symlink layout. This must replace
    ad hoc marker trust as the semantic authority.
- `BzlmodCellGraphKey`
  - cell roots, external module symlinks, generated extension cells, apparent
    aliases, scoped aliases, bundled tool repos, and dynamic generated repos.
- `LockfileContentKey` and lockfile policy values
  - visible and hidden lockfile read/parse results, digest identity, error/off/
    update semantics, selected yanked versions, extension cache entries, facts,
    and write intent. Ordinary build/query paths stay read-only.

## Bridge Burn-Down Operating Rule

Plan 61 is large enough that useful cleanup can look like progress while leaving
the structural bridge intact. Each implementation slice should identify the
specific remaining bridge surface it reduces and the target DICE/Skyframe-shaped
value that should own the behavior instead.

Before editing, state which production surface will be removed, made test-only,
replaced with a named DICE key, or made impossible to reach. Examples include
legacy-produced cell graph injection, scanner fallback, process-global
alias/cell state, direct filesystem polling, or marker-trust materialization.
Pair that with the intended
owner from the target shape above, such as `ModuleSourceKey`,
`BzlmodResolutionKey`, `RepoMappingKey`, `ModuleExtensionReplayInputKey`,
`RepoSpecKey`, `RepositoryExecutionKey`, `RepoMaterializationManifestKey`,
`BzlmodCellGraphKey`, or lockfile policy keys.

Cleanups, directive-parity fixes, API hiding, and additional guardrails are valid
when they directly enable or protect one of those structural deletions. Their
plan note or commit message should say which fallback or bridge use-site is now
gone, test-only, or unreachable. If two consecutive verified slices do not
shrink a bridge surface, stop normal slicing and re-plan from `## Remaining Work`
before continuing.

Useful before/after checks include targeted searches such as:

```sh
rg -n "BzlmodProjectionBridgeDiceKey|BzlmodProjectionData|process-global|fallback scanner|std::fs|marker trust" \
  app/slug_common app/slug_bzlmod app/slug_core app/slug_external_cells app/slug_analysis
```

The search does not have to reach zero, and not every hit is a bug. It is a
forcing function to keep the plan reducing legacy ownership rather than only
hardening behavior around it.

## Remaining Work

1. Replace the legacy resolution bridge.
   - Production bridge replacement is done. `BzlmodProjectionBridgeDiceKey` has
     been replaced by true graph producers for the persisted config-load path.
     Direct no-updater bootstrap/completion callers now create a temporary DICE
     instance and compute the same clean graph before parsing cells; the
     internal cell parser errors for project parses that do not supply that
     clean graph. The standalone direct bzlmod cell-graph parser and its direct
     filesystem helper path were removed.
   - Build the resolved graph from DICE-owned module-file/source keys.
     Do not start by wrapping `BuckConfigBasedCells`' legacy projection
     resolver under a new key name; that preserves the architecture this item
     is meant to delete.
   - Ensure graph identity includes every command policy value that Bazel uses:
     lockfile mode, repo env, nonstrict repo env, registry config, network
     policy, yanked-version allow-list, compatibility policy, and extension
     isolation.
   - Migration should be by semantic output class, not by named dependency.
     A single `bazel_dep()` can affect MVS selection, lockfile facts, source
     paths, apparent repo mappings, extension aggregation, toolchain
     registration, and the final cell graph, so per-dependency production
     cutover is not a safe boundary. Use per-dependency fixtures only to prove
     coverage.
   - The clean producer sequence started as a shadow path and has now become
     production authority for persisted config-load injection. The initial
     viability slice was a
     `BzlmodResolvedModuleGraphKey`-style producer for the resolved graph plus
     module-version and resolution-fact outputs. It may reuse lower-level
     `slug_bzlmod` primitives such as `MvsResolver`, `ModuleCache`, parsed
     module values, and lockfile types, but it must not call the direct
     no-updater bootstrap/completion cell-graph parser.
     The follow-up slice promotes `BzlmodModuleVersionsDataValue` and
     `BzlmodResolutionFactsValue` injection to the clean resolved-graph key, so
     the legacy projection bridge no longer carries those output classes.
     Evidence: `cargo check -p slug_common`,
     `cargo test -p slug_common persisted_projection_injects_clean_root_module_version_data -- --nocapture`,
     and
     `cargo test -p slug_common clean_resolved_module_graph_produces_local_override_facts -- --nocapture`.
     The same producer now also owns registered toolchain and execution
     platform injection; the legacy projection bridge no longer carries those
     output classes.
     Extension aggregation injection is also derived from the clean producer's
     parsed module set; the legacy projection bridge no longer carries
     `BzlmodExtensionAggregationsDataValue`.
     Repo-mapping injection is now derived from the clean producer's parsed
     modules plus resolved graph identity; the legacy projection bridge no
     longer carries `BzlmodRepoMappingsDataValue`.
     Persisted config-load now injects `BzlmodCellGraphValue` from the clean
     resolved-graph key as well. `BzlmodProjectionBridgeDiceKey` and its
     bridge-specific extension replay summary digest were removed. Direct
     no-updater bootstrap/completion parsing is separate only in DICE lifetime:
     it uses a temporary DICE instance, not a separate resolver implementation.
     Evidence: `pytest -q tests/core/bzlmod/test_plan61_guardrails.py` (155
     passed), `cargo test -p slug_common` (127 passed plus doctests), `cargo
     test -p slug_bzlmod` (380 passed plus doctests), `cargo build -p slug`,
     `cargo fmt --check`, and `git diff --check`.
     Bootstrap/completion follow-up evidence:
     `cargo test -p slug_common clean_no_updater_bzlmod_cell_graph_preserves_explicit_output_base -- --nocapture`
     and
     `cargo test -p slug_common project_bzlmod_parse_requires_clean_graph_input -- --nocapture`.
     The previously failing bootstrap/error-context subset covering root,
     local override, git/archive override, include UTF-8/parse/cycle failures,
     and `--ignore_dev_dependency` early validation passed as 16 focused pytest
     cases before the full guardrail rerun.
   - Migrate output classes in this order:
     1. source/module-file input producers for root, registry, project-local
        and out-of-project local overrides, git/archive overrides, and patch
        files;
     2. resolved module graph, selected versions, source identities, registry
        hashes, and selected yanked-version facts;
     3. simple injected facts: `BzlmodResolutionFactsValue` and
        `BzlmodModuleVersionsDataValue`;
     4. `register_toolchains()` and `register_execution_platforms()` facts,
        including `dev_dependency` policy and the current bundled
        `rules_python` auto-injection behavior;
     5. repo mappings and apparent aliases from `module(repo_name=...)`,
        `bazel_dep(repo_name=...)`, transitive scoped aliases,
        `override_repo()`, and `inject_repo()`;
     6. extension aggregation, `use_extension()`, `use_repo()`,
        `use_repo_rule()`, and lockfile-seeded generated repo preseed facts;
     7. final `BzlmodCellGraphValue` authority, including module cells,
        extension cells, bundled cells, root aliases, scoped aliases, dynamic
        aliases, and external symlink layout. The persisted config-load path and
        direct no-updater bootstrap/completion callers are now on this clean
        authority.
   - Shadow equivalence does not need to wait for the final cell graph. Compare
     old and new values by output class, starting with selected versions,
     module source paths, registry hashes, selected yanked versions,
     module-version data, and resolution facts. Swap a production injected
     output only after same-daemon invalidation proves the new producer changes
     for an explicit DICE input reason.
   - Current-workspace helpers that need graph facts still read the named cell
     graph, but data-only module-version and registration helpers now derive
     current workspace identity from their injected data. Module-version
     consumers now get the root module name from injected module-version data
     instead of computing the cell graph. The persisted config-load key carries
     the daemon output base, including
     no-`MODULE.bazel` empty DICE inputs. Data-only projection keys now rely on
     their own source workspace provenance instead of deriving identity through
     the cell graph. Their data payloads are injected from the clean resolved
     graph producer. Daemon bootstrap no-updater parsing computes the clean
     graph with its isolated buck-out path before parsing cells.
   - Prove warm reuse by DICE cutoffs, not by a process-global bridge cache.
     The process-global fast path, projection bridge key, and direct cell-graph
     parser are removed; remaining proof belongs on the named graph, replay,
     lockfile, and materialization keys.

2. Finish module-file DICE inputs for git, archive, and out-of-project local
   override/registry-cache sources.
   - Root, included, and project-local local override module segments now use
     tracked project-file DICE inputs; out-of-project local override and
     cached git/archive override `MODULE.bazel` files are observed inside
     named DICE keys. The DICE-backed resolver now rejects missing tracked root
     module input instead of direct-parsing the root module in the DICE path.
      `NonRootModuleFilesKey` exists with same-key recompute guardrails, and
      is now wired into the clean resolved-graph producer. Registry and
      git/archive override `MODULE.bazel` files discovered during MVS resolution
      are read through `NonRootModuleFilesKey` instead of direct
      `std::fs::read_to_string`. The `parse_module_with_polled_includes` path
      in the clean graph compute has been replaced.
   - Registry cache `MODULE.bazel`, `source.json`, and `bazel_registry.json`
     files are tracked when the cache lives under the project root. Missing
     locked supported non-file registry entries are now fetched through the
     registry client before checksum validation, matching Bazel's registry fetch
     shape while preserving strict behavior for `file:` registries and
     unsupported paths. Out-of-root cache paths are
     directly observed inside
     `RegistryFileInputsKey` and force same-key recompute through
     `has_untracked_inputs` while the final watched-input graph is still
     pending. Locked
     registry `source.json`
     checksum, parse/UTF-8 failure, and create/delete transitions now have
     same-daemon guardrails, locked registry `MODULE.bazel` parse/UTF-8
     failure and create/delete transitions have same-daemon guardrails, and locked
     top-level `bazel_registry.json` create/delete transitions have same-daemon
     guardrail coverage; its metadata validation preserves Bazel's
     blank-JSON-as-absent behavior.
   - Cached `git_override` and `archive_override` `MODULE.bazel` files now both
     have same-daemon warm-reuse, edit-invalidation, and create/delete
     transition guardrails. Both cached override source classes also have
     parse-error, UTF-8-error, and include-cycle guardrails.
   - Replace remaining direct `std::fs` validity hacks with tracked filesystem
     dependencies or equivalent DICE input nodes. Repository materialization
     marker/layout/recorded-input reads are now child DICE nodes, but those
     children still poll until the tracked filesystem API is available below
     `slug_common`.
   - Include create/delete transitions, parse failures, include cycles, and
     UTF-8 failures for every module source class. Cached git/archive override
     coverage now includes create/delete, parse/UTF-8 failures, and include
     cycles; project-local and out-of-project local override coverage now
     includes create/delete, parse/UTF-8 failures, and include cycles. Root
     included segments now have parse/UTF-8 failure, include-cycle, and
     create/delete coverage. Root `MODULE.bazel` plus locked registry
     `MODULE.bazel`, `source.json`, and `bazel_registry.json` have parse/UTF-8
     failure and create/delete coverage.
   - Model registry selection and source metadata for overrides. Single-version
     and multiple-version override registry source metadata now both have
     same-daemon creation, deletion, parse-failure, and UTF-8 failure coverage.

3. Make lockfile replay complete.
   - Visible workspace lockfile bytes now use tracked project-file DICE inputs;
     hidden/output-base lockfile content is no longer carried as an observed
     payload or precomputed poll digest into `TrackedLockfileContentKey`.
     Out-of-project hidden lockfiles are still directly polled by that named
     key and invalidated across transactions until the final watched-input graph
     replaces the direct polling.
   - The clean bzlmod graph producer gets visible/hidden lockfile values from a
     named lockfile-input bridge key instead of producing those reads inline.
     The resulting `BzlmodLockfileInputsValue` is still a bridge-shaped value
     until the true lockfile policy/value graph replaces it. It is injected as
     a named DICE input beside the direct `BzlmodCellGraphValue`, so no
     monolithic projection payload carries lockfile-input facts.
   - DICE-backed bzlmod resolution now requires the tracked visible and hidden
     lockfile values when lockfile mode/path policy says those inputs are
     active, instead of silently falling back to a direct lockfile read inside
     the DICE compute path.
   - Preserve Bazel's hidden-lockfile fail-open behavior without hiding
     invalidation.
   - Preserve same-daemon hidden-lockfile create/edit/delete/facts coverage
     while moving the implementation out of the transitional graph.
   - Model facts, selected yanked versions, registry file hashes, recorded
     inputs, and lockfile mode as explicit dependencies. Visible lockfile
     selected-yanked-version edits now have same-daemon guardrail coverage.
   - Keep ordinary build/query paths read-only; count write attempts as test
     failures unless the command is explicitly a lockfile update command.

4. Replace best-effort extension `.bzl` digesting with the actual loaded module
   graph.
   - Reuse the Starlark loader or expose its load graph to bzlmod keys.
     `ExtensionBzlTransitiveDigestKey` now uses a late-bound
     interpreter-side loaded graph digest when an aggregation exists;
     Slug's implicit `@slug_builtins` autoload is excluded from the Bazel
     lockfile digest, and missing-load cases now fail through the loader before
     replay instead of falling back to the transitional scanner. Non-DICE
     bootstrap/preseed callers still use the transitional scanner.
     Loaded-graph digest values are transaction-valid; fallback-scanner digest
     values remain invalid across transactions.
   - Keep the current external `bazel-external/<repo>` and mapped literal-load
     digest coverage while replacing it with file digest changes from the
     actual loader graph, load failures, and deleted files.
   - Lockfile spoke pre-seeding uses the tracked project-file digest producer
     when DICE inputs are available, including deterministic project-local
     missing-file digest state without a direct filesystem read;
     `ExtensionBzlTransitiveDigestKey` now errors on real executor load
     failures, while non-DICE bootstrap/preseed callers still use the scanner
     directly.
   - Reject replay when any loaded implementation file changes, not only
     literal loads that the transitional scanner can find.

5. Move extension spoke and generated repo registration out of process globals.
   - Represent generated repo specs, sibling spokes, seeded cells, and
     materialization state as DICE values.
   - `SEEDED_EXTENSIONS` and `SPOKE_REGISTRY` are removed as bzlmod semantic
     state. Continue moving generated repo cell registration and materialized
     output state into DICE-owned values.
   - Runtime file-ops spoke lookup now uses the injected bzlmod workspace
     identity instead of deriving one from the project root, and sibling spokes
     discovered from the current DICE spoke value are registered on the active
     resolver instead of the process-global dynamic registry. Extension
     execution also registers captured generated repos on the active resolver,
     and startup runtime extension cells now come from the resolver snapshot
     instead of process-global dynamic maps. Alias compatibility fallback still
     uses process-global transitional plumbing.
   - Extension repo execution/materialization keys now preserve the workspace
  identity and output base, but generated repo cell graph ownership and
  final materialization state are still not fully DICE-owned.
- Dynamic generated-repo cell maps are now scoped by the active transitional
  workspace identity, including output base when replayed from a bzlmod cell
  graph, but they remain process-global maps rather than a DICE-owned
  `BzlmodCellGraphKey`. Resolver-local promoted dynamic cells from graph
  snapshots and current extension-spoke values are graph-owned, and direct
  resolver lookup no longer materializes runtime snapshot cells by internal
  generated repo name, but directory scans and alias-compatibility maps are
  still process-local lookup accelerators rather than DICE inputs.
  Directory-scan fallbacks are disabled once the active transitional scope
  includes an output base.
- Module extension and repository-context label path resolution now receive
  resolver-owned cell path maps from the active cell resolver, and those maps are
  authoritative when present. `module_ctx.path(Label(...))`,
  `repository_ctx.path(Label(...))`, repository path-like APIs, and the
  `repository_ctx.path(Label(...))` lazy materialization trigger no longer fill a
  resolver-owned miss from process-global dynamic aliases, directory scans, or a
  synthetic workspace-root path. `module_ctx` Label-taking methods now require
  that resolver-owned map rather than falling back when it is absent, and
  `repository_ctx` path-like label resolution always uses the context's explicit
  cell-path map. The remaining bridge is the producer: the map is still derived
  from a legacy-produced cell graph rather than a true `BzlmodCellGraphKey`.
- Native repository-rule `build_file`/`patches` label resolution can now receive
  a resolver-owned cell path map from the bzlmod cell graph, and the normal
  executor path now requires that map instead of retaining a
  `bazel-external` directory-scan fallback. Patch resolution preserves the
  existing non-fatal repository-rule behavior by warning and continuing after
  resolution errors. The remaining bridge is the producer: the cell graph value
  supplying those paths is still legacy-produced via `BzlmodCellGraphDataKey`
  instead of derived from true DICE module/repo/spec keys.
- Bzlmod load-path wrong-cell equivalence and load-path canonicalization,
  toolchain implementation label parsing, metadata label parsing, and C++
  toolchain metadata/action-path formatting can now use declared aliases and
  runtime aliases/cells from the active cell alias resolver instead of consulting
  process-global dynamic aliases. Load-path canonicalization, `Label()`
  explicit/owner-scoped repo canonicalization, and toolchain implementation
  label parsing no longer consult process-global dynamic aliases on resolverless
  or no-runtime-snapshot resolver misses; production metadata/C++ metadata
  contexts now make the same miss behavior owner-only, with process-global
  metadata fallback kept only in test-only compatibility contexts.
- `config_setting(flag_values = ...)` build-setting lookup now also uses the
  active cell alias resolver for bzlmod repo-spelling normalization before
  consulting process-global dynamic aliases.
- Bazel-style transition input/output build-setting label parsing now also uses
  the active cell alias resolver, so transition-produced settings no longer
  need process-global generated-repo aliases when a runtime snapshot is
  available.
- Generic build-setting label parsing now uses only a caller-supplied active
  cell alias resolver for bzlmod repo-spelling normalization; resolverless and
  no-runtime-snapshot misses keep the apparent repo spelling instead of
  consulting process-global generated-repo aliases.
- Configured provider `Label` values exposed through normal analysis `ctx.attr`,
  dependency objects, query-result dependencies, source-file targets, derived
  same-package/relative labels, subtargets, and `ctx.label` now carry the
  active cell alias resolver. Their Bazel-visible workspace/repo stringification
  uses only declared aliases and runtime aliases from that resolver;
  resolverless and no-runtime-snapshot misses keep the apparent cell spelling
  instead of consulting process-global dynamic aliases. The remaining bridge is
  the producer: the resolver snapshot is still derived from legacy-produced cell
  graph data rather than a true `BzlmodCellGraphKey`.
- Target-owned output path formatting now uses the configured target label's
  stored package cell name directly instead of process-global bzlmod
  alias/module canonicalization. The remaining bridge is the producer: those
  labels still ultimately come from the legacy-produced cell graph until
  `BzlmodCellGraphKey` is true graph-owned data.
- Clean review of the output-path slice found two remaining ownership leaks:
  `ArtifactPath` still asked process-global bzlmod alias state to canonicalize
  external repo names, and bzlmod module cells/repo-mapping targets could still
  be stored under apparent module names such as `b` while Bazel's canonical
  module repository is `b+`. This slice burns down the output/file-path and
  repo-mapping bridge surface by making `ArtifactPath` consume the stored label
  cell name, storing resolved bzlmod module cells under
  `bazel_canonical_module_repo_name`, canonicalizing repo-mapping target values
  to selected graph cell names, and resolving placeholder labels through the
  active alias owner before preserving apparent names as a compatibility
  fallback. The intended owner is `BzlmodCellGraphKey` plus `RepoMappingKey`;
  the current owner remains `BzlmodCellGraphValue` /
  `BzlmodCellGraphDataKey` until the cell graph is derived from true module,
  resolution, and repo-mapping DICE producers. Before evidence included
  `ArtifactPath`'s `canonical_external_cell_name` process-global call,
  `CellName::unchecked_new(name)` / `CellName::unchecked_new(module_name)` for
  bzlmod module cells, and focused Plan 61 failures on `b//...` dependency keys
  while the known cell was `b+`. After evidence: targeted searches for
  `slug_core::cells::canonical_bazel_repo_name_for_cell` /
  `canonical_external_cell_name` in output-path formatting and
  `CellName::unchecked_new(name|module_name)` in bzlmod cell creation return no
  hits; searches show `canonicalize_repo_mapping_snapshot_targets` and
  placeholder `resolve_declared_or_runtime_alias(cell_alias)` in the owner path.
  Validation passed with focused `slug_common` canonical-module tests, the
  focused `slug_execute` artifact-path regression, focused
  `slug_interpreter_for_build` placeholder-label regressions, affected-crate
  `cargo check`, `cargo build -p slug`, the explicit-binary Plan 61 selector for
  explicit module repo names and scoped/root alias leakage (`3 passed, 152
  deselected`), `cargo fmt --check`, and `git diff --check`.
- Follow-up clean reviews found that `override_repo()` / `inject_repo()` target
  values could still be persisted as root-visible apparent aliases in
  `RepoMappingOverrides`, in `BzlmodRepoMapping::for_module()` snapshot rows,
  and in the pre-projection replay-summary lockfile lookup path before being
  copied into extension-generated repo mappings. This slice canonicalizes the
  shared repo-mapping state through selected graph cell names when graph data is
  available, then through the root repo mapping's alias closure, before the
  mappings feed `add_extension_generated_repo_mappings`,
  `BzlmodRepoMappingsDataValue`, replay-summary hashing, or lockfile cache
  lookup. Bridge surface reduced: root override/inject mapping rows no longer
  preserve apparent root aliases such as `helper_alias`; the projection path
  stores graph-owned cells such as `dep+`, while the pre-projection replay
  summary at least removes the apparent alias before true graph data exists.
  The intended owner is still `RepoMappingKey` plus `BzlmodCellGraphKey`; this
  keeps the transitional projection's stored repo mappings aligned with that
  shape. Validation passed with focused `cargo test -p slug_common repo_mapping
  -- --nocapture`, `cargo check -p slug_common -p slug_bzlmod`, `cargo build -p
  slug`, and the explicit-binary Plan 61 selector for inject/override repo
  mapping and root alias leakage (`4 passed, 151 deselected`), plus `cargo fmt
  --check` and `git diff --check`.
- The remaining public `slug_core::cells::canonical_bazel_repo_name_for_cell`
  process-global helper and the unused resolver method that could still call it
  on no-runtime-snapshot misses are deleted. Bridge surface reduced: callers
  can no longer ask `slug_core` to canonicalize arbitrary cell names through
  process-global dynamic alias/module scans; Starlark-visible output and action
  path formatting keeps its private resolver-owned helper that only consumes
  declared/runtime aliases from the active `CellAliasResolver`. The intended
  owner remains `BzlmodCellGraphKey` plus `RepoMappingKey`; until then the
  narrow compatibility scanners stay behind explicitly named module/dynamic
  helpers rather than a generic canonicalization API. Before evidence:
  `rg -n "canonical_bazel_repo_name_for_cell|canonical_bzlmod_repo_name_for_cell" app/slug_core/src/cells.rs`
  found the public helper, resolver method, and tests. After evidence: the same
  search has no hits in `slug_core`; remaining hits are the private
  `slug_build_api` resolver-owned helper. Validation passed with focused
  `cargo test -p slug_core canonical_bzlmod_module_cell_name_uses_empty_version_module_suffix -- --nocapture`
  and `cargo check -p slug_core -p slug_build_api`.
- `CellResolver::get` root-alias lookup now uses only resolver-owned declared
  aliases and runtime aliases/cells, so no-snapshot misses no longer consult
  process-global generated-repo aliases during ordinary unknown-cell lookup.
- Path-to-cell projection now checks graph-owned dynamic cells and the
  resolver-owned runtime snapshot before root-scoped process-global dynamic
  cells; when a resolver-owned bzlmod runtime snapshot is present, root-scoped
  process-global dynamic cells are no longer consulted for path projection
  misses. Bridge surface reduced: a stale process-global dynamic cell cannot
  classify `bazel-external/<repo>/...` paths for a resolver whose graph snapshot
  does not own that repo. The intended owner is `BzlmodCellGraphKey` via the
  resolver-owned runtime snapshot; the final cell graph is still injected from
  legacy-produced data. Validation passed with focused `cargo test -p slug_core
  get_cell_path_ -- --nocapture`, `cargo check -p slug_core`, `cargo fmt
  --check`, and `git diff --check`.
- Runtime-snapshot `CellResolver` name lookup now follows the same ownership
  boundary as path projection: resolver-local graph-owned dynamic cells remain
  valid, but root-scoped process-global dynamic cells are ignored whenever a
  resolver-owned runtime snapshot exists. Bridge surface reduced: a stale
  root-scoped dynamic cell can no longer satisfy either
  `CellResolver::get(name)` or `get_cell_path(path)` for a resolver whose
  `BzlmodCellGraphKey`-backed runtime snapshot does not own that generated repo.
  The remaining bridge is still the producer: the runtime snapshot is injected
  from legacy-produced cell graph data. Validation passed with focused
  `cargo test -p slug_core
  get_cell_path_with_runtime_snapshot_rejects_root_scoped_dynamic_cell_miss --
  --nocapture`.
- Lazy extension repository path classification now reads the resolver's
  runtime cell graph snapshot before process-global dynamic discovery, but the
  graph is still injected from legacy-produced data.
- Temporary root-cell and non-root cell-name adapters are scoped by the same
  transitional workspace adapter when available, but remain process-global
  compatibility adapters.
- `BzlmodCellGraphDataKey` now names the legacy-produced cell graph in DICE,
     including bundled bzlmod cells, and resolver assembly, runtime
     installation, module-version projection, and extension-aggregation
     projection consume that value. Registered toolchain/platform projection
     keys use their own workspace-checked injected data, while
     current-workspace helpers still use the cell graph to choose the active
  workspace. The deferred registered-toolchain pool and markers now depend on
     the same workspace/list signature as eager loading before process-global
     reuse, but this remains transitional until the installed lookup registry is
     a DICE-owned value. Toolchain resolution, target-setting pre-processing,
     and registered-toolchain package loading now receive caller/resolver-owned
     snapshots before process-global fallback, but the snapshot producers are
     still transitional rather than DICE-owned values. Runtime module-symlink
     replay now writes under that graph's workspace output base, and
     extension-repo symlink replay uses the same graph output base, but the
     graph itself is still legacy-produced and runtime registration remains
     process-global transitional plumbing.
   - Ensure two workspaces and two command policies cannot share generated repo
     state by accident.

6. Complete Bazel 9 directive semantics.
   - Implement or explicitly Bazel-ground the behavior for
     remaining `dev_dependency` surfaces, `single_version_override(registry/patches)`,
     `multiple_version_override(registry)`, `archive_override`, `git_override`,
     remaining `override_repo` validation, remaining `inject_repo` validation,
     and isolated extension usages.
   - Preserve root `bazel_dep(dev_dependency=True)` default inclusion and
     `--ignore_dev_dependency` exclusion while moving command policy out of the
     transitional resolver.
   - Add negative tests where Bazel 9 fails.

7. Make repository execution replay-correct.
   - Track `repository_ctx.watch`, `watch_tree`, environment reads, repo mapping
     reads, label paths, downloads, archive/git source identity, patches,
     overlays, and generated files.
   - Replace marker-file trust with a manifest value that proves the current
     repo spec and observed output tree are compatible. The current manifest
     value has DICE child state for marker/layout/recorded-input checks, but
     does not yet own the full repository output-tree identity. Known repo-spec
     extension file-ops now delegates marker/content/output-state staleness,
     recorded-input staleness, missing declared BUILD-file checks, and
     layout-validity probes, including foreign top-level symlink checks, to
     this manifest path. Legacy invalid empty target-label checks also belong
     to manifest layout state now. No-spec extension cells no longer reuse a
     marker-only materialization: they either validate through a current DICE
     spoke repo spec or enter the use_repo_rule execution key, leaving the
     materialization manifest as the reuse authority. Extension repo execution
     enters the native repository-rule executor through a fresh path after the
     manifest decision, so the native marker shortcut is test-only rather than
     an additional production reuse authority for known extension repo specs.
   - Built-in `local_repository`/`new_local_repository`, captured Starlark
     `repository_rule(local=True)` repos, and root-local custom
     `use_repo_rule("//:repo.bzl", "rule")` definitions with
     `repository_rule(local=True)` are non-cacheable. The root-local
     precompute path now inspects the repository-rule definition's `local` bit,
     including root-local `.bzl` loads, before serializing the `RepoSpec`, and
     the focused guardrail
     `test_custom_use_repo_rule_local_definition_reexecutes_after_input_edit`
     proves rematerialization after an unwatched input edit. Root-local `.bzl`
     modules that need the normal loader context now refresh the local bit at
     repository execution time after the cell graph and Starlark loader are
     installed, and
     `test_custom_use_repo_rule_local_probe_failure_does_not_block_execution`
     proves rematerialization after an unwatched input edit for that class.
     External/cross-repo `.bzl` files also rely on that execution-time loaded
     rule bit after the bzlmod cell graph is established, and
     `test_external_use_repo_rule_local_definition_reexecutes_after_input_edit`
     proves rematerialization after an unwatched input edit for that class.
   - `repository_ctx.read(Label(...))` with default `watch = "auto"` now records
     the read file as a repository materialization input, so editing the source
     label file rematerializes the generated repository in the same daemon. This
     closes the implicit-read half of Bazel's `readFile`/`RepoRecordedInput.File`
     behavior for workspace-label reads.
   - `repository_ctx.template(..., Label(...))` with default
     `watch_template = "auto"` now records the template file as a repository
     materialization input, so editing the source template rematerializes the
     generated repository in the same daemon.
   - `repository_ctx.patch(Label(...))` with default `watch_patch = "auto"` now
     records the patch file as a repository materialization input, so editing
     the patch rematerializes the generated repository in the same daemon.
   - `repository_ctx.extract(Label(...))` with default `watch_archive = "auto"`
     now records the archive file as a repository materialization input, so
     editing a binary source archive rematerializes the generated repository in
     the same daemon.
   - `repository_ctx.watch_tree(Label(...))` now tracks binary file leaves via
     metadata/digest DICE dependencies instead of UTF-8 source reads, so binary
     edits inside a watched tree rematerialize the generated repository in the
     same daemon.
   - `repository_ctx.download*`, `module_ctx.download*`, and native
     `http_archive`/`http_file`/`http_jar` cache lookups now include
     `canonical_id` restrictions, so checksum-identical cache entries are not
     reused across distinct non-empty canonical ids.

8. Make the bzlmod cell graph a DICE value.
   - Derive module cells, extension-generated cells, aliases, scoped mappings,
     external symlinks, and bundled repos from DICE values.
   - `BzlmodCellGraphDataKey` currently exposes injected graph data rather than
     computing the graph itself. The production payload is now derived from the
     clean resolved-graph producer and carries that producer's graph digest.
     The returned graph's root module name is derived from `ModuleVersionsKey`;
     root aliases, scoped aliases, and dynamic aliases are derived from
     `BzlmodRepoMappingsKey`; module symlinks are derived from module cell setup
     where possible. Legacy cell parsing still takes the remaining cell,
     extension-cell, and non-derivable local-override symlink vectors from this
     payload. The old `BzlmodProjectionData` wrapper has been deleted.
   - Ensure cell graph changes invalidate analysis and package loading
     correctly in the same daemon.
   - Prove apparent aliases do not leak across module scopes.

9. Delete transitional APIs.
   - `BzlmodSessionData`, `BzlmodSessionDataKey`, and
     `BzlmodProjectionData` are removed, and `BuckConfigBasedCells` no longer
     stores a bzlmod payload or returns it to the server updater. Persisted
     config-load now gets `BzlmodCellGraphValue` from the clean graph producer
     instead of a legacy resolver bridge; lockfile inputs, repo-env,
     resolution facts, repo mappings, registered toolchains, registered
     execution platforms, extension aggregations, and module versions are
     passed as separate named injections. The remaining transitional API is the
     injected `BzlmodCellGraphDataKey` itself: production now injects clean graph
     data addressed by the clean resolved-graph digest, but the key does not
     compute the graph from DICE dependencies yet.
   - Generic empty session/projection construction is removed from production
     paths. Remaining empty bzlmod-input construction must explicitly carry
     workspace identity while direct bootstrap/completion parsing is being
     unwound. The no-project sentinel is named on `WorkspaceId`; callers now
     install explicit empty DICE inputs instead of a full transitional payload.
   - Extension repository execution constructors that derive workspace identity
     from project root are test-only; production code must pass explicit
     workspace identity and repo-env. Bzlmod projection-key and
     materialization-manifest helpers that derive workspace identity from only a
     project root are also test-only. Zero-repo-env convenience constructors
     for extension execution and materialization manifests are test-only too.
     Module extension execution keys require workspace identity instead of
     carrying optional provenance. The remaining bzlmod projection data wrappers
     also require workspace provenance instead of accepting absent provenance.
     The legacy resolver entry point requires an explicit workspace identity too.
     The outer parse helper also requires callers to choose the empty-projection
     workspace identity explicitly.
   - The config-load command repo-env global readback and module/repository
     runtime repo-env adapters are removed; keep repo-env wired through explicit
     DICE/key/context inputs as the graph migrates.
   - Remove bridge cache fast paths whose correctness depends on hand-curated
     cacheability predicates.
   - Keep only compatibility shims that are demonstrably non-semantic or needed
     for plumbing during a short, tracked migration window.

10. Re-run SDK parity and performance checks after the real DICE graph lands.
    - Repeat focused guardrails first.
    - Then run `/var/mnt/dev/zeromatter-kuro //sdk:sdk_contents`.
    - Compare Bazel 9.0.1 and Slug manifests/modes/hashes.
    - Record performance and memory separately from correctness. A slow but
      replay-correct implementation is not complete for product quality, but a
      fast bridge is not acceptable as a correctness substitute.

## Strong Guardrails

The plan is blocked, not complete, unless all of these are true.

### Completion Guardrails

- No key named or described as a legacy bzlmod resolution bridge is required
  for normal build/query/audit/test bzlmod operation.
- No bzlmod correctness path depends on process-global mutable state for
  extension spokes, generated repo cells, repo env, seeded markers, or resolved
  graph facts.
- Every successful DICE cache hit for module resolution, extension replay, repo
  mapping, and repository materialization has an auditable key/input reason.
- Visible and hidden lockfile edits, creation, deletion, parse errors, and facts
  changes are observed in the same daemon according to Bazel 9 semantics.
- Extension replay invalidates on any change to the actual transitive `.bzl`
  load graph, including external repository loads.
- Registry file hash mismatches and missing checksums under
  `--lockfile_mode=error` fail before mutable content is fetched.
- Repository rule recorded inputs are either supported and keyed or rejected as
  replay misses. Unsupported recorded inputs must never replay stale specs.
- Generated repo materialization is keyed by repo spec, command policy, watched
  inputs, and output manifest, not by marker existence alone.
- Two workspaces with colliding module names, extension names, generated repo
  names, and lockfile entries do not share bzlmod state in one daemon.
- Root apparent aliases and transitive apparent aliases are scoped exactly like
  Bazel. No root alias leaks into another module.
- Root and non-root `dev_dependency` behavior matches Bazel 9 command policy,
  including `--ignore_dev_dependency` behavior if supported by Slug.
- Ordinary build/query/audit paths do not write visible lockfiles.
- Guardrails must include at least one negative test for every replay input
  class: root module, include file, local override module, registry module file,
  source metadata, visible lockfile, hidden lockfile, extension implementation
  `.bzl`, extension tag value, repo env, repo mapping, facts, repository watched
  file, repository watched tree, and materialized output marker/manifest.

### Status Guardrail

The `## Status` section must not say `Complete`, `Done`, or equivalent until
the completion guardrails above pass and the remaining-work list has no
semantic items left. SDK parity evidence may be summarized as a checkpoint, but
it must not be used as the close condition for Plan 61.

### Test Guardrails

Required focused tests:

- `cargo test -p slug_bzlmod`
- `cargo test -p slug_common bzlmod`
- `cargo test -p slug_external_cells`
- `TEST_EXECUTABLE=/var/mnt/dev/slug/target/debug/slug python -m pytest -q tests/core/bzlmod/test_plan61_guardrails.py -rx --tb=short`

Required same-daemon scenarios:

- warm no-op reuses DICE-owned bzlmod resolution without running legacy bridge
  code;
- editing root `MODULE.bazel` invalidates only affected bzlmod nodes;
- editing an included module segment invalidates only affected bzlmod nodes;
- editing a local override `MODULE.bazel` invalidates only affected bzlmod
  nodes;
- editing visible `MODULE.bazel.lock` invalidates or fails according to mode;
- editing hidden lockfile invalidates hidden-replay consumers or fail-opens
  exactly like Bazel;
- editing an extension implementation dependency rejects stale replay;
- changing repo env invalidates recorded ENV replay;
- changing repo mapping invalidates recorded REPO_MAPPING replay;
- changing a repository watched file/tree invalidates repository replay;
- replacing a repo output marker without matching manifest does not produce a
  stale materialization hit.

Required real-world smokes:

- A focused owning-abstraction repro for any newly found frontier.
- Full `/var/mnt/dev/zeromatter-kuro //sdk:sdk_contents` Slug smoke after
  focused fixes.
- Bazel 9.0.1 comparison for the same target.
- Manifest/mode/hash comparison, with known output-root ELF differences
  classified separately from bzlmod replay correctness.

## Validation Workflow

For every future Plan 61 slice:

1. State the Bazel 9 source or observed Bazel 9 behavior being matched.
2. Add or strengthen the smallest owning-abstraction regression first.
3. Implement the DICE-owned value or dependency edge.
4. Prove same-daemon invalidation or replay behavior with counters/logs.
5. Run the relevant focused test package.
6. Use `//sdk:sdk_contents` only as frontier confirmation after the local bug
   class is understood.
7. Update this plan with a compact result, not a play-by-play.

## Out Of Scope

- Bazel 8 compatibility.
- WORKSPACE support.
- Migration shims for old Slug prototype behavior.
- Exact-byte ELF parity caused only by output-root strings. Track that as an
  output-root design item unless it exposes a bzlmod input/replay bug.

## Source Of Truth

Use Bazel 9 source and behavior as the source of truth:

- Symbol removal and global surface:
  `src/main/java/com/google/devtools/build/lib/analysis/BaseRuleClasses.java`
  and relevant `rules-*.java` registries.
- Module resolution, lockfile, repo specs, extension evaluation, yanked
  versions, and registry hashing:
  `src/main/java/com/google/devtools/build/lib/bazel/bzlmod/`.
- Repository execution and fetch markers:
  `src/main/java/com/google/devtools/build/lib/rules/repository/`.
- Label parsing and repository mappings:
  `src/main/java/com/google/devtools/build/lib/cmdline/Label.java` and
  `src/main/java/com/google/devtools/build/lib/cmdline/RepositoryMapping.java`.
- Bundled `@bazel_tools` content:
  upstream `src/<path>/BUILD.tools` and installed Bazel `embedded_tools/`.
