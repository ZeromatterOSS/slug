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

## Current Evidence

Keep these logs as evidence for the SDK parity frontier reached by the
transitional implementation. They do not prove Plan 61 completion.

- Slug full SDK build log:
  `/tmp/slug-plan61/plan61-sdk-contents-after-cc-all-files-20260520-150740.log`.
- Bazel/Slug mode manifests:
  `/tmp/slug-plan61/bazel-sdk-contents-modes-after-cc-all-files.txt` and
  `/tmp/slug-plan61/slug-sdk-contents-modes-after-cc-all-files.txt`.
- Bazel/Slug SHA manifests:
  `/tmp/slug-plan61/bazel-sdk-contents-sha-after-cc-all-files.txt` and
  `/tmp/slug-plan61/slug-sdk-contents-sha-after-cc-all-files.txt`.
- Fresh post-lease Slug full SDK build log:
  `/tmp/slug-plan61/plan61-sdk-after-execroot-lease-20260522-151708.log`.
- Fresh Bazel 9.0.1 full SDK build log:
  `/tmp/slug-plan61/bazel-sdk-contents-after-execroot-lease-20260522-155820.log`.
- Bazel/Slug post-lease mode manifests:
  `/tmp/slug-plan61/bazel-sdk-contents-modes-after-execroot-lease.txt` and
  `/tmp/slug-plan61/slug-sdk-contents-modes-after-execroot-lease.txt`.
- Bazel/Slug post-lease SHA manifests:
  `/tmp/slug-plan61/bazel-sdk-contents-sha-after-execroot-lease.txt` and
  `/tmp/slug-plan61/slug-sdk-contents-sha-after-execroot-lease.txt`.
- Final warm-audit logs:
  `/tmp/slug-plan61/plan61-audit-cell-final-20260522-160543.log`,
  `/tmp/slug-plan61/plan61-audit-cell-final-20260522-160543-warm.out`, and
  `/tmp/slug-plan61/plan61-audit-cell-final-20260522-160543-counters.json`.
- Post bridge-cache-removal Slug full SDK build logs:
  `/tmp/slug-plan61/plan61-cache-removal-20260522-120100.log` and
  `/tmp/slug-plan61/plan61-cache-removal-20260522-120100-retry1.log`.
- Post bridge-cache-removal Slug manifests:
  `/tmp/slug-plan61/slug-sdk-contents-modes-after-cache-removal.txt` and
  `/tmp/slug-plan61/slug-sdk-contents-sha-after-cache-removal.txt`.

Observed SDK result at the checkpoint:

- Slug and Bazel 9.0.1 both build `//sdk:sdk_contents`.
- Directory/file manifests and modes match.
- All non-ELF file hashes match.
- Remaining accepted differences are four ELF outputs:
  `bin/zm`, `bin/zerobuf`, `bin/zerosystem`, and
  `lib/libzeromatter_ffi.so`.
- The accepted ELF difference class is output-root strings embedded in
  ELF/debug/build metadata (`buck-out` or future `slug-out` versus Bazel's
  `bazel-out`). Exact-byte parity remains separate design work, not a bzlmod
  replay-completeness signal.
- After removing the process-global bridge cache, a first 900s Slug smoke
  timed out while still making action-execution progress. Reusing the same
  isolation tree with a 2700s bound completed `//sdk:sdk_contents` in 31m36s
  with 8,360 local commands and peak RSS about 6.5 GiB. Comparing the new Slug
  manifests against the existing Bazel 9.0.1 post-lease manifests showed exact
  mode parity, matching non-ELF hashes, and the same four accepted ELF hash
  differences listed above.
- `LegacyBzlmodResolutionDiceKey` now includes hidden lockfile identity in
  equality and hashing, matching the existing visible-lockfile bridge identity.
  A focused Rust regression test covers this key property, and the hidden
  lockfile Python guardrail subset plus the full Plan 61 guardrail file pass.
- Hidden-lockfile replay now has a same-daemon edit guardrail: a generated repo
  first replays from the daemon hidden lockfile, then editing that hidden
  lockfile removes the cached extension entry and forces the extension to run
  and fail instead of reusing stale replay state.
- Hidden-lockfile facts now have same-daemon create/edit/delete coverage: an
  extension reads `module_ctx.facts` from the daemon hidden lockfile, succeeds
  when the hidden facts are created with the expected value, fails after an
  edit to stale facts, succeeds after restoration, and fails again after the
  hidden lockfile is deleted.
- Best-effort extension `.bzl` digests now include existing external
  repository load files materialized under `bazel-external/<repo>` in addition
  to project-local literal loads, and resolve apparent external loads through
  the caller's available `RepoMappingSnapshot`. Focused Rust coverage and a
  Plan 61 Python same-daemon build guardrail cover this transitional digest
  behavior: a replayed generated repo first hits the lockfile, then editing the
  mapped external helper loaded through an apparent repo alias rejects replay
  and runs the edited extension implementation.
- Best-effort extension `.bzl` digests also include missing project-local load
  paths as read-error state, so creating a helper that had been absent when the
  lockfile digest was written rejects stale replay instead of silently trusting
  the old generated repo spec.
- The same best-effort digest path now has explicit same-daemon coverage for
  deletion of a previously loaded mapped external helper file.
- Root `bazel_dep(..., dev_dependency = True)` now participates in normal
  resolution by default for both local overrides and registry-backed modules,
  and `--ignore_dev_dependency` removes those root dev dependencies from the
  command's bzlmod graph. Local Bazel 9.1.0 evidence showed the same root dev
  dependency builds by default and disappears with `--ignore_dev_dependency`.
- Root `use_repo_rule(..., dev_dependency = True)` now carries the
  `dev_dependency` bit into repo-rule invocations, participates by default, and
  is excluded under `--ignore_dev_dependency`; non-root dev repo rules are
  filtered from precomputed and eager repo-rule registration paths.
- Root `use_extension(..., dev_dependency = True)` now participates by default
  and is excluded from extension aggregation, precomputed repo cells, and
  lockfile replay under `--ignore_dev_dependency`.
- Root `inject_repo()` now accepts Bazel's keyword alias form and feeds the
  injected apparent-to-root repo mapping into extension generated-repo mappings
  and the transitional DICE session replay key. Bazel source anchor:
  `ModuleFileGlobals.injectRepo` records `mustExist = false`, and
  `ModuleThreadContext.buildUsage` validates the root-visible repo before
  storing the override. Local Bazel 9.1.0 repros showed
  `inject_repo(ext, injected_helper = "helper")` makes
  `@injected_helper` visible from the generated repo, while omitting the
  directive fails with "No repository visible as '@injected_helper' from
  repository '@@+ext+generated'"; Slug now has a same-daemon guardrail for the
  keyword alias and repo-mapping replay transition.
- Root `inject_repo()`/`override_repo()` rows are now omitted from generated
  repo mappings under `--ignore_dev_dependency`, matching Bazel's
  `ModuleFileGlobals` early return for those calls. A focused guardrail verifies
  that an injected helper repo is available by default and invisible with
  `--ignore_dev_dependency`.
- Root `inject_repo()`/`override_repo()` directive validation now matches the
  Bazel `ModuleExtensionUsageBuilder` shape for this slice: duplicate
  generated/injected repo names on one extension usage fail across both
  directives and across same-extension proxies, and `use_repo()` cannot import
  a repo that was injected with `mustExist = false`. Focused `slug_bzlmod`
  parser regressions cover duplicate override, override-vs-inject, and
  injected-use_repo cases.
- `override_repo(ext, "repo")` now accepts Bazel's positional same-name
  shorthand in addition to keyword mappings. A focused guardrail verifies that a
  generated repo's sibling mapping resolves `@repo` to the same-named root
  module replacement.
- `use_extension(..., isolate = True)` has been Bazel-grounded as a larger
  blocker, not a safe small patch. Bazel 9.0.1 rejects it unless
  `--experimental_isolated_extension_usages` is set; with the flag, each
  isolated usage evaluates separately, generated repo names include the
  isolation key, and `module_ctx.is_isolated` is true. Slug currently lacks the
  exported proxy variable name needed for Bazel's `IsolationKey`, aggregates by
  extension id only, names generated repos without the isolation component, and
  hard-codes `module_ctx.is_isolated` false. Until Slug implements that
  experimental mode, `use_extension(isolate = True)` now fails instead of
  silently running with non-isolated semantics.
- Registry selection for `single_version_override(registry = ...)` and
  `multiple_version_override(registry = ...)` now follows Bazel's
  `RegistryOverride.getRegistry()` behavior for module discovery, yanked
  metadata, source fetching, and lockfile registry-file validation. Bazel source
  anchors: `ModuleFileFunction` restricts registry lookup to the override
  registry when non-empty, and `RegistryOverride` is implemented by both single
  and multiple version overrides. Focused Plan 61 guardrails prove both
  directives select a cached override registry instead of the default registry.
  Override patch fields remain blocked: Bazel validates main-repo patch labels,
  applies `single_version_override` patches to the discovered `MODULE.bazel`,
  and appends the same patches to the final repo spec; non-registry
  `archive_override`/`git_override` patches also affect repository
  materialization. Slug now fails loudly when override `patches = [...]` are
  present instead of silently ignoring part of Bazel's behavior. Full support
  still needs DICE-tracked patch-file inputs plus repository materialization
  patch identity.
- Root `register_toolchains(..., dev_dependency = True)` and
  `register_execution_platforms(..., dev_dependency = True)` are now filtered
  under `--ignore_dev_dependency`, while non-root dev registrations remain
  skipped. Focused `slug_common` unit coverage verifies the collection policy.
- Non-root `use_repo_rule(..., dev_dependency = True)` and
  `use_extension(..., dev_dependency = True)` now have explicit negative
  guardrails proving the generated repos stay unavailable both by default and
  under `--ignore_dev_dependency`. The full Plan 61 guardrail file passed with
  50 tests after adding this coverage.
- The command-policy digest feeding the transitional bzlmod resolution key is
  now produced through `BzlmodCommandPolicyKey`, and the hidden lockfile path is
  part of policy equality/hashing. Focused `slug_common` coverage verifies
  hidden-lockfile policy identity, and the full 50-test Plan 61 guardrail target
  passed after the change.
- The bzlmod config-loading path no longer re-reads the interpreter
  build-config repo-env process global. `ServerCommandContext` stores the
  effective command repo environment computed from request flags and workspace
  root, then threads that value through config overrides as
  `bzlmod.repo_env_json`; bzlmod resolution/replay digests consume that
  explicit option. Focused `slug_common` coverage verifies repo-env option
  parsing, and the full 50-test Plan 61 guardrail target passed after the
  earlier config-key change; later slices moved the module and repository
  Starlark runtime APIs to explicit context/key inputs.
- `module_ctx.getenv()` and `module_ctx.os.environ` now read the effective repo
  environment from `ModuleContext`, seeded by `ModuleExtensionExecutionKey`'s
  command repo-env value, instead of consulting the interpreter build-config
  global at extension execution time.
- `repository_ctx.getenv()` and `repository_ctx.os.environ` now read the
  effective command repo-env from `RepositoryContext`. Extension repo execution
  keys carry that repo-env, and generated-repo completion markers use a
  repo-env-aware execution identity so a changed `--repo_env` cannot reuse stale
  materialized repository output by marker existence alone.
- The seeded-extension process global was removed. Lazy spoke registration now
  relies on DICE replay/compute when the extension repo file-ops path needs
  sibling repos, instead of a cross-command seeded marker. Focused
  `slug_bzlmod` spoke-materialization tests and the full 50-test Plan 61
  guardrail target passed after the change.
