# Plan 61: True DICE-Owned Bzlmod

> Parent: [Slug Bazel-Compatible Build Tool](../2026-01-21-slug-bazel-compatible-build-tool.md)
>
> Created: 2026-05-18

## Status

Proposed.

This plan supersedes the completion claims in Plans 02, 09, and 10 when
"DICE bzlmod" means replay-correct graph-owned semantics. The current bzlmod
implementation is useful scaffolding. It is not yet the authority for module
resolution, extension replay, repo mapping, or repo materialization.

Every structural claim in this plan must be grounded in one of:

- Bazel source/docs from a pinned Bazel 9 checkout.
- Current Slug source or an existing plan documenting the current bug shape.
- A named local experiment that compares pinned Bazel output and Slug output.

The local Bazel checkout currently available at `/var/mnt/dev/bazel` is a
Bazel-9-era checkout (`9e1683ffb649b354124fe3cbd7413ae12d05ed3d`). Before
implementation, refresh citations against the exact Bazel 9 release tag chosen
for parity, and do not cite moving `main`.

Recent official docs checked during this pass:

- `https://bazel.build/versions/9.0.0/external/lockfile`, last updated
  2026-05-07, for lockfile generation, modes, hidden lockfile, and extension
  lockfile fields.
- `https://bazel.build/versions/9.0.0/external/module` for module discovery,
  overrides, `use_repo_rule`, and module canonical names.
- `https://bazel.build/versions/9.0.0/external/extension` for extension
  repository visibility and `inject_repo` / `override_repo`.
- `https://bazel.build/versions/9.0.0/external/overview` for
  apparent-vs-canonical repository names.

## Problem Statement

Slug's current bzlmod path is functional but not truly DICE-owned.

The authoritative module graph is assembled in legacy cell setup before DICE,
then injected into DICE and supported by process globals, lockfile caches,
marker files, stubs, and repair heuristics. DICE keys exist for late extension
and repo work, but they do not own all semantic inputs. In daemon/watch mode,
that can reuse stale bzlmod facts or cross-contaminate workspaces.

Current anti-patterns:

| Anti-pattern | Current grounding | Replacement |
|---|---|---|
| Graph computed before DICE | `resolve_bzlmod_dependencies()` is called during legacy cell setup in `app/slug_common/src/legacy_configs/cells.rs`; `CellResolverKey` receives an injected resolver in `app/slug_common/src/dice/cells.rs`. | `BzlmodResolutionKey` and projections. |
| Process-global semantic facts | `MODULE_VERSIONS` / toolchain globals in `app/slug_bzlmod/src/lib.rs`, dynamic cell globals in `app/slug_core/src/cells.rs`, extension/spoke globals in `extension_execution_dice.rs` / `spoke_materialization.rs`. | Workspace-scoped DICE values. |
| Under-keyed DICE values | Current extension/repo keys carry `project_root` but exclude it from hash/equality in `extension_execution_dice.rs` and `repository_execution.rs`. | A first-class `WorkspaceId` participates in every bzlmod semantic key or parent value. |
| Approximate replay digests | Current `compute_bzl_transitive_digest` is extension-id based; `recordedInputs` is parsed but not validated. | Bazel-shaped `bzlTransitiveDigest`, `usagesDigest`, and `RepoRecordedInput` validation. |
| Bare marker trust | `.slug_repo_complete` is trusted by repo/external-cell paths; Plan 38 documents marker-gated warm-build failures. | Bazel-shaped marker/recorded-input validation or a Slug DICE manifest with an explicit parity experiment. |
| Stub repos / empty specs on failure | Unknown repo rules and extension/repo-rule failures still create stubs in `repository_executor.rs` and `extension_repo.rs`. | Direct Bazel-shaped failure; no generated repo directory/marker on failure. |
| Canonical identity reconstruction | Current code accepts fallback spellings, suffix scans, and `bazel-external` discovery. | Typed module/extension/repo identity and scoped repo mappings. |
| Build-time lockfile mutation risk | `Lockfile::write` remains public and `--lockfile_mode` is not a complete policy boundary. | Explicit write capability for future `slug mod update`; ordinary paths cannot call the writer. |

## Non-Negotiables

- Bazel 9 parity only. No Bazel 8 compatibility, no WORKSPACE fallback, no
  compatibility shims for prototype behavior. Grounding: project `AGENTS.md`.
- DICE owns semantic facts. Process globals may exist temporarily as migration
  adapters, but each phase must reduce rather than expand them.
- Workspace identity is part of bzlmod identity. Two workspaces in one daemon
  cannot share extension aggregations, spokes, aliases, module versions,
  lockfile data, materialization roots, or project-root state.
- Extension/repo failures fail directly. They do not create stubs, silently
  repair outputs, or continue with empty generated repo sets.
- Apparent repo names are per-scope aliases, not identities. Canonical module
  repos, extension repos, innate `use_repo_rule` repos, bundled repos, and
  generated repos must have typed origin kinds.
- Lockfile policy is mode-aware. Bazel writes `MODULE.bazel.lock` in
  UPDATE/REFRESH modes (`RepositoryOptions.java`, `BazelLockFileModule.java`).
  Slug ordinary build/query/audit paths remain read-only as an interim safety
  policy until exact Bazel write parity exists; that safety policy is not a
  Bazel parity claim.

