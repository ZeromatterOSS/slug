# Plan 61: True DICE-Owned Bzlmod

> Parent: [Slug Bazel-Compatible Build Tool](../2026-01-21-slug-bazel-compatible-build-tool.md)
>
> Created: 2026-05-18
>
> Consolidated: 2026-05-22

## Status

Open. The prior Plan 61 work produced a useful DICE-assisted bzlmod bridge and
fixed many SDK-blocking parity bugs, but it is not yet a replay-complete
DICE/Skyframe implementation.

Do not mark this plan complete because `//sdk:sdk_contents` passes, because the
current guardrail file passes, or because a warm daemon smoke reuses the
transitional bridge. Those are necessary evidence, not sufficient acceptance
criteria.

Current classification:

- Slug has DICE keys for selected bzlmod inputs and extension/repository
  execution surfaces.
- The resolved module graph and cell graph are still primarily produced by
  legacy startup cell parsing and injected into DICE as command session data.
- Replay correctness still depends on non-DICE process state, best-effort
  digests, and transitional bridge behavior in places where Bazel owns explicit
  Skyframe keys.

The plan can only be closed when bzlmod module-file parsing, module resolution,
repo mapping, extension aggregation, extension replay inputs, repository specs,
repository materialization manifests, and lockfile policy are represented as
DICE-owned values with explicit dependencies, invalidation, and guardrails.

## Current Checkpoint

Historical slice logs and detailed validation transcripts now live in
[61-true-dice-bzlmod-history.md](./61-true-dice-bzlmod-history.md). Future
workers should read this main plan first and open the history file only when
they need exact older evidence, command transcripts, or provenance for a prior
bridge-burn-down slice.

Current state to preserve:

- Plan 61 is open. Slug has a useful DICE-assisted bzlmod bridge, but the
  resolved module graph and cell graph are still primarily produced by legacy
  startup cell parsing and injected into DICE as command session data.
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
  and lockfile preseed paths still have a legacy fallback-scanner bridge.
- Repository materialization now has a named manifest key and child state for
  marker/layout/recorded-input checks, but those child reads still poll
  filesystem state until lower-level tracked filesystem keys are available.

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
`BzlmodProjectionBridgeDiceKey`, `BzlmodProjectionData`, legacy-produced cell
graph injection, scanner fallback, process-global alias/cell state, direct
filesystem polling, or marker-trust materialization. Pair that with the intended
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
   - Replace `BzlmodProjectionBridgeDiceKey` with true graph producers. The
     old `LegacyBzlmodResolutionDiceKey` name has been demoted away, but the
     bridge still wraps the legacy resolver.
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
   - First build clean shadow producers that leave the legacy injection path as
     production authority. The initial viability slice is a
     `BzlmodResolvedModuleGraphKey`-style producer for the resolved graph plus
     module-version and resolution-fact outputs. It may reuse lower-level
     `slug_bzlmod` primitives such as `MvsResolver`, `ModuleCache`, parsed
     module values, and lockfile types, but it must not call
     `BzlmodProjectionBridgeDiceKey`,
     `parse_with_config_args_and_persisted_bzlmod_projection_bridge`, or
     `resolve_bzlmod_dependencies_with_options`.
     The first code path can run as an opt-in diagnostic shadow
     (`SLUG_BZLMOD_CLEAN_RESOLUTION_SHADOW=1`) while legacy injection remains
     the production authority.
     The follow-up slice promotes `BzlmodModuleVersionsDataValue` and
     `BzlmodResolutionFactsValue` injection to the clean resolved-graph key, so
     the legacy projection bridge no longer carries those output classes.
     Evidence: `cargo check -p slug_common`,
     `cargo test -p slug_common persisted_projection_injects_clean_root_module_version_data -- --nocapture`,
     and
     `cargo test -p slug_common clean_resolved_module_graph_produces_local_override_facts -- --nocapture`.
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
        aliases, and external symlink layout.
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
     no-`MODULE.bazel` empty projections. Data-only projection keys now rely on
     their own source workspace provenance instead of deriving identity through
     the cell graph, but their data payloads are still injected from the legacy
     resolver. Daemon bootstrap direct parsing now passes its isolated buck-out
     path into the transitional workspace identity too.
   - Prove warm reuse by DICE cutoffs, not by a process-global bridge cache.
     The process-global fast path is removed, but the transitional key still
     wraps the legacy resolver.

2. Finish module-file DICE inputs for git, archive, and out-of-project local
   override/registry-cache sources.
   - Root, included, and project-local local override module segments now use
     tracked project-file DICE inputs; out-of-project local override and
     cached git/archive override `MODULE.bazel` files are observed inside
     named DICE keys. The DICE-backed resolver now rejects missing tracked root
     module input instead of direct-parsing the root module in the DICE path.
     Non-root module files discovered from the transitional cell graph now read
     through `NonRootModuleFilesKey`; project-root paths are DICE-tracked in the
     parse key, while out-of-project paths are directly polled by named
     absolute-file child keys and force same-key recompute through
     `has_untracked_inputs`.
   - Registry cache `MODULE.bazel`, `source.json`, and `bazel_registry.json`
     files are tracked when the cache lives under the project root, and
     out-of-root cache paths are directly observed inside
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
   - The projection bridge now gets visible/hidden lockfile values from a named
     lockfile-input bridge key instead of producing those reads inline, but the
     resulting `BzlmodLockfileInputsValue` still feeds the legacy resolver until
     the true lockfile policy/value graph replaces the projection bridge. The
     value is injected separately from `BzlmodProjectionData`, so the monolithic
     projection payload no longer carries lockfile-input facts.
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
   - `BzlmodCellGraphDataKey` currently exposes the legacy-produced graph, but
     the graph is not yet derived from DICE producers and is not the installed
     lookup authority. Legacy cell parsing now takes only that graph-shaped
     value rather than the whole `BzlmodProjectionData` payload.
   - Ensure cell graph changes invalidate analysis and package loading
     correctly in the same daemon.
   - Prove apparent aliases do not leak across module scopes.

9. Delete transitional APIs.
   - `BzlmodSessionData` and `BzlmodSessionDataKey` are removed, and
     `BuckConfigBasedCells` no longer stores a bzlmod payload or returns it to
     the server updater. The legacy resolver bridge now returns
     `BzlmodCellGraphValue` directly instead of wrapping it in
     `BzlmodProjectionData`; `BzlmodProjectionData` remains at the transitional
     injection API while lockfile inputs, repo-env, resolution facts, repo
     mappings, registered toolchains, registered execution platforms, extension
     aggregations, and module versions are passed as separate named injections.
     Delete or rename the remaining graph-shaped injection API only after
     module graph and cell-graph facts have true DICE producers.
   - Generic empty session construction is removed from production paths.
     Remaining empty projection construction must explicitly carry workspace
     identity while the projection bridge is still being unwound. The
     no-project sentinel is named on `WorkspaceId`; empty projection
     construction now remains only where a full transitional payload is needed.
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