- The extension spoke process-global registry was removed. Synchronous
  `module_ctx.path(Label(...))` spoke materialization now uses the active
  extension DICE context to find the owning extension result and then computes
  the repository execution key directly. Focused spoke-materialization tests,
  affected-crate checks, `cargo build -p slug`, and the full 50-test Plan 61
  guardrail target passed after the change.
- Extension spoke lookup now goes through `ExtensionSpokesKey`, a DICE-owned
  value for one module extension's generated repo specs, canonical names, spec
  hashes, and serialized repo specs. Runtime materialization still registers
  temporary dynamic cells as transitional output plumbing, but sibling lookup
  no longer scans extension aggregations by extension name alone.
- Runtime extension-repo materialization and synchronous
  `module_ctx.path(Label(...))` spoke materialization now use
  `ExtensionSpokesByExtensionIdKey` and `ExtensionSpokesByCanonicalRepoKey`
  lookup keys. The sync bridge carries the active workspace identity, so
  materialization plumbing no longer reads injected `BzlmodSessionData`
  directly to find sibling spokes.
- Extension repo file-ops spoke lookup now obtains its `WorkspaceId` from
  `BzlmodExtensionAggregationsDataKey` and forms
  `ExtensionSpokesByExtensionIdKey::for_workspace_id`; it no longer derives
  spoke lookup identity from the project root. Focused coverage uses a
  non-default output base to prove the injected identity is preserved, and the
  full `slug_bzlmod`, `slug_external_cells`, and Plan 61 Python guardrails
  passed after the change.
- Extension repo execution and `RepoMaterializationManifestKey` now have
  explicit `WorkspaceId` constructors. Runtime extension repo file-ops and
  synchronous spoke materialization use the session/active workspace identity,
  so repository materialization keys preserve a non-default output base instead
  of silently rebuilding identity from the project root.
- Extension spoke lookup keys now depend on `ExtensionBzlTransitiveDigestKey`
  before forming the `ExtensionSpokesKey`. The digest key still recomputes the
  transitional best-effort `.bzl` scanner every transaction, but successful
  spoke lookup values can now cut off only after the scanned digest has been
  refreshed.
- `ExtensionSpokesKey` now passes its already-keyed `.bzl` digest into the
  module-extension execution key instead of rebuilding the execution key with a
  second direct scanner read and mutating the digest afterward.
- Registered toolchain and execution platform consumers now read
  `RegisteredToolchainsKey` and `RegisteredExecutionPlatformsKey` instead of
  directly reading those fields from injected `BzlmodSessionData`. The keys are
  still transitional producers over the injected session graph, but analysis
  and execution-platform selection now depend on named DICE values.
- Extension replay and generated-repo spoke lookup now read
  `BzlmodExtensionAggregationsDataKey`. The narrower injected value carries
  extension aggregations, root module name, and workspace identity.
  `BzlmodExtensionSessionDataKey` was removed, so extension consumers no longer
  depend on unrelated registered toolchain/platform, module-version,
  repo-mapping, or replay-input session fields.
- Extension repo mappings and root override rows now read through
  workspace-checked `BzlmodRepoMappingsKey`. The underlying repo-mapping data
  is still injected from the legacy resolver, but extension replay,
  generated-repo spoke lookup, and module-version invalidation now depend on
  the named repo-mapping projection rather than bundling repo mapping state
  with extension aggregation state.
- Extension replay inputs were split out of extension aggregation state, and
  the temporary replay-data wrapper was later replaced by named
  `BzlmodLockfileInputsKey` and `BzlmodRepoEnvKey` projections. Lockfile paths,
  tracked visible/hidden lockfile contents and digests, lockfile mode, and
  command repo env are no longer bundled with extension aggregation state.
- Interpreter module-version lookup now reads `ModuleVersionsKey` instead of
  directly reading injected `BzlmodSessionData`. This is still a transitional
  producer over the injected session graph, but the Starlark interpreter adapter
  no longer consumes the injected session value directly. The key uses normal
  DICE value equality over the module-version map plus the transitional session
  invalidation identity, so warm no-op cutoffs are allowed only when lockfile,
  repo-env, registry/yanked-version, and repo-mapping bridge inputs still match.
- Module-version, registered-toolchain, and registered-execution-platform
  consumers now obtain the current workspace identity from their narrower
  injected DICE data and then compute the corresponding keyed value. These
  consumers no longer derive key identity from the IO project root, and focused
  coverage uses a non-default output base to prove the session workspace
  identity is preserved.
- `ModuleVersionsKey` now computes a narrower
  `BzlmodModuleVersionsDataKey` instead of the whole `BzlmodSessionDataKey`.
  The injected module-version value carries a conservative invalidation
  identity for lockfile contents/digests, lockfile mode, repo env, registry and
  yanked-version facts, and repo mappings, so warm no-op builds can still reuse
  DICE state without losing hidden-lockfile/facts invalidation.
- The monolithic injected `BzlmodSessionDataKey` was removed after the
  module-version split left it with no live consumers. `BzlmodSessionData`
  remains as the legacy resolver payload for populating the narrower injected
  DICE values, but the DICE graph no longer exposes that payload as a direct
  computed dependency.
- `use_repo_rule()` materialization is no longer replayed as a legacy
  resolution side effect. The existing precomputed `RepoSpec` extension-cell
  path now owns both builtin and Starlark repo-rule invocations, so repository
  contents are materialized through the DICE repository execution path when the
  generated repo is accessed.
- Extension repo file-ops access for known `RepoSpec` cells now validates
  through `ExtensionRepoExecutionKey` and its
  `RepoMaterializationManifestKey` dependency instead of short-circuiting on
  duplicated marker checks in `slug_external_cells`. Unknown-spec extension
  replay still uses the existing lazy extension-spoke lookup before it can form
  a repository execution key.
- The server no longer rewrites `BzlmodSessionData.repo_env` from process
  global build config after resolution. The injected session data now keeps the
  same explicit `bzlmod.repo_env_json` value that fed the transitional
  resolution key.
- The transitional `.bzl` replay digest now preserves Bazel canonical external
  repo names ending in `+` when resolving existing files under
  `bazel-external/<repo>`, so edits to files under directories such as
  `bazel-external/rules_python+/...` change the replay digest.
- The transitional `.bzl` replay digest now applies repo mappings at literal
  `load()` sites when a `RepoMappingSnapshot` is available to the caller, so an
  extension implementation load such as `@apparent_helper//:helper.bzl` can be
  hashed from the mapped canonical repository under `bazel-external/` instead
  of the apparent name.
- Dynamic generated-repo suffix scan caches are now cleared by the same
  bzlmod-root reset path that clears dynamic cells, setups, apparent aliases,
  and scoped aliases, preventing stale suffix lookups from surviving a fresh
  root reset.
- Starlark repository rules now record `repository_ctx.watch()` and
  `repository_ctx.watch_tree()` inputs into a materialization sidecar, validate
  that sidecar before reusing extension repo completion markers, and add DICE
  filesystem reads for watched root files and watched directory trees during
  repository execution. New same-daemon guardrails edit a watched root file and
  a nested file under a watched tree, proving the generated repository is
  re-executed rather than serving stale materialization.
- Extension repository marker/layout/recorded-input reuse is now represented by
  a typed `RepoMaterializationManifestValue` derived from
  `RepoMaterializationManifestKey`, and `ExtensionRepoExecutionKey` hashes that
  manifest value instead of an anonymous materialized-state string. This keeps
  the existing bounded marker/layout behavior while naming the current
  materialized tree identity as the next DICE-owned migration surface.
- Extension repository execution now consumes `RepoMaterializationManifestKey`
  through DICE instead of recomputing the manifest helper directly inside
  `ExtensionRepoExecutionKey::compute`. The key carries the repo spec needed for
  the same marker/layout/recorded-input classification. Marker state, layout
  state, and recorded-input state now compute as separate child DICE keys, so
  the manifest value itself can remain valid while those child dependencies
  explain marker/layout/input-state changes. These child keys still poll disk
  because the tracked project-file APIs currently live in `slug_common`, which
  already depends on `slug_bzlmod`; moving them to lower-level filesystem keys
  remains required for final replay completeness. Focused Rust coverage proves
  execution follows the named manifest key and that marker child-state changes
  invalidate the manifest across DICE transactions.
- `module(bazel_compatibility = [...])` is no longer parsed-and-ignored.
  Slug now validates the declared constraints against its Bazel 9.0.1
  compatibility target and fails incompatible modules. Local Bazel 9.1.0
  evidence showed `bazel_compatibility = [">=99.0.0"]` fails during main
  repository mapping with `Bazel compatibility check failed`, and the new Plan
  61 guardrail covers the same negative class in Slug.
- `bazel_dep(max_compatibility_level = ...)` is explicitly grounded as a Bazel
  9 no-op rather than an unknown gap. Local Bazel 9.1.0 evidence showed the
  directive only warns that the attribute is a no-op and still builds a local
  override dependency; the new Slug guardrail preserves that accepted behavior.
- Registered toolchain loading no longer uses a process-global "loaded once"
  flag that can mask same-daemon bzlmod registration changes. The temporary
  global registry is still process state, but the fast path is now keyed by the
  DICE-derived project root plus registered-toolchain list and clears/reloads
  when that signature changes.
- Root and included `MODULE.bazel` reads now run through a `slug_common` DICE
  key backed by `DiceFileComputations::read_project_file_if_exists` for the
  root module and `DiceFileComputations::read_project_file` for included
  segments. The parser keeps `RootModuleFileValue`,
  `ParsedModuleFileWithInputs`, and input digest helpers in `slug_bzlmod`, but
  include recursion is driven by `ModuleFileParseSession` so the caller owns
  file reads. The old non-cacheable `slug_bzlmod::RootModuleFileKey`
  direct-`std::fs` bridge was removed. Same-daemon root and included module
  edit guardrails now assert a warm no-op first, then prove the edit bumps
  module-file parse and bzlmod-resolution counters.
- Non-root `MODULE.bazel` files used by extension aggregation now flow through
  `NonRootModuleFilesKey` instead of an inline `Path::exists()` plus direct
  `parse_module_bazel()` scan. Project-local dependency module files and their
  included segments are DICE project-file reads; out-of-project module paths feed
  a polled digest into the key. A same-daemon guardrail edits a non-root included
  module segment so a dependency module starts importing a generated extension
  repo, proving the aggregation graph invalidates instead of keeping the warm
  parsed-module list.
- Extension evaluation no longer eagerly computes `ExtensionRepoExecutionKey`
  for every generated spoke. It still registers generated spoke cells as
  transitional lookup plumbing, but repository materialization dependencies now
  stay on the repository execution path instead of being recorded as extension
  evaluation inputs. This fixes the missing-lockfile same-daemon warm replay
  guardrail: the extension executes once on the cold command and is reused on
  the warm command.
- Project-local `local_path_override()` module files now use the same tracked
  project-file read path as root and included module segments. Out-of-project
  absolute/normalized local override paths are registered through
  `bazel-external/<module>+` symlinks and feed a polled digest into the
  transitional `LocalOverrideModuleInputsKey`, so warm reuse is keyed by the
  observed external module state. Local-override guardrails now prove
  same-daemon warm no-ops do not reparse or rerun bzlmod resolution for either
  project-local or out-of-project override module files, and edits to either
  source class invalidate the graph.