## Target DICE Graph

These names describe the target shape; exact Rust identifiers can differ. The
grounding column states whether the key follows Bazel structure or fixes a Slug
bug shape.

| Key / value | Owns | Grounding |
|---|---|---|
| `BzlmodWorkspaceKey { canonical_project_root, output_base } -> WorkspaceId` | Stable workspace identity used by all bzlmod semantic keys. | Current Slug under-keys by excluding `project_root`; daemon isolation acceptance requires this. Do not include command mode in `WorkspaceId`, or the same workspace becomes a different identity when flags change. |
| `BzlmodCommandPolicyKey { workspace_id, bazel_release_id, starlark_semantics_digest, bzlmod_flags, lockfile_mode, registry_config_digest, repository_cache_config, network_policy, repo_env_digest, nonstrict_repo_env_digest, ignore_dev_dependency, allow_yanked_versions, bazel_compatibility_policy, isolated_extension_usages_flag } -> BzlmodCommandPolicy` | Command-scoped policy and options that affect resolution, replay, and side effects. | Bazel 9 lockfile docs say lockfiles are specific to the Bazel version; Bazel `RepositoryOptions`, `ModuleFileFunction.IGNORE_DEV_DEPENDENCY`, `ModuleFileGlobals` dev-dependency/isolate handling, `RegularRunnableExtension` repo env inputs, and `SingleExtensionEvalFunction` Starlark semantics use. |
| `RootModuleFileKey { workspace_id } -> ParsedModuleFile` | Root `MODULE.bazel` parse and digest. | Bazel `ModuleFileValue` / `ModuleFileFunction`; current Slug parses in legacy `cells.rs`. |
| `ModuleSourceKey { workspace_id, command_policy_digest, module_key, registry_or_override_identity } -> ModuleSource` | Registry/local/archive source identity, registry order, patches/overlays, yanked state, auth/offline/cache policy. | Bazel `IndexRegistry`, `RepoSpecFunction`, module docs; current registry/cache code performs side effects. |
| `ModuleFileKey { workspace_id, source } -> ParsedModuleFile` | Transitive module parse from registry/local sources and included module files. | Bazel `ModuleFileFunction` root/non-root values and include handling. |
| `LocalOverrideSourceKey { workspace_id, declaring_module, override_literal, resolved_abs_path, module_file_digest }` | Local override identity and invalidation on create/edit/delete. | Bazel loads non-registry overrides through `RepositoryDirectoryValue`/`FileValue`; current Slug resolves local paths before DICE. |
| `LockfileContentKey { workspace_id, kind: workspace_or_hidden, path } -> { existence, digest, parse_result }` | Workspace lockfile and hidden output-base lockfile as filesystem inputs, including parse/version errors. In `off` mode, no semantic consumer should request this key. | Bazel 9 lockfile docs describe workspace and hidden lockfiles; `BazelLockFileValue.KEY` / `HIDDEN_KEY`; current `cached_lockfile` caches `None` process-wide. |
| `LockfileExtensionEntryKey { workspace_id, extension_instance_id, lockfile_digest, lockfile_mode, eval_factors }` | One extension's per-eval-factor lockfile replay entry. | Bazel `SingleExtensionEvalFunction` reads workspace then hidden lockfile, keyed by `ModuleExtensionEvalFactors`, unless mode is `off`. |
| `BzlmodResolutionKey { workspace_id, command_policy_digest } -> BzlmodResolutionValue` | Discovery, MVS selection, overrides, yanked versions, registry hashes, root/dev filtering. | Bazel `BazelModuleResolutionFunction`, `Selection`, `BazelModuleResolutionValue`, `RepositoryOptions`, and Bazel 9 module/lockfile docs. |
| `ResolvedModuleIdentity { workspace_id, module_key }` | Module canonical repo name, apparent repo name, `repo_name`, version metadata. | Bazel `ModuleKey` and external docs on canonical/apparent names. |
| `BzlmodCellGraphKey { workspace_id, resolution_digest } -> BzlmodCellGraph` | Slug root/external cells, bzlmod module cells, bundled cells, extension placeholder cells. | Slug-specific cell resolver shape; Bazel grounding is `BazelDepGraphValue` canonical repository graph. Include resolution identity explicitly to avoid hiding command-policy changes behind a workspace-only cell graph key. |
| `RepoMappingKey { workspace_id, resolution_digest, scope: RepoMappingScope } -> RepoMapping` | Scoped apparent-to-canonical mappings for module repos, extension implementation, generated repos, and innate repos. | Bazel repo mapping docs, `BazelDepGraphValue`, `ModuleExtensionRepoMappingEntriesFunction`; current Slug has global/scoped alias maps. |
| `RegisteredToolchainsKey { workspace_id, resolution_digest } -> Vec<RegisteredToolchain>` | Ordered `register_toolchains()` projection. | Bazel `ModuleFileGlobals`, `RegisteredToolchainsFunction`; current Slug global registry. |
| `RegisteredExecutionPlatformsKey { workspace_id, resolution_digest } -> Vec<CanonicalLabel>` | Ordered `register_execution_platforms()` projection. | Bazel `ModuleFileGlobals`, `RegisteredExecutionPlatformsFunction`; current Slug global registry. |
| `ModuleExtensionId { bzl_file_label: CanonicalLabel, extension_name, isolation_key }` | Extension identity. | Bazel `ModuleExtensionId`; current Slug string-normalizes extension ids. |
| `ExtensionUniqueName { workspace_id, extension_instance_id }` | Producer-owned generated repo namespace with Bazel's no-prefix-after-`+` uniqueness property. | Bazel `BazelDepGraphValue#getExtensionUniqueNames`, `BazelDepGraphFunction.calculateUniqueNameForUsedExtensionId`, and `SingleExtensionUsagesValue#getExtensionUniqueName`; generated repo canonical names are based on extension unique name plus internal repo name. |
| `ModuleExtensionAggregationKey { workspace_id, resolution_digest, extension_instance_id } -> ExtensionAggregationValue` | Aggregated usages/tags, eval factors, root module name, isolated usage separation. | Bazel `SingleExtensionUsagesFunction` / `SingleExtensionUsagesValue`; current Slug `EXTENSION_AGGREGATIONS` global. |
| `ModuleExtensionReplayInputKey { workspace_id, extension_instance_id, lockfile_entry_digest, bzl_transitive_digest, usages_digest, recorded_inputs }` | Normalized replay metadata while preserving individual `RepoRecordedInput.WithValue` entries for validation. Facts are provided to execution/replay values but are not normal replay invalidators. | Bazel 9 lockfile docs fields `bzlTransitiveDigest`, `usagesDigest`, `generatedRepoSpecs`, `moduleExtensionMetadata`; `RegularRunnableExtension`, `SingleExtensionEvalFunction`, `RepoRecordedInput`, facts docs. |
| `ModuleExtensionExecutionKey { workspace_id, extension_instance_id, command_policy_digest, replay_inputs_digest } -> ModuleExtensionResult` | Generated repo specs, metadata/facts, declared generated repo set. | Bazel `SingleExtensionValue` includes generated repos and facts; current Slug drops metadata from `ModuleExtensionResult`. Include command/Starlark semantics and repo env policy explicitly because extension evaluation can observe them. |
| `InnateExtensionKey { workspace_id, owner_module_key, bzl_label, rule_name, invocation_id }` | `use_repo_rule` as Bazel's innate extension shape, including dev-dependency and scoped visibility. | Bazel docs/source model `use_repo_rule` as an innate extension; current Slug stores flat `RepoRuleInvocation`. |
| `ExtensionSpokesKey { workspace_id, extension_instance_id } -> ExtensionSpokesValue` | Slug bridge for hub/spoke registration from lockfile replay or evaluated extension result. | Slug-specific Plan 36/38 bug shape; not Bazel terminology. |
| `ExtensionRepoExecutionKey { workspace_id, canonical_repo, repo_spec_digest, repo_rule_impl_digest, repo_replay_inputs_digest } -> RepositoryRuleResult` | Repo-rule execution from typed repo identity and canonical `RepoSpec`. | Bazel generated repo specs; current Slug `ExtensionRepoExecutionKey` is under-keyed. |
| `RepoMaterializationManifestKey { workspace_id, output_base, canonical_repo, repo_spec_digest } -> RepoLayoutManifest` | Slug DICE correctness mechanism for materialized layout validation. | This is not direct Bazel parity; Bazel uses marker files keyed by predeclared input/recorded input digests (`DigestWriter`, `RepositoryFetchFunction`). |
| `ExternalSymlinkLayoutKey { workspace_id, output_base, cell_graph_digest } -> ()` | Slug `external/`, `bazel-external/`, and buck-out layout side effects. | Slug-specific layout side effect; not a semantic dependency of resolution. |