- Visible workspace `MODULE.bazel.lock` reads now flow through a tracked
  project-file DICE key in `slug_common`, and the file watcher treats
  `MODULE.bazel.lock` changes as pre-config invalidations. Hidden/output-base
  lockfiles use the same value type and now feed a polled content digest into
  the lockfile key when read outside the project root, so warm same-daemon
  commands reuse the hidden-lockfile value while create/edit/delete transitions
  still change key identity. The visible-lockfile guardrail proves a
  same-daemon warm no-op does not reread the lockfile before an invalid edit is
  observed and rejected under `--lockfile_mode=error`, and hidden-lockfile
  guardrails cover warm reuse plus replay/facts create-edit-delete transitions.
- Locked registry cache files now use tracked project-file DICE reads when the
  configured `XDG_CACHE_HOME` is under the project root, covering cached
  registry `MODULE.bazel`, `source.json`, and `bazel_registry.json` checksum
  inputs. Cache files outside the project root remain polled direct filesystem
  reads, but the transitional `RegistryFileInputsKey` now includes a digest of
  those external files so warm reuse is keyed by the observed external cache
  state instead of a stale path-only child value. Guardrails prove warm bzlmod
  reuse and same-daemon checksum failures after editing each cached registry
  file class, including an out-of-project `bazel_registry.json`.
- Cached non-registry override module files now feed the transitional
  resolution key through `NonRegistryOverrideModuleInputsKey`. The key shares
  the override cache directory calculation with `ModuleCache`, reads
  project-local files through tracked DICE file inputs, and polls out-of-project
  cache contents into key identity. A same-daemon guardrail proves a cached
  `git_override` `MODULE.bazel` warm no-op reuses bzlmod resolution, then an
  edit to that cached module file forces resolution to recompute.
- Non-registry override fetch cache directories now include the Bazel-relevant
  source/extraction identity instead of only commit or archive URL/integrity:
  `git_override` includes remote, commit, and shallow-since, while
  `archive_override` includes URLs, integrity, and strip prefix. Bazel source
  anchors: `ModuleFileGlobals.archiveOverride`/`gitOverride` forward override
  kwargs into `RepoSpec`, `GitRepoSpecBuilder` models remote/commit/shallow
  attrs, and `ArchiveRepoSpecBuilder` models URLs/integrity/strip-prefix attrs.
  Patch identity remains blocked with override patch support.
- The unused non-cacheable `slug_bzlmod::LockfileContentKey` bridge was removed
  after visible and hidden lockfile reads moved to the tracked key in
  `slug_common`.
- Extension replay now consumes the tracked visible and hidden
  `LockfileContentValue`s carried in `BzlmodSessionData` instead of reopening
  those lockfiles inside `ModuleExtensionExecutionKey::compute`. A focused
  replay guardrail now proves a visible-lockfile replay hit consumes exactly the
  one tracked lockfile read and does not add a second direct read from extension
  execution. This is still transitional because the values are carried through
  injected session data rather than a final lockfile replay-input key.
- Current slice validation passed with `cargo build -p slug`, `cargo test -p
  slug_bzlmod -- --nocapture`, `cargo test -p slug_common bzlmod --
  --nocapture`, `cargo test -p slug_external_cells -- --nocapture`, `cargo test
  -p slug_file_watcher -- --nocapture`, `cargo test -p
  slug_interpreter_for_build module_extension_executor_impl -- --nocapture`,
  `TEST_EXECUTABLE=/var/mnt/dev/slug/target/debug/slug python -m pytest -q
  tests/core/bzlmod/test_plan61_guardrails.py -rx --tb=short`, `cargo fmt
  --check`, and `git diff --check`. The full Plan 61 Python guardrail passed
  with 61 tests.
- Local-override tracked-input validation passed with `cargo build -p slug`,
  `cargo test -p slug_common bzlmod -- --nocapture`, focused
  `TEST_EXECUTABLE=/var/mnt/dev/slug/target/debug/slug python -m pytest -q
  tests/core/bzlmod/test_plan61_guardrails.py -k
  'local_override_module_edit_invalidates_only_affected_nodes' -rx --tb=short`,
  the full Plan 61 Python guardrail, `cargo fmt --check`, and
  `git diff --check`.
- Visible-lockfile tracked-input validation passed with `cargo build -p slug`,
  `cargo test -p slug_bzlmod -- --nocapture`, `cargo test -p slug_common
  bzlmod -- --nocapture`, focused `TEST_EXECUTABLE=/var/mnt/dev/slug/target/debug/slug
  python -m pytest -q tests/core/bzlmod/test_plan61_guardrails.py -k
  'visible_lockfile_edit_is_observed_in_same_daemon' -rx --tb=short`, the full
  Plan 61 Python guardrail, `cargo fmt --check`, and `git diff --check`.
- Registry-cache tracked/polled-input validation passed with `cargo build -p
  slug`, `cargo test -p slug_common bzlmod -- --nocapture`, `cargo test -p
  slug_bzlmod -- --nocapture`, `cargo test -p slug_external_cells --
  --nocapture`, focused Plan 61 guardrails for
  `warm_noop_locked_registry_dep_reuses_bzlmod_resolution`,
  `locked_registry_source_json_and_registry_metadata_are_bridge_inputs`, and
  `warm_noop_out_of_project_registry_cache_reuses_polled_dice_input`, the full
  Plan 61 Python guardrail with 62 tests, `cargo fmt --check`, and `git diff
  --check`.
- Out-of-project local-override validation passed with `cargo build -p slug`,
  `cargo test -p slug_bzlmod -- --nocapture`, `cargo test -p slug_common
  bzlmod -- --nocapture`, focused Plan 61 guardrails for
  `warm_noop_local_override_audit_cell_reuses_bzlmod_resolution`,
  `warm_noop_out_of_project_local_override_reuses_polled_dice_input`, and
  `local_override_module_edit_invalidates_only_affected_nodes`, and the full
  Plan 61 Python guardrail with 64 tests.
- Hidden-lockfile polled-input validation passed with `cargo build -p slug`,
  `cargo test -p slug_common bzlmod -- --nocapture`, focused Plan 61 guardrails
  for `hidden_lockfile_read_is_observable_before_extension_replay`,
  `malformed_hidden_lockfile_is_ignored`,
  `hidden_lockfile_edit_invalidates_replay_in_same_daemon`, and
  `hidden_lockfile_facts_create_edit_delete_are_observed`, and the full Plan 61
  Python guardrail with 64 tests.
- Lockfile bridge cleanup validation passed with `cargo test -p slug_bzlmod --
  --nocapture`.
- Mapped extension `.bzl` digest validation passed with `cargo test -p
  slug_bzlmod test_project_bzl_digest_resolves_mapped_apparent_external_loads
  -- --nocapture`, `cargo test -p slug_bzlmod -- --nocapture`, `cargo test -p
  slug_common bzlmod -- --nocapture`, `cargo build -p slug`, the focused Plan
  61 Python guardrail
  `mapped_external_extension_bzl_load_edit_rejects_replay`, the full Plan 61
  Python guardrail with 64 tests, `cargo fmt --check`, and `git diff --check`.
- Non-registry override module-input validation passed with `cargo test -p
  slug_bzlmod -- --nocapture`, `cargo test -p slug_common bzlmod --
  --nocapture`, `cargo build -p slug`, the focused Plan 61 Python guardrail
  `cached_git_override_module_edit_invalidates_bzlmod_resolution`, the full Plan
  61 Python guardrail with 65 tests, `cargo fmt --check`, and `git diff
  --check`.
- Non-registry override source-identity validation passed with `cargo test -p
  slug_bzlmod cache -- --nocapture`, `cargo test -p slug_bzlmod --
  --nocapture`, `cargo test -p slug_common bzlmod -- --nocapture`, `cargo build
  -p slug`, the focused Plan 61 Python guardrail
  `cached_git_override_module_edit_invalidates_bzlmod_resolution`, and the full
  Plan 61 Python guardrail with 65 tests, `cargo fmt --check`, and `git diff
  --check`.
- Command-context repo-env threading validation passed with `cargo build -p
  slug`, focused Plan 61 Python guardrails for
  `recorded_env_input_change_rejects_cache` and
  `recorded_env_input_change_rejects_mixed_graph_cache`, and the full Plan 61
  Python guardrail with 65 tests.
- Tracked-lockfile extension-replay validation passed with `cargo check -p
  slug_common`, `cargo build -p slug`, `cargo test -p slug_bzlmod --
  --nocapture`, `cargo test -p slug_common bzlmod -- --nocapture`, the focused
  Plan 61 Python guardrail
  `warm_noop_extension_replay_audit_cell_reuses_bzlmod_resolution`, and the full
  Plan 61 Python guardrail with 65 tests.
- Module-context repo-env validation passed with `cargo check -p
  slug_interpreter_for_build`, `cargo test -p slug_interpreter_for_build
  module_context_repo_env -- --nocapture`, `cargo build -p slug`, the focused
  Plan 61 Python guardrail `module_ctx_repo_env_uses_command_key_input`, and the
  full Plan 61 Python guardrail with 66 tests.
- Repository-context repo-env validation passed with `cargo check -p
  slug_bzlmod -p slug_external_cells -p slug_interpreter_for_build`, `cargo test
  -p slug_bzlmod test_extension_repo_key_hash_includes_repo_env --
  --nocapture`, `cargo test -p slug_interpreter_for_build
  test_repository_context_repo_env_is_context_owned -- --nocapture`, `cargo
  check -p slug_external_cells`, `cargo build -p slug`, and the focused Plan 61
  Python guardrail `repository_ctx_repo_env_uses_command_key_input`. After the
  stale marker fix, `cargo test -p slug_bzlmod -- --nocapture`, `cargo test -p
  slug_external_cells -- --nocapture`, `cargo test -p slug_common bzlmod --
  --nocapture`, and the full Plan 61 Python guardrail passed with 67 tests.
- Missing `.bzl` load replay validation passed with `cargo test -p slug_bzlmod
  test_project_bzl_digest_includes_missing_project_load_state -- --nocapture`,
  `cargo build -p slug`, the focused Plan 61 Python guardrail
  `missing_transitive_extension_bzl_load_creation_rejects_replay`, and the full
  Plan 61 Python guardrail with 68 tests.
- Mapped external `.bzl` delete replay validation passed with `cargo test -p
  slug_bzlmod test_project_bzl_digest_includes_existing_external_loads --
  --nocapture`, the focused Plan 61 Python guardrail
  `mapped_external_extension_bzl_load_deletion_rejects_replay`, and the full
  Plan 61 Python guardrail with 69 tests.
- Non-root module parse key validation passed with `cargo check -p
  slug_common`, `cargo build -p slug`, `cargo test -p slug_common bzlmod --
  --nocapture`, `cargo test -p slug_bzlmod -- --nocapture`, `cargo test -p
  slug_external_cells -- --nocapture`, the focused Plan 61 Python guardrail
  `non_root_included_module_segment_edit_invalidates_extension_graph`, and the
  full Plan 61 Python guardrail with 70 tests.
- Dynamic generated-repo suffix lookup is now deterministic in the transitional
  cell resolver path: apparent/suffix lookups use the canonical helper instead
  of unordered process-global map iteration, and the last-resort
  `bazel-external/` directory scan sorts candidates before selecting a match.
  Focused validation passed with `cargo test -p slug_core
  cell_resolver_dynamic_suffix_lookup_is_deterministic -- --nocapture`,
  `cargo build -p slug`, and the focused Plan 61 Python guardrail
  `two_workspaces_do_not_share_bzlmod_state`.
- Generated repo materialization no longer recomputes
  `BzlmodSessionDataKey` in the extension-repo file-ops path to find the
  command repo environment. `ExtensionRepoCellSetup` carries serialized
  repo-env for lockfile/use_repo_rule fallback paths, while normal module
  extension materialization prefers the current `ExtensionSpokesValue`
  repo-env from the DICE lookup so stale cell origins cannot pin the first
  command's env. Validation passed with `cargo check -p slug_external_cells`,
  `cargo build -p slug`, `cargo test -p slug_common bzlmod -- --nocapture`,
  `cargo test -p slug_external_cells -- --nocapture`, focused Plan 61
  guardrails for repository repo-env, recorded env replay, warm replay, and
  mapped external load edit replay, and the full Plan 61 Python guardrail with
  70 tests.