## Values To Move Out Of Globals

Plan 61 retires these as semantic authorities:

- `MODULE_VERSIONS`
- `REGISTERED_TOOLCHAINS`
- `REGISTERED_EXECUTION_PLATFORMS`
- `LOCKFILE_CACHE`
- `EXTENSION_AGGREGATIONS`
- `SPOKE_REGISTRY`
- `SEEDED_EXTENSIONS`
- `ROOT_CELL_NAME`
- `EXTERNAL_CELL_NAMES`
- `DYNAMIC_EXTENSION_CELLS`
- `DYNAMIC_EXTENSION_CELL_SETUPS`
- `DYNAMIC_EXTENSION_CELL_ALIASES`
- `SCOPED_BZLMOD_REPO_ALIASES`
- `BZLMOD_APPARENT_ALIAS_CACHE`
- `DYNAMIC_PROJECT_ROOT`
- mutable `CellResolverInternals::dynamic_cells` insertion as a semantic path

Temporary adapters can call into DICE-backed values while call sites migrate, but
the final state must make these globals unnecessary.

## Desired Dependency Edges

- `CellResolverKey` depends on `BzlmodCellGraphKey { workspace_id,
  resolution_digest }`, not directly on pre-DICE resolution output. Grounding:
  current `CellResolverKey` consumes an injected resolver; legacy `cells.rs`
  assembles bzlmod state before DICE.
- BUILD and `.bzl` label resolution depends on `RepoMappingKey` for the owner
  mapping scope. Grounding: Bazel strict repo mapping docs and Slug's current
  `repo_mapping.rs` canonicalization paths.
- `native.module_name()`, `native.module_version()`, BUILD-file
  `module_name()`, and `module_version()` depend on `ResolvedModuleIdentity`.
  Grounding: Bazel BUILD/native globals docs and Slug `MODULE_VERSIONS` global.
- Toolchain and execution-platform registration depend on bzlmod graph
  projections. Grounding: Bazel registered toolchain/platform SkyFunctions.
- Extension repo access depends on `ExtensionSpokesKey`, then
  `ExtensionRepoExecutionKey`, then layout materialization. Grounding: current
  Slug `extension_repo.rs` access path and spoke-materialization bridge.
- Lockfile replay is an optimization edge under extension execution. A valid
  lockfile hit can provide generated repo specs/facts; stale or incomplete
  replay inputs force extension evaluation or error according to lockfile mode.
  Grounding: Bazel `SingleExtensionEvalFunction`; current Slug cache hit path
  only checks digests.

## Validation First

Plan 61 must add proof infrastructure before moving ownership:

- Baseline xfail/known-bad fixtures for root edit, local override edit, two
  workspaces in one daemon, lockfile SHA stability/mode behavior, extension
  `.bzl` edit, transitive `.bzl` edit, bad extension, unknown repo rule, stale
  marker, and no-stub failures.
- Structured events/counters: `bzlmod_resolution_compute`, `module_file_parse`,
  `extension_eval`, `extension_replay_hit`, `extension_replay_miss_reason`,
  `repo_materialization_hit`, `repo_materialization_miss_reason`,
  `lockfile_read`, `lockfile_write_attempt`, and `stub_fallback_attempt`.
- Every parity fixture records: pinned Bazel source/doc anchor, local Bazel
  output, local Slug output, and the decision for any source/doc/policy mismatch.

## Migration Phases

### 61.1 Inventory, Citations, And Guardrails

- Add typed bzlmod graph values without behavior changes.
- Document exact call sites for every global, marker trust path, path repair,
  lockfile writer, and stub fallback.
- Add the validation fixtures and counters listed above.
- Mark current tests that assert cross-workspace key equality as known-bad until
  replaced by workspace-scoped tests.

Exit criteria:

- New value types compile.
- A checked inventory links every current bzlmod semantic global to a DICE owner.
- No later phase can merge without using the structured events/counters.

### 61.2 Parsed Module Inputs As DICE Keys

- Move root/transitive `MODULE.bazel` parsing, include files, registry module
  files, local overrides, source JSON, patches/overlays, auth/offline/cache
  policy, and archive integrity behind DICE keys. Command-sensitive policy
  belongs in `BzlmodCommandPolicyKey`; stable workspace identity does not change
  just because `--lockfile_mode`, registry list, or dev-dependency flags change.
- Model create/edit/delete of root and local override `MODULE.bazel` files.
- Preserve current BCR/local override behavior while changing ownership.

Grounding: Bazel `ModuleFileValue`, `ModuleFileFunction`, `IndexRegistry`, and
`RepoSpecFunction`; current Slug source-kind registration in legacy `cells.rs`.

Exit criteria:

- Editing or creating/deleting root `MODULE.bazel`, included module files, local
  override module files, registry metadata, or `source.json` invalidates only the
  relevant parsed/resolution keys.
- Named experiment `plan61-module-input-dice-diff` compares pinned Bazel and
  Slug behavior.

### 61.3 Resolved Bzlmod Graph As A DICE Value

- Move discovery, MVS, overrides, selected/yanked versions, dev/root filtering,
  command policy, Bazel-version compatibility checks, and module metadata into
  `BzlmodResolutionKey`.
- Match pinned Bazel behavior for compatibility-level conflicts and multiple
  version override selection.
- Project toolchain/platform registrations from the graph.

Grounding: Bazel `BazelModuleResolutionFunction`, `Selection`,
`BazelDepGraphValue`, `RegisteredToolchainsFunction`, and
`RegisteredExecutionPlatformsFunction`.

Exit criteria:

- Compatibility-level conflicts fail like pinned Bazel.
- Two workspaces with the same module names but different versions/overrides in
  one daemon get distinct resolution values, module metadata, and registrations.
- Selected yanked-version behavior matches pinned Bazel or is explicitly
  classified as a remaining parity gap.

### 61.4 Shadow Cell Graph, Then Switch

- First add `BzlmodCellGraphKey` as a shadow projection and compare it against
  current startup-built cells.
- Switch `CellResolverKey` to the DICE-owned graph only after repo mapping,
  extension replay, and materialization tests cover the dynamic-cell cases.
- Do not delete dynamic/global adapters in this phase.

Grounding: Bazel does not have Slug cells; `BazelDepGraphValue` grounds
canonical repository graph semantics. Slug-specific removal is grounded in
Plan 38 and the current dynamic cell globals.

Exit criteria:

- Shadow and current cell graphs match for existing bzlmod fixtures.
- `@bazel_tools` remains an explicit bundled/well-known repo entry.
- No filesystem scan or mutable `CellResolver::get` insertion is the authority
  for canonical repo identity.

### 61.5 Canonical Identity And Repo Mapping

- Define `ResolvedModuleIdentity`, `ModuleExtensionId`, `ExtensionUniqueName`,
  `RepoOriginKind`, and `RepoMappingScope`.