- Repository rule env reads now become explicit recorded inputs:
  `repository_ctx.getenv()` records `ENV` entries in the materialization
  sidecar, `repository_ctx.os.environ` records the current visible repo-env
  snapshot, and `RepoMaterializationManifestKey` carries the command repo-env
  needed to validate those sidecar entries. Validation passed with `cargo check
  -p slug_bzlmod -p slug_interpreter_for_build -p slug_external_cells`, `cargo
  test -p slug_bzlmod recorded_env -- --nocapture`, `cargo test -p
  slug_interpreter_for_build test_repository_context_records_env_inputs --
  --nocapture`, `cargo test -p slug_bzlmod -- --nocapture`, `cargo test -p
  slug_external_cells -- --nocapture`, `cargo build -p slug`, and focused Plan
  61 repo-env/replay guardrails.
- Registered toolchain and execution-platform facts now have their own
  injected DICE values. `RegisteredToolchainsKey` and
  `RegisteredExecutionPlatformsKey` no longer compute the whole
  `BzlmodSessionDataKey`. At this checkpoint `ModuleVersionsKey` intentionally
  remained on the wider injected session with value cutoffs disabled because
  narrowing it prematurely regressed hidden-lockfile fact invalidation.
  Validation passed with `cargo check -p slug_bzlmod -p slug_analysis -p
  slug_configured`, `cargo test -p slug_bzlmod -- --nocapture`, `cargo build
  -p slug`, the focused hidden-lockfile facts guardrail, and the full Plan 61
  Python guardrail with 70 tests.
- Extension replay/session split validation passed with `cargo fmt --check`,
  `cargo check -p slug_bzlmod -p slug_external_cells`, `cargo test -p
  slug_bzlmod -- --nocapture`, `cargo build -p slug`, focused Plan 61 Python
  guardrails for hidden-lockfile facts, repo-env input, and warm extension
  replay, the full Plan 61 Python guardrail with 70 tests, and `git diff
  --check`.
- Module-version split validation passed with `cargo check -p slug_bzlmod -p
  slug_interpreter_for_build`, `cargo test -p slug_bzlmod -- --nocapture`,
  `cargo build -p slug`, focused Plan 61 Python guardrails for missing-lockfile
  warm reuse, hidden-lockfile facts, and repo-env inputs, the full Plan 61
  Python guardrail with 70 tests, `cargo fmt --check`, and `git diff --check`.
- Monolithic session-key removal validation passed with `cargo fmt --check`,
  `cargo check -p slug_bzlmod -p slug_interpreter_for_build`,
  `cargo test -p slug_bzlmod -- --nocapture`, `cargo build -p slug`, focused
  Plan 61 Python guardrails for missing-lockfile warm reuse, hidden-lockfile
  facts, and repo-env inputs, the full Plan 61 Python guardrail with 70 tests,
  and `git diff --check`.
- Extension repo materialization routing validation passed with `cargo check
  -p slug_external_cells -p slug_bzlmod`, `cargo test -p slug_bzlmod
  repository_execution -- --nocapture`, `cargo test -p slug_external_cells --
  --nocapture`, `cargo build -p slug`, the focused Plan 61 Python guardrails
  for marker, recorded-env, and watch/watch-tree behavior, the full Plan 61
  Python guardrail with 70 tests, `cargo fmt --check`, and `git diff --check`.
- Repo-mapping split validation passed with `cargo fmt --check`, `git diff
  --check`, `cargo check -p slug_bzlmod -p slug_external_cells -p
  slug_interpreter_for_build`, `cargo test -p slug_bzlmod -- --nocapture`,
  `cargo build -p slug`, focused Plan 61 Python guardrails for mapped external
  loads, recorded repo mappings, `inject_repo()`, and `override_repo()`, and
  the full Plan 61 Python guardrail with 70 tests.
- Extension replay-input split validation passed with `cargo fmt --check`,
  `git diff --check`, `cargo check -p slug_bzlmod -p slug_external_cells -p
  slug_interpreter_for_build`, `cargo test -p slug_bzlmod -- --nocapture`,
  `cargo build -p slug`, focused Plan 61 Python guardrails for visible/hidden
  lockfiles, repo env, missing-lockfile warm reuse, mapped external loads, and
  recorded repo mappings, and the full Plan 61 Python guardrail with 70 tests.
- Extension aggregation split validation passed with `cargo fmt --check`,
  `git diff --check`, `cargo check -p slug_bzlmod -p slug_external_cells -p
  slug_interpreter_for_build`, `cargo test -p slug_bzlmod -- --nocapture`,
  `cargo build -p slug`, focused Plan 61 Python guardrails for visible/hidden
  lockfiles, repo env, missing-lockfile warm reuse, mapped external loads, and
  recorded repo mappings, and the full Plan 61 Python guardrail with 70 tests.
- Missing mapped external `.bzl` load-state validation passed with `cargo
  fmt --check`, `git diff --check`, `cargo test -p slug_bzlmod
  test_project_bzl_digest -- --nocapture`, `cargo build -p slug`, focused
  Plan 61 Python guardrails for mapped external helper create/edit/delete
  replay transitions, and the full Plan 61 Python guardrail with 71 tests.
- Runtime extension-cell snapshot/installer validation passed with `cargo
  check -p slug_core -p slug_common`, `cargo test -p slug_core
  cells::bzlmod_apparent_alias_cache_tests -- --nocapture --test-threads=1`,
  `cargo test -p slug_common bzlmod -- --nocapture`, `cargo build -p slug`,
  focused Plan 61 Python guardrails for warm extension replay, two-workspace
  isolation, and valid lockfile replay materialization, and the full Plan 61
  Python guardrail with 71 tests. A broader `cargo test -p slug_core
  dynamic_extension -- --nocapture` remains order-sensitive because it runs
  process-global dynamic-cell tests in parallel, so the cache submodule was run
  serially for signal.
- Repository materialization manifest child-key validation passed with
  `cargo fmt --check`, `git diff --check`, `cargo check -p slug_bzlmod`,
  `cargo test -p slug_bzlmod materialization_manifest -- --nocapture`,
  `cargo test -p slug_bzlmod -- --nocapture`, `cargo build -p slug`, focused
  Plan 61 Python guardrails for repository watch/watch-tree and stale marker
  behavior, and the full Plan 61 Python guardrail with 71 tests.
- Repository output-state marker validation passed with `cargo fmt --check`,
  `git diff --check`, `cargo test -p slug_bzlmod -- --nocapture`, `cargo test
  -p slug_external_cells -- --nocapture`, `cargo build -p slug`, the focused
  Plan 61 Python guardrail
  `materialized_repo_marker_revalidates_corrupted_output_digest`, and the full
  Plan 61 Python guardrail with 72 tests. Slug now treats its
  `complete:<spec>:output:<digest>` marker as current only when the current
  repository tree digest still matches the marker, and the external-cell
  marker gate applies the same stale-output check before trusting an existing
  materialized repo directory. This tightens Slug's internal marker authority;
  Bazel source anchor: `RepositoryDirectoryValue` intentionally disables
  change pruning because the success value does not capture fetched contents,
  and `RepoRecordedInput` explains marker-recorded inputs used to decide
  whether a repository is up to date.
- Module-version equality validation passed with `cargo fmt --check`, `cargo
  test -p slug_bzlmod
  module_versions_key_equality_tracks_versions_value_and_invalidation --
  --nocapture`, `cargo build -p slug`, focused Plan 61 Python guardrails for
  hidden-lockfile replay/facts, warm extension replay, and module repo-env, the
  full `cargo test -p slug_bzlmod -- --nocapture`, and the full Plan 61 Python
  guardrail with 72 tests. A version-map-only equality attempt regressed the
  hidden-lockfile facts restore transition, so `ModuleVersionsValue` now carries
  a conservative invalidation identity composed from named bzlmod projections
  until the remaining interpreter inputs are explicit DICE dependencies.
- Extension-spoke lookup digest validation passed with `cargo fmt --check`,
  `cargo test -p slug_bzlmod
  extension_spokes_lookup_keys_cache_after_digest_dependency -- --nocapture`,
  `cargo test -p slug_bzlmod extension_spokes -- --nocapture`, `cargo build -p
  slug`, focused Plan 61 Python guardrails for warm replay, two-workspace
  isolation, valid lockfile replay materialization, missing-lockfile extension
  reuse, and mapped external `.bzl` load edits, the full `cargo test -p
  slug_bzlmod -- --nocapture`, and the full Plan 61 Python guardrail with 72
  tests. A first attempt to cache the lookup keys without a DICE digest
  dependency regressed the mapped external load-edit replay guardrail, so the
  digest remains an always-recomputed child key until the real Starlark load
  graph replaces the scanner.
- Extension-spoke execution-key digest reuse validation passed with `cargo
  fmt --check`, `cargo test -p slug_bzlmod
  create_extension_execution_key_uses_replay_data -- --nocapture`, `cargo test
  -p slug_bzlmod extension_spokes -- --nocapture`, `cargo build -p slug`,
  `cargo test -p slug_bzlmod -- --nocapture`, focused Plan 61 Python replay
  guardrails for warm replay, valid lockfile replay materialization,
  missing-lockfile extension reuse, and mapped external `.bzl` load edits, and
  the full Plan 61 Python guardrail with 72 tests.
- Root extension replay-summary `.bzl` digesting now has a
  `slug_common` DICE key. `TrackedExtensionBzlDigestKey` reuses the
  `slug_bzlmod` literal-load and label-resolution helpers, but reads
  project-root implementation files through `DiceFileComputations` while
  forming the legacy resolution bridge's replay-summary digest. The key remains
  intentionally non-cacheable because it still shares the transitional scanner
  and missing-file creations are not yet a stable child-key cutoff boundary.
  A cacheable attempt regressed
  `missing_transitive_extension_bzl_load_creation_rejects_replay` by counting
  stale replay hits before the refreshed digest was observed. Validation passed
  with `cargo test -p slug_common
  tracked_extension_bzl_digest_matches_legacy_project_load_digest --
  --nocapture`, `cargo test -p slug_bzlmod extension_spokes -- --nocapture`,
  `cargo build -p slug`, a focused Plan 61 Python replay subset covering warm
  replay plus project/mapped `.bzl` create/edit/delete transitions, `cargo test
  -p slug_bzlmod -- --nocapture`, `cargo test -p slug_common bzlmod --
  --nocapture`, `cargo fmt --check`, `git diff --check`, and the full Plan 61
  Python guardrail with 72 tests.
- Extension-spoke aggregation projection validation passed with `cargo check
  -p slug_bzlmod`, `cargo test -p slug_bzlmod
  extension_aggregation_key_projects_single_extension -- --nocapture`, `cargo
  test -p slug_bzlmod extension_spokes -- --nocapture`, `cargo test -p
  slug_bzlmod -- --nocapture`, `cargo build -p slug`, focused Plan 61 Python
  guardrails for warm replay, two-workspace isolation, valid-lockfile replay
  materialization, missing-lockfile extension reuse, and mapped external `.bzl`
  edits, `cargo fmt --check`, `git diff --check`, and the full Plan 61 Python
  guardrail with 72 tests. `ExtensionSpokesByExtensionIdKey` and
  `ExtensionSpokesKey` now project the injected aggregation map through
  `BzlmodExtensionAggregationKey`, so unrelated extension aggregation changes
  can cut off at a narrower per-extension value before execution-key
  construction.