- Implement module repo canonical names, multiple-version identity, `repo_name`,
  extension ids from canonical `.bzl` labels, isolated usages, generated repo
  names, `inject_repo`, `override_repo`, `use_repo`, and `use_repo_rule`.
- Use typed internal identities even where Bazel source currently performs
  prefix checks; externally preserve pinned Bazel canonical label behavior.

Grounding: Bazel external docs for canonical/apparent repo names, `ModuleKey`,
`ModuleExtensionId`, `BazelDepGraphFunction`, `ModuleExtensionUsage`,
`SingleExtensionValue`, `BazelDepGraphValue`, and
`ModuleExtensionRepoMappingEntriesFunction`.

Exit criteria:

- Tests cover single-version and multiple-version module canonical names.
- Extension-generated repos see host module visible repos plus same-extension
  generated repos with Bazel precedence. Include a conflict test where host
  visible `@foo` and generated `foo` both exist.
- `inject_repo` and `override_repo` resolve through root/module mappings,
  validate bad targets, handle positional/kwargs forms where Bazel does, and do
  not materialize overridden generated repos.
- `use_repo_rule` is modeled as an innate extension with invocation-level
  `dev_dependency` and current-module visibility.

### 61.6 DICE-Owned Extension Aggregation And Replay Inputs

- Replace `EXTENSION_AGGREGATIONS` with workspace-scoped DICE values.
- Compute real `.bzl` transitive digest from loaded extension modules; current
  extension-id-based digest is unsound.
- Match pinned Bazel `usagesDigest` serialization, including module keys, eval
  factors, isolate status, and tag order where Bazel treats order as relevant.
- Include Bazel release id, Starlark semantics, repo env, non-strict repo env,
  timeout/process/remote-executor policy, and OS/architecture eval factors in
  the command/replay inputs according to pinned Bazel source. The lockfile docs
  state the visible lockfile is Bazel-version-specific; the implementation
  still needs source and experiment checks for which other policy changes affect
  replay versus execution.
- Validate recorded inputs using Bazel `RepoRecordedInput.WithValue` semantics:
  file, directory entries, directory tree, env var, and repo mapping entries.
- Facts are passed to execution/replay values and returned in
  `ModuleExtensionResult` or a sibling DICE value, but fact contents are not
  normal replay invalidators. In lockfile `ERROR` mode, match Bazel's
  post-execution facts validation behavior.

Grounding: Bazel `SingleExtensionUsagesFunction`,
`SingleExtensionUsagesValue`, `RegularRunnableExtension`,
`SingleExtensionEvalFunction`, `RepoRecordedInput`, `ModuleExtensionContext`,
and facts docs.

Exit criteria:

- Editing an extension `.bzl`, transitive loaded `.bzl`, tag attr, env input,
  file/dir/tree input, or recorded repo mapping rejects replay or re-executes as
  pinned Bazel does.
- Legitimate empty `generatedRepoSpecs` can replay; empty specs alone are not
  treated as stale historical stub data.
- Valid lockfile replay registers generated repos without executing the
  extension.
- Ordinary read-only Slug paths do not mutate the visible lockfile.

### 61.7 Repo Execution And Materialization Authority

- Replace Slug's bare `.slug_repo_complete` trust with Bazel-shaped
  marker/recorded-input validation or an equivalent Slug DICE manifest. A Slug
  output-tree manifest is a stronger internal correctness mechanism, not direct
  Bazel parity unless a Bazel experiment proves output-content validation.
- Repository rules do not declare outputs like normal actions. Correctness must
  come from recording `repository_ctx` side effects plus final tree digest, or
  from scanning an isolated repo output tree after execution.
- Track file digest, executable bit, directory entries, symlink target text,
  deleted/extra paths, repo-rule implementation `.bzl` transitive digest, and
  repo-rule recorded env/file/dir/repo-mapping/PATH inputs.
- Remove stale absolute-path repair, generated BUILD repair, empty-target label
  repair, and stub repo fallback from normal paths.

Grounding: Bazel `DigestWriter`, `RepositoryFetchFunction`,
`RepoRecordedInput`, `StarlarkBaseExternalContext`, and current Slug marker,
repair, and stub paths in `repository_execution.rs`, `repository_executor.rs`,
and `extension_repo.rs`.

Exit criteria:

- A valid marker with a deleted/corrupted file, changed mode, stale symlink, or
  stale recorded input rematerializes or fails directly.
- Unknown repo rule, repo-rule failure, extension failure, DICE error, missing
  generated repo, and lazy Label materialization failure create no stub repo, no
  `.slug_repo_complete` stub marker, and no downstream "No such file" masking.
- `rg "Creating stub|materialize_stub_repo|stub_marker"` has no normal bzlmod
  path remaining.
- `module_ctx.path(Label)` / `module_ctx.read(Label)` and
  `repository_ctx.path(Label)` / `repository_ctx.read(Label)` plus
  Label-accepting operations such as `symlink`, `template`, `patch`,
  `load_wasm`, and `execute_wasm` propagate materialization failure. Plain
  `execute()` has string args; tests should cover environment, `PATH`, and
  working-directory recording separately rather than claiming `execute(Label)`.
- Symlink tests cover relative symlink, absolute in-workspace symlink, absolute
  out-of-workspace symlink, broken symlink, Windows fallback, and non-symlink
  collision in `buck-out/v2/external_cells`.
- Missing absolute local paths fail; no suffix repair; local source content
  changes invalidate; cache corruption is detected or discarded deterministically.

### 61.8 Facts And Lockfile Policy

- Implement mode-aware lockfile semantics: `update`, `refresh`, `error`, and
  `off` are separate policy inputs. Until exact Bazel write parity exists,
  ordinary Slug build/query/audit paths use the interim read-only safety policy.
- Replace process-wide `cached_lockfile` with `LockfileContentKey` values keyed
  by workspace/hidden path, existence, digest, and parse result. The consuming
  replay/resolution keys are mode-aware; in `off` mode they must not read or
  update either lockfile, matching Bazel 9 docs.
- Read workspace/hidden lockfile facts where Bazel does; expose
  `module_ctx.facts`; accept and carry `extension_metadata(facts = ...)`.
- Persist newly returned facts/specs only through an explicit future
  lockfile-update path with a write capability such as
  `LockfileWritePurpose::ExplicitModUpdate`, or a Slug-owned cache documented as
  non-Bazel-visible. Ordinary paths cannot call `Lockfile::write`.

Grounding: Bazel `RepositoryOptions`, `BazelLockFileModule`,
`BazelLockFileFunction`, `SingleExtensionEvalFunction`,
`ModuleExtensionContext`, `ModuleExtensionMetadata`, `Facts`, and Slug Plan 57.

Exit criteria:

- Named experiment `plan61-lockfile-modes` records pinned Bazel and Slug SHA
  behavior for default/update, refresh, error, and off.
- Creating, deleting, and editing `MODULE.bazel.lock` after a prior daemon
  miss/hit invalidates the DICE lockfile value.
- `module_ctx.facts` and returned metadata survive through DICE values even when
  ordinary commands do not persist them.

### 61.9 Delete Transitional APIs

- Delete or quarantine bzlmod globals, marker trust paths, stub repo fallbacks,
  path repair heuristics, and manual cache invalidation hooks.
- Keep only compatibility readers needed for old test fixtures if they do not
  affect normal Bazel 9 behavior.

Exit criteria:

- `rg` for the global names above finds no semantic use in normal paths.
- Stub repo materialization is impossible for bzlmod extension/repo failures.

### 61.10 Real-World And Warm-Daemon Validation

- Add focused Rust unit tests for every new key/value.
- Add Python integration tests for daemon invalidation and lockfile policy.
- Add real-world smoke coverage for rules_cc, rules_python, rules_rs/rules_rust,
  bazel_features, and a bounded zeromatter target.
- Run named experiment `plan61-warm-daemon-noop`: instrument warm query/build to
  assert no unchanged module resolution, extension evaluation, or repo
  materialization reruns.

Exit criteria:

- Tests prove warm daemon reuse without stale cross-workspace state.
- Real-world smokes stay usable throughout the migration.

## Acceptance Criteria

| Claim | Grounding / validation |
|---|---|
| Editing root `MODULE.bazel` invalidates the resolved graph and cell graph. | Bazel `BazelModuleResolutionFunction` depends on root `ModuleFileValue`; current Slug parses before DICE in legacy `cells.rs`; experiment `plan61-module-input-dice-diff`. |
| Editing or creating/deleting a local override's `MODULE.bazel` invalidates only affected bzlmod graph nodes. | Bazel non-registry override loading in `ModuleFileFunction`; current Slug local override resolution in legacy `cells.rs`; experiment `plan61-local-override-dice-diff`. |
| Registry metadata/source JSON/integrity changes are modeled as DICE inputs. | Bazel lockfile docs and `IndexRegistry` / `RepoSpecFunction`; experiment mutates local registry metadata and `source.json`. |
| MVS compatibility conflicts fail like pinned Bazel. | Bazel `Selection`; experiment with conflicting compatibility levels and multiple-version override. |
| Extension `.bzl` changes invalidate or reject cached extension replay. | Bazel `RegularRunnableExtension` / `SingleExtensionEvalFunction`; experiment edits implementation and transitive helper `.bzl`. |
| Tag usage changes update `usagesDigest` and invalidate/reject replay. | Bazel `SingleExtensionUsagesValue`; experiment mutates attrs and tag order where pinned Bazel treats order as relevant. |
| Recorded env/file/dir/tree/repo-mapping input mismatches reject replay. | Bazel `RepoRecordedInput`, `StarlarkBaseExternalContext`, `SingleExtensionEvalFunction`; experiment mutates each input class. |
| Valid lockfile `generatedRepoSpecs` can register and materialize generated repos without executing the extension. | Bazel `SingleExtensionEvalFunction` lockfile hit path; Slug Plan 38 current lockfile preseed bridge. |
| Missing lockfile entries execute the extension once in DICE, then reuse DICE state inside the daemon. | Bazel lockfile miss execution path; Slug DICE counter `extension_eval` proves one execution on warm daemon. |
| Label-taking `module_ctx` / `repository_ctx` operations materialize needed extension repos or fail directly. | Bazel `ModuleExtensionContext` extends `StarlarkBaseExternalContext`; source shows `path`, `read`, `watch`, `symlink`, `template`, `patch`, `load_wasm`, and `execute_wasm` accept labels, while plain `execute()` records env/PATH/working-directory effects separately. Plan 36 bug shape; experiments split path/read/watch/write-like operations. |
| Unknown repo rule, extension failure, repo-rule failure, missing generated repo, and invalid override fail directly with no stub repo. | Bazel `SingleExtensionFunction`, `RepoDefinitionFunction`, `RepositoryFetchFunction`; current Slug stub paths; no-stub tests assert no repo directory/marker. |
| Repo materialization reuse is not based on bare `.slug_repo_complete`. | Bazel marker/recorded-input semantics or Slug manifest experiment; current Slug marker bug shape in Plan 38 and source. |
| Two workspaces in one daemon do not share bzlmod state. | Current process globals and under-keyed tests are known-bad; experiment uses identical extension/repo names with different workspace roots and lockfiles. |
| Ordinary Slug read-only paths do not mutate `MODULE.bazel.lock`; future write path matches Bazel modes. | Bazel `RepositoryOptions` / `BazelLockFileModule`; Slug policy is explicit interim safety, validated by `plan61-lockfile-modes`. |