- Canonical-repo extension-owner projection validation passed with `cargo check
  -p slug_bzlmod`, `cargo test -p slug_bzlmod
  extension_id_by_canonical_repo_key_projects_owner_extension -- --nocapture`,
  `cargo test -p slug_bzlmod extension_spokes -- --nocapture`, `cargo test -p
  slug_bzlmod -- --nocapture`, `cargo build -p slug`, the same focused Plan 61
  Python replay subset used for spoke lookup, and the full Plan 61 Python
  guardrail with 72 tests. `ExtensionSpokesByCanonicalRepoKey` now gets the
  owning extension through `ExtensionIdByCanonicalRepoKey`, so canonical repo
  lookup no longer directly depends on the whole injected extension aggregation
  map.
- Spoke lookup wrapper dependency validation passed with `cargo check -p
  slug_bzlmod`, `cargo test -p slug_bzlmod
  missing_extension_spoke_lookup_does_not_require_replay_inputs --
  --nocapture`, `cargo test -p slug_bzlmod extension_spokes -- --nocapture`,
  `cargo test -p slug_bzlmod -- --nocapture`, `cargo build -p slug`, the same
  focused Plan 61 Python replay subset, and the full Plan 61 Python guardrail
  with 72 tests. `ExtensionSpokesByExtensionIdKey` and
  `ExtensionSpokesByCanonicalRepoKey` now leave replay-data and repo-mapping
  reads to `ExtensionSpokesKey`, so absent-extension lookups can return `None`
  without depending on unrelated injected replay inputs.
- Diagnostic toolchain materialization scan removal validation passed with
  `cargo check -p slug_common`, `cargo test -p slug_common bzlmod --
  --nocapture`, `cargo build -p slug`, the focused Plan 61 Python replay
  subset, and the full Plan 61 Python guardrail with 72 tests. Legacy bzlmod
  cell setup no longer polls `bazel-external` to log pending toolchain repos;
  semantic repo materialization remains owned by label resolution and
  external-cell delegates.
- Transitional extension execution/spoke helper API cleanup validation passed
  with `cargo check -p slug_bzlmod`, focused `slug_bzlmod` tests for canonical
  repo owner projection and execution-key replay data, `cargo test -p
  slug_bzlmod extension_spokes -- --nocapture`, full `cargo test -p
  slug_bzlmod -- --nocapture`, `cargo build -p slug`, the focused Plan 61
  Python replay subset, the full Plan 61 Python guardrail with 72 tests,
  `cargo fmt --check`, and `git diff --check`. The old public helpers that
  formed extension execution/spoke keys directly from injected
  session-shaped data were removed from the crate API; production lookup now
  goes through the DICE keys.
- Extension repo-spec capture registry scope validation passed with `cargo
  test -p slug_bzlmod repo_spec -- --nocapture`, full `cargo test -p
  slug_bzlmod -- --nocapture`, `cargo build -p slug`, the focused Plan 61
  Python replay subset, the full Plan 61 Python guardrail with 72 tests,
  `cargo fmt --check`, and `git diff --check`. The thread-local RepoSpec
  capture registry now restores the previous scope on drop, so nested or
  unwinding extension-evaluation plumbing cannot silently erase or leak the
  surrounding capture context.
- Extension execution constructor surface cleanup validation passed with
  `cargo check -p slug_bzlmod`, `cargo test -p slug_bzlmod
  module_extension_key -- --nocapture`, full `cargo test -p slug_bzlmod --
  --nocapture`, `cargo build -p slug`, the focused Plan 61 Python replay
  subset, the full Plan 61 Python guardrail with 72 tests, `cargo fmt
  --check`, and `git diff --check`. Test-only constructors that recompute the
  best-effort `.bzl` digest directly are no longer public production APIs; the
  production constructor remains the keyed-digest path used by
  `ExtensionSpokesKey`.
- Repository invocation registry guard validation passed with `cargo test -p
  slug_bzlmod repository_invocations -- --nocapture`, full `cargo test -p
  slug_bzlmod -- --nocapture`, `cargo build -p slug`, the focused Plan 61
  Python replay subset, the full Plan 61 Python guardrail with 72 tests,
  `cargo fmt --check`, and `git diff --check`. The thread-local MODULE/repo
  rule invocation registry guard now restores any previous registry on drop
  instead of clearing ambient state unconditionally.
- Extension execution workspace-identity threading validation passed with
  `cargo check -p slug_bzlmod -p slug_interpreter_for_build`, focused
  `slug_bzlmod` module-extension-key and spoke tests, full `cargo test -p
  slug_bzlmod -- --nocapture`, `cargo build -p slug`, the focused Plan 61
  Python replay subset, the full Plan 61 Python guardrail with 72 tests,
  `cargo fmt --check`, and `git diff --check`. `ExtensionSpokesKey` now
  carries its exact `WorkspaceId` into `ModuleExtensionExecutionKey`, module
  extension Starlark execution, and the synchronous spoke-materialization
  bridge; Starlark repository-rule execution also uses the workspace identity
  from its materialization key when it enters the same sync bridge.
- Module-extension executor workspace-id API hardening validation passed with
  `cargo check -p slug_bzlmod -p slug_interpreter_for_build`, `cargo test -p
  slug_bzlmod test_extension_execution_requires_workspace_id -- --nocapture`,
  `cargo test -p slug_bzlmod module_extension_key -- --nocapture`, full
  `cargo test -p slug_bzlmod -- --nocapture`, `cargo build -p slug`, the
  focused Plan 61 Python replay subset, the full Plan 61 Python guardrail with
  72 tests, `cargo fmt --check`, and `git diff --check`. The late-bound module
  extension executor now requires a concrete `WorkspaceId`; execution keys
  without one fail before Starlark evaluation instead of letting the
  interpreter implementation re-derive workspace identity from project root.
- Transitional session workspace-id injection validation passed with `cargo
  check -p slug_bzlmod -p slug_common`, `cargo test -p slug_bzlmod
  set_bzlmod_session_data_uses_session_workspace_id -- --nocapture`, full
  `cargo test -p slug_bzlmod -- --nocapture`, `cargo test -p slug_common
  bzlmod -- --nocapture`, `cargo build -p slug`, the focused Plan 61 Python
  replay subset, the full Plan 61 Python guardrail with 72 tests, `cargo
  fmt --check`, and `git diff --check`. `BzlmodSessionData` now carries the
  current `WorkspaceId` explicitly, and the DICE injection step fans that
  exact root/output-base identity into the narrower bzlmod data values instead
  of reconstructing it from `project_root`.
- Redundant session project-root field removal validation passed with `cargo
  check -p slug_bzlmod -p slug_common`, `cargo test -p slug_bzlmod
  set_bzlmod_session_data_uses_session_workspace_id -- --nocapture`, `cargo
  test -p slug_common bzlmod -- --nocapture`, full `cargo test -p
  slug_bzlmod -- --nocapture`, `cargo build -p slug`, the focused Plan 61
  Python replay subset, the full Plan 61 Python guardrail with 72 tests,
  `cargo fmt --check`, and `git diff --check`. `BzlmodSessionData` no longer
  carries a separate `project_root`; the transitional session has one
  workspace identity source, `workspace_id`.
- Dynamic generated-repo cell paths, setup payloads, unscoped aliases, and
  scoped aliases now carry the dynamic project root captured at registration
  time, and lookups require that root to match. This is still process-global
  transitional plumbing, but stale entries from another root can no longer
  satisfy generated repo lookup after the active root changes. Validation passed
  with `cargo check -p slug_core`, `cargo test -p slug_core
  dynamic_bzlmod_entries_are_scoped_to_current_project_root -- --nocapture`,
  `cargo build -p slug`, the focused Plan 61 Python replay subset, the full
  Plan 61 Python guardrail with 72 tests, `cargo fmt --check`, and
  `git diff --check`.
- Resolver-local promoted dynamic cells now carry the same dynamic project-root
  identity as the global generated-repo maps. `CellResolver::get()` and
  `get_cell_path()` ignore promoted dynamic cells after the active bzlmod root
  changes, so a stale resolver cache cannot bypass the root-scoped registry
  checks. Validation passed with `cargo check -p slug_core`, `cargo test -p
  slug_core dynamic_bzlmod_entries_are_scoped_to_current_project_root --
  --nocapture`, `cargo build -p slug`, the focused Plan 61 Python replay subset,
  the full Plan 61 Python guardrail with 72 tests, `cargo fmt --check`, and
  `git diff --check`.
- The transitional apparent-module alias and generated-repo suffix scan caches
  now store root-scoped positive and negative lookup results. A stale directory
  scan result from one bzlmod root can no longer satisfy a later lookup after the
  active root changes without reset. Validation passed with `cargo check -p
  slug_core`, `cargo test -p slug_core
  dynamic_bzlmod_entries_are_scoped_to_current_project_root -- --nocapture`,
  `cargo test -p slug_core cells::bzlmod_apparent_alias_cache_tests --
  --nocapture --test-threads=1`, `cargo build -p slug`, the focused Plan 61
  Python replay subset, the full Plan 61 Python guardrail with 72 tests, `cargo
  fmt --check`, and `git diff --check`.
- The temporary root-cell and non-root cell-name adapters now carry the active
  dynamic project-root identity. Root-name checks and known external-cell checks
  ignore stale values after the active bzlmod root changes instead of letting an
  older resolver's cell names influence canonical generated-repo lookup. The
  same focused root-switch regression covers this, and validation passed with
  `cargo check -p slug_core`, `cargo test -p slug_core
  dynamic_bzlmod_entries_are_scoped_to_current_project_root -- --nocapture`,
  `cargo test -p slug_core cells::bzlmod_apparent_alias_cache_tests --
  --nocapture --test-threads=1`, `cargo build -p slug`, the focused Plan 61
  Python replay subset, the full Plan 61 Python guardrail with 72 tests, `cargo
  fmt --check`, and `git diff --check`.
- The legacy bzlmod resolver now injects a named `BzlmodCellGraphDataKey`, and
  `BzlmodCellGraphKey` projects it by explicit `WorkspaceId`. The value records
  root module name, module cells, extension/generated cells, root aliases,
  module symlinks, scoped aliases, and dynamic aliases as DICE-visible data.
  Runtime installation still uses the transitional process-global
  `slug_core::cells` adapter, so this is a named migration surface rather than
  final cell-graph ownership. Validation passed with `cargo check -p
  slug_bzlmod -p slug_common`, `cargo test -p slug_bzlmod
  set_bzlmod_session_data_uses_session_workspace_id -- --nocapture`, full
  `cargo test -p slug_bzlmod -- --nocapture`, `cargo test -p slug_common
  bzlmod -- --nocapture`, `cargo build -p slug`, the focused Plan 61 Python
  replay subset, the full Plan 61 Python guardrail with 72 tests, `cargo
  fmt --check`, and `git diff --check`.
- Bzlmod runtime cell installation now consumes the published
  `BzlmodCellGraphValue` instead of assembling a separate install snapshot from
  parallel `BzlmodResolutionResult` fields. The replay path uses that graph for
  module symlinks, eager extension-cell symlinks, root aliases, scoped aliases,
  dynamic aliases, and eager/lazy extension-cell registration. Cell resolver
  assembly and the backing runtime maps are still transitional. Validation
  passed with `cargo check -p slug_common`, `cargo test -p slug_common
  runtime_cell_install_snapshot_derives_from_cell_graph -- --nocapture`,
  `cargo test -p slug_common bzlmod -- --nocapture`, `cargo build -p slug`,
  the focused Plan 61 Python replay subset, the full Plan 61 Python guardrail
  with 72 tests, `cargo fmt --check`, and `git diff --check`.
- Bzlmod cell-resolver assembly now also consumes `BzlmodCellGraphValue`.
  Graph module cells carry optional remote-module setup metadata, and the
  config-load path derives module cells, eager extension cells, and root aliases
  from the graph instead of parallel `BzlmodResolutionResult` fields. The graph
  is still legacy-produced rather than DICE-derived, but the transitional
  session now has one cell-graph payload for resolver assembly and runtime
  installation. Validation passed with `cargo check -p slug_bzlmod -p
  slug_common`, `cargo test -p slug_common
  module_setup_derives_from_cell_graph -- --nocapture`, `cargo test -p
  slug_common runtime_cell_install_snapshot_derives_from_cell_graph --
  --nocapture`, `cargo test -p slug_bzlmod
  set_bzlmod_session_data_uses_session_workspace_id -- --nocapture`, `cargo
  test -p slug_common bzlmod -- --nocapture`, `cargo build -p slug`, the
  focused Plan 61 Python replay subset, the full Plan 61 Python guardrail with
  72 tests, `cargo fmt --check`, and `git diff --check`.
- Bzlmod `CellResolver` construction now carries the graph-derived runtime
  cell install snapshot, so exact lazy generated-repo cell lookup can create
  the cell from resolver-local graph state before consulting the transitional
  process-global dynamic registry. The globals still back apparent/scoped alias
  compatibility and runtime registration, but canonical lazy repo lookup no
  longer requires the global map as its first semantic source. Validation
  passed with `cargo test -p slug_core
  bzlmod_resolver_uses_runtime_snapshot_for_lazy_extension_cell --
  --nocapture`, `cargo test -p slug_core
  dynamic_bzlmod_entries_are_scoped_to_current_project_root -- --nocapture`,
  `cargo test -p slug_core cells::bzlmod_apparent_alias_cache_tests --
  --nocapture --test-threads=1`, `cargo check -p slug_core -p slug_common -p
  slug_external_cells`, `cargo test -p slug_bzlmod -- --nocapture`, `cargo
  test -p slug_common bzlmod -- --nocapture`, `cargo test -p
  slug_external_cells -- --nocapture`, `cargo build -p slug`, the focused Plan
  61 Python replay/root-isolation subset, the full Plan 61 Python guardrail
  with 72 tests, `cargo fmt --check`, and `git diff --check`.
- Bundled bzlmod cells are now part of `BzlmodCellGraphValue` instead of
  separate config-load auto-registration. The graph records bundled cells with
  an explicit `bundled` bit, and resolver assembly marks those graph cells with
  `ExternalCellOrigin::Bundled`. Validation passed with `cargo check -p
  slug_bzlmod -p slug_common`, `cargo test -p slug_common cell_graph --
  --nocapture`, `cargo test -p slug_bzlmod
  set_bzlmod_session_data_uses_session_workspace_id -- --nocapture`, `cargo
  test -p slug_common bzlmod -- --nocapture`, `cargo build -p slug`, the
  focused Plan 61 Python replay subset, the full Plan 61 Python guardrail with
  72 tests, `cargo fmt --check`, and `git diff --check`.
- The now-empty `BzlmodResolutionResult` wrapper was removed. The legacy
  resolution key returns the transitional `BzlmodSessionData` payload directly,
  and config-load replay consumes that payload without a parallel result shell.
  Validation passed with `cargo check -p slug_bzlmod -p slug_common`, `cargo
  test -p slug_common cell_graph -- --nocapture`, `cargo test -p slug_common
  bzlmod -- --nocapture`, `cargo build -p slug`, the focused Plan 61 Python
  replay subset, the full Plan 61 Python guardrail with 72 tests, `cargo
  fmt --check`, and `git diff --check`.
- `BzlmodSessionData` no longer carries a separate `root_module_name` field.
  The injection fan-out now reads the root module name from
  `BzlmodCellGraphValue`, keeping the transitional payload's root cell identity
  on the graph. Validation passed with `cargo check -p slug_bzlmod -p
  slug_common`, `cargo test -p slug_bzlmod
  set_bzlmod_session_data_uses_session_workspace_id -- --nocapture`, `cargo
  test -p slug_common cell_graph -- --nocapture`, `cargo test -p slug_common
  bzlmod -- --nocapture`, `cargo build -p slug`, the focused Plan 61 Python
  replay subset, the full Plan 61 Python guardrail with 72 tests, `cargo
  fmt --check`, and `git diff --check`.
- `BzlmodSessionData` no longer carries duplicate visible/hidden lockfile
  digest fields. The injection fan-out derives replay and module-version
  invalidation digests from the tracked `LockfileContentValue`s, so the
  transitional session payload has one lockfile content identity source while
  the narrower replay keys still keep explicit digest identity. Validation
  passed with `cargo check -p slug_bzlmod -p slug_common`, `cargo test -p
  slug_bzlmod
  set_bzlmod_session_data_derives_lockfile_digests_from_values -- --nocapture`,
  `cargo test -p slug_bzlmod -- --nocapture`, `cargo test -p slug_common
  bzlmod -- --nocapture`, `cargo build -p slug`, the focused Plan 61 Python
  replay subset, the full Plan 61 Python guardrail with 72 tests, `cargo
  fmt --check`, and `git diff --check`. The first full `slug_bzlmod` test run
  hit an environmental doctest linker `SIGBUS` because `/tmp` was full; reruns
  with `TMPDIR=/var/mnt/dev/slug-test-tmp` passed.
- Runtime bzlmod state replay now accepts the published
  `BzlmodCellGraphValue` directly instead of the whole transitional
  `BzlmodSessionData` payload. The installed lookup state is still
  process-global transitional plumbing, but the replay helper can no longer
  accidentally depend on session-shaped data outside the graph. Validation
  passed with `cargo check -p slug_common`, `cargo test -p slug_common bzlmod
  -- --nocapture`, `cargo build -p slug`, the focused Plan 61 Python replay
  subset, the full Plan 61 Python guardrail with 72 tests, `cargo fmt
  --check`, and `git diff --check`.
- Lockfile replay identity now has a named `BzlmodLockfileInputsValue` shared
  by extension replay data and module-version invalidation instead of
  repeating the visible/hidden lockfile path, digest, parsed value, and mode
  fields in both projections. `ModuleExtensionExecutionKey` still keeps its
  explicit hashed lockfile identity, but that identity is now formed from the
  shared lockfile-input bundle. Validation passed with `cargo check -p
  slug_bzlmod -p slug_common`, focused `slug_bzlmod` tests for session
  injection, extension execution-key construction, and module-version equality,
  full `cargo test -p slug_bzlmod -- --nocapture`, `cargo test -p slug_common
  bzlmod -- --nocapture`, `cargo build -p slug`, the focused Plan 61 Python
  replay subset, the full Plan 61 Python guardrail with 72 tests, `cargo fmt
  --check`, and `git diff --check`.
- `BzlmodSessionData` now carries the same `BzlmodLockfileInputsValue` at the
  transitional legacy resolver boundary, instead of separate lockfile path,
  value, and mode fields. The resolver constructs the bundle from the tracked
  lockfile values, and DICE injection fans out that same bundle to replay and
  module-version invalidation projections. Validation passed with `cargo check
  -p slug_bzlmod -p slug_common`, focused `slug_bzlmod` tests for session
  injection, extension execution-key construction, and module-version equality,
  full `cargo test -p slug_bzlmod -- --nocapture`, `cargo test -p slug_common
  bzlmod -- --nocapture`, `cargo build -p slug`, the focused Plan 61 Python
  replay subset, the full Plan 61 Python guardrail with 72 tests, `cargo fmt
  --check`, and `git diff --check`.
- Lockfile replay inputs now have their own injected data key and
  workspace-checked `BzlmodLockfileInputsKey`. Extension execution-key
  construction reads lockfile identity through that key instead of bundling it
  inside extension replay data, and `ModuleVersionsKey` composes its
  conservative invalidation identity from the same keyed lockfile value. This
  is still transitional because the lockfile bundle is populated from the
  legacy resolver payload, but replay and module-version consumers now depend
  on a named lockfile-input projection. Validation passed with `cargo check -p
  slug_bzlmod -p slug_common`, focused `slug_bzlmod` tests for lockfile
  session injection, extension execution-key construction, and absent-spoke
  dependency avoidance, full `cargo test -p slug_bzlmod -- --nocapture`,
  `cargo test -p slug_common bzlmod -- --nocapture`, `cargo build -p slug`,
  the focused Plan 61 Python replay subset, the full Plan 61 Python guardrail
  with 72 tests, `cargo fmt --check`, and `git diff --check`.
- Command repo-env now has its own injected data key and workspace-checked
  `BzlmodRepoEnvKey`. The old extension replay-data wrapper was removed:
  extension execution-key construction reads repo env through the repo-env key,
  lockfile identity through `BzlmodLockfileInputsKey`, and repo mappings
  through `BzlmodRepoMappingsDataKey`; `ModuleVersionsKey` composes its
  conservative invalidation identity from the same keyed repo-env and lockfile
  values. This remains transitional because repo env is still injected from the
  legacy resolver payload. Validation passed with `cargo check -p slug_bzlmod
  -p slug_common`, focused `slug_bzlmod` tests for repo-env/lockfile execution
  key construction, lockfile/repo-env session injection, and absent-spoke
  dependency avoidance, full `cargo test -p slug_bzlmod -- --nocapture`,
  `cargo test -p slug_common bzlmod -- --nocapture`, `cargo build -p slug`,
  the focused Plan 61 Python replay subset, the full Plan 61 Python guardrail
  with 72 tests, `cargo fmt --check`, and `git diff --check`.
- Repo mappings now have a workspace-checked `BzlmodRepoMappingsKey` in front
  of the injected repo-mapping data. Extension `.bzl` digesting,
  extension-spoke execution-key construction, and `ModuleVersionsKey`
  invalidation now consume this key, so stale workspace identity is rejected at
  the projection boundary instead of by ad hoc caller checks. This remains
  transitional because the repo-mapping snapshot is still produced by the
  legacy resolver. Validation passed with `cargo check -p slug_bzlmod -p
  slug_common`, focused `slug_bzlmod` tests for session workspace identity,
  mapped external load digesting, extension execution-key construction, and
  absent-spoke dependency avoidance, full `cargo test -p slug_bzlmod --
  --nocapture`, `cargo test -p slug_common bzlmod -- --nocapture`, `cargo
  build -p slug`, the focused Plan 61 Python replay subset, the full Plan 61
  Python guardrail with 72 tests, `cargo fmt --check`, and `git diff --check`.
- `ModuleVersionsKey` now reads the root module name from
  `BzlmodCellGraphKey` when composing its conservative invalidation identity,
  instead of carrying a duplicate root-name copy in the injected
  module-version data. The cell graph is still legacy-produced, but the
  module-version consumer now depends on the named cell-graph projection for
  root module identity. Validation passed with `cargo check -p slug_bzlmod -p
  slug_common`, focused `slug_bzlmod` coverage proving the session cell graph
  root name feeds module-version invalidation, full `cargo test -p
  slug_bzlmod -- --nocapture`, `cargo test -p slug_common bzlmod --
  --nocapture`, `cargo build -p slug`, the focused Plan 61 Python replay
  subset, the full Plan 61 Python guardrail with 72 tests, `cargo fmt
  --check`, and `git diff --check`.