## Test Matrix

| Area | Tests / grounding |
|---|---|
| Lockfile policy | Valid lockfile cold/warm repo, stale digest, stale recorded input, failed build, interrupted materialization, create/delete/edit after prior daemon miss/hit; grounded in Bazel lockfile docs/source and Plan 57. |
| DICE invalidation | Root module edit, include edit, local override edit, registry metadata edit, extension `.bzl` edit, tag attr edit, recorded input edit, facts availability on re-execution but not facts invalidation; grounded in Bazel `SingleExtensionEvalFunction` facts behavior. |
| Extension execution | Simple extension hub/spoke generation, valid replay hit, miss execution, no eager full-closure materialization unless pinned Bazel does the same; grounded in Bazel Skyframe extension execution and Plan 36 eager-materialization correction. |
| Label materialization | Separate tests for `path`, `read`, `watch`, `symlink`, `template`, `patch`, `load_wasm`, and `execute_wasm`; plain `execute()` tests cover env, `PATH`, and working-directory invalidation, not `execute(Label)`. Grounded in `ModuleExtensionContext`, `StarlarkBaseExternalContext`, and Plan 36. |
| Repo mapping | Apparent/canonical names, module repo identity, multiple-version identity, same-extension internal repos, `use_repo`, `inject_repo`, `override_repo`, `use_repo_rule`, isolated extensions; grounded in Bazel docs/source listed above. |
| Failure behavior | Bad extension, unknown repo rule, wrong symbol type, repo-rule failure, invalid repo spec, stale marker, missing generated target, invalid override; grounded in Bazel failure sources and current Slug stub fallbacks. |
| Materialization | Stale marker with changed `RepoSpec`, changed content with same marker, deleted file, corrupted file, changed mode, stale/broken symlink, local path missing, cache corruption; grounded in current Slug success-by-marker paths and Bazel marker docs. |
| Daemon isolation | Two workspaces with different module graphs, same extension id, same generated repo names, and different lockfiles/facts in one daemon; grounded in current global registry bug shape. |
| Real-world | rules_cc, rules_python, rules_rs/rules_rust, bazel_features, bounded zeromatter target; grounded in Plans 02, 36, and 57. |
| Performance guardrail | Named experiment `plan61-warm-daemon-noop` compares trace counters before/after a no-op warm invocation. |

## Out Of Scope

- Remote execution performance.
- Full sandbox strategy.
- Bazel 8 lockfile compatibility.
- WORKSPACE support.
- New language-rule parity outside failures directly exposed by bzlmod.
- Exact visible-lockfile writes from ordinary Slug builds before a
  mode-aware explicit lockfile write path exists.

## Source Of Truth And Validation Matrix

### Citation Index

Use these concrete anchors when implementing or updating line numbers. The local
Bazel checkout is a Bazel-9-era checkout; before implementation, pin the exact
Bazel 9 release tag and refresh line numbers.