- Registry-file hashes and selected yanked-version facts now flow through a
  workspace-checked `BzlmodResolutionFactsKey`, leaving
  `BzlmodModuleVersionsDataValue` as the injected module-version map rather
  than a carrier for unrelated conservative invalidation facts.
  `ModuleVersionsKey` composes its invalidation identity from the named cell
  graph, lockfile, repo-env, repo-mapping, and resolution-facts projections.
  This remains transitional because the facts are still produced by the legacy
  resolver. Validation passed with `cargo check -p slug_bzlmod -p
  slug_common`, focused `slug_bzlmod` coverage for lockfile/repo-env/facts
  session injection, full `cargo test -p slug_bzlmod -- --nocapture`, `cargo
  test -p slug_common bzlmod -- --nocapture`, `cargo build -p slug`, the
  focused Plan 61 Python replay subset, the full Plan 61 Python guardrail with
  72 tests, `cargo fmt --check`, and `git diff --check`.
- Extension aggregation root-module identity now comes from
  `BzlmodCellGraphKey` instead of a duplicate field in the injected aggregation
  map. `BzlmodExtensionAggregationKey` and canonical-repo owner lookup read the
  named cell graph only when a matching aggregation exists; absent-spoke lookup
  still avoids adding replay or cell-graph dependencies. Validation passed with
  `cargo check -p slug_bzlmod -p slug_common`, focused `slug_bzlmod` coverage
  for extension aggregation projection, canonical repo owner lookup, and absent
  spoke dependency avoidance, full `cargo test -p slug_bzlmod -- --nocapture`,
  `cargo test -p slug_common bzlmod -- --nocapture`, `cargo build -p slug`, the
  focused Plan 61 Python replay subset, the full Plan 61 Python guardrail with
  72 tests, `cargo fmt --check`, and `git diff --check`.
- Registered toolchain and execution-platform data now inject only the
  registration lists; `RegisteredToolchainsKey` and
  `RegisteredExecutionPlatformsKey` derive workspace identity through
  `BzlmodCellGraphKey`. The current-workspace helpers now use the named cell
  graph projection instead of reading workspace identity from the registered
  data payloads. Validation passed with `cargo check -p slug_bzlmod -p
  slug_common`, focused `slug_bzlmod` coverage for cell-graph workspace
  projection and current-workspace helpers, full `cargo test -p slug_bzlmod --
  --nocapture`, `cargo test -p slug_common bzlmod -- --nocapture`, full `cargo
  test -p slug_external_cells -- --nocapture`, `cargo build -p slug`, the
  focused Plan 61 Python replay subset, the full Plan 61 Python guardrail with
  72 tests, `cargo fmt --check`, and `git diff --check`.
- Module-version data now injects only the selected module-version map;
  `ModuleVersionsKey` derives workspace identity through `BzlmodCellGraphKey`
  while continuing to compose lockfile, repo-env, repo-mapping, and
  resolution-fact invalidation through their named keys. The current-workspace
  helper now uses the named cell graph projection instead of reading workspace
  identity from the module-version payload. Validation passed with `cargo
  check -p slug_bzlmod -p slug_common`, focused `slug_bzlmod` coverage for
  cell-graph workspace projection, full `cargo test -p slug_bzlmod --
  --nocapture`, `cargo test -p slug_common bzlmod -- --nocapture`, full `cargo
  test -p slug_external_cells -- --nocapture`, `cargo build -p slug`, the
  focused Plan 61 Python replay subset, the full Plan 61 Python guardrail with
  72 tests, `cargo fmt --check`, and `git diff --check`.
- `BzlmodCellGraphKey` is now the single narrow DICE projection that owns
  injected bzlmod workspace identity. Lockfile inputs, repo env, repo mappings,
  resolution facts, extension aggregations, module versions, registered
  toolchains, and registered execution platforms no longer carry duplicate
  workspace ids in their injected payloads; their keys validate or derive
  identity through the named cell graph. The external-cell spoke lookup also
  reads the cell graph rather than extension-aggregation data for workspace
  identity. Validation passed with `cargo check -p slug_bzlmod -p
  slug_common`, focused `slug_bzlmod` coverage for semantic projections,
  extension aggregation projection, and absent-spoke dependency avoidance,
  focused `slug_external_cells` coverage for spoke workspace lookup, full
  `cargo test -p slug_bzlmod -- --nocapture`, `cargo test -p slug_common
  bzlmod -- --nocapture`, full `cargo test -p slug_external_cells --
  --nocapture`, `cargo build -p slug`, the focused Plan 61 Python replay
  subset, the full Plan 61 Python guardrail with 72 tests, `cargo fmt
  --check`, and `git diff --check`.
- `BzlmodSessionData` no longer carries a sibling workspace-id field beside
  the named cell graph. The legacy resolver boundary still returns a
  transitional session payload, but config-load replay and DICE injection now
  read workspace identity through `BzlmodCellGraphValue` only. Validation
  passed with `cargo check -p slug_bzlmod -p slug_common`, focused
  `slug_bzlmod` and `slug_common` session/cell-graph tests, full `cargo test
  -p slug_bzlmod -- --nocapture`, `cargo test -p slug_common bzlmod --
  --nocapture`, full `cargo test -p slug_external_cells -- --nocapture`,
  `cargo build -p slug`, the focused Plan 61 Python replay subset, the full
  Plan 61 Python guardrail with 72 tests, `cargo fmt --check`, and
  `git diff --check`.
- `BzlmodSessionData` now carries registry-file hashes and selected
  yanked-version facts as a single `BzlmodResolutionFactsValue` at the legacy
  resolver boundary. DICE injection publishes that same value through
  `BzlmodResolutionFactsDataKey` instead of rebuilding it from two parallel
  session fields. Validation passed with `cargo check -p slug_bzlmod -p
  slug_common`, focused `slug_bzlmod` session/projection tests, full `cargo
  test -p slug_bzlmod -- --nocapture`, `cargo test -p slug_common bzlmod --
  --nocapture`, full `cargo test -p slug_external_cells -- --nocapture`,
  `cargo build -p slug`, the focused Plan 61 Python replay subset, the full
  Plan 61 Python guardrail with 72 tests, `cargo fmt --check`, and
  `git diff --check`.
- `BzlmodSessionData` now carries the selected module-version map as the same
  `BzlmodModuleVersionsDataValue` injected into DICE. The legacy resolver still
  constructs the map, but session fan-out no longer rebuilds a separate module
  version value from a raw `HashMap`. Validation passed with `cargo check -p
  slug_bzlmod -p slug_common`, focused `slug_bzlmod` session/projection tests,
  full `cargo test -p slug_bzlmod -- --nocapture`, `cargo test -p
  slug_common bzlmod -- --nocapture`, full `cargo test -p slug_external_cells
  -- --nocapture`, `cargo build -p slug`, the focused Plan 61 Python replay
  subset, the full Plan 61 Python guardrail with 72 tests, `cargo fmt
  --check`, and `git diff --check`.
- `BzlmodSessionData` now carries repo env, extension aggregations, registered
  toolchains, and registered execution platforms as their existing injected
  DICE data-value structs. The legacy resolver still populates those values,
  but session fan-out no longer reconstructs them from parallel raw fields.
  Validation passed with `cargo check -p slug_bzlmod -p slug_common`, focused
  `slug_bzlmod` session/projection tests, full `cargo test -p slug_bzlmod --
  --nocapture`, `cargo test -p slug_common bzlmod -- --nocapture`, full
  `cargo test -p slug_external_cells -- --nocapture`, `cargo build -p slug`,
  the focused Plan 61 Python replay subset, the full Plan 61 Python guardrail
  with 72 tests, `cargo fmt --check`, and `git diff --check`.
- `BzlmodSessionData` now carries repo mappings and root override rows as the
  same `BzlmodRepoMappingsDataValue` injected into DICE. Resolver assembly
  still mutates local mapping variables while building extension/generated repo
  rows, but the final session payload no longer stores parallel raw mapping
  fields. Validation passed with `cargo check -p slug_bzlmod -p slug_common`,
  focused `slug_bzlmod` session/projection tests, full `cargo test -p
  slug_bzlmod -- --nocapture`, `cargo test -p slug_common bzlmod --
  --nocapture`, full `cargo test -p slug_external_cells -- --nocapture`,
  `cargo build -p slug`, the focused Plan 61 Python replay subset, the full
  Plan 61 Python guardrail with 72 tests, `cargo fmt --check`, and
  `git diff --check`.

## Consolidated Learnings

What worked:

- Root `MODULE.bazel` and included module segments are now read through tracked
  DICE filesystem inputs in the persisted config path. Visible/hidden lockfile
  reads are still bridge keys and remain a separate tracked-input migration
  surface.
- Extension evaluation and extension repository execution now run through DICE
  keys instead of immediate startup-side materialization.
- Lockfile replay reads, facts validation, registry checksum policy,
  yanked-version policy, include invalidation, and local/non-registry override
  module input invalidation gained focused guardrails.
- Repository materialization no longer blindly trusts stale marker files. The
  marker path distinguishes known repo-spec/output-state cases and rejects old
  incomplete layouts. Slug output-state markers are now verified against the
  current materialized tree before the DICE repository execution path or the
  external-cell marker gate accepts them.
- The process-global legacy bzlmod resolution bridge cache was removed from
  the persisted config load path. Warm no-op reuse now has to come from the
  DICE key path rather than `LEGACY_BZLMOD_RESOLUTION_CACHE`; focused warm
  guardrails and the full Plan 61 guardrail file pass after this change.
- Dynamic extension repo mapping learned exact canonical generated repository
  identities, sibling generated repos, and root `override_repo()` mappings.
- Several non-bzlmod analysis blockers were fixed while driving the SDK
  frontier: Rust allocator bootstrap analysis, label flag provider forwarding,
  lazy C++ runtime demand, configured dependency edge kind preservation,
  canonical external symlinks, module-extension `Label()` lexical repository
  behavior, generated-file ownership under per-action execroots, and retained
  execroot cleanup.

What did not work or remains risky:

- A single transitional `LegacyBzlmodResolutionDiceKey` still wraps the legacy
  resolver. Its command-policy identity now comes from a DICE key, but the
  wrapped resolver is still not a Skyframe-shaped module graph.
- The resolved graph, repo-mapping snapshots, cell graph, and registered
  toolchain and execution platform facts are still assembled during legacy
  cell setup, then injected as transitional command data. Registered
  toolchain/platform consumers, extension aggregation consumers, extension
  replay-input consumers, repo-mapping consumers, and module-version consumers
  now read narrower injected DICE values. The named cell graph now owns
  workspace identity for those projections rather than duplicating it in each
  injected payload. The module-version value still carries a conservative
  session invalidation identity until the remaining
  interpreter/materialization inputs are explicit, and `BzlmodSessionData`
  still exists as the legacy resolver payload even though it is no longer an
  injected DICE key and no longer sits behind a separate
  `BzlmodResolutionResult` wrapper. The session payload now carries workspace
  identity only inside its named cell graph and carries resolution facts,
  module versions, repo env, repo mappings, extension aggregations, and
  registrations as the same values injected into DICE, but the narrower
  injected values are still populated from the legacy resolver output.
- Non-root module parsing for extension aggregation is now a named DICE key, but
  module source discovery, fetch/cache layout, selected graph construction, and
  the final parsed-module list still live inside the legacy resolution bridge.
- Visible workspace lockfile content is now a tracked project-file DICE input.
  Hidden lockfile identity is included in the transitional bridge key equality
  and hashing path, and hidden replay has same-daemon edit coverage. Extension
  replay no longer reopens those lockfiles after the tracked values are computed.
  Broader hidden lockfile replay/fail-open behavior now has stronger guardrails,
  and replay/module-version consumers now depend on a named lockfile-input key,
  but that key is still populated from injected transitional resolver output
  rather than final lockfile/replay-input producer keys.
- Extension `.bzl` transitive digests are still best-effort. Project-local
  literal loads, missing project-local load paths, and existing external files
  under `bazel-external/<repo>` are hashed, missing mapped external load paths
  are included in the transitional digest, and repo mappings are applied where
  the caller has a `RepoMappingSnapshot`. Same-daemon generated-repo access now
  rejects replay after mapped external helper create/edit/delete transitions
  and after a missing project-local helper is created. Other external load
  failures, audit-cell-only external load changes, and the full interpreter
  load graph are not replay-complete. The digest now has explicit DICE keys for
  spoke lookup invalidation and for the legacy root replay-summary bridge. The
  root bridge key reads project-local implementation files through
  `DiceFileComputations`, but both digest producers intentionally mark
  themselves invalid across transactions because the shared scanner is still
  not the actual Starlark loader graph and some path discovery remains
  transitional. Production extension execution no longer exposes convenience
  constructors that recompute this digest directly; remaining direct-digest
  constructors are compiled only for tests.
- Extension spoke materialization no longer uses a bzlmod process-global
  registry or extension-name-only scans for sibling lookup. Generated repo
  materialization now goes through DICE lookup keys with workspace identity,
  projects canonical repos to owning extension ids, reads per-extension
  aggregation projections, leaves repo-mapping and replay-input reads to the
  final execution key, uses DICE spoke repo-env where available, and threads
  the exact `WorkspaceId` into extension execution and the sync
  materialization bridge instead of re-deriving workspace identity from project
  root; the late-bound executor API now rejects missing workspace identity.
  Extension aggregation projections now read root-module identity from the
  named cell-graph key instead of duplicating it in aggregation data, but the
  aggregation map is still produced by the legacy resolver.
  Generated repo cells and dynamic alias registration now go through a
  typed runtime install snapshot and installer boundary in `slug_core::cells`,
  but the backing state is still process-global transitional cell-registration
  plumbing rather than a final DICE-owned cell graph. The direct public helpers
  for forming extension execution and spoke keys from injected session-shaped
  values were removed, reducing accidental bypass surfaces without changing
  that remaining cell-graph ownership gap.
- `use_repo_rule()` no longer has a duplicate eager execution/replay path, but
  the generated repo cell graph that exposes those `RepoSpec`s is still
  assembled by the transitional legacy cell parser. Extension repo-spec
  capture remains thread-local plumbing, but the capture scope now restores
  previous state on drop instead of clearing ambient state unconditionally.
  MODULE/repo-rule invocation capture uses the same restore-on-drop guard shape
  for its thread-local registry.
- Known-spec extension repo file-ops access now routes through the DICE
  repository execution/materialization manifest key, but
  `RepoMaterializationManifestKey` still relies on polling marker/layout and
  recorded-input state through child DICE keys. This is cacheable at the parent
  manifest layer but remains transitional until the child reads are backed by
  lower-level tracked filesystem keys instead of direct `std::fs` polling.
  Output-state marker digest checks now catch corrupt existing repo trees when
  materialization is requested or when the external-cell gate inspects an
  existing directory, but already-loaded same-daemon package/target state can
  still avoid asking the repository materialization key until a higher-level
  invalidation path reaches it.
- Module extension and repository rule Starlark APIs now read their effective
  repo environment from explicit contexts seeded by command-key inputs.
  Extension execution and module-version invalidation now consume that command
  repo env through `BzlmodRepoEnvKey`. The generated repo cell graph that
  exposes repository rule specs remains transitional, and the repo-env key is
  still populated from injected resolver output, but repo-env itself no longer
  comes from the interpreter build-config adapter or a materialization-time
  injected-session lookup at runtime.
- Registered toolchain and execution-platform facts now reach analysis through
  narrower DICE values, and the eager-load fast path is keyed by the
  DICE-derived registration signature, but the final `DeclaredToolchainInfo`
  registry remains process-global output plumbing rather than a DICE value.
  The legacy setup path also no longer scans `bazel-external` for
  diagnostic-only pending toolchain repo logs; removing that poll reduces
  incidental filesystem state without completing the final DICE-owned
  toolchain/materialization graph.
- Repository-rule watched inputs are now captured in a sidecar, and root-file,
  recursive `watch_tree()`, and repo-env reads participate in same-daemon DICE
  invalidation. This is still marker/layout plumbing rather than a final
  DICE-owned repository materialization manifest.
- The external `+` repo fix only tightens the transitional literal-load scanner.
  It still does not replace the required Starlark loader graph with repo
  mappings, load failures, and delete transitions.
- Dynamic generated-repo state is still held in process-global maps. Clearing
  the suffix cache, making suffix lookup deterministic, and scoping registered
  dynamic entries, promoted dynamic entries, and directory-scan cache entries to
  the active project root close leaks in the transitional reset/lookup path.
  Root-cell and non-root cell-name adapters are also root-scoped now, and the
  legacy resolver now publishes the assembled cell graph as
  `BzlmodCellGraphDataKey`, including bundled bzlmod cells. Runtime
  installation and cell-resolver assembly consume that published value, and
  exact lazy generated-repo lookup can now instantiate from the resolver-local
  runtime snapshot before using process globals. The graph is still
  legacy-produced, and apparent/scoped alias compatibility plus runtime
  registration remain process-global transitional plumbing, so this does not
  yet make the runtime bzlmod cell graph DICE-owned.
- Some Bazel 9 semantics are explicitly rejected until fully modeled, including
  override patch materialization and isolated extension usages. Remaining
  command policy around non-root dev dependencies still needs migration out of
  the transitional resolver.
- Registry and repository cache behavior is still a blend of DICE identity,
  lockfile checksums, and filesystem markers. It is better than the old path,
  but it is not yet a complete `RepoSpecFunction` /
  `RepositoryFetchFunction`-shaped graph.
- Warm reuse must not be confused with replay correctness. A cache hit is only
  valid when every Bazel-relevant input is represented in the key or in a
  tracked dependency edge; removing the process-global bridge cache is only one
  step while the legacy resolver key still wraps an opaque graph.

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

## Remaining Work

1. Replace the legacy resolution bridge.
   - Delete or strictly demote `LegacyBzlmodResolutionDiceKey`.
   - Build the resolved graph from DICE-owned module-file/source keys.
   - Ensure graph identity includes every command policy value that Bazel uses:
     lockfile mode, repo env, nonstrict repo env, registry config, network
     policy, yanked-version allow-list, compatibility policy, and extension
     isolation.
   - Module-version/toolchain/platform consumers, current-workspace helpers,
     and the remaining session bridge now derive workspace identity from the
     named cell graph, but their data payloads are still injected from the
     legacy resolver.
   - Prove warm reuse by DICE cutoffs, not by a process-global bridge cache.
     The process-global fast path is removed, but the transitional key still
     wraps the legacy resolver.

2. Finish module-file DICE inputs for git, archive, and out-of-project local
   override/registry-cache sources.
   - Root, included, and project-local local override module segments now use
     tracked project-file DICE inputs; out-of-project local override module
     files are polled into key identity. Keep extending that shape to every
     non-root module source.
   - Registry cache `MODULE.bazel`, `source.json`, and `bazel_registry.json`
     files are tracked when the cache lives under the project root, and
     out-of-root cache paths are polled into key identity while the final
     watched-input graph is still pending.
   - Replace remaining direct `std::fs` validity hacks with tracked filesystem
     dependencies or equivalent DICE input nodes. Repository materialization
     marker/layout/recorded-input reads are now child DICE nodes, but those
     children still poll until the tracked filesystem API is available below
     `slug_common`.
   - Include create/delete transitions, parse failures, include cycles, and
     UTF-8 failures for every module source class.
   - Model registry selection and source metadata for overrides.

3. Make lockfile replay complete.
   - Visible workspace lockfile bytes now use tracked project-file DICE inputs;
     hidden/output-base lockfile bytes are polled into lockfile key identity
     while the final watched-input graph is still pending.
   - Preserve Bazel's hidden-lockfile fail-open behavior without hiding
     invalidation.
   - Preserve same-daemon hidden-lockfile create/edit/delete/facts coverage
     while moving the implementation out of the transitional graph.
   - Model facts, selected yanked versions, registry file hashes, recorded
     inputs, and lockfile mode as explicit dependencies.
   - Keep ordinary build/query paths read-only; count write attempts as test
     failures unless the command is explicitly a lockfile update command.

4. Replace best-effort extension `.bzl` digesting with the actual loaded module
   graph.
   - Reuse the Starlark loader or expose its load graph to bzlmod keys.
   - Keep the current external `bazel-external/<repo>` and mapped literal-load
     digest coverage while replacing it with file digest changes from the
     actual loader graph, load failures, and deleted files.
   - Reject replay when any loaded implementation file changes, not only
     literal loads that the transitional scanner can find.

5. Move extension spoke and generated repo registration out of process globals.
   - Represent generated repo specs, sibling spokes, seeded cells, and
     materialization state as DICE values.
   - `SEEDED_EXTENSIONS` and `SPOKE_REGISTRY` are removed as bzlmod semantic
     state. Continue moving generated repo cell registration and materialized
     output state into DICE-owned values.
   - Runtime file-ops spoke lookup now uses the injected bzlmod workspace
     identity instead of deriving one from the project root, but generated repo
     cell registration itself remains process-global transitional plumbing.
   - Extension repo execution/materialization keys now preserve the workspace
     identity and output base, but generated repo cell graph ownership and
     final materialization state are still not fully DICE-owned.
   - Dynamic generated-repo cell maps are now project-root scoped, but they
     remain process-global maps rather than a DICE-owned `BzlmodCellGraphKey`.
     Resolver-local promoted dynamic cells also carry project-root identity, but
     remain a cache over that transitional global graph. Directory-scan caches
     are also root-scoped, including negative entries, but still process-local
     lookup accelerators rather than DICE inputs.
   - Temporary root-cell and non-root cell-name adapters are project-root scoped,
     but remain process-global compatibility adapters.
   - `BzlmodCellGraphDataKey` now names the legacy-produced cell graph in DICE,
     including bundled bzlmod cells, and resolver assembly, runtime
     installation, module-version projection, extension-aggregation projection,
     and registered toolchain/platform projection consume that value. Remaining
     installed lookup state still needs to depend on it instead of
     process-global maps.
   - Ensure two workspaces and two command policies cannot share generated repo
     state by accident.

6. Complete Bazel 9 directive semantics.
   - Implement or explicitly Bazel-ground the behavior for
     remaining `dev_dependency` surfaces, `single_version_override(registry/patches)`,
     `multiple_version_override(registry)`, `archive_override`, `git_override`,
     `override_repo`, remaining `inject_repo` validation, and isolated
     extension usages.
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
     does not yet own the full repository output-tree identity.
   - Ensure local repository rules are non-cacheable where Bazel does not reuse
     cached local repository contents.

8. Make the bzlmod cell graph a DICE value.
   - Derive module cells, extension-generated cells, aliases, scoped mappings,
     external symlinks, and bundled repos from DICE values.
   - `BzlmodCellGraphDataKey` currently exposes the legacy-produced graph, but
     the graph is not yet derived from DICE producers and is not the installed
     lookup authority.
   - Ensure cell graph changes invalidate analysis and package loading
     correctly in the same daemon.
   - Prove apparent aliases do not leak across module scopes.

9. Delete transitional APIs.
   - Remove `BzlmodSessionData` fields as the authority for graph semantics.
   - The config-load command repo-env global readback and module/repository
     runtime repo-env adapters are removed; keep repo-env wired through explicit
     DICE/key inputs as the graph migrates.
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