| Topic | Bazel source/docs anchors | Current Slug / plan anchors |
|---|---|---|
| Current official Bazel 9 docs | `https://bazel.build/versions/9.0.0/external/lockfile` (checked 2026-05-18; page last updated 2026-05-07); `https://bazel.build/versions/9.0.0/external/module`; `https://bazel.build/versions/9.0.0/external/extension`; `https://bazel.build/versions/9.0.0/external/overview` | Use these only as versioned behavioral docs; still validate exact behavior with pinned source and local Bazel experiments. |
| Module parsing inputs | `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/bazel/bzlmod/ModuleFileValue.java`; `ModuleFileFunction.java`; `/var/mnt/dev/bazel/site/en/external/module.md` | `app/slug_common/src/legacy_configs/cells.rs`; `app/slug_bzlmod/src/parser.rs` |
| Resolution, MVS, yanked versions | `BazelModuleResolutionFunction.java`; `Selection.java`; `BazelDepGraphValue.java`; `/var/mnt/dev/bazel/site/en/external/module.md`; `/var/mnt/dev/bazel/site/en/external/lockfile.md` | `app/slug_bzlmod/src/resolution.rs`; legacy `cells.rs` |
| Canonical/apparent repo identity | `ModuleKey.java`; `BazelDepGraphFunction.java`; `ModuleExtensionId.java`; `ModuleExtensionUsage.java`; `ModuleExtensionRepoMappingEntriesFunction.java`; `/var/mnt/dev/bazel/site/en/external/overview.md`; `/var/mnt/dev/bazel/site/en/external/extension.md` | `app/slug_bzlmod/src/repo_mapping.rs`; `pending_repo_cells.rs`; `extension_execution_dice.rs` |
| Extension aggregation/replay | `SingleExtensionUsagesFunction.java`; `SingleExtensionUsagesValue.java`; `RegularRunnableExtension.java`; `SingleExtensionEvalFunction.java`; `SingleExtensionValue.java` | `app/slug_bzlmod/src/extensions.rs`; `extension_execution_dice.rs`; Plan 10 |
| Recorded inputs | `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/rules/repository/RepoRecordedInput.java`; `StarlarkBaseExternalContext.java` | `app/slug_bzlmod/src/lockfile.rs`; `app/slug_interpreter_for_build/src/module_ctx`; `repository_ctx.rs` |
| Repo materialization and markers | `DigestWriter.java`; `RepositoryFetchFunction.java`; `RepoDefinitionFunction.java`; `RepositoryDirectoryValue.java` | `app/slug_bzlmod/src/repository_execution.rs`; `repository_executor.rs`; `app/slug_external_cells/src/extension_repo.rs`; Plan 38 |
| Lockfile modes and writes | `RepositoryOptions.java`; `BazelLockFileModule.java`; `BazelLockFileFunction.java`; `SingleExtensionEvalFunction.java` | Plan 57 lockfile safety policy; `app/slug_bzlmod/src/lockfile.rs`; client `--lockfile_mode` parsing |
| Facts | `ModuleExtensionContext.java`; `ModuleExtensionMetadata.java`; `Facts.java`; `FactsAdapter.java`; `SingleExtensionEvalFunction.java`; `/var/mnt/dev/bazel/site/en/external/extension.md` | Plan 57 facts reuse; `module_ctx/methods.rs`; `module_extension_executor_impl.rs`; `extension_execution_dice.rs` |
| Toolchain/platform registration | `ModuleFileGlobals.java`; `RegisteredToolchainsFunction.java`; `RegisteredExecutionPlatformsFunction.java` | `app/slug_bzlmod/src/lib.rs`; legacy `cells.rs` |
| Slug-specific cell graph/layout | Bazel grounding is only canonical repository graph semantics, not Slug cells | `app/slug_core/src/cells.rs`; `app/slug_common/src/dice/cells.rs`; legacy `cells.rs`; Plans 36/38/61 |

| Structural claim | Bazel source/docs | Slug source/plans | Required experiment |
|---|---|---|---|
| Slug implements Bazel-9 bzlmod-only external dependency policy with no WORKSPACE fallback. | Project policy is `AGENTS.md`; pinned Bazel 9 docs/source and a local experiment must classify exact WORKSPACE behavior before claiming wording-level Bazel parity. | `AGENTS.md`. | Fixture with only `WORKSPACE`; record exact Bazel 9 and Slug behavior, including whether Bazel ignores, warns, or errors. |
| Module parsing and resolution are graph-owned. | `ModuleFileValue`, `ModuleFileFunction`, `BazelModuleResolutionFunction`, `Selection`. | Legacy `cells.rs` pre-DICE parse/resolution. | `plan61-module-input-dice-diff`. |
| Lockfile replay uses bzl digest, usages digest, recorded inputs, generated repo specs, metadata/facts, hidden/workspace lockfile lookup, and lockfile modes. | Bazel 9 lockfile docs; `BazelLockFileFunction`, `BazelLockFileValue.KEY` / `HIDDEN_KEY`, `SingleExtensionEvalFunction`, `LockFileModuleExtension`, `RegularRunnableExtension`, `RepoRecordedInput`. | Plan 57; current `lockfile.rs` digest-only validation. | `plan61-lockfile-modes` and recorded-input mutation fixtures. |
| Canonical identity and repo mapping are producer-owned. | External repo docs, `ModuleKey`, `ModuleExtensionId`, `BazelDepGraphFunction`, `ModuleExtensionUsage`, `ModuleExtensionRepoMappingEntriesFunction`. | Current Slug string/fallback alias paths. | Printed canonical label fixtures for module, extension, innate, isolated, inject/override cases. |
| Repo materialization is not bare marker trust. | Bazel `DigestWriter`, `RepositoryFetchFunction`, `RepoRecordedInput`; note Bazel marker semantics rather than output-manifest parity. | Plan 38 and current `.slug_repo_complete` / stub paths. | Stale marker and output mutation fixtures. |
| Facts are JSON-like metadata, not normal replay invalidators. | `ModuleExtensionContext`, `ModuleExtensionMetadata`, `Facts`, `SingleExtensionEvalFunction`. | Plan 57 and current metadata drop in `extension_execution_dice.rs`. | Facts reuse fixture plus `--lockfile_mode=error` facts validation fixture. |

A Plan 61 implementation claim is accepted only when the row has pinned Bazel
source/docs, local Bazel output, local Slug output, and an explicit decision for
any source/doc/policy mismatch. "Slug already works on this fixture" is not
evidence unless the matching Bazel 9 source or executable behavior is cited.
