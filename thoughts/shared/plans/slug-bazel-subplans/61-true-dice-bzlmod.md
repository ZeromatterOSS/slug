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
- The former `LegacyBzlmodResolutionDiceKey` now includes hidden lockfile identity in
  equality and hashing, matching the existing visible-lockfile bridge identity.
  A focused Rust regression test covers this key property, and the hidden
  lockfile Python guardrail subset plus the full Plan 61 guardrail file pass.
- Hidden-lockfile replay now has a same-daemon edit guardrail: a generated repo
  first replays from the daemon hidden lockfile, then editing that hidden
  lockfile removes the cached extension entry and forces the extension to run
  and fail instead of reusing stale replay state.
- Hidden/output-base lockfile content is no longer carried as an observed
  payload through the lockfile bridge key. `TrackedLockfileContentKey` now reads
  through the normal text-file input path, and out-of-project hidden lockfiles
  contribute only a poll digest to the key identity so warm no-op reuse is
  preserved while edits still create a new key. Bridge burn-down note: the
  production surface reduced is direct hidden lockfile content injection via
  `hidden_lockfile_observed` / `observed: Option<AbsoluteTextFileInputValue>`.
  The intended owner is a named lockfile input chain:
  `BzlmodLockfileInputsBridgeKey` -> `TrackedLockfileContentKey` ->
  project-file DICE deps or `AbsoluteTextFileInputKey`, with the final owner
  still a true lockfile policy/value graph. Before/after evidence:
  `rg -n "hidden_lockfile_observed|observed: Option<AbsoluteTextFileInputValue>|observed: self\\.hidden_lockfile_observed|observed: None" app/slug_common/src/legacy_configs/cells.rs`
  now returns no hits. Validation passed with `cargo test -p slug_common
  tracked_lockfile_content_key_identity_includes_poll_digest -- --nocapture`,
  `cargo test -p slug_common bzlmod_lockfile_inputs_bridge -- --nocapture`,
  `cargo check -p slug_common`, `cargo build -p slug`, the explicit-binary Plan
  61 hidden-lockfile selector for read observability, fail-open malformed
  hidden lockfiles, and same-daemon hidden-lockfile edit invalidation (`3
  passed, 152 deselected`), `cargo fmt --check`, and `git diff --check`.
  Hidden-lockfile identity follow-up: the out-of-project hidden lockfile poll
  digest is no longer computed before `BzlmodLockfileInputsBridgeKey`
  construction or carried in `TrackedLockfileContentKey` identity.
  `TrackedLockfileContentKey` now owns the text-file read and reports validity
  only for project-file DICE-tracked values; out-of-project lockfile values are
  invalid across committed transactions until a lower-level watched filesystem
  key replaces the direct poll. Bridge surface reduced: the hidden lockfile
  path no longer has a pre-key `absolute_text_file_input_poll_digest` /
  `absolute_text_file_digest` read. Focused validation passed with `cargo test
  -p slug_common bzlmod_lockfile_inputs_bridge -- --nocapture` and `cargo test
  -p slug_common tracked_lockfile_content_key -- --nocapture`.
- Latest Plan 61 guardrail validation passed with
  `TMPDIR=/var/mnt/dev/.slug-tmp TEST_EXECUTABLE=/var/mnt/dev/slug/target/debug/slug
  python -m pytest -q tests/core/bzlmod/test_plan61_guardrails.py -rx --tb=short`
  (`146 passed in 106.22s`) after rebuilding `target/debug/slug`.
- Root-main-repo `archive_override(patches = ...)` and
  `git_override(patches = ...)` are now supported for direct non-registry
  override fetches: Slug validates Bazel's main-repository patch-label rule,
  including explicit root `repo_name` spellings such as
  `@root_repo//:fix.patch`, applies local patch labels after fetch/extract and
  before reading the override `MODULE.bazel`, includes `patches`,
  `patch_strip`, and local patch file bytes in the non-registry override cache
  directory identity, and materializes patched BUILD targets from the fetched
  override tree.
  `single_version_override` patch fields are also supported for registry
  modules: discovery-time patches are filtered to `MODULE.bazel` before parsing
  the registry module, final source materialization applies the same root-local
  patches plus `patch_cmds`, and patched registry sources use a cache identity
  that includes patch labels, patch bytes, `patch_strip`, and patch command
  strings. Bazel anchors:
  `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/bazel/bzlmod/ModuleFileGlobals.java`
  for override kwargs and patch-label validation,
  `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/bazel/bzlmod/ModuleFileFunction.java`
  for fetching non-registry overrides before parsing `MODULE.bazel` and for
  applying SVO patches to registry module discovery,
  `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/bazel/bzlmod/InterimModule.java`
  for appending SVO patch attrs to the final repo spec, and
  `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/bazel/repository/RepoDefinitionFunction.java`
  for root-repo patch-label conversion. Validation passed with
  `TMPDIR=/var/mnt/dev/.slug-tmp/plan61-override-patches cargo check -p
  slug_bzlmod`, focused cache identity tests, `cargo build -p slug`, and the
  explicit-binary Plan 61 selector for archive/git override patch application,
  SVO patch rejection, and external patch-label rejection (`5 passed, 149
  deselected`). A clean-review follow-up fixed the explicit root `repo_name`
  patch-label spelling and revalidated with
  `TMPDIR=/var/mnt/dev/.slug-tmp/plan61-override-root-repo cargo check -p
  slug_bzlmod`, focused cache identity tests, `cargo build -p slug`, and the
  same explicit-binary Plan 61 selector (`5 passed, 149 deselected`). Remaining
  SVO validation passed with `TMPDIR=/var/mnt/dev/.slug-tmp/plan61-svo-patches
  cargo check -p slug_bzlmod`, `cargo check -p slug_common`, focused parser,
  cache, and single-file patch helper tests, `cargo build -p slug`, and the
  explicit-binary Plan 61 selector for SVO module/source patching, patch_cmd
  failure, archive/git patch application, and external patch-label rejection
  (`5 passed, 149 deselected`). Follow-up direct-read bridge validation passed
  with `TMPDIR=/var/mnt/dev/.slug-tmp/plan61-patch-inputs cargo check -p
  slug_bzlmod`, `cargo check -p slug_common`, focused parser and patch helper
  tests, `cargo build -p slug`, and the explicit-binary Plan 61 selector for
  same-daemon SVO patch-file edit materialization plus adjacent override patch
  cases (`6 passed, 149 deselected`). The production DICE bridge now computes
  root-local override patch label contents through an `OverridePatchInputsKey`
  and feeds those bytes into the resolver/source-fetch patch digest and apply
  calls. This checkpoint still left `SourceFetcher` direct-read fallback
  helpers for non-DICE/bootstrap callers and exposed a separate
  package/cache invalidation bridge after bzlmod source paths changed.
  Bridge burn-down note for this checkpoint: the production bridge surface
  reduced is marker-trusted registry source materialization for patched
  registry modules. Before this slice, registry source cache identity was only
  `registry/modules/<name>/<version>/source`, so enabling
  `single_version_override(patches = ..., patch_cmds = ...)` would have let
  patched and unpatched trees share one completion marker. After this slice,
  `ModuleCache::source_dir_with_identity` and
  `SourceFetcher::fetch_source_with_identity` materialize patched registry
  sources under a patch-effect-specific source directory whose identity includes
  patch labels, patch bytes, `patch_strip`, and `patch_cmds`. The intended final
  owner is still a DICE-owned `ModuleSourceKey`/`RepoSpecKey` feeding
  `RepositoryExecutionKey`; this checkpoint protects that deletion path but
  does not remove the remaining direct patch-file reads. The next slice must
  reduce that direct-read bridge, not add another directive-only hardening
  layer.
  Bridge burn-down note for the direct-read follow-up: the production bridge
  surface reduced is root-local override patch file reopening inside the
  transitional resolver/source-fetch path. Before this slice, resolver call
  sites reached `std::fs::read(&patch_path)` through
  `apply_local_override_patches`, `local_override_patch_digest`,
  `local_override_patch_effect_digest`, and
  `apply_single_version_module_patches`. After this slice, production
  `MvsResolver` instances created by `BzlmodProjectionBridgeDiceKey` receive
  DICE-computed `OverridePatchInputs` and call only the `*_with_inputs`
  variants; the only remaining `std::fs::read(&patch_path)` hit is the
  non-DICE fallback helper. The intended final owner remains
  `ModuleSourceKey`/`RepoSpecKey` feeding `RepositoryExecutionKey`, but the
  patch bytes now have an auditable DICE input edge before that larger split.
  Same-daemon package/cache follow-up passed with
  `cargo check -p slug_common`, `cargo build -p slug`, and the explicit-binary
  Plan 61 selector for SVO module/source patching, same-daemon SVO patch-file
  edit package invalidation, patch_cmd failure, archive/git patch application,
  and external patch-label rejection (`6 passed, 149 deselected`). Bridge
  burn-down note for this package replay follow-up: the production bridge
  surface reduced is daemon bootstrap project-file invalidation plus bzlmod
  cell/package replay under stable external cell names. Before this slice,
  `OverridePatchInputsKey` read root-local patch labels through
  `ProjectReadFileKey`, but the daemon's pre-config project-file invalidation
  only recognized MODULE/lock/registry files; a patch edit could create the new
  `source-<patch-effect-digest>` directory while the daemon replayed the old
  `bazel-external/<module>+` symlink and package target list. In the same path,
  `CellResolverKey` was marked `InvalidationSourcePriority::Ignored`, so a
  changed `BzlmodCellSetup.source_path` did not have to invalidate file-op and
  package dependents. After this slice, bzlmod project-file bootstrap reads
  register their exact `ProjectReadFileKey` paths for pre-config invalidation,
  and `CellResolverKey` uses normal injected-key invalidation. The intended
  owner is `OverridePatchInputsKey` feeding `BzlmodProjectionBridgeDiceKey` /
  `BzlmodCellGraphKey`, with package and file-op keys depending on the active
  resolver rather than stale symlink state. The remaining bridge is still the
  transitional resolver wrapper and the non-DICE patch-read fallback helper,
  not same-daemon replay for root-local override patch edits.
  Clean-review follow-up for this package replay slice found one remaining
  production bridge in the Watchman backend: `WatchmanFileWatcher::sync`
  inferred pre-config commits by rescanning event paths for `MODULE.bazel` /
  `.MODULE.bazel`, so dynamically registered bzlmod project inputs such as
  override patch files could still miss the pre-config commit on that backend.
  After this follow-up, `WatchmanQueryProcessor` returns
  `FileChangeTracker::requires_pre_config_commit()` in `WatchmanSyncOutput`,
  `WatchmanFileWatcher::sync` uses that DICE-side project-file invalidation bit
  just like notify/Eden/fs-hash, and the Watchman-specific suffix scan is gone.
  The intended owner is `ProjectReadFileKey` invalidation through
  `FileChangeTracker`, feeding `OverridePatchInputsKey` and the bzlmod cell
  graph rather than backend-local path heuristics. Validation passed with
  `cargo check -p slug_file_watcher`, `cargo build -p slug`, the same
  explicit-binary Plan 61 selector (`6 passed, 149 deselected`),
  `cargo fmt --check`, and `git diff --check`.
  Bridge burn-down note for the direct-read fallback deletion: the production
  bridge surface reduced is the implicit patch-file reopening fallback inside
  `SourceFetcher` itself. Before this slice, `apply_local_override_patches`,
  `local_override_patch_digest`, `local_override_patch_effect_digest`, and
  `apply_single_version_module_patches` could all pass `None` through
  `local_override_patch_content`, which then reopened the root patch file from
  disk during digesting or application. After this slice, those no-input helpers
  are gone, every patch digest/apply helper requires an `OverridePatchInputs`
  value, `MvsResolver` carries an `Arc<OverridePatchInputs>` rather than an
  optional fallback, and the only normal-command bootstrap caller constructs a
  named `OverridePatchInputs` value before invoking the resolver. The intended
  owner is still `OverridePatchInputsKey` over tracked `ProjectReadFileKey`
  inputs feeding the bzlmod resolver/cell graph; the remaining bridge is now the
  explicit no-DICE `bootstrap_override_patch_inputs` adapter in legacy config
  parsing, not a hidden read fallback in source fetching. Before/after evidence:
  `rg -n "std::fs::read\\(&patch_path\\)|patch_inputs: Option<&.*OverridePatchInputs|apply_local_override_patches\\(|local_override_patch_digest\\(|local_override_patch_effect_digest\\(|apply_single_version_module_patches\\(" app/slug_bzlmod/src app/slug_common/src tests -g '*.rs' -g '*.py'`
  now returns no hits. Validation passed with `cargo test -p slug_bzlmod
  override_patch_helpers_require_tracked_inputs -- --nocapture`, `cargo test -p
  slug_bzlmod single_version_module_patch_skips_non_module_hunks --
  --nocapture`, `cargo check -p slug_bzlmod`, `cargo check -p slug_common`,
  `cargo build -p slug`, the same explicit-binary Plan 61 selector (`6 passed,
  149 deselected`), `cargo fmt --check`, and `git diff --check`.
  Constructor follow-up for the same bridge: the production surface reduced is
  implicit resolver construction without an explicit patch-input owner. Before
  this slice, `MvsResolver::new` and `MvsResolver::with_registry` installed
  `OverridePatchInputs::default()` internally, `resolve_with_lockfile` could
  construct that empty-input resolver shape, and legacy config parsing patched
  the owner in afterward with `set_override_patch_inputs`. After this slice, the
  resolver constructors and `resolve_with_lockfile` require an
  `Arc<OverridePatchInputs>`, the setter is gone, and normal config parsing
  passes either the DICE-computed `OverridePatchInputsKey` value or the explicit
  `bootstrap_override_patch_inputs` adapter at construction time. Before/after
  evidence: `rg -n "MvsResolver::new\\(|MvsResolver::with_registry\\(|set_override_patch_inputs\\(" app/slug_bzlmod/src app/slug_common/src tests -g '*.rs' -g '*.py'`
  now shows only two explicit constructor calls, both with `override_patch_inputs`
  arguments. Validation passed with `cargo test -p slug_bzlmod
  override_patch_helpers_require_tracked_inputs -- --nocapture`, `cargo test -p
  slug_bzlmod single_version_module_patch_skips_non_module_hunks --
  --nocapture`, `cargo check -p slug_bzlmod`, `cargo check -p slug_common`,
  `cargo build -p slug`, the same explicit-binary Plan 61 selector (`6 passed,
  149 deselected`), `cargo fmt --check`, and `git diff --check`.
  Extension `.bzl` digest follow-up: the production bridge surface reduced is
  scanner fallback inside the DICE replay-input digest key. Before this slice,
  `ExtensionBzlTransitiveDigestKey` returned a `dice_tracked=false` value by
  calling the literal-load filesystem scanner when no aggregation was available;
  that kept a fallback-scanner digest path inside the normal DICE key. After
  this slice, missing aggregation is an error for direct digest-key computation,
  successful digest values are always executor/loaded-graph values, and the
  `dice_tracked` split is gone. The intended owner is
  `ModuleExtensionReplayInputKey` fed by `ExtensionBzlTransitiveDigestKey` over
  the loaded Starlark graph; the scanner remains only in explicit non-DICE
  bootstrap/preseed helpers. Before/after evidence:
  `sed -n '260,307p' app/slug_bzlmod/src/extension_execution_dice.rs | rg -n "compute_bzl_transitive_digest_for_project_with_repo_mappings|BzlmodRepoMappingsKey|dice_tracked"`
  and
  `rg -n "dice_tracked|ExtensionBzlTransitiveDigestValue::new\\([^\\n]+,\\s*(true|false)" app/slug_bzlmod/src/extension_execution_dice.rs`
  now return no hits. Validation passed with `cargo test -p slug_bzlmod
  extension_bzl_digest_key_rejects_missing_aggregation -- --nocapture`,
  `cargo test -p slug_bzlmod
  extension_spokes_lookup_keys_cache_after_digest_dependency -- --nocapture`,
  `cargo check -p slug_bzlmod`, `cargo build -p slug`, the explicit-binary Plan
  61 extension replay selector for local/transitive/mapped `.bzl` edits,
  creations, and deletions (`6 passed, 149 deselected`), `cargo fmt --check`,
  and `git diff --check`.
  Preseed digest follow-up: the remaining explicit lockfile preseed scanner now
  runs inside `TrackedExtensionBzlDigestKey::compute` instead of being polled
  before key construction and injected as `poll_digest`. Bridge surface
  reduced: extension `.bzl` digest preseed no longer hides a direct scanner read
  in key identity; the named DICE key owns the transitional scan and is invalid
  across transactions until the Starlark loaded-module graph replaces it.
  Focused validation passed with `cargo test -p slug_common
  tracked_extension_bzl_digest -- --nocapture`.
- Runtime bzlmod module symlink replay now writes `external_cells/bzlmod` under
  `BzlmodCellGraphValue.workspace_id.output_base` rather than hard-coding
  `<project>/buck-out/v2`; focused coverage verifies a custom output base gets
  the symlink and the default path is untouched. Validation passed with focused
  `cargo test -p slug_common
  bzlmod_runtime_state_uses_workspace_output_base_for_external_cell_symlinks
  -- --nocapture`, `cargo check -p slug_common -p slug_server`,
  `cargo build -p slug`, `cargo fmt --check`, and `git diff --check`.
- Extension-generated repository symlink replay now also writes
  `external_cells/extension_repo` under the current bzlmod workspace output
  base read from the named bzlmod cell-graph projection instead of hard-coding
  `<project>/buck-out/v2`. Focused coverage verifies a custom output base gets
  the symlink and the default path is untouched. Validation passed with focused
  `cargo test -p slug_external_cells
  extension_repo_symlink_uses_workspace_output_base -- --nocapture`,
  `cargo check -p slug_external_cells -p slug_server`, `cargo build -p slug`,
  `cargo fmt --check`, and `git diff --check`.
- Extension repo file-ops now leaves recorded-input staleness for known
  repo-spec materializations to `ExtensionRepoExecutionKey` and its
  `RepoMaterializationManifestKey` child state, instead of deleting the repo
  from a pre-DICE file-ops check first. The no-spec fallback keeps the legacy
  precheck because it has no manifest key yet, and repository-execution miss
  classification now uses the computed manifest marker state rather than a
  redundant marker `exists()` read. Validation passed with focused
  `cargo test -p slug_external_cells
  known_repo_spec_defers_recorded_input_staleness_to_manifest -- --nocapture`,
  `cargo test -p slug_bzlmod
  materialization_manifest_key_observes_marker_state_dependency -- --nocapture`,
  `cargo test -p slug_bzlmod
  extension_repo_execution_consumes_materialization_manifest_key -- --nocapture`,
  `cargo check -p slug_bzlmod -p slug_external_cells -p slug_server`,
  `cargo build -p slug`, `cargo fmt --check`, and `git diff --check`.
- Known repo-spec materialization layout now treats a repo with declared
  `build_file`/`build_file_content` but no `BUILD`/`BUILD.bazel` as
  `layout-missing-build-file` inside `RepoMaterializationManifestKey`, rather
  than letting extension file-ops pre-delete the repo before the manifest key
  runs. Validation passed with focused `cargo test -p slug_bzlmod
  materialization_manifest_layout_rejects_missing_declared_build_file
  -- --nocapture`, `cargo test -p slug_external_cells
  known_repo_spec_defers_recorded_input_staleness_to_manifest -- --nocapture`,
  `cargo check -p slug_bzlmod -p slug_external_cells -p slug_server`,
  `cargo build -p slug`, `cargo fmt --check`, and `git diff --check`.
- Known repo-spec extension file-ops no longer performs its own
  `repo_spec_layout_is_invalid` probe before repository execution; the
  existing `RepoMaterializationManifestKey` layout child is now the single
  owner for those layout-validity misses. Validation passed with focused
  `cargo test -p slug_external_cells
  known_repo_spec_defers_recorded_input_staleness_to_manifest -- --nocapture`,
  `cargo test -p slug_bzlmod
  materialization_manifest_layout_rejects_missing_declared_build_file
  -- --nocapture`, `cargo check -p slug_bzlmod -p slug_external_cells -p
  slug_server`, `cargo build -p slug`, `cargo fmt --check`, and
  `git diff --check`.
- Known repo-spec extension file-ops now also leaves non-complete markers,
  marker/spec mismatches, and output-state marker mismatches to
  `RepoMaterializationManifestKey`; those pre-DICE stale checks now run only
  for the no-spec fallback that lacks a manifest key. Validation passed with
  focused `cargo test -p slug_external_cells
  known_repo_spec_defers_recorded_input_staleness_to_manifest -- --nocapture`,
  `cargo test -p slug_bzlmod
  materialization_manifest_key_observes_marker_state_dependency -- --nocapture`,
  `cargo check -p slug_external_cells -p slug_bzlmod -p slug_server`,
  `cargo build -p slug`, `cargo fmt --check`, and `git diff --check`.
- Foreign top-level symlink detection for known repo-spec materializations now
  belongs to `RepoMaterializationManifestKey` layout state as
  `layout-foreign-top-level-symlink`; extension file-ops keeps that direct
  check only for the no-spec fallback. Validation passed with focused
  `cargo test -p slug_bzlmod
  materialization_manifest_layout_rejects_foreign_top_level_symlink
  -- --nocapture`, `cargo test -p slug_external_cells
  known_repo_spec_defers_recorded_input_staleness_to_manifest -- --nocapture`,
  `cargo check -p slug_bzlmod -p slug_external_cells -p slug_server`,
  `cargo build -p slug`, `cargo fmt --check`, and `git diff --check`.
- Legacy invalid empty target-label detection for known repo-spec
  materializations now belongs to `RepoMaterializationManifestKey` layout state
  as `layout-invalid-empty-target-label`; extension file-ops no longer performs
  the in-place BUILD-file repair side effect and no longer reads the completion
  marker for known repo specs before forming `ExtensionRepoExecutionKey`.
  Validation passed with focused `cargo test -p slug_bzlmod
  materialization_manifest_layout_rejects_invalid_empty_target_label
  -- --nocapture`, `cargo test -p slug_external_cells
  known_repo_spec_defers_recorded_input_staleness_to_manifest -- --nocapture`,
  `cargo check -p slug_bzlmod -p slug_external_cells -p slug_server`,
  `cargo build -p slug`, `cargo fmt --check`, and `git diff --check`.
- The DICE-backed bzlmod projection bridge now fails if it reaches the legacy resolver
  without the tracked root `MODULE.bazel` parse result or tracked visible
  lockfile value, and similarly refuses to direct read a configured hidden
  lockfile when the tracked hidden value was not supplied. This keeps direct
  root module and lockfile fallback limited to non-DICE bootstrap paths.
  Validation passed with focused `cargo test -p slug_common
  'bzlmod_projection_bridge_requires_tracked' -- --nocapture`,
  `cargo check -p slug_common -p slug_server`, `cargo build -p slug`, and
  focused Plan 61 Python guardrails
  `visible_lockfile_read_is_observable_and_ordinary_audit_is_read_only` plus
  `hidden_lockfile_read_is_observable_before_extension_replay`.
- Precomputed extension repo setups that lack embedded `repo_spec_json` now
  validate through the current `ExtensionSpokesKey` repo spec when that spoke
  value exists, instead of trusting the legacy marker prechecks. The direct
  marker scan remains only for the no-spoke fallback used by direct
  use-repo-rule cells. Validation passed with focused `cargo test -p
  slug_external_cells known_repo_spec_defers_recorded_input_staleness_to_manifest
  -- --nocapture`, `cargo check -p slug_external_cells`, `cargo build -p
  slug`, and focused Plan 61 Python guardrails
  `missing_lockfile_extension_executes_once_then_reuses_dice_state` plus
  `valid_lockfile_replay_materializes_generated_repo_without_extension_eval`;
  `cargo fmt --check` and `git diff --check` also passed.
- Extension repository execution now calls a fresh native repository-rule
  executor path after `RepoMaterializationManifestKey` has classified reuse,
  so the manifest-owned extension path no longer falls through the native
  executor's marker shortcut. The legacy marker-reuse executor entrypoint is
  now test-only and is no longer exported from `slug_bzlmod`; production native
  repository execution uses the fresh path after manifest classification.
  Validation passed with focused `cargo test -p slug_bzlmod
  fresh_repository_execution_bypasses_marker_shortcut -- --nocapture`,
  `cargo test -p slug_bzlmod
  extension_repo_execution_consumes_materialization_manifest_key -- --nocapture`,
  `cargo check -p slug_bzlmod -p slug_external_cells -p slug_server`,
  `cargo build -p slug`, and focused Plan 61 Python guardrail
  `materialized_repo_marker_revalidates_corrupted_output_digest`.
- Hidden-lockfile facts now have same-daemon create/edit/delete coverage: an
  extension reads `module_ctx.facts` from the daemon hidden lockfile, succeeds
  when the hidden facts are created with the expected value, fails after an
  edit to stale facts, succeeds after restoration, and fails again after the
  hidden lockfile is deleted.
- Unsupported extension replay recorded inputs now have same-daemon coverage:
  a lockfile entry with matching digests/specs but an unsupported recorded
  `FILE` path form is rejected as an extension replay miss and does not count
  as a replay hit. Validation passed with focused Plan 61 guardrail
  `test_lockfile_replay_unsupported_recorded_input_rejects_cache` and the
  recorded-input replay subset selected by `-k 'lockfile_replay_recorded'`.
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
- Root `use_repo_rule(...)(..., dev_dependency = True)` now carries the
  `dev_dependency` bit into repo-rule invocations, participates by default, and
  is excluded under `--ignore_dev_dependency`; non-root dev repo rules are
  filtered from precomputed and eager repo-rule registration paths. The
  `use_repo_rule()` factory itself rejects `dev_dependency`, matching Bazel's
  parameter split between `ModuleFileGlobals.useRepoRule` and
  `RepoRuleProxy.call`.
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
- Non-root `override_repo()` usages are now ignored when constructing module
  repo mappings and precomputed extension repo aliases, matching Bazel's
  `ModuleFileGlobals.overrideRepo` docs and early return before `addOverride`
  when the current module is not allowed to contribute dev-dependency-scoped
  directives. A same-daemon Plan 61 guardrail proves a dependency module's
  `override_repo(ext, generated = "helper")` does not redirect its
  `use_repo(ext, "generated")` import to `@helper`; the generated repo remains
  visible. Validation passed with focused `slug_bzlmod` repo-mapping and
  pending-cell tests, `cargo build -p slug`, and the Plan 61 guardrail selected
  by `-k 'non_root_override_repo_is_ignored'` (`1 passed, 123 deselected`).
- After adding the non-root `override_repo()` guardrail, the full Plan 61
  Python guardrail passed with `124 passed in 126.29s`; no stale `slugd`
  process remained after cleanup.
- Non-root `inject_repo()` usages now take the same ignored-directive path:
  root module parsing keeps strict duplicate/injected-use_repo validation, but
  non-root module-file parsing records the directives without failing before
  the resolver can ignore them. This matches `ModuleFileGlobals.injectRepo`,
  whose docs and early return use the same root-only/ignore-dev-dependency
  policy as `override_repo()`. Validation passed with focused `slug_bzlmod`
  parser tests, `cargo check -p slug_bzlmod -p slug_common`, `cargo build -p
  slug`, and the Plan 61 guardrail selected by `-k
  'non_root_inject_repo_is_ignored'` (`1 passed, 124 deselected`).
- After adding the non-root `inject_repo()` guardrail, the full Plan 61 Python
  guardrail passed with `125 passed in 126.27s`; no stale `slugd` process
  remained after cleanup.
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
  Same-daemon guardrails now also cover override-registry source metadata: after
  warming a `single_version_override(registry = ...)` module, corrupting that
  override registry's `source.json` with a matching lockfile hash fails the next
  `audit cell`, and repair introduces a new dependency from the override
  registry module instead of reusing the old graph. The same metadata path is
  covered for `multiple_version_override(registry = ...)` creation, deletion,
  parse, and UTF-8 failures, and `single_version_override(registry = ...)` now
  has creation, deletion-transition, and UTF-8 failure coverage for the override
  registry's selected `source.json`.
  Validation passed with `-k
  'single_version_override_registry_source_json_parse_failure or
  multiple_version_override_registry_source_json_utf8_failure'` (`2 passed, 116
  deselected`) and with `-k
  'single_version_override_registry_source_json_delete'` (`1 passed, 122
  deselected`) after adding delete-plus-repair and UTF-8-plus-repair to that
  flow; the multiple-version guardrail selected by
  `-k 'multiple_version_override_registry_source_json_utf8_failure'` now covers
  delete-plus-repair and parse-plus-repair before the UTF-8 transition (`1
  passed, 122 deselected`). Missing-to-present creation validation passed with
  `-k 'single_version_override_registry_source_json_creation or
  multiple_version_override_registry_source_json_creation'` (`2 passed, 130
  deselected`).
  `multiple_version_override(versions = ...)` now follows Bazel's parser
  minimum by rejecting fewer than two versions before resolution. Bazel source
  anchor: `ModuleFileGlobals.java:1035-1052`. The source-metadata guardrails
  use syntactically valid two-entry override lists; this closes the directive
  parser mismatch without claiming full multiple-version coexistence/selection
  support. Validation passed with `cargo test -p slug_bzlmod
  multiple_version_override -- --nocapture` (`3 passed`) and the
  explicit-binary Plan 61 selector
  `-k 'multiple_version_override_requires_two_versions or
  multiple_version_override_registry_uses_override_registry or
  multiple_version_override_registry_source_json_utf8_failure or
  multiple_version_override_registry_source_json_creation'` (`4 passed, 131
  deselected`).
  Override patch support now covers Bazel's main-repo patch-label rule,
  `single_version_override` registry `MODULE.bazel` discovery patches, SVO final
  source materialization patches and `patch_cmds`, and non-registry
  `archive_override`/`git_override` patch materialization. Slug supports
  explicit root `repo_name` labels such as `@root_repo//:fix.patch` and keeps
  patched registry module sources in a patch-identity-specific cache directory
  so patched and unpatched sources cannot share the same materialized tree.
  Bazel source anchors:
  `ModuleFileGlobals.java:522-545` and `:930-995`,
  `ModuleFileFunction.java:823-840`,
  `InterimModule.java:252-269`, and
  `ModuleFileFunctionTest.java:1717-1780` plus `:1803-1928`. Focused Plan 61
  guardrails now cover `archive_override` and `git_override` patch application,
  `single_version_override` module/source patch application,
  `single_version_override` patch command failure, and the external-repository
  patch-label error. Earlier validation passed with
  `cargo test -p slug_bzlmod patches -- --nocapture` (`4 passed`),
  `cargo test -p slug_bzlmod single_version_override_patch -- --nocapture`
  (`2 passed`), and explicit-binary Plan 61 pytest selector
  `-k 'override_patches_external_repo_labels or
  single_version_override_patches_fail_until_supported or
  archive_override_patches_fail_until_supported or
  git_override_patches_fail_until_supported'` (`4 passed, 129 deselected`).
  Follow-up explicit-binary selector
  `-k 'single_version_override_patches_fail_until_supported or
  single_version_override_patch_cmds_and_strip_fail_until_supported'` passed
  (`2 passed, 132 deselected`).
  Current archive/git patch-application validation passed with
  `TMPDIR=/var/mnt/dev/.slug-tmp/plan61-override-root-repo cargo check -p
  slug_bzlmod`, focused cache identity tests, `cargo build -p slug`, and
  explicit-binary Plan 61 selector
  `-k 'archive_override_patches_apply_to_fetched_module or
  git_override_patches_apply_to_fetched_module or
  override_patches_external_repo_labels_follow_bazel_main_repo_rule or
  single_version_override_patches_fail_until_supported or
  single_version_override_patch_cmds_and_strip_fail_until_supported'`
  (`5 passed, 149 deselected`).
  Current SVO patch validation passed with
  `TMPDIR=/var/mnt/dev/.slug-tmp/plan61-svo-patches cargo check -p
  slug_bzlmod`, `cargo check -p slug_common`, focused parser, cache, and
  single-file patch helper tests, `cargo build -p slug`, and explicit-binary
  Plan 61 selector
  `-k 'single_version_override_patches_apply_to_module_and_source or
  single_version_override_patch_cmd_failure_is_reported or
  archive_override_patches_apply_to_fetched_module or
  git_override_patches_apply_to_fetched_module or
  override_patches_external_repo_labels_follow_bazel_main_repo_rule'`
  (`5 passed, 149 deselected`). Current patch-input bridge validation passed
  with the same check/build base plus explicit-binary selector
  `-k 'single_version_override_patch_edit_materializes_same_daemon or
  single_version_override_patches_apply_to_module_and_source or
  single_version_override_patch_cmd_failure_is_reported or
  archive_override_patches_apply_to_fetched_module or
  git_override_patches_apply_to_fetched_module or
  override_patches_external_repo_labels_follow_bazel_main_repo_rule'`
  (`6 passed, 149 deselected`). Production resolver patch reads now use
  DICE-computed root-local patch inputs; direct patch-file reads remain only in
  non-DICE fallback helpers.
  Bazel module-name validation now runs while parsing `module(name = ...)`,
  `bazel_dep(name = ...)`, and every override directive's `module_name`,
  instead of only in later command-line yanked-version parsing. Bazel source
  anchors: `ModuleFileGlobals.java:70-78`, `:172-180`, `:284`,
  `:976`, `:1040`, `:1088`, `:1133`, `:1174`, and
  `ModuleFileFunctionTest.java:1320-1356`. Validation passed with
  `cargo test -p slug_bzlmod invalid_module_names -- --nocapture`
  (`1 passed`), `cargo test -p slug_bzlmod
  allowed_yanked_versions_rejects_bad_format_and_module_name -- --nocapture`
  (`1 passed`), and the explicit-binary Plan 61 selector
  `-k 'invalid_module_names_fail_at_module_parse'` (`1 passed, 135
  deselected`).
  User-provided repo-name validation now runs for `module(repo_name = ...)`,
  `bazel_dep(repo_name = ...)`, `use_repo()` imports, and
  `use_repo_rule(...)(name = ...)`, matching the Bazel parse points that do
  not depend on root/`--ignore_dev_dependency` filtering. Bazel source anchors:
  `RepositoryName.java:57-58`, `:201-207`,
  `ModuleFileGlobals.java:179`, `:301-307`, `:853-867`,
  `ModuleThreadContext.java:221-229`, and
  `ModuleFileFunctionTest.java:1360-1406`. Root-policy-sensitive
  `override_repo()`/`inject_repo()` repo-name validation is handled by the
  directive-semantics pass after Bazel's root/`--ignore_dev_dependency` early
  return, rather than being forced into the raw function calls. Validation
  passed with `cargo test -p slug_bzlmod
  user_provided_repo_names -- --nocapture` (`1 passed`) and the
  explicit-binary Plan 61 selector
  `-k 'invalid_user_provided_repo_names_fail_at_module_parse'` (`1 passed,
  136 deselected`).
  Root `override_repo()`/`inject_repo()` rows now validate both the exported
  generated repo name and the overriding visible repo name, then reject an
  overriding repo that is not visible in the current module scope. This mirrors
  Bazel's `addRepoOverride` and `buildUsage` checks while preserving the early
  ignored path for non-root or `--ignore_dev_dependency` directives. Bazel
  source anchors: `ModuleFileGlobals.java:716-735` and `:776-795`, plus
  `ModuleThreadContext.java:239-258` and `:287-299`. Validation passed with
  `cargo test -p slug_bzlmod parse_ -- --nocapture` (`62 passed`) and the
  explicit-binary Plan 61 selector
  `-k 'override_and_inject_repo_missing_visible_repo_fails_at_module_parse or
  invalid_user_provided_repo_names_fail_at_module_parse or
  non_root_override_repo_is_ignored or non_root_inject_repo_is_ignored'`
  (`4 passed, 138 deselected`).
  The command path now keeps root MODULE parsing policy-neutral for bootstrap
  and DICE input tracking, then validates those root directive rows only in the
  command/DICE projection bridge when `--ignore_dev_dependency` is inactive.
  This preserves Bazel's `shouldIgnoreDevDeps()` early return for ignored
  root `override_repo()`/`inject_repo()` rows: missing or invalid overriding
  repos do not fail before the directive is ignored, while default-mode
  commands still reject the same rows before registry resolution. Validation
  passed with `cargo test -p slug_bzlmod
  ignored_extension_repo_directives -- --nocapture` (`1 passed`) and the
  explicit-binary Plan 61 selector
  `-k 'override_and_inject_repo_are_ignored_before_validation_under_ignore_dev
  or override_and_inject_repo_missing_visible_repo_fails_at_module_parse or
  invalid_user_provided_repo_names_fail_at_module_parse or
  explicit_module_repo_name_frees_module_name_for_dep_repo or
  inject_repo_is_ignored_under_ignore_dev_dependency'`
  (`5 passed, 139 deselected`).
  Extension repo import/override bookkeeping now also matches Bazel's shared
  per-extension usage builder: duplicate imports of the same exported generated
  repo fail during `use_repo()`, independent of root/non-root module status and
  `--ignore_dev_dependency`, while root-policy-sensitive `override_repo()` and
  `inject_repo()` validation remains ignored on Bazel's ignored paths. A visible
  repo used as one override also cannot itself be overridden by another
  `override_repo()` edge. Bazel source anchors:
  `ModuleThreadContext.java:221-237`, `:300-315`, and `:390-405`, plus
  `ModuleFileFunctionTest.java:1155-1172` and `:2027-2054`. Validation passed
  with `cargo test -p slug_bzlmod parse_ -- --nocapture` (`65 passed`),
  focused `cargo test -p slug_bzlmod duplicate_imported_extension_repo
  -- --nocapture` (`2 passed`), focused `cargo test -p slug_bzlmod
  ignored_extension_repo_directives -- --nocapture` (`2 passed`), and the
  explicit-binary Plan 61 selector
  `-k 'extension_repo_import_and_override_conflicts_fail_at_module_parse or
  non_root_duplicate_extension_repo_import_fails_at_module_parse or
  override_and_inject_repo_are_ignored_before_validation_under_ignore_dev or
  override_and_inject_repo_missing_visible_repo_fails_at_module_parse or
  invalid_user_provided_repo_names_fail_at_module_parse'`
  (`5 passed, 141 deselected`).
  Root-module repo-name collision validation now covers the Bazel parse-time
  ownership rows for `module(name = ...)`, `module(repo_name = ...)`, and
  visible `bazel_dep()` repo names, including duplicate `repo_name` aliases
  across included MODULE.bazel segments. It also preserves Bazel's non-collision
  case where an explicit `module(repo_name = ...)` means the module `name` is
  no longer a visible apparent repo name and may be reused by a dependency's
  `repo_name`. Repo-name usage diagnostics now retain the previous Starlark
  location instead of only the previous usage text. Bazel source anchors:
  `ModuleFileGlobals.java:177-180`, `:301-317`, and
  `ModuleThreadContext.java:110-118`, plus
  `ModuleFileFunctionTest.java:509-529` and `:1505-1514`. This does not claim
  the larger `bazel_dep(repo_name = None)` nodep behavior is complete.
  Validation passed with `cargo test -p slug_bzlmod repo_name -- --nocapture`
  (`9 passed`) and the explicit-binary Plan 61 selector
  `-k 'explicit_module_repo_name_frees_module_name_for_dep_repo or
  repo_name_collisions_fail_at_module_parse or
  override_and_inject_repo_missing_visible_repo_fails_at_module_parse or
  invalid_user_provided_repo_names_fail_at_module_parse or
  non_root_override_repo_is_ignored or non_root_inject_repo_is_ignored'`
  (`6 passed, 137 deselected`).
  `module()` ordering now follows Bazel's shared MODULE.bazel context rule:
  if `module()` is present, it must execute before any other directive, including
  an `include()` whose included segment tries to call `module()`. Bazel source
  anchors: `ModuleFileGlobals.java:157-171`,
  `ModuleThreadContext.java:132-137`, and
  `ModuleFileFunctionTest.java:484-502` plus `:1518-1544`.
  Validation passed with `cargo test -p slug_bzlmod module_called --
  --nocapture` (`2 passed`) and the explicit-binary Plan 61 selector
  `-k 'module_called_after_non_module_directive_fails'` (`1 passed, 137
  deselected`).
  `module(bazel_compatibility = [...])` argument-shape validation now matches
  Bazel's raw compatibility string grammar: each entry must start with `<`,
  `<=`, `>`, `>=`, or `-` and then contain exactly three numeric release
  segments. Slug no longer accepts bare versions, `=`, or `==` compatibility
  arguments, and `-X.Y.Z` now means any version except the exact release.
  Bazel source anchors: `ModuleFileGlobals.java:63-67` and `:212-223`,
  `BazelVersion.java:75-96`, and
  `BazelModuleResolutionFunctionTest.java:39-51` plus `:200-211`.
  Validation passed with `cargo test -p slug_bzlmod bazel_compatibility --
  --nocapture` (`7 passed`) and the explicit-binary Plan 61 selector
  `-k 'bazel_compatibility_argument_shape_follows_bazel or
  bazel_compatibility_incompatible_version_fails'` (`2 passed, 137
  deselected`).
- Root `register_toolchains(..., dev_dependency = True)` and
  `register_execution_platforms(..., dev_dependency = True)` are now filtered
  under `--ignore_dev_dependency`, while non-root dev registrations remain
  skipped. Focused `slug_common` unit coverage verifies the collection policy.
- `use_repo_rule()` now matches Bazel's dev-dependency call shape: the factory
  takes only the `.bzl` label and rule symbol, while the returned proxy call
  accepts `dev_dependency = True`, validates the repository name, carries the
  bit into the implicit repo-rule extension proxy, and omits that keyword from
  the repository-rule attrs. Bazel source anchors:
  `ModuleFileGlobals.java:819-829` and `:840-867`. Validation passed with
  `cargo test -p slug_bzlmod use_repo_rule -- --nocapture` (`4 passed`) and
  the explicit-binary Plan 61 selector
  `-k 'use_repo_rule_rejects_dev_dependency_on_factory or
  root_use_repo_rule_dev_dependency_follows_ignore_policy or
  non_root_use_repo_rule_dev_dependency_is_always_ignored'` (`3 passed, 137
  deselected`).
- Non-root `use_repo_rule(...)(..., dev_dependency = True)` and
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
- The leftover `RepositoryOs::new()` adapter and public build-config repo-env
  readback helpers are removed, so module/repository runtime `os.environ`
  construction must use explicit context-owned repo-env snapshots. Focused
  validation passed with `cargo test -p slug_interpreter_for_build
  repository_os -- --nocapture`.
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
  module-version split left it with no live consumers. At that checkpoint,
  `BzlmodSessionData` remained only as the legacy resolver payload for
  populating the narrower injected DICE values, and the DICE graph no longer
  exposed that payload as a direct computed dependency.
- `BuckConfigBasedCells` no longer stores `BzlmodSessionData` or carries it
  through server config state. The persisted config-load path injects the
  narrower bzlmod DICE projections immediately after successful cell parsing,
  including the explicit empty-session projection for no-`MODULE.bazel`
  workspaces. Validation passed with focused `cargo test -p slug_common
  explicit_output_base -- --nocapture` and `cargo check -p slug_common -p
  slug_server`, followed by `cargo build -p slug` and the full Plan 61 Python
  guardrail file (`125 passed in 151.47s`). No stale `slugd` process remained
  after post-run cleanup. This still leaves the legacy resolver payload and
  projection API in place until the resolved graph is
  produced directly by DICE keys.
- The transitional DICE injection API now accepts `BzlmodProjectionData`
  instead of `BzlmodSessionData`. At that checkpoint, the legacy resolver
  still returned `BzlmodSessionData`, but the persisted config-load path
  converted it before touching DICE, and test/bootstrap helpers seeded only
  the projection payload. Validation passed with focused `cargo test -p slug_bzlmod
  set_bzlmod_projection_data -- --nocapture`, focused `cargo test -p
  slug_common explicit_output_base -- --nocapture`, affected-crate `cargo
  check -p slug_bzlmod -p slug_common -p slug_interpreter_for_build -p
  slug_external_cells -p slug_analysis`, `cargo build -p slug`, and the full
  Plan 61 Python guardrail file (`125 passed in 147.72s`). No stale `slugd`
  process remained after post-run cleanup.
- The former `LegacyBzlmodResolutionDiceKey` now returns the narrower
  `BzlmodProjectionData` payload instead of caching `BzlmodSessionData` as its
  DICE value. The wrapped resolver still populated a legacy session-shaped
  structure internally at this checkpoint, then converted it at the key
  boundary, so this is a demotion of the bridge payload rather than the final
  Skyframe-shaped resolver rewrite. Validation
  passed with focused `cargo test -p slug_common explicit_output_base --
  --nocapture`, `cargo check -p slug_common -p slug_server`, `cargo build -p
  slug`, and the full Plan 61 Python guardrail file (`125 passed in 162.30s`).
  No stale `slugd` process remained after post-run cleanup.
- The `BzlmodSessionData` type was removed entirely. The legacy resolver now
  constructs the transitional `BzlmodProjectionData` payload directly, and
  `app`/`tests` have no remaining `BzlmodSessionData` or
  `set_bzlmod_session_data` references. This removes another named
  session-shaped API without changing the remaining structural blocker: the
  projection payload is still assembled by the legacy resolver until narrower
  module/source/graph DICE keys own those facts. Validation passed with focused
  `cargo test -p slug_bzlmod set_bzlmod_projection_data -- --nocapture`,
  focused `cargo test -p slug_common explicit_output_base -- --nocapture`,
  `cargo check -p slug_bzlmod -p slug_common -p slug_server`, `cargo build -p
  slug`, touched-file `rustfmt --edition 2024 --check`, `git diff --check`,
  and the full Plan 61 Python guardrail file (`125 passed in 139.29s`). No
  stale `slugd[...]` process remained after post-run cleanup.
- The internal legacy DICE wrapper is now named
  `BzlmodProjectionBridgeDiceKey` instead of
  `LegacyBzlmodResolutionDiceKey`, and its compute path/error context now says
  it computes the projection bridge rather than a DICE-owned bzlmod resolution
  graph. This is a naming/API demotion only: the bridge still wraps the legacy
  resolver and still returns `BzlmodProjectionData` until module graph,
  repo-mapping, lockfile, repo-env, registration, and cell-graph facts have
  true DICE producers. The direct no-op `BzlmodProjectionData::from`
  conversions and stale session-named projection tests were removed in the
  same cleanup. Validation passed with focused `cargo test -p slug_common
  bzlmod_projection_bridge_key -- --nocapture`, focused `cargo test -p
  slug_bzlmod projection -- --nocapture`, `cargo check -p slug_bzlmod -p
  slug_common -p slug_server`, touched-file `rustfmt --edition 2024`, and
  `git diff --check`.
- The transitional DICE projection keys no longer use the stale
  `injected-bzlmod-session` sentinel string. Their default
  `resolution_digest` now uses a single `injected-bzlmod-projection` constant,
  so key display/equality output matches the actual bridge payload while still
  making clear these are not true resolution-derived graph keys. Validation
  passed with focused `cargo test -p slug_bzlmod dice_graph::tests --
  --nocapture`, focused `cargo test -p slug_bzlmod projection --
  --nocapture`, `cargo check -p slug_bzlmod -p slug_common -p slug_server`,
  touched-file `rustfmt --edition 2024`, and `git diff --check`.
- Config parsing helpers and direct guardrail tests now use transitional
  projection-bridge naming, and bridge guardrail
  errors say they protect the projection bridge rather than a finished DICE
  resolution graph. This is a naming/API cleanup only; the wrapped legacy
  resolver remains the structural blocker. Validation passed with focused
  `cargo test -p slug_common bzlmod_projection_bridge -- --nocapture`,
  `cargo check -p slug_common -p slug_server`, touched-file
  `rustfmt --edition 2024`, and `git diff --check`.
- Unused config parsing entrypoints that allowed direct root-module/lockfile
  injection, plus the unused non-persisted projection-bridge parser entrypoint,
  were removed. Normal command setup now enters bzlmod through the persisted
  projection bridge or the remaining non-DICE bootstrap parser, reducing the
  transitional API surface without changing the remaining legacy-resolver
  blocker. Validation passed with focused `cargo test -p slug_common
  bzlmod_projection_bridge -- --nocapture`, `cargo check -p slug_common -p
  slug_server`, touched-file `rustfmt --edition 2024`, and `git diff --check`.
- The private config parser no longer accepts root-module or visible-lockfile
  injection slots. Its only bzlmod override is now the explicit transitional
  projection bridge payload, while non-DICE bootstrap callers still use the
  direct parser path. This further narrows the session-era API surface without
  replacing the legacy resolver. Validation passed with focused `cargo test -p
  slug_common bzlmod_projection_bridge -- --nocapture`, `cargo check -p
  slug_common -p slug_server`, touched-file `rustfmt --edition 2024`, and
  `git diff --check`.
- `ModuleExtensionExecutionKey` now includes the tracked visible/hidden
  `LockfileContentValue` identity in equality and hashing instead of relying
  only on separate digest fields. This keeps the replay key honest about the
  actual lockfile values consumed by extension execution; at that checkpoint,
  lockfile-input production still remained a follow-up not yet split out of the
  projection bridge. Validation passed
  with focused `cargo test -p slug_bzlmod tracked_lockfile_value_identity --
  --nocapture`, broader `cargo test -p slug_bzlmod lockfile -- --nocapture`
  (`64 passed`), `cargo check -p slug_bzlmod -p slug_common`, touched-file
  `rustfmt --edition 2024`, and `git diff --check`.
- Lockfile input production now has a named
  `BzlmodLockfileInputsBridgeKey` that owns visible/hidden
  `TrackedLockfileContentKey` reads and returns the shared
  `BzlmodLockfileInputsValue`; `BzlmodProjectionBridgeDiceKey` carries that
  bundle instead of separate visible/hidden lockfile fields. This is still a
  transitional bridge because the legacy resolver consumes the bundle, but the
  lockfile read/replay producer is now separate from projection-bridge key
  assembly. Validation passed with focused `cargo test -p slug_common
  bzlmod_lockfile_inputs_bridge -- --nocapture`, focused `cargo test -p
  slug_common bzlmod_projection_bridge -- --nocapture`, `cargo check -p
  slug_bzlmod -p slug_common -p slug_server`, `cargo build -p slug`, selected
  `pytest tests/core/bzlmod/test_plan61_guardrails.py -k
  'lockfile_mode_off_does_not_read_lockfiles or
  visible_lockfile_edit_is_observed_in_same_daemon or
  visible_lockfile_creation_is_observed_in_same_daemon or
  visible_lockfile_deletion_is_observed_in_same_daemon'`, touched-file
  `rustfmt --edition 2024`, and `git diff --check`.
- `TrackedLockfileContentKey` now reports real tracking provenance: project-root
  lockfiles remain transaction-valid DICE file dependencies, while
  out-of-project hidden/output-base lockfiles are explicitly polled children and
  invalid across transactions. The lockfile-input bridge no longer accepts
  caller-precomputed visible/hidden poll digests, so poll ownership stays inside
  the lockfile read key rather than `BzlmodProjectionBridgeDiceKey` assembly.
  Validation passed with focused `cargo test -p slug_common
  bzlmod_lockfile_inputs_bridge -- --nocapture`, focused `cargo test -p
  slug_common bzlmod_projection_bridge -- --nocapture`, `cargo check -p
  slug_common -p slug_server`, `cargo build -p slug`, selected `pytest
  tests/core/bzlmod/test_plan61_guardrails.py -k
  'lockfile_mode_off_does_not_read_lockfiles or
  visible_lockfile_edit_is_observed_in_same_daemon or
  visible_lockfile_creation_is_observed_in_same_daemon or
  visible_lockfile_deletion_is_observed_in_same_daemon'`, touched-file
  `rustfmt --edition 2024`, and `git diff --check`.
- Local override and cached git/archive override `MODULE.bazel` poll identity
  now flows through named DICE poll keys
  (`LocalOverrideModuleInputsPollKey` and
  `NonRegistryOverrideModuleInputsPollKey`). The current out-of-project
  observation is still gathered before key construction until the watched
  filesystem API exists, but that observation is part of key identity and the
  key returns the same observed value, preserving warm no-op projection reuse
  while edits create a different key. Validation passed
  with focused `cargo test -p slug_common
  override_module_inputs_poll_key_repolls -- --nocapture`, focused `cargo test
  -p slug_common bzlmod_projection_bridge -- --nocapture`, `cargo check -p
  slug_common -p slug_server`, `cargo build -p slug`, and selected `pytest
  tests/core/bzlmod/test_plan61_guardrails.py -k
  'warm_noop_out_of_project_local_override_reuses_polled_dice_input or
  out_of_project_local_override_module_creation_invalidates_bzlmod_resolution or
  cached_git_override_module_edit_invalidates_bzlmod_resolution or
  cached_archive_override_module_edit_invalidates_bzlmod_resolution'`,
  touched-file `rustfmt --edition 2024`, and `git diff --check`.
- Registry lockfile-hash file poll identity now flows through the named
  `RegistryFileInputsPollKey`. Project-root cache files stay `project-tracked`
  in the observed poll digest and rely on `RegistryFileInputsKey`'s DICE file
  dependencies; out-of-project cache paths are directly observed before key
  construction until the watched-filesystem API exists, and that observation is
  part of the poll-key identity. Validation passed with focused `cargo test -p slug_common
  registry_file_inputs_poll_digest -- --nocapture`, focused `cargo test -p
  slug_common bzlmod_projection_bridge -- --nocapture`, `cargo check -p
  slug_common -p slug_server`, `cargo build -p slug`, selected `pytest
  tests/core/bzlmod/test_plan61_guardrails.py -k
  'warm_noop_locked_registry_dep_reuses_bzlmod_resolution or
  locked_registry_source_json_and_registry_metadata_are_bridge_inputs or
  locked_registry_metadata_delete_invalidates_bzlmod_resolution or
  locked_registry_source_json_parse_failure_invalidates_bzlmod_resolution'`,
  touched-file `rustfmt --edition 2024`, and `git diff --check`.
- Non-root `MODULE.bazel` poll identity now flows through the named
  `NonRootModuleFilesPollKey`. Project-root non-root module files stay
  `project-tracked` in the observed poll digest and rely on
  `NonRootModuleFilesKey`'s DICE file dependencies; out-of-project non-root
  module files and their included segments are directly observed before key
  construction until the watched-filesystem API exists, and that observation is
  part of the poll-key identity. Validation passed with focused
  `cargo test -p slug_common non_root_module_files_poll -- --nocapture`,
  focused `cargo test -p slug_common bzlmod_projection_bridge -- --nocapture`,
  `cargo check -p slug_common -p slug_server`, `cargo build -p slug`, selected
  `pytest tests/core/bzlmod/test_plan61_guardrails.py -k
  'non_root_included_module_segment_edit_invalidates_extension_graph or
  non_root_use_extension_dev_dependency_is_always_ignored or
  non_root_use_repo_rule_dev_dependency_is_always_ignored or
  non_root_override_repo_is_ignored or non_root_inject_repo_is_ignored'`,
  touched-file `rustfmt --edition 2024`, and `git diff --check`.
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
  DICE-derived workspace identity plus registered-toolchain list and clears/reloads
  when that signature changes.
- Deferred registered-toolchain loading now carries the same workspace/list
  signature as the eager registry. The temporary deferred pool, per-entry
  loaded markers, and load-all marker are still process state, but they are
  ignored and cleared on signature mismatch rather than shared across output
  bases or registered-toolchain changes.
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
  still change key identity. The visible-lockfile edit guardrail proves a
  same-daemon warm no-op does not reread the lockfile before an invalid edit is
  observed under `--lockfile_mode=error`; the create/delete guardrails prove
  warm resolution reuse followed by same-daemon transition observation. Hidden
  lockfile guardrails cover warm reuse plus replay/facts create-edit-delete
  transitions.
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
  edit to that cached module file forces resolution to recompute. The matching
  `archive_override` cache guardrail now proves the same warm reuse and
  same-daemon edit invalidation (`1 passed, 78 deselected`), and the focused
  local/git/archive/registry module-input subset passed (`6 passed, 73
  deselected`). Additional create/delete transition guardrails prove a cached
  `git_override` missing-to-present `MODULE.bazel` transition and a cached
  `archive_override` present-to-missing transition both recompute bzlmod
  resolution (`4 passed, 77 deselected`). Cached non-registry override failure
  transitions now also have focused same-daemon guardrails: a cached
  `git_override` `MODULE.bazel` parse error and a cached `archive_override`
  invalid-UTF-8 `MODULE.bazel` both fail the next `audit cell`, then repair and
  produce a graph containing a newly introduced generated repo instead of
  reusing the warm value. Validation passed with the two new guardrails (`2
  passed, 81 deselected`) and the cached git/archive override module-input
  subset (`6 passed, 77 deselected`).
  Mirror guardrails now cover cached `archive_override` parse failures and
  cached `git_override` UTF-8 failures, and cached git/archive include-cycle
  guardrails start from a valid included module segment, then edit only that
  segment into a cycle before repairing it to introduce a generated repo.
  Validation passed with the override failure subset selected by `-k
  'override_module_parse_failure or override_module_utf8_failure or
  override_module_include_cycle'` (`6 passed, 86 deselected`), and the corrected
  failure-input matrix below passed as part of a 12-test subset.
- Out-of-project `local_path_override` module files now have focused
  same-daemon failure-transition guardrails: parse errors, invalid UTF-8, and
  cyclic included module segments fail the next `audit cell`, then repair to a
  graph containing a newly introduced generated repo instead of reusing the
  warm value. The include-cycle guardrail edits only the included segment after
  warming a valid include. Validation passed with the focused subset selected
  by `-k 'out_of_project_local_override_parse_failure or
  out_of_project_local_override_utf8_failure or
  out_of_project_local_override_include_cycle'` (`3 passed, 92 deselected`),
  and with the corrected 12-test failure-input subset.
- Root included module segments now have same-daemon parse-error, invalid
  UTF-8, and include-cycle failure-transition guardrails. Each starts from a
  valid included segment, fails after editing only that segment, then repairs
  to a graph containing a newly introduced module instead of reusing the warm
  value. Validation passed with the corrected failure-input subset selected by
  `-k 'override_module_parse_failure or override_module_utf8_failure or
  override_module_include_cycle or out_of_project_local_override_parse_failure
  or out_of_project_local_override_utf8_failure or
  out_of_project_local_override_include_cycle or
  included_module_segment_parse_failure or
  included_module_segment_utf8_failure or
  included_module_segment_include_cycle'` (`12 passed, 87 deselected`).
- Root `MODULE.bazel` now has same-daemon parse-error and invalid UTF-8
  failure-transition guardrails. Each starts from a valid root module, fails
  after corrupting only `MODULE.bazel`, then repairs to a graph containing a
  newly introduced local module instead of reusing the warm value. Validation
  passed with the focused subset selected by `-k 'root_module_parse_failure or
  root_module_utf8_failure'` (`2 passed, 99 deselected`).
- Root `MODULE.bazel` deletion is now covered as an observable same-daemon
  transition: a warm root module graph is established, deleting the root module
  file makes the next `audit cell` fail during cell resolver creation instead
  of reusing the stale bzlmod cell graph. The included-segment create/delete
  guardrail was tightened to assert a warm no-op before deleting the included
  segment and checking the missing-include failure. Validation passed with the
  focused subset selected by `-k 'root_module_deletion or
  included_module_segment_create_delete'` (`2 passed, 109 deselected`).
- Root `MODULE.bazel` missing-to-present creation is now covered as an observable
  same-daemon transition: the first no-module `audit cell` fails during cell
  resolver creation, creating `MODULE.bazel` in the same daemon produces a
  bzlmod root graph, and a follow-up warm audit reuses the created graph.
  Validation passed with the focused subset selected by `-k
  'root_module_creation'` (`1 passed, 126 deselected`).
- Project-local `local_path_override` module files now have same-daemon
  parse-error, invalid UTF-8, and included-segment cycle guardrails matching the
  out-of-project local override cases. Validation passed with the focused
  local-override failure subset selected by `-k
  'project_local_override_parse_failure or project_local_override_utf8_failure
  or project_local_override_include_cycle'` (`6 passed, 101 deselected`,
  including the out-of-project mirror tests selected by the same pattern).
- Project-local and out-of-project `local_path_override` module files now also
  have missing-to-present creation and present-to-missing deletion guardrails.
  The creation guardrails warm an empty override module, create `MODULE.bazel`,
  and require the repaired graph to expose a new generated repo. Validation
  passed with the focused creation subset selected by `-k
  'project_local_override_module_creation or
  out_of_project_local_override_module_creation'` (`2 passed, 112 deselected`);
  the deletion subset selected by `-k 'project_local_override_module_deletion
  or out_of_project_local_override_module_deletion'` passed earlier (`2 passed,
  107 deselected`).
- Locked registry `source.json` metadata now has focused parse-error, invalid
  UTF-8, creation, and deletion failure-transition guardrails: after a warm
  same-daemon registry module resolution, corrupting or deleting cached source
  metadata with matching lockfile hashes fails the next `audit cell`, then
  repair introduces a new registry dependency instead of reusing the warm value.
  A missing-to-present case starts with a locked missing cached source metadata
  file, creates the exact bytes named by the lockfile hash, and then warms
  cleanly. Validation passed with the focused parse/UTF-8 subset selected by `-k
  'locked_registry_source_json_parse_failure or
  locked_registry_source_json_utf8_failure'` (`2 passed, 108 deselected`) and
  the deletion guardrail selected by `-k 'locked_registry_source_json_delete'`
  (`1 passed, 120 deselected`) plus the creation guardrail selected by `-k
  'locked_registry_source_json_creation'` (`1 passed, 128 deselected`).
- Locked registry `MODULE.bazel` files now have focused parse-error, invalid
  UTF-8, creation, and deletion failure-transition guardrails: after a warm
  same-daemon registry module resolution, corrupting or deleting cached module
  metadata with matching lockfile hashes fails the next `audit cell`, then
  repair introduces a new registry dependency instead of reusing the warm value.
  A missing-to-present case starts with a locked missing cached module file,
  creates the exact bytes named by the lockfile hash, and then warms cleanly.
  Validation passed with the focused parse/UTF-8 subset selected by `-k
  'locked_registry_module_parse_failure or locked_registry_module_utf8_failure'`
  (`2 passed, 102 deselected`) and the deletion guardrail selected by `-k
  'locked_registry_module_delete'` (`1 passed, 119 deselected`) plus the
  creation guardrail selected by `-k 'locked_registry_module_creation'` (`1
  passed, 127 deselected`).
- Locked top-level registry metadata now has creation, deletion, parse-error,
  and invalid-UTF-8 coverage: after a warm same-daemon registry module
  resolution, deleting cached `bazel_registry.json` while the visible lockfile
  still carries its checksum fails the next `audit cell` with a registry
  checksum error, and corrupting the same file with a matching lockfile hash
  fails during registry metadata parsing instead of reusing the stale warm graph.
  A missing-to-present case starts with a locked missing metadata file, creates
  the exact bytes named by the lockfile hash, and then warms cleanly.
  The Slug parse check is anchored to Bazel's `IndexRegistry`, which treats
  blank JSON as absent metadata and otherwise parses top-level
  `bazel_registry.json` into its `BazelRegistryJson` metadata shape before using
  registry mirrors. Validation passed with the focused guardrail selected by
  `-k 'locked_registry_metadata_parse_and_utf8_failures'` (`1 passed, 125
  deselected`), the creation guardrail selected by `-k
  'locked_registry_metadata_creation'` (`1 passed, 129 deselected`), and the
  registry metadata/source subset selected by `-k
  'locked_registry_metadata_delete or
  locked_registry_metadata_parse_and_utf8_failures or
  locked_registry_source_json_parse_failure or
  locked_registry_source_json_utf8_failure or
  locked_registry_source_json_delete'` (`5 passed, 121 deselected`).
- Visible lockfile `selectedYankedVersions` now has same-daemon edit coverage:
  a locked registry module warms as not-yanked, editing only the lockfile
  selected-yanked entry for that selected version fails the next `audit cell`,
  and removing the entry succeeds again. Validation passed with
  `test_lockfile_selected_yanked_version_edit_invalidates_bzlmod_resolution`.
- Visible lockfile registry hash policy now has same-daemon missing-checksum
  coverage under `--lockfile_mode=error`: a locked registry module warms with
  all registry file hashes present, removing only the selected `MODULE.bazel`
  registry hash from the lockfile fails before mutable registry content is
  accepted, restoring the hash succeeds again, and removing only the selected
  `source.json` registry hash fails before source metadata is accepted.
  Validation passed with the focused guardrail selected by `-k
  'missing_registry_checksum'` (`1 passed, 122 deselected`).
- After adding the locked registry metadata deletion and override-registry
  source deletion guardrails, the full Plan 61 Python guardrail passed with
  `123 passed in 155.11s`; no stale `slugd` process remained after cleanup.
- Visible lockfile missing-to-present creation and present-to-missing deletion
  now have same-daemon error-mode coverage: creating an invalid
  `MODULE.bazel.lock` makes the next `audit cell` fail instead of reusing the
  warm no-lockfile value, and deleting a valid lockfile forces the next
  `audit cell` to recompute instead of reusing the warm lockfile value.
  Validation passed with the focused selector `-k 'visible_lockfile_creation
  or visible_lockfile_deletion or visible_lockfile_edit'` (`3 passed, 113
  deselected`).
- Extension tag values now have a behavioral stale-replay guardrail: a build
  first replays generated repo specs from a lockfile whose `usagesDigest`
  matches the initial tag value, then editing only that tag value makes the
  consuming build evaluate the extension and fail instead of reusing stale
  replay output. Validation passed with the focused guardrail selected by `-k
  'extension_tag_attr_edit'` (`1 passed, 115 deselected`).
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
- Recorded-input validation for DICE extension replay is now owned by the named
  child key `ModuleExtensionRecordedInputsKey`: lockfile cache selection chooses
  the matching entry and cached repo specs, then visible/hidden replay computes
  the child key before accepting a replay hit. The key still uses the
  transitional polled FILE/DIRENTS/DIRTREE marker helpers until lower-level
  watched filesystem keys exist. Lockfile spoke pre-seeding remains on the
  synchronous bootstrap path because it does not have a `DiceComputations`
  handle. Focused validation passed with `cargo test -p slug_bzlmod
  recorded_file_input_changed_rejects_replay -- --nocapture`, `cargo test -p
  slug_bzlmod recorded_inputs_key_rejects_file_edit -- --nocapture`, `cargo
  test -p slug_bzlmod
  visible_lockfile_replay_validates_recorded_file_through_dice_key --
  --nocapture`, `cargo test -p slug_bzlmod
  hidden_lockfile_replay_validates_recorded_file_through_dice_key --
  --nocapture`, `cargo check -p slug_bzlmod`, `cargo fmt --check`, and `git
  diff --check`. The visible replay path no longer records a generic
  `digest_or_entry_miss` when cache selection succeeded and only recorded-input
  validation rejected the replay.
- Earlier broad replay validation passed with `cargo build -p slug`, `cargo
  test -p slug_bzlmod -- --nocapture`, `cargo test -p slug_common bzlmod --
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
- Module extension fresh execution now captures recorded inputs for
  `module_ctx.watch()`, `module_ctx.getenv()`, and declared
  `module_extension(environ = [...])`, threads them through
  `ExtensionExecutionOutput` and `ModuleExtensionResult`, and exposes a
  lockfile cache writer helper that preserves those inputs. The capture path
  now de-duplicates repeated inputs, rejects conflicting same-input values,
  rejects explicit watches under the temporary module-extension working
  directory or outside the workspace/external-repo classifier, and records
  accepted files with Bazel-style repo-friendly identities such as `@@//...`
  and `@@repo+//...` while hashing the actual path. `module_ctx.read(...,
  watch = ...)` and `module_ctx.extract(..., watch_archive = ...)` now share
  that classifier, including `auto`/`yes`/`no` handling. Fresh
  `ModuleExtensionResult` values carry recorded-input validation context so
  DICE value reuse is rejected after a recorded file changes, and fresh
  execution records a `ModuleExtensionRecordedInputsKey` child dependency before
  accepting the result instead of only validating lockfile replay hits. Plain
  `module_ctx.os.environ` dictionary access remains non-recording to match
  Bazel docs that reading the dictionary itself does not establish an env
  dependency. Validation passed with `TMPDIR=/var/mnt/dev/.slug-tmp cargo check
  -p slug_bzlmod -p slug_interpreter_for_build -p slug_external_cells`, focused
  `slug_interpreter_for_build` module-context tests, focused `slug_bzlmod`
  recorded-input/result-validity tests, `cargo test -p slug_bzlmod --
  --nocapture`, `cargo test -p slug_external_cells -- --nocapture`, `cargo
  build -p slug`, the fresh recorded-file Plan 61 guardrail, `cargo fmt
  --check`, and `git diff --check`.
- Warm no-op DICE cutoffs for polled bzlmod inputs now put the current poll
  digest in key identity instead of forcing the child key invalid every
  transaction. `AbsoluteTextFileInputKey` covers generic out-of-project text
  inputs, `TrackedLockfileContentKey` carries the observed hidden/output-base
  lockfile input when that file is outside the project root, and
  `TrackedExtensionBzlDigestKey` carries the direct transitional literal-load
  digest used by lockfile pre-seeding and root extension replay-summary
  formation. This preserves edit/create/delete transitions through a new key
  but lets same-key warm values stay valid, so the replay-summary bridge no
  longer recomputes on every no-op command. Validation
  passed with `TMPDIR=/var/mnt/dev/.slug-tmp cargo test -p slug_common
  tracked_extension_bzl_digest -- --nocapture`, `TMPDIR=/var/mnt/dev/.slug-tmp
  cargo test -p slug_common
  absolute_text_file_input_key -- --nocapture`, `cargo build -p slug`, a
  focused Plan 61 replay subset covering warm replay and project/mapped `.bzl`
  create/edit/delete transitions, the full Plan 61 Python guardrail with 119
  tests, `TMPDIR=/var/mnt/dev/.slug-tmp cargo test -p slug_common bzlmod --
  --nocapture`, `TMPDIR=/var/mnt/dev/.slug-tmp cargo check -p slug_common -p
  slug_bzlmod -p slug_interpreter_for_build -p slug_external_cells`, `cargo fmt
  --check`, and `git diff --check`. A clean review found two TOCTOU hazards in
  this slice; follow-up fixes made `AbsoluteTextFileInputKey` return the same
  polled observation that formed its digest key and made
  `module_ctx.extract(watch_archive = ...)` record the archive before reading
  archive bytes. Follow-up validation passed with `TMPDIR=/var/mnt/dev/.slug-tmp
  cargo test -p slug_common absolute_text_file_input_key -- --nocapture`,
  `TMPDIR=/var/mnt/dev/.slug-tmp cargo test -p slug_interpreter_for_build
  module_context -- --nocapture`, `TMPDIR=/var/mnt/dev/.slug-tmp cargo test -p
  slug_common tracked_extension_bzl_digest -- --nocapture`, `TMPDIR=/var/mnt/dev/.slug-tmp
  cargo test -p slug_common bzlmod -- --nocapture`, `TMPDIR=/var/mnt/dev/.slug-tmp
  cargo check -p slug_common -p slug_bzlmod -p slug_interpreter_for_build -p
  slug_external_cells`, `cargo build -p slug`, and the full Plan 61 Python
  guardrail with 119 tests.
- Follow-up warm no-op poll-key and hidden-lockfile validation passed with
  `TMPDIR=/var/mnt/dev/.slug-tmp cargo test -p slug_common
  override_module_inputs_poll_key_repolls -- --nocapture`,
  `TMPDIR=/var/mnt/dev/.slug-tmp cargo test -p slug_common
  registry_file_inputs_poll_digest -- --nocapture`,
  `TMPDIR=/var/mnt/dev/.slug-tmp cargo test -p slug_common
  non_root_module_files_poll -- --nocapture`, `TMPDIR=/var/mnt/dev/.slug-tmp
  cargo test -p slug_common bzlmod_lockfile_inputs_bridge -- --nocapture`,
  `TMPDIR=/var/mnt/dev/.slug-tmp cargo check -p slug_common -p slug_server`,
  `TMPDIR=/var/mnt/dev/.slug-tmp cargo build -p slug`, focused Plan 61
  guardrails for out-of-project poll-key warm reuse and hidden lockfile warm
  replay, and the full explicit-binary Plan 61 guardrail
  (`125 passed in 89.69s`).
- Out-of-project text inputs now move their direct filesystem observation
  inside `AbsoluteTextFileInputKey::compute` instead of reading content before
  key construction and injecting it as `observed`. Bridge surface reduced:
  out-of-project MODULE/lockfile-like text reads now have a named child key that
  owns the poll operation, and the key is invalid across transactions until a
  lower-level watched filesystem key replaces the direct poll. Focused
  validation passed with `cargo test -p slug_common
  absolute_text_file_input_key -- --nocapture` and `cargo test -p slug_common
  bzlmod_lockfile_inputs_bridge_tracks_visible_lockfile_edits -- --nocapture`.
- Out-of-project local override, non-registry override, registry cache, and
  non-root module poll keys now move their direct filesystem observations
  inside `LocalOverrideModuleInputsPollKey`,
  `NonRegistryOverrideModuleInputsPollKey`, `RegistryFileInputsPollKey`, and
  `NonRootModuleFilesPollKey::compute` instead of reading before key
  construction and injecting an `observed` payload. Bridge surface reduced:
  these transitional poll identities are now named DICE key computations; when
  they poll outside the project root, key validity is false across committed
  transactions until lower-level watched filesystem keys replace the direct
  polls. Focused validation passed with `cargo test -p slug_common
  module_inputs_poll_key_repolls -- --nocapture`, `cargo test -p slug_common
  non_root_module_files_poll -- --nocapture`, and `cargo test -p slug_common
  registry_file_inputs_poll -- --nocapture`.
- Higher-level local override, non-registry override, registry cache, and
  non-root module input keys no longer carry poll digests in key identity.
  Their compute paths now record whether any root module, included segment, or
  registry cache file came from an out-of-project absolute-file child key and
  use `has_untracked_inputs` validity to force same-key recompute across DICE
  transactions. The transitional poll keys are test-only guardrails now; the
  production bridge surface reduced is poll digest propagation into
  `LocalOverrideModuleInputsKey`, `NonRegistryOverrideModuleInputsKey`,
  `RegistryFileInputsKey`, and `NonRootModuleFilesKey`. Focused validation
  passed with `cargo test -p slug_common same_out_of_project_key --
  --nocapture`, `cargo test -p slug_common module_inputs -- --nocapture`,
  `cargo test -p slug_common registry_file_inputs -- --nocapture`, `cargo test
  -p slug_common non_root_module_files -- --nocapture`, and `cargo check -p
  slug_common`.
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
- The legacy resolution bridge's extension replay-summary digest now also
  covers root extension usages whose implementation lives in a local override
  module with source-repo mappings. Local override `MODULE.bazel` parsing
  carries the parsed module data it already computed into the summary path, so
  mapped external helper `.bzl` create/edit/delete transitions change the
  bridge key before `audit cell` can reuse stale extension replay state.
  Uncached sibling extensions now contribute explicit `uncached` digest entries
  instead of collapsing the whole replay-summary digest to absent, so one
  uncached extension cannot hide a cached extension's mapped helper changes.
  Validation passed with `cargo check -p slug_common`, `cargo build -p slug`,
  focused audit-cell-only Plan 61 Python guardrails for mapped external
  `.bzl` edit/create/delete transitions including a mixed cached/uncached root
  extension case, and the existing build-based mapped external `.bzl`
  edit/create/delete replay subset.
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
  This is a named migration surface rather than final cell-graph ownership.
  Validation passed with `cargo check -p
  slug_bzlmod -p slug_common`, `cargo test -p slug_bzlmod
  set_bzlmod_session_data_uses_session_workspace_id -- --nocapture`, full
  `cargo test -p slug_bzlmod -- --nocapture`, `cargo test -p slug_common
  bzlmod -- --nocapture`, `cargo build -p slug`, the focused Plan 61 Python
  replay subset, the full Plan 61 Python guardrail with 72 tests, `cargo
  fmt --check`, and `git diff --check`.
- Bzlmod runtime cell snapshots are derived from the published
  `BzlmodCellGraphValue` instead of parallel `BzlmodResolutionResult` fields.
  Startup replay still refreshes module/external symlinks from that graph, but
  no longer installs eager/lazy extension cells, scoped aliases, or dynamic
  aliases into process-global maps; those runtime cells and aliases are carried
  by the resolver-owned snapshot. The runtime snapshot also no longer carries
  the now-dead eager/lazy process-global registration flag. Validation passed
  with `cargo test -p
  slug_core bzlmod_resolver_ -- --nocapture`, `cargo test -p slug_common
  runtime_cell_install_snapshot_derives_from_cell_graph -- --nocapture`,
  `cargo test -p slug_common
  bzlmod_runtime_state_uses_workspace_output_base_for_external_cell_symlinks
  -- --nocapture`, `cargo build -p slug`, and focused Plan 61 Python replay
  coverage selected by `-k 'two_workspace or
  valid_lockfile_replay_materializes_generated_repo_without_extension_eval or
  lockfile_replay_recorded_repo_mapping_from_extension_repo_source'` (`3
  passed, 115 deselected`).
- Cell path classification for lazy extension repositories now consults the
  resolver's `BzlmodCellGraphValue` runtime snapshot before falling back to
  process-global dynamic-cell discovery. A path under a graph-owned lazy
  generated repo can now be classified from the injected cell graph before any
  global registry promotion. Validation passed with focused `cargo test -p
  slug_core bzlmod_resolver_uses_runtime_snapshot_for_lazy_extension_cell
  -- --nocapture`, `cargo check -p slug_core -p slug_common -p
  slug_external_cells -p slug_server`, `cargo build -p slug`, and focused Plan
  61 Python guardrail
  `valid_lockfile_replay_materializes_generated_repo_without_extension_eval`.
- After the override-patch guardrails, repository-manifest fresh native
  execution path, and lazy repo path-classification slices, the full Plan 61
  Python guardrail file passed with `74 passed in 72.84s` using
  `TEST_EXECUTABLE=/var/mnt/dev/slug/target/debug/slug`; stale `slugd`
  processes from the guardrail run were cleaned afterward.
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
- Bzlmod `CellAliasResolver` construction now carries the same graph-derived
  runtime cell install snapshot, so dynamic generated-repo aliases, scoped repo
  aliases, and exact generated-repo names resolve from resolver-local graph
  state before consulting transitional process-global alias maps. The globals
  still back compatibility and runtime registration, but normal bzlmod alias
  lookup now has a graph snapshot first source. Validation passed with `cargo
  test -p slug_core bzlmod_resolver_uses_runtime_snapshot_for_lazy_extension_cell
  -- --nocapture`, `cargo check -p slug_core -p slug_common`, `cargo test -p
  slug_core dynamic_bzlmod_entries_are_scoped_to_current_project_root --
  --nocapture`, `cargo test -p slug_core
  cells::bzlmod_apparent_alias_cache_tests -- --nocapture --test-threads=1`,
  `cargo check -p slug_core -p slug_common -p slug_external_cells`, `cargo test
  -p slug_bzlmod -- --nocapture`, `cargo test -p slug_common bzlmod --
  --nocapture`, `cargo test -p slug_external_cells -- --nocapture`, `cargo
  build -p slug`, the focused Plan 61 Python replay/root-isolation/alias subset,
  the full Plan 61 Python guardrail with 72 tests, `cargo fmt --check`, and
  `git diff --check`.
- Generated-repo aliases created by `override_repo()` now project into
  `BzlmodCellGraphValue.dynamic_aliases`, so the runtime snapshot carries the
  exact generated-repo-to-selected-module mapping instead of relying on ad hoc
  process-global alias registration. Validation passed with `cargo test -p
  slug_common generated_override_aliases_project_to_dynamic_runtime_aliases --
  --nocapture`, `cargo test -p slug_common bzlmod -- --nocapture`, `cargo check
  -p slug_core -p slug_common -p slug_external_cells`, `cargo test -p
  slug_bzlmod -- --nocapture`, `cargo test -p slug_external_cells --
  --nocapture`, `cargo build -p slug`, the focused Plan 61 Python override/replay
  subset, the full Plan 61 Python guardrail with 72 tests, `cargo fmt --check`,
  and `git diff --check`.
- Bazel-style transition input/output build-setting labels now parse with the
  active bzlmod root `CellAliasResolver` when DICE has a cell resolver, so
  generated-repo aliases resolve through the resolver-owned runtime snapshot
  before any process-global alias fallback. Focused coverage proves a stale
  process-global alias cannot override the runtime snapshot for transition
  build-setting labels. Validation passed with `cargo test -p slug_transition
  transition_build_setting_labels_prefer_runtime_alias_snapshot --
  --nocapture`, `cargo test -p slug_transition --lib -- --nocapture`, `cargo
  check -p slug_transition`, `cargo build -p slug`, the focused Plan 61
  guardrail `deferred_toolchain_retry_recomputes_target_settings`, `cargo fmt`,
  and `git diff --check`.
- Resolver-local promoted generated-repo cells now distinguish graph-owned cells
  from transitional root-scoped discoveries. Cells created from the
  graph-derived runtime snapshot remain available through that resolver even
  after the process-global root adapter resets, while cells discovered from
  process-global maps or directory scans remain scoped to the root that
  published them. Validation passed with `cargo test -p slug_core
  bzlmod_resolver_uses_runtime_snapshot_for_lazy_extension_cell --
  --nocapture`, `cargo test -p slug_core
  dynamic_bzlmod_entries_are_scoped_to_current_project_root -- --nocapture`,
  `cargo test -p slug_core cells::bzlmod_apparent_alias_cache_tests --
  --nocapture --test-threads=1`, `cargo check -p slug_core`, `cargo check -p
  slug_core -p slug_common -p slug_external_cells`, `cargo test -p
  slug_bzlmod -- --nocapture`, `cargo test -p slug_common bzlmod --
  --nocapture`, `cargo test -p slug_external_cells -- --nocapture`, `cargo
  build -p slug`, the focused Plan 61 Python replay/root-isolation/alias subset,
  the full Plan 61 Python guardrail with 72 tests, `cargo fmt --check`, and
  `git diff --check`.
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
- Extension aggregation injected data now carries source workspace provenance
  so `BzlmodExtensionAggregationKey` and canonical-repo owner lookup reject a
  stale aggregation map before pairing it with the current cell graph. This
  keeps current workspace identity derived from `BzlmodCellGraphKey` while
  making cross-workspace injected aggregation mismatches auditable at the
  projection boundary. Validation passed with focused `slug_bzlmod`
  extension-aggregation and canonical-repo owner lookup tests, `cargo check -p
  slug_core -p slug_common -p slug_external_cells`, full `cargo test -p
  slug_bzlmod -- --nocapture`, `cargo test -p slug_common bzlmod --
  --nocapture`, full `cargo test -p slug_external_cells -- --nocapture`,
  `cargo build -p slug`, the focused Plan 61 Python replay/aggregation subset,
  the full Plan 61 Python guardrail with 72 tests, `cargo fmt --check`, and
  `git diff --check`.
- Repo-mapping injected data now also carries source workspace provenance, and
  `BzlmodRepoMappingsKey` rejects stale cross-workspace mapping snapshots
  before returning mappings to extension digesting, extension replay-key
  construction, and module-version invalidation. Validation passed with
  focused `slug_bzlmod` semantic projection and extension execution-key tests,
  `cargo check -p slug_core -p slug_common -p slug_external_cells`, full
  `cargo test -p slug_bzlmod -- --nocapture`, `cargo test -p slug_common
  bzlmod -- --nocapture`, full `cargo test -p slug_external_cells --
  --nocapture`, `cargo build -p slug`, the focused Plan 61 Python
  replay/repo-mapping subset, the full Plan 61 Python guardrail with 72 tests,
  `cargo fmt --check`, and `git diff --check`.
- Repo-env and lockfile-input injected data now carry source workspace
  provenance as well. `BzlmodRepoEnvKey` and `BzlmodLockfileInputsKey` reject
  stale cross-workspace replay inputs before extension execution and
  module-version invalidation can consume them. Validation passed with focused
  `slug_bzlmod` replay-input and semantic projection tests, `cargo check -p
  slug_core -p slug_common -p slug_external_cells`, full `cargo test -p
  slug_bzlmod -- --nocapture`, `cargo test -p slug_common bzlmod --
  --nocapture`, full `cargo test -p slug_external_cells -- --nocapture`,
  `cargo build -p slug`, the focused Plan 61 Python repo-env/lockfile replay
  subset, the full Plan 61 Python guardrail with 72 tests, `cargo
  fmt --check`, and `git diff --check`.
- Module-version data, resolution facts, registered toolchain data, and
  registered execution-platform data now also carry source workspace
  provenance. Their named DICE keys reject stale cross-workspace projection
  data before module-version invalidation or analysis registration consumers
  can pair it with the current cell graph. Validation passed with focused
  `slug_bzlmod` projection and semantic projection tests, `cargo check -p
  slug_core -p slug_common -p slug_external_cells`, full `cargo test -p
  slug_bzlmod -- --nocapture`, `cargo test -p slug_common bzlmod --
  --nocapture`, full `cargo test -p slug_external_cells -- --nocapture`,
  `cargo build -p slug`, the focused Plan 61 Python repo-env/lockfile replay
  subset, the full Plan 61 Python guardrail with 72 tests, `cargo
  fmt --check`, and `git diff --check`.
- Data-only bzlmod projection keys now trust their own workspace-checked
  injected values instead of computing `BzlmodCellGraphKey` as an incidental
  prerequisite. This keeps `BzlmodLockfileInputsKey`, `BzlmodRepoEnvKey`,
  `BzlmodRepoMappingsKey`, `BzlmodResolutionFactsKey`,
  `RegisteredToolchainsKey`, and `RegisteredExecutionPlatformsKey` independent
  from cell-graph root facts, while `ModuleVersionsKey` still depends on the
  named cell graph because it needs the root module name for conservative
  invalidation. Validation passed with `cargo fmt --check`, focused
  `cargo test -p slug_bzlmod data_only_projection -- --nocapture`,
  `cargo test -p slug_bzlmod semantic_projection -- --nocapture`,
  `cargo test -p slug_bzlmod replay_input_data -- --nocapture`, and
  `cargo check -p slug_bzlmod -p slug_common`.
- `SetBzlmodProjectionData` now rejects an internally mixed transitional
  projection payload before fanning it out into DICE. The legacy bridge can no
  longer publish a cell graph from one workspace with repo-env, repo-mapping,
  resolution, registration, module-version, or extension-aggregation data from
  another workspace. Validation passed with `cargo fmt --check`, focused
  `cargo test -p slug_bzlmod set_bzlmod_projection_data -- --nocapture`,
  focused `cargo test -p slug_bzlmod data_only_projection -- --nocapture`, and
  `cargo check -p slug_bzlmod -p slug_common`.
- Lockfile-input projection payloads first gained source workspace provenance:
  `BzlmodProjectionData.lockfile_inputs` stored
  `BzlmodLockfileInputsDataValue`, `SetBzlmodProjectionData` validated it
  against the cell graph before injection, and the legacy bridge wrapped
  tracked visible/hidden lockfile inputs with the keyed workspace identity.
  Validation passed with `cargo fmt --check`, focused `cargo test -p
  slug_bzlmod set_bzlmod_projection_data -- --nocapture`, focused `cargo test
  -p slug_bzlmod replay_input_data -- --nocapture`, focused `cargo test -p
  slug_bzlmod data_only_projection -- --nocapture`, `cargo check -p
  slug_bzlmod -p slug_common`, `cargo test -p slug_common
  bzlmod_projection_bridge -- --nocapture`, and `cargo test -p slug_common
  bzlmod_lockfile_inputs_bridge -- --nocapture`.
- `BzlmodLockfileInputsDataValue` now requires `WorkspaceId` provenance rather
  than storing it as an optional field. This removes the last absent-provenance
  path from lockfile-input projection data; focused validation passed with
  `cargo fmt --check`, `cargo test -p slug_bzlmod
  set_bzlmod_projection_data -- --nocapture`, `cargo test -p slug_bzlmod
  replay_input_data -- --nocapture`, `cargo test -p slug_bzlmod
  data_only_projection -- --nocapture`, and `cargo check -p slug_bzlmod -p
  slug_common`.
- `BzlmodProjectionData` no longer carries lockfile inputs at all. Production
  config-load now injects the value computed by `BzlmodLockfileInputsBridgeKey`
  alongside the remaining transitional projection payload through
  `set_bzlmod_projection_data_with_lockfile_inputs`, which accepts and validates
  separate `BzlmodLockfileInputsDataValue` workspace provenance. Visible/hidden
  lockfile content is owned by the named lockfile-input bridge rather than the
  monolithic legacy projection data. Focused validation passed with `cargo
  fmt`, `cargo test -p slug_bzlmod set_bzlmod_projection_data --
  --nocapture`, `cargo test -p slug_common bzlmod_projection_bridge --
  --nocapture`, `cargo fmt --check`, `cargo check -p slug_bzlmod -p
  slug_common`, `cargo test -p slug_common bzlmod_lockfile_inputs_bridge --
  --nocapture`, `cargo test -p slug_bzlmod replay_input_data -- --nocapture`,
  `cargo test -p slug_bzlmod data_only_projection -- --nocapture`, and `cargo
  test -p slug_bzlmod current_workspace_helpers_use_projection_workspace_id --
  --nocapture`. Review found no missing production lockfile-injection path and
  identified that the split setter needed the same workspace-provenance
  validation as the old payload field; the final API accepts
  `BzlmodLockfileInputsDataValue` and validates it before injection.
- `BzlmodProjectionData` no longer carries repo-env either. Production
  config-load injects `BzlmodRepoEnvDataValue` separately from command policy
  state through `set_bzlmod_projection_data_with_inputs`, and the setter
  validates its workspace provenance before publishing `BzlmodRepoEnvDataKey`.
  The legacy resolver still uses the command repo-env locally while building
  lockfile-seeded extension cells and runtime extension setups, but the
  monolithic projection payload no longer owns the injected repo-env fact.
  Focused validation passed with `cargo fmt`, `cargo test -p slug_bzlmod
  set_bzlmod_projection_data -- --nocapture`, `cargo test -p
  slug_external_cells extension_repo_setup_repo_env_uses_current_dice_projection
  -- --nocapture`, and `cargo test -p slug_common bzlmod_projection_bridge --
  --nocapture`, `cargo fmt --check`, `cargo check -p slug_bzlmod -p
  slug_common -p slug_external_cells`, `cargo test -p slug_bzlmod
  data_only_projection -- --nocapture`, and `cargo test -p slug_bzlmod
  current_workspace_helpers_use_projection_workspace_id -- --nocapture`.
- Legacy cell parsing now consumes only `BzlmodCellGraphValue` instead of the
  full transitional `BzlmodProjectionData` payload. The persisted bridge still
  computes the legacy projection so DICE injection can publish the remaining
  narrow values, but `parse_with_file_ops_and_options_inner` cannot depend on
  module versions, registrations, extension aggregations, resolution facts, or
  repo mappings when it only needs cell definitions, aliases, runtime dynamic
  state, and external-cell setup. Focused validation passed with `cargo fmt`,
  `cargo test -p slug_common bzlmod_projection_bridge -- --nocapture`, `cargo
  test -p slug_common bzlmod_cell_resolver_uses -- --nocapture`, `cargo fmt
  --check`, and `cargo check -p slug_common`.
- `BzlmodProjectionData` no longer carries resolution facts or repo mappings.
  The legacy resolver now returns a local `BzlmodProjectionBridgeValue` so
  `BzlmodProjectionBridgeDiceKey` can keep those still-legacy-produced facts
  explicit while `SetBzlmodProjectionData` injects
  `BzlmodResolutionFactsValue` and `BzlmodRepoMappingsDataValue` separately
  with workspace-provenance validation. This reduces the monolithic projection
  payload without claiming true DICE ownership yet; repo mappings and
  resolution facts still need Skyframe-shaped producers before the bridge can
  go away. Focused validation passed with `cargo fmt`, `cargo test -p
  slug_bzlmod set_bzlmod_projection_data -- --nocapture`, `cargo test -p
  slug_common bzlmod_projection_bridge -- --nocapture`, `cargo test -p
  slug_common bzlmod_cell_resolver_uses -- --nocapture`, `cargo test -p
  slug_external_cells
  extension_repo_setup_repo_env_uses_current_dice_projection -- --nocapture`,
  `cargo fmt --check`, `cargo check -p slug_bzlmod -p slug_common -p
  slug_external_cells`, and `git diff --check`.
- `BzlmodProjectionData` no longer carries registered toolchain or registered
  execution platform data either. The same local `BzlmodProjectionBridgeValue`
  keeps those legacy-produced registration values explicit until
  `RegisteredToolchainsKey` and `RegisteredExecutionPlatformsKey` have true
  graph producers, while `SetBzlmodProjectionData` injects them as separate
  workspace-validated values. This further narrows the monolithic projection
  payload; it does not replace the legacy resolver as the registration owner
  yet. Focused validation passed with `cargo fmt`, `cargo test -p slug_bzlmod
  set_bzlmod_projection_data -- --nocapture`, `cargo test -p slug_common
  bzlmod_projection_bridge -- --nocapture`, and `cargo test -p
  slug_external_cells
  extension_repo_setup_repo_env_uses_current_dice_projection -- --nocapture`,
  `cargo fmt --check`, `cargo check -p slug_bzlmod -p slug_common -p
  slug_external_cells`, and `git diff --check`.
- `BzlmodProjectionData` no longer carries extension aggregation data. The
  legacy resolver returns `BzlmodExtensionAggregationsDataValue` through the
  local `BzlmodProjectionBridgeValue`, and the setter injects
  `BzlmodExtensionAggregationsDataKey` from that separate workspace-validated
  value. This keeps extension replay inputs out of the monolithic projection
  payload while preserving the current transitional producer until extension
  aggregation and replay-input facts are derived from true DICE graph keys.
  Focused validation passed with `cargo fmt`, `cargo test -p slug_bzlmod
  set_bzlmod_projection_data -- --nocapture`, `cargo test -p slug_common
  bzlmod_projection_bridge -- --nocapture`, `cargo test -p
  slug_external_cells
  extension_repo_setup_repo_env_uses_current_dice_projection -- --nocapture`,
  `cargo fmt --check`, `cargo check -p slug_bzlmod -p slug_common -p
  slug_external_cells`, and `git diff --check`.
- `BzlmodProjectionData` no longer carries module-version data. The projection
  payload is now cell-graph-shaped, while the legacy resolver returns
  `BzlmodModuleVersionsDataValue` through `BzlmodProjectionBridgeValue` and
  the setter injects it separately with workspace-provenance validation. This
  still does not make module versions true graph-owned data: the value is
  assembled by the legacy resolver until `ModuleVersionsKey` can derive it from
  module graph producers and explicit invalidation inputs. Focused validation
  passed with `cargo fmt`, `cargo test -p slug_bzlmod
  set_bzlmod_projection_data -- --nocapture`, `cargo test -p slug_bzlmod
  current_workspace_helpers_use_projection_workspace_id -- --nocapture`,
  `cargo test -p slug_common bzlmod_projection_bridge -- --nocapture`, `cargo
  test -p slug_external_cells
  extension_repo_setup_repo_env_uses_current_dice_projection -- --nocapture`,
  `cargo fmt --check`, `cargo check -p slug_bzlmod -p slug_common -p
  slug_external_cells`, and `git diff --check`.
- Extension-repo execution and materialization-manifest constructors that
  default command repo-env to empty are now test-only unless they are already
  an internal test helper. Production callers compile only through constructors
  that carry both explicit workspace identity and explicit repo-env. Validation
  passed with `cargo fmt --check`, `cargo test -p slug_bzlmod
  extension_repo_key -- --nocapture`, `cargo test -p slug_bzlmod
  materialization_manifest -- --nocapture`, `cargo check -p slug_bzlmod -p
  slug_external_cells -p slug_common`, and `git diff --check`.
- `ModuleExtensionExecutionKey` now requires workspace identity instead of
  storing optional provenance. Production extension execution keys still inherit
  the aggregation workspace, while test-only minimal constructors use an
  explicit test sentinel instead of modeling an absent-workspace production
  state. Validation passed with `cargo fmt --check`, `cargo test -p
  slug_bzlmod extension_execution -- --nocapture`, `cargo test -p
  slug_bzlmod replay_input_data -- --nocapture`, `cargo check -p slug_bzlmod
  -p slug_external_cells`, and `git diff --check`.
- Bzlmod projection data wrappers now require workspace provenance instead of
  accepting absent provenance. Repo-env, repo-mapping, resolution-facts,
  module-version, extension-aggregation, registered-toolchain, and
  registered-execution-platform data keys now reject cross-workspace injection
  unconditionally. Validation passed with `cargo fmt --check`, `cargo test -p
  slug_bzlmod set_bzlmod_projection_data -- --nocapture`, `cargo test -p
  slug_bzlmod data_only_projection -- --nocapture`, `cargo test -p
  slug_bzlmod extension_aggregation -- --nocapture`, `cargo test -p
  slug_bzlmod semantic_projection -- --nocapture`, `cargo check -p
  slug_bzlmod -p slug_common -p slug_external_cells`, and `git diff --check`.
- The legacy bzlmod resolver entry point now requires an explicit
  `WorkspaceId` instead of accepting `Option<WorkspaceId>` and deriving one
  from project root. Both persisted bridge and non-persisted bootstrap paths now
  pass the workspace identity chosen by their caller. Validation passed with
  `cargo fmt --check`, `cargo test -p slug_common
  bzlmod_projection_bridge -- --nocapture`, `cargo test -p slug_common
  explicit_output_base -- --nocapture`, `cargo check -p slug_common`, and
  `git diff --check`.
- The outer `parse_with_file_ops_and_options_inner` helper now also requires
  its empty-projection workspace identity explicitly. Public project-root
  wrappers choose the default or caller-provided output base, persisted bridge
  parsing passes the bridge key workspace, and no-project test parsing passes
  the named no-project sentinel. Validation passed with `cargo fmt --check`,
  `cargo test -p slug_common explicit_output_base -- --nocapture`, `cargo test
  -p slug_common bzlmod_projection_bridge -- --nocapture`, `cargo check -p
  slug_common -p slug_interpreter_for_build`, and `git diff --check`; a
  `testing_parse` filter matched zero tests and was not counted as evidence.
- The no-project workspace sentinel is now named directly on `WorkspaceId`, so
  callers that only need identity no longer construct an empty
  `BzlmodProjectionData` just to extract its cell graph workspace. The empty
  no-project projection remains available for test-only interpreter setup that
  still needs a full injected payload. Validation passed with `cargo fmt
  --check`, `cargo test -p slug_bzlmod
  workspace_id_names_no_project_sentinel -- --nocapture`, `cargo test -p
  slug_common bzlmod_projection_bridge -- --nocapture`, `cargo check -p
  slug_bzlmod -p slug_common -p slug_interpreter_for_build`, and `git diff
  --check`.
- Transitional process-global bzlmod dynamic-cell entries are now scoped by
  workspace identity, including output base, instead of only project root.
  Runtime bzlmod replay sets that scope from the DICE-projected cell graph
  workspace, so generated repo paths, setups, apparent aliases, and scoped
  repo aliases registered through the compatibility maps cannot be reused by
  another output-base workspace under the same project root. This does not make
  the maps DICE-owned, and filesystem directory-scan compatibility fallbacks
  remain transitional. Validation passed with `cargo fmt --check`, `cargo test
  -p slug_core dynamic_bzlmod_entries_are_scoped_to_current_output_base --
  --nocapture`, `cargo test -p slug_core dynamic_extension -- --nocapture`,
  `cargo test -p slug_core bzlmod_runtime_snapshot -- --nocapture`, `cargo
  test -p slug_common bzlmod_projection_bridge -- --nocapture`, `cargo check
  -p slug_core -p slug_common`, and `git diff --check`.
- Workspace-scoped bzlmod contexts no longer use the physical
  `bazel-external` directory-scan compatibility fallbacks to synthesize module
  or extension-generated cells. Project-root-only legacy scopes keep the old
  scan behavior, but once a command has an output-base workspace identity the
  resolver must use explicit runtime snapshots or scoped dynamic registrations
  rather than rediscovering shared on-disk repos. Validation passed with `cargo
  fmt --check`, `cargo test -p slug_core
  workspace_scoped_bzlmod_entries_do_not_scan_bazel_external -- --nocapture`,
  `cargo test -p slug_core dynamic_extension -- --nocapture`, `cargo test -p
  slug_core canonical_bazel_repo_name -- --nocapture`, `cargo test -p
  slug_core bzlmod_runtime_snapshot -- --nocapture`, `cargo check -p
  slug_core`, `cargo build -p slug`, `pytest
  tests/core/bzlmod/test_plan61_guardrails.py` (146 passed), and `git diff
  --check`. The test-created `slugd` daemons were cleaned up after the Python
  guardrail run.
- Workspace-scoped action external-name formatting and `external/` symlink
  repair no longer use shared `external/` symlinks or physical
  `bazel-external` suffix scans as canonical generated-repo evidence. They can
  still use explicit current-scope dynamic registrations and canonical
  `bazel-external/<repo-with-plus>` cell paths, but stale project-root
  filesystem state cannot rewrite apparent names or preserve a stale
  `external/<apparent>` link for another output-base workspace, even before the
  desired repo target has been materialized. Entering an output-base workspace
  scope also skips the legacy symlink repair pass instead of running it under
  the previous process-global scope, while entering a legacy project-root scope
  after a workspace-scoped request still runs the legacy module-form repair.
  Validation passed with `cargo fmt --check`, `cargo test -p slug_core
  workspace_scoped_external_symlink_replaces_stale_physical_fallback --
  --nocapture`, `cargo test -p slug_core
  workspace_scoped_external_symlink_replaces_unmaterialized_stale_link --
  --nocapture`, `cargo test -p slug_core
  workspace_scope_reset_does_not_run_legacy_external_symlink_repair --
  --nocapture`, `cargo test -p slug_core
  project_root_scope_reset_runs_legacy_external_symlink_repair_after_workspace_scope
  -- --nocapture`, `cargo test -p slug_core external_symlink -- --nocapture`,
  `cargo test -p slug_core cells::tests -- --nocapture --test-threads=1`,
  `cargo test -p slug_core dynamic_extension -- --nocapture`, `cargo check -p
  slug_core`, and `git diff --check`.
  A clean reviewing subagent re-reviewed the full action/symlink fallback
  series after the transition fixes and reported no findings, with focused
  `slug_core` workspace, external-symlink, and action-external-name tests
  passing. After rebuilding `target/debug/slug`, the full Plan 61 Python
  guardrail passed again with
  `TMPDIR=/var/mnt/dev/.slug-tmp TEST_EXECUTABLE=/var/mnt/dev/slug/target/debug/slug
  python -m pytest -q tests/core/bzlmod/test_plan61_guardrails.py -rx
  --tb=short` (`146 passed in 111.32s`). The four test-created `slugd`
  daemons were killed by PID after the run, and a follow-up `pgrep -af
  'target/debug/slugd|slugd'` found no remaining daemon.
- The injected `BzlmodCellGraphDataKey` is no longer exported as a downstream
  `slug_bzlmod` API. Current-workspace callers now use named helper APIs that
  compute the workspace-checked `BzlmodCellGraphKey`, and downstream tests that
  need bzlmod state injection use full `BzlmodProjectionData` through
  `SetBzlmodProjectionData` instead of writing the injected cell-graph key
  directly. The underlying graph is still legacy-produced, but the direct
  injected-data key is now crate-private transitional plumbing rather than a
  downstream-reachable key through either the crate root or `dice_graph` module
  path. A clean-review follow-up caught that `pub mod dice_graph` still made the
  key reachable by module path while the struct itself was public; making the
  key `pub(crate)` closes that surface. Validation passed with `cargo fmt
  --check`, `cargo test -p slug_bzlmod
  current_workspace_helpers_use_projection_workspace_id -- --nocapture`,
  `cargo test -p slug_common
  persisted_empty_bzlmod_projection_preserves_explicit_output_base --
  --nocapture`, `cargo test -p slug_external_cells
  extension_spoke_lookup_uses_injected_workspace_identity -- --nocapture`,
  `cargo test -p slug_analysis
  test_registered_toolchain_loading_records_dice_workspace_id -- --nocapture`,
  `cargo test -p slug_analysis
  test_registered_toolchain_lookup_error_clears_loaded_signature_without_caching_fallback
  -- --nocapture`, `cargo check -p slug_bzlmod -p slug_common -p
  slug_external_cells -p slug_analysis -p slug_interpreter_for_build`, and
  `git diff --check`. The `pub(crate)` follow-up also passed the same scoped
  multi-crate `cargo check`, plus `cargo test -p slug_bzlmod
  current_workspace_helpers_use_projection_workspace_id -- --nocapture` and
  `cargo test -p slug_external_cells
  extension_spoke_lookup_uses_injected_workspace_identity -- --nocapture`. A
  scoped search found no remaining downstream
  `slug_bzlmod::BzlmodCellGraphDataKey` or
  `slug_bzlmod::dice_graph::BzlmodCellGraphDataKey` references.
  A clean reviewing subagent later reported no findings over the API-hiding
  slice and independently reran the focused multi-crate check and helper tests
  in a separate target directory.
- Bzlmod load-path canonicalization now uses resolver-owned declared aliases and
  runtime snapshot aliases directly instead of the fallback-bearing
  `CellAliasResolver::canonical_bzlmod_repo_name_for_cell` helper. When a
  resolver has a runtime bzlmod alias snapshot, runtime aliases and module
  aliases are authoritative and misses do not fall back to stale process-global
  dynamic aliases; no-runtime-snapshot resolver misses now stay on the apparent
  repo name too. Both `InterpreterForDir` load resolution and DICE eval-import
  key canonicalization use this owner-only path, and `InterpreterForDir`
  explicit `load("@repo//...")` parsing now uses
  `parse_import_with_declared_or_runtime_aliases` so alias parsing cannot
  rewrite the repo through `CellAliasResolver::resolve(...)` before
  canonicalization. Bridge burn-down before/after evidence: before, `rg -n
  "canonical_bzlmod_repo_name_for_cell\\(path\\.cell|bzlmod_eval_import_cell_path_keeps_legacy_global_fallback|bzlmod_load_path_uses_empty_version_module_suffix" app/slug_interpreter_for_build/src/interpreter`
  found both production bridge calls plus tests preserving legacy
  directory/global fallback behavior; after it returns no hits, and `rg -n
  "parse_import_with_declared_or_runtime_aliases|resolve_declared_or_runtime_alias\\(path\\.cell|load_import_resolution_no_snapshot_rejects_global_miss|no_snapshot_miss_ignores_global_alias|uses_declared_empty_version_module_alias" app/slug_interpreter app/slug_interpreter_for_build/src/interpreter`
  shows the owner-only parse/canonicalization calls and stale-global miss
  coverage. A clean-review follow-up found that explicit `load("@repo//...")`
  parsing still reached process-global aliases before canonicalization, so this
  owner-only parse path was added. An earlier clean-review follow-up found that
  wrong-cell equivalence still accepted extension internal-name equivalence for
  repos absent from the runtime snapshot; that path now accepts internal-name
  equivalence only when there is no runtime snapshot or the snapshot owns the
  canonical extension repo. Validation passed with `cargo fmt`, `cargo test -p
  slug_core canonical_bzlmod_repo_name_for_cell -- --nocapture`, reran `cargo
  test -p slug_interpreter_for_build bzlmod_load_path -- --nocapture` (`5
  passed`) and `cargo test -p slug_interpreter_for_build
  bzlmod_eval_import_cell_path -- --nocapture` (`3 passed`), `cargo test -p
  slug_interpreter_for_build load_cell_equivalence_with_runtime_aliases --
  --nocapture`, reran `cargo test -p slug_interpreter_for_build
  load_import_resolution -- --nocapture` (`2 passed`), and `cargo check -p
  slug_interpreter -p slug_interpreter_for_build -p slug_analysis`; reran
  `cargo check -p slug_interpreter_for_build`, `cargo build -p slug`, `cargo
  fmt --check`, `git diff --check`, the before/after `rg` evidence above, and
  `TEST_EXECUTABLE=/var/mnt/dev/slug/target/debug/slug python -m pytest -q
  tests/core/bzlmod/test_plan61_guardrails.py -k
  'mapped_external_extension_bzl_load_edit_rejects_replay or
  missing_mapped_external_extension_bzl_load_creation_rejects_replay or
  mapped_external_extension_bzl_load_deletion_rejects_replay or
  mapped_external_extension_bzl_load_edit_rejects_audit_cell_replay or
  mapped_external_extension_bzl_load_edit_with_uncached_extension_rejects_audit_cell_replay or
  missing_mapped_external_extension_bzl_load_creation_rejects_audit_cell_replay
  or mapped_external_extension_bzl_load_deletion_rejects_audit_cell_replay' -rx
  --tb=short` (`7 passed, 148 deselected`) before commit.
- Bazel-style build-setting repo normalization now uses resolver-owned declared
  aliases and runtime snapshot aliases directly instead of the fallback-bearing
  `CellAliasResolver::canonical_bzlmod_repo_name_for_cell` helper and
  process-global `resolve_dynamic_extension_cell_alias(repo)` fallback in
  `config_setting(flag_values = ...)` matching. Runtime-snapshot and
  no-runtime-snapshot resolver misses stay authoritative and do not fall back to
  stale process-global aliases. Bridge burn-down before/after evidence: before,
  `rg -n
  "canonical_bzlmod_repo_name_for_cell\\(repo\\)|resolve_dynamic_extension_cell_alias\\(repo\\)" app/slug_analysis/src/analysis/calculation.rs`
  found the production bridge; after it returns no hits, and `rg -n
  "resolve_declared_or_runtime_alias\\(repo\\)|no_snapshot_miss_ignores_global_alias" app/slug_analysis/src/analysis/calculation.rs`
  shows the owner-only helper and stale-global miss coverage. A clean-review
  follow-up found that already-canonical `@@owner++extension+repo`
  build-setting labels could keep the `@@` sigil when the runtime snapshot owned
  that repo; canonical-sigil stripping now runs even when no repo rewrite is
  needed, and the runtime-miss guardrail now proves the label stays on the
  unresolved repo rather than a stale global alias. Earlier validation passed
  with `cargo test -p slug_analysis build_setting_lookup_normalization --
  --nocapture`, reran as `cargo test -p slug_analysis build_setting_lookup_ --
  --nocapture` (`4 passed`), `cargo check -p slug_analysis`, `cargo build -p
  slug`, `cargo fmt --check`, and `git diff --check`; the clean-review
  follow-up passed with
  `TMPDIR=/var/mnt/dev/.slug-tmp/rustc-plan61-build-setting
  CARGO_TARGET_DIR=/var/mnt/dev/.slug-tmp/slug-plan61-build-setting-target
  cargo test -p slug_core build_setting_labels -- --nocapture`.
- Generic `BuildSettingLabel` repo normalization now uses only the caller's
  supplied `CellAliasResolver` declared aliases and runtime snapshot aliases.
  The removed bridge surface is `resolve_bzlmod_build_setting_repo` reaching
  fallback-bearing `CellAliasResolver::canonical_bzlmod_repo_name_for_cell`
  and then process-global `resolve_dynamic_extension_cell_alias(repo)`.
  Resolverless parsing and no-runtime-snapshot resolver misses now keep the
  apparent repo spelling, while `@@` still strips to Slug's single-`@` internal
  syntax. The DICE/Skyframe-shaped owner is the build-setting caller's
  resolver-owned alias view, backed by the runtime cell graph snapshot and
  ultimately `BzlmodCellGraphKey`; parser calls without that owner are
  syntactic only. Bridge burn-down before/after evidence: before, `rg -n
  "canonical_bzlmod_repo_name_for_cell\\(repo\\)|resolve_dynamic_extension_cell_alias\\(repo\\)"
  app/slug_core/src/configuration/build_setting.rs` found both production
  bridge calls; after, it returns no hits, and `rg -n
  "resolve_declared_or_runtime_alias\\(repo\\)|no_snapshot|without_alias_owner|runtime_miss"
  app/slug_core/src/configuration/build_setting.rs` shows the owner-only helper
  plus stale-global miss coverage. Validation passed with
  `TMPDIR=/var/mnt/dev/.slug-tmp/cargo-build cargo test -p slug_core
  build_setting_labels -- --nocapture` (`5 passed`),
  `TMPDIR=/var/mnt/dev/.slug-tmp/cargo-build cargo test -p slug_configured
  target_platform_resolution -- --nocapture` (`7 passed`),
  `TMPDIR=/var/mnt/dev/.slug-tmp/cargo-build cargo test -p slug_analysis
  build_setting_lookup_ -- --nocapture` (`4 passed`),
  `TMPDIR=/var/mnt/dev/.slug-tmp/cargo-build cargo check -p slug_core -p
  slug_configured -p slug_analysis -p slug_build_api`, and
  `TMPDIR=/var/mnt/dev/.slug-tmp/cargo-build cargo build -p slug`.
- Target-owned output path cell naming now uses the configured target label's
  stored package cell directly instead of consulting process-global bzlmod
  alias/module helpers. The removed bridge surface is
  `BaseDeferredKey::make_hashed_path` and `make_unhashed_path` reaching
  `slug_core::cells::canonical_bazel_repo_name_for_cell` through
  `bazel_output_cell_name` while formatting `buck-out`/Bazel-output paths. The
  DICE/Skyframe-shaped owner is the configured target label produced from the
  active cell graph; output path formatting should not reinterpret that cell
  through mutable process-global alias state. Bridge burn-down before/after
  evidence: before, `rg -n
  "canonical_bazel_repo_name_for_cell\\(cell_name\\)|canonical_bazel_repo_name_for_cell\\("
  app/slug_core/src/deferred/base_deferred_key.rs` found the production bridge
  call; after, it returns no hits, and `rg -n
  "bazel_output_cell_name|without_global_alias|output_cell_name_without_owner|target_label_output_path"
  app/slug_core/src/deferred/base_deferred_key.rs` shows stored-cell formatting
  plus stale-global miss coverage. Validation passed with
  `TMPDIR=/var/mnt/dev/.slug-tmp/cargo-build cargo test -p slug_core
  output_cell -- --nocapture` (`1 passed`),
  `TMPDIR=/var/mnt/dev/.slug-tmp/cargo-build cargo test -p slug_core
  target_label_output_path -- --nocapture` (`1 passed`),
  `TMPDIR=/var/mnt/dev/.slug-tmp/cargo-build cargo test -p slug_core buck_ --
  --nocapture` (`6 passed`), `TMPDIR=/var/mnt/dev/.slug-tmp/cargo-build cargo
  check -p slug_core`, and `TMPDIR=/var/mnt/dev/.slug-tmp/cargo-build cargo
  build -p slug`. The first validation attempt exposed `/var/mnt/dev` at 100%
  full; generated temp Cargo target dirs and `target/debug/incremental` were
  removed, freeing about 108G before rerunning the tests.
- `CellResolver::get` root-alias cell-name lookup now uses only the resolver's
  declared aliases and runtime snapshot aliases instead of full
  `CellAliasResolver::resolve`, whose no-snapshot compatibility path can
  consult process-global dynamic/scoped aliases and directory-derived state.
  The removed bridge surface is unknown-cell lookup filling a miss from
  `root_cell_alias_resolver.resolve(cell.as_str())` before dynamic/runtime
  cell handling. The DICE/Skyframe-shaped owner is the active `CellResolver`'s
  root alias resolver, backed by the bzlmod runtime cell graph snapshot and
  ultimately `BzlmodCellGraphKey`; no-snapshot resolver misses now stay misses
  unless a declared alias owns them. Bridge burn-down before/after evidence:
  before, `rg -n "root_cell_alias_resolver\\.resolve\\(cell\\.as_str\\(\\)"
  app/slug_core/src/cells.rs` found the production bridge call; after, it
  returns no hits, and `rg -n
  "resolve_declared_or_runtime_alias\\(cell\\.as_str\\(\\)|cell_resolver_get_no_snapshot"
  app/slug_core/src/cells.rs` shows the owner-only lookup plus stale-global
  miss coverage. Validation passed with
  `TMPDIR=/var/mnt/dev/.slug-tmp/cargo-build cargo test -p slug_core
  cell_resolver_get_no_snapshot -- --nocapture` (`1 passed`),
  `TMPDIR=/var/mnt/dev/.slug-tmp/cargo-build cargo test -p slug_core
  cells::tests -- --nocapture --test-threads=1` (`52 passed`),
  `TMPDIR=/var/mnt/dev/.slug-tmp/cargo-build cargo check -p slug_core -p
  slug_common`, and `TMPDIR=/var/mnt/dev/.slug-tmp/cargo-build cargo build -p
  slug`.
- Toolchain implementation label parsing now uses only the active cell
  resolver's declared aliases and runtime snapshot aliases for repo-name
  canonicalization. Resolverless and no-runtime-snapshot resolver misses stay on
  the apparent repo instead of falling back to the process-global dynamic alias
  map. Bridge burn-down before/after evidence: before, `rg -n
  "resolve_dynamic_extension_cell_alias\\(repo_name\\)" app/slug_analysis/src/analysis/env.rs`
  found the production `resolve_impl_label_repo_name` fallback; after it returns
  no hits, and `rg -n
  "resolve_declared_or_runtime_alias\\(repo_name\\)|parse_impl_label_to_target_label_no_snapshot_resolver_miss_ignores_global_alias" app/slug_analysis/src/analysis/env.rs`
  shows the resolver-owned path and stale-global miss coverage. Validation
  passed with `cargo test -p slug_analysis parse_impl_label_to_target_label --
  --nocapture` (`4 passed`) and `cargo check -p slug_analysis`.
- Production metadata label/path canonicalization now uses only the active cell
  resolver's declared aliases and runtime snapshot aliases. `MetadataLabelContext::new`
  disables transitional process-global alias, module-cell, and scoped-alias
  fallbacks, so resolverless and no-runtime-snapshot resolver misses stay on the
  apparent repo; the old process-global behavior is isolated behind the test-only
  `MetadataLabelContext::empty` compatibility context. Bridge burn-down
  before/after evidence: before, `rg -n
  "canonical_dynamic_extension_cell_name\\(cell_name\\)|canonical_bzlmod_module_cell_name\\(cell_name\\)|resolve_scoped_bzlmod_repo_alias_for_current_cell" app/slug_analysis/src/analysis/env.rs`
  found production metadata fallbacks reachable from `MetadataLabelContext::new`;
  after, `rg -n
  "allow_process_global_fallbacks|metadata_paths_no_snapshot_resolver_miss_ignores_global_alias|metadata_owner_scoped_alias_no_snapshot_resolver_miss_ignores_global_alias" app/slug_analysis/src/analysis/env.rs`
  shows those helpers guarded by the test-only fallback flag plus stale-global
  dynamic/scoped miss coverage. Validation passed with `cargo test -p
  slug_analysis metadata_ -- --nocapture` (`15 passed`), `cargo check -p
  slug_analysis`, `cargo build -p slug`, `cargo fmt --check`, and `git diff
  --check`.
- Starlark `Label("//...")` lexical current-repo canonicalization now uses the
  active build context's cell alias resolver declared aliases and runtime
  snapshot instead of process-global module/dynamic canonical-name helpers.
  Resolverless and no-runtime-snapshot resolver misses keep the lexical file
  cell, while runtime-owned aliases still canonicalize through the owner
  snapshot. Bridge burn-down before/after evidence: before, `rg -n
  "canonical_bzlmod_module_cell_name\\(file_cell\\)|canonical_dynamic_extension_cell_name\\(file_cell\\)" app/slug_interpreter_for_build/src/interpreter/natives.rs`
  found the production current-repo bridge; after it returns no hits, and `rg
  -n
  "lexical_current_repo_name_for_label_context\\(|label_context_current_repo_no_snapshot_miss_ignores_global_alias|label_context_current_repo_prefers_runtime_aliases_before_globals" app/slug_interpreter_for_build/src/interpreter/natives.rs`
  shows the resolver-owned current-repo path and stale-global miss coverage.
  Validation passed with `cargo test -p slug_interpreter_for_build
  label_context_ -- --nocapture` (`11 passed`) and `cargo check -p
  slug_interpreter_for_build`.
- Configured provider/Bazel `Label` stringification now uses only the carried
  analysis-time `CellAliasResolver` declared aliases and runtime snapshot for
  Bazel-visible workspace/repo names. The bridge surface removed is
  `StarlarkConfiguredProvidersLabel::bazel_workspace_name`,
  `bazel_label_from_configured_with_alias_resolver`, and the analysis context
  repo/output helper reaching fallback-bearing
  `CellAliasResolver::canonical_bzlmod_repo_name_for_cell` or
  `slug_core::cells::canonical_bazel_repo_name_for_cell`; resolverless and
  no-runtime-snapshot misses now keep the apparent cell spelling while the root
  cell still maps to Bazel's empty repo name. The target owner is the carried
  `CellAliasResolver` alias view backed by the runtime cell graph snapshot and
  ultimately `BzlmodCellGraphKey`. Bridge burn-down before/after evidence:
  before, `rg -n
  "canonical_bzlmod_repo_name_for_cell\\(|slug_core::cells::canonical_bazel_repo_name_for_cell\\("
  app/slug_interpreter/src/types/configured_providers_label.rs
  app/slug_build_api/src/interpreter/rule_defs/bazel_label.rs
  app/slug_build_api/src/interpreter/rule_defs/context.rs` found production
  bridge calls; after, it returns no hits, and `rg -n
  "resolve_declared_or_runtime_alias\\(|no_snapshot|without_owner|without_alias_owner|runtime_miss"
  app/slug_interpreter/src/types/configured_providers_label.rs
  app/slug_build_api/src/interpreter/rule_defs/bazel_label.rs
  app/slug_build_api/src/interpreter/rule_defs/context.rs` shows owner-only
  calls plus stale-global miss coverage. Validation passed with
  `TMPDIR=/var/mnt/dev/.slug-tmp/cargo-build cargo test -p slug_interpreter
  configured_label -- --nocapture` (`5 passed`),
  `TMPDIR=/var/mnt/dev/.slug-tmp/cargo-build cargo test -p slug_build_api
  configured_label -- --nocapture` (`4 passed`),
  `TMPDIR=/var/mnt/dev/.slug-tmp/cargo-build cargo test -p slug_build_api
  analysis_context_repo_name -- --nocapture` (`5 passed`),
  `TMPDIR=/var/mnt/dev/.slug-tmp/cargo-build cargo check -p slug_interpreter
  -p slug_build_api`, and
  `TMPDIR=/var/mnt/dev/.slug-tmp/cargo-build cargo build -p slug`.
- Extension repo materialization now reads the current command repo-env through
  `BzlmodRepoEnvKey` when no current DICE spoke value is available, instead of
  using the serialized `repo_env_json` on `ExtensionRepoCellSetup` as the
  semantic input. The serialized value is still parsed for compatibility
  validation and can reveal malformed setup state, but stale serialized values
  no longer drive repository execution. Validation passed with `cargo fmt
  --check`, `cargo test -p slug_external_cells
  extension_repo_setup_repo_env_uses_current_dice_projection -- --nocapture`,
  `cargo test -p slug_external_cells extension_repo -- --nocapture`, `cargo
  check -p slug_external_cells -p slug_bzlmod`, and `git diff --check`.
- Successful extension repo file-ops delegate creation now always replays the
  output-base `external_cells/extension_repo/<canonical>` symlink through a
  single helper, including known-spec and use_repo_rule early-return paths.
  Previously only the late extension-execution branch replayed that symlink,
  which left some materialized repos without the output-base source path used
  by action command lines. Validation passed with `cargo fmt --check`, `cargo
  test -p slug_external_cells
  extension_repo_delegate_replays_workspace_output_base_symlink --
  --nocapture`, `cargo test -p slug_external_cells extension_repo --
  --nocapture`, `cargo check -p slug_external_cells -p slug_bzlmod`, and `git
  diff --check`.
- After the extension-repo repo-env and delegate-replay slices, the full Plan
  61 Python guardrail passed again with a freshly rebuilt Slug binary:
  `TMPDIR=/var/mnt/dev/.slug-tmp TEST_EXECUTABLE=/var/mnt/dev/slug/target/debug/slug
  python -m pytest -q tests/core/bzlmod/test_plan61_guardrails.py -rx
  --tb=short` (`146 passed in 106.03s`). The four test-created `slugd`
  daemons were killed by PID after the run, and a follow-up `pgrep -af
  'target/debug/slugd|slugd'` found no remaining daemon.
- Resolver-local runtime snapshots no longer make generated repo internal names
  root-visible aliases just because the generated repo cell exists in the
  snapshot. Alias resolution now treats the snapshot's extension-cell existence
  set as canonical-name-only, and same-extension sibling fallback checks that
  resolver-local snapshot before consulting transitional process-global dynamic
  maps. Validation passed with focused `slug_core` runtime-snapshot and exact
  generated-repo alias tests, serial `cargo test -p slug_core cells::tests --
  --nocapture --test-threads=1`, `cargo check -p slug_core -p slug_common -p
  slug_external_cells`, `cargo build -p slug`, the focused Plan 61 generated
  repo alias guardrail subset, the full Plan 61 Python guardrail with 72 tests,
  `cargo fmt --check`, and `git diff --check`. A broader serial
  `cargo test -p slug_core -- --test-threads=1` still has two unrelated
  pre-existing isolated failures in
  `build_setting_labels_resolve_dynamic_extension_aliases` and
  `pattern::pattern::tests::test_relaxed`.
- `BzlmodSessionData` no longer implements `Default`; empty session projection
  data must name an explicit project root, with the no-project sentinel
  confined to test/basic setup call sites. This does not remove the session
  bridge, but it demotes one accidental authority surface where code could
  synthesize a fake empty bzlmod workspace without acknowledging the workspace
  identity. Validation passed with focused `cargo test -p slug_bzlmod
  set_bzlmod_session_data_uses_session_workspace_id -- --nocapture`, `cargo
  check -p slug_bzlmod -p slug_common -p slug_interpreter_for_build`, `cargo
  test -p slug_common bzlmod -- --nocapture`, `cargo build -p slug`, the
  focused Plan 61 generated repo alias guardrail subset, `cargo fmt --check`,
  and `git diff --check`.
- Build-setting label canonicalization now normalizes Bazel's `@@` canonical
  repo sigil away when a dynamic generated-repo alias maps to a Slug cell name,
  instead of carrying `@@` into the internal `TargetLabel` cell. Validation
  passed with focused `cargo test -p slug_core
  build_setting_labels_resolve_dynamic_extension_aliases -- --nocapture`,
  `cargo check -p slug_core`, `cargo build -p slug`, `cargo fmt --check`, and
  `git diff --check`.
- Extension repository execution constructors that synthesize workspace
  identity from only a project root are now compiled only for tests. Production
  callers must use constructors that take explicit `WorkspaceId`/repo-env
  inputs, reducing another accidental bypass around output-base-sensitive
  repository materialization keys. Validation passed with `cargo check -p
  slug_bzlmod`, focused `cargo test -p slug_bzlmod repository_execution --
  --nocapture`, `cargo fmt --check`, and `git diff --check`.
- The persisted bzlmod config-load resolution key now threads the daemon
  output base from `ServerCommandContext` into `WorkspaceId` instead of
  hard-coding `<project>/buck-out/v2`, and the legacy resolver session is
  seeded from that keyed workspace identity. This keeps isolated output-base
  identity attached to the transitional DICE key while the resolver graph is
  still legacy-produced. Validation passed with focused `cargo test -p
  slug_common bzlmod_resolution_key_uses_explicit_output_base --
  --nocapture`, `cargo check -p slug_common -p slug_server`, `cargo build -p
  slug`, the focused Plan 61 generated-repo alias guardrail subset, `cargo
  fmt --check`, and `git diff --check`.
- Project-root convenience constructors for bzlmod projection keys and
  materialization-manifest helper paths are now compiled only for tests.
  Production callers must form those DICE keys from explicit `WorkspaceId`
  values instead of silently accepting the default output base. Validation
  passed with focused `cargo test -p slug_bzlmod dice_graph::tests --
  --nocapture`, focused `cargo test -p slug_bzlmod
  repository_execution::tests -- --nocapture`, `cargo check -p slug_bzlmod -p
  slug_common -p slug_external_cells -p slug_server`, `cargo build -p slug`,
  `cargo fmt --check`, and `git diff --check`.
- The persisted config-load path now also preserves the keyed output base when
  no `MODULE.bazel` exists. The empty bzlmod projection is initialized from
  the same `WorkspaceId` used by the projection bridge key, rather than
  falling back to `<project>/buck-out/v2` after DICE reports no root module.
  Validation passed with focused `cargo test -p slug_common
  persisted_empty_bzlmod_projection_preserves_explicit_output_base --
  --nocapture`, `cargo check -p slug_common -p slug_server`, `cargo build -p
  slug`, `cargo fmt --check`, and `git diff --check`.
- `BzlmodSessionData::empty_for_project_root` is now test-only. The remaining
  basic interpreter setup without a `ProjectRoot` uses a named no-project
  sentinel constructor, while production project paths initialize empty session
  projections through explicit `WorkspaceId` values. Validation passed with
  focused `cargo test -p slug_bzlmod
  set_bzlmod_session_data_uses_session_workspace_id -- --nocapture`, `cargo
  check -p slug_bzlmod -p slug_common -p slug_interpreter_for_build -p
  slug_build_api_tests`, `cargo build -p slug`, `cargo fmt --check`, and `git
  diff --check`.
- The direct config parser now has an explicit-output-base entry point, and
  daemon startup uses `InvocationPaths::buck_out_path()` when bootstrapping
  legacy cells with bzlmod lockfile mode disabled. Direct non-DICE bzlmod
  resolution now seeds the transitional session from that workspace identity
  instead of re-deriving `<project>/buck-out/v2`. Validation passed with
  focused `cargo test -p slug_common explicit_output_base -- --nocapture`,
  `cargo check -p slug_common -p slug_server -p slug_cmd_completion_client -p
  slug_client_ctx`, `cargo build -p slug`, `cargo fmt --check`, and `git diff
  --check`.

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
  external-cell marker gate accepts them. Known repo-spec extension file-ops
  now lets the repository execution manifest own marker/content/output-state
  staleness, recorded-input staleness, and layout validity, including missing
  declared BUILD-file, foreign top-level symlink, and invalid empty
  target-label checks, instead of deleting or repairing from a pre-DICE check.
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

- A single transitional `BzlmodProjectionBridgeDiceKey` still wraps the legacy
  resolver. Its command-policy identity now comes from a DICE key, and its
  cached value is the narrowed `BzlmodProjectionData` payload, but the wrapped
  resolver is still not a Skyframe-shaped module graph.
- The resolved graph, repo-mapping snapshots, cell graph, and registered
  toolchain and execution platform facts are still assembled during legacy
  cell setup, then injected as transitional command data. Registered
  toolchain/platform consumers, extension aggregation consumers, extension
  replay-input consumers, repo-mapping consumers, and module-version consumers
  now read narrower injected DICE values. Data-only projections carry their own
  source workspace provenance and no longer force an unrelated cell-graph
  compute; module-version and extension-aggregation consumers still read the
  named cell graph where they need root module or graph facts. The
  module-version value still carries a conservative projection invalidation
  identity until the remaining
  interpreter/materialization inputs are explicit. `BzlmodSessionData` and
  `BzlmodSessionDataKey` have been removed, but `BzlmodProjectionData` remains
  a transitional legacy-produced payload rather than a set of true
  Skyframe-shaped DICE producers. The projection payload now carries the
  current workspace identity inside its named cell graph, while module-version
  data, resolution facts, registrations, repo-mapping, and
  extension-aggregation data also carry source workspace provenance so stale
  cross-workspace projection data cannot be paired with that graph. Lockfile
  inputs, repo-env, resolution facts, repo mappings, registered toolchains,
  registered execution platforms, extension aggregations, and module versions
  have been split out of the projection payload and are injected separately
  with their own provenance. The narrower injected values are still populated
  from the legacy resolver output. The persisted config-load key now receives the
  server output base instead of synthesizing the default output base for
  workspace identity, and the no-`MODULE.bazel` empty-session projection now
  preserves that keyed output base. The daemon bootstrap direct parser now also
  accepts an explicit output base. Runtime module-symlink replay now uses the
  named cell graph's workspace output base for `external_cells/bzlmod` instead
  of hard-coding the project default, and extension-generated repo symlink
  replay uses the same output-base identity for
  `external_cells/extension_repo`. These paths still wrap or call the legacy
  resolver.
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
  and after a missing project-local helper is created. Audit-cell-only mapped
  external helper create/edit/delete transitions are now covered when the
  extension owner comes from a local override module and the root usage has a
  lockfile replay entry, including a mixed cached/uncached root-extension graph.
  Other external load failures and the full interpreter load graph are not
  replay-complete. The digest now has explicit DICE keys for
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
  Startup generated repo cells and dynamic aliases now go through the
  resolver-owned runtime snapshot instead of a process-global installer.
  Generated repos captured after extension execution still use transitional
  resolver-local/runtime plumbing rather than a final DICE-owned cell graph.
  The direct public helpers for forming extension execution and spoke keys from
  injected session-shaped values were removed, reducing accidental bypass
  surfaces without changing that remaining cell-graph ownership gap.
- Runtime extension file-ops now registers sibling spokes from the current
  `ExtensionSpokesValue` on the active `CellResolver` as graph-owned dynamic
  cells instead of publishing those siblings through the process-global dynamic
  registry. Validation passed with focused `cargo test -p slug_core
  bzlmod_resolver_registers_runtime_spoke_without_global_registry
  -- --nocapture`, `cargo test -p slug_external_cells
  known_repo_spec_defers_recorded_input_staleness_to_manifest -- --nocapture`,
  `cargo check -p slug_core -p slug_external_cells -p slug_server`,
  `cargo build -p slug`, and focused Plan 61 Python guardrail
  `lockfile_replay_recorded_repo_mapping_from_extension_repo_source`.
- Extension execution now registers captured generated repos on the active
  `CellResolver` with a full `ExtensionRepoCellSetup` instead of publishing
  only canonical names into the process-global dynamic registry. Validation
  passed with `cargo check -p slug_interpreter_for_build -p slug_core -p
  slug_server`, `cargo build -p slug`, and focused Plan 61 Python guardrails
  `missing_lockfile_extension_executes_once_then_reuses_dice_state` plus
  `lockfile_replay_recorded_repo_mapping_from_extension_repo_source`.
- After the tracked root input and resolver-owned generated-repo registration
  slices, the full Plan 61 Python guardrail file passed with
  `env TMPDIR=/var/mnt/dev/slug-test-tmp
  TEST_EXECUTABLE=/var/mnt/dev/slug/target/debug/slug python -m pytest -q
  tests/core/bzlmod/test_plan61_guardrails.py -rx --tb=short`:
  `74 passed in 72.08s`. The four slugd processes left by the test harness
  were cleaned up afterward.
- Module extension label path resolution now projects generated repo cells,
  dynamic aliases, and root aliases from the active `CellResolver` bzlmod
  runtime snapshot before falling back to process-global dynamic maps. This
  lets `module_ctx.path(Label(...))` resolve common generated-repo labels from
  resolver-local graph state during extension evaluation. Validation passed with
  `cargo test -p slug_core
  bzlmod_label_cell_paths_project_runtime_snapshot_without_globals --
  --nocapture`, `cargo check -p slug_core -p slug_interpreter_for_build -p
  slug_external_cells -p slug_server`, `cargo build -p slug`, focused Plan 61
  Python guardrails for generated-repo materialization, injected/override repo
  labels, and recorded repo mappings (`6 passed, 68 deselected`), the full Plan
  61 Python guardrail with `74 passed in 83.33s`, `cargo fmt --check`, and
  `git diff --check`. The four slugd processes left by the full guardrail were
  cleaned up afterward.
- Repository rule label path resolution now receives the same resolver-owned
  cell path map, including bzlmod runtime snapshot aliases, when
  `RepositoryContext` is created by the Starlark repo-rule executor. This
  reduces `repository_ctx.path/read/symlink/template/patch/watch` label
  resolution reliance on process-global generated-repo maps for labels visible
  in the active resolver. Validation passed with `cargo test -p
  slug_interpreter_for_build
  repository_context_label_paths_use_resolver_owned_cell_paths_before_globals
  -- --nocapture`, `cargo check -p slug_interpreter_for_build -p slug_server`,
  `cargo build -p slug`, focused Plan 61 label/repo-mapping guardrails
  (`6 passed, 68 deselected`), the full Plan 61 Python guardrail with
  `74 passed in 79.03s`, `cargo fmt --check`, and `git diff --check`. The four
  slugd processes left by the full guardrail were cleaned up afterward.
- `repository_ctx.path(Label(...))` lazy materialization now also derives the
  canonical extension-generated repository name from the resolver-owned cell
  path map before falling back to the legacy process-global dynamic maps. This
  closes the follow-up gap where a resolver-only generated-repo alias could
  return the correct filesystem path but not fetch-trigger through the same
  resolver-owned state. Validation passed with `cargo test -p
  slug_interpreter_for_build repository_context_ -- --nocapture` (`4 passed`),
  `cargo check -p slug_interpreter_for_build -p slug_server`, `cargo build -p
  slug`, focused Plan 61 label/repo-mapping guardrails (`6 passed, 68
  deselected`), the full Plan 61 Python guardrail with `74 passed in 80.20s`,
  `cargo fmt --check`, and `git diff --check`. The four slugd processes left
  by the full guardrail were cleaned up afterward.
- Bzlmod load-path alias equivalence now asks a declared/runtime-only
  `CellAliasResolver` helper before falling back to process-global dynamic
  alias maps when deciding whether a reformed canonical load path is equivalent
  to the originally parsed path. This lets resolver-owned runtime aliases
  explain load-path equivalence without requiring a compatibility global, and
  the focused regression installs a conflicting process-global alias to prove
  ordering. Validation passed with `cargo test -p slug_interpreter_for_build
  load_cell_equivalence_ -- --nocapture` (`5 passed`), `cargo check -p
  slug_core -p slug_interpreter_for_build -p slug_server`, `cargo build -p
  slug`, focused Plan 61 scoped-alias/mapped-load guardrails (`4 passed, 70
  deselected`), the full Plan 61 Python guardrail with `74 passed in 80.85s`,
  `cargo fmt --check`, and `git diff --check`. The four slugd processes left
  by the full guardrail were cleaned up afterward.
- `CellResolver::get_cell_path()` now prefers graph-owned dynamic cells and
  resolver-owned bzlmod runtime snapshot cells before falling back to
  root-scoped process-global dynamic cells. This keeps legacy compatibility
  while preventing stale root-scoped dynamic mappings from explaining a path
  ahead of the active resolver's own runtime graph. Validation passed with
  focused `cargo test -p slug_core
  get_cell_path_prefers_runtime_snapshot_over_root_scoped_dynamic_cell --
  --nocapture`, `cargo test -p slug_core
  bzlmod_resolver_uses_runtime_snapshot_for_lazy_extension_cell --
  --nocapture`, `cargo test -p slug_core
  bzlmod_label_cell_paths_project_runtime_snapshot_without_globals --
  --nocapture`, `cargo check -p slug_core -p slug_interpreter_for_build -p
  slug_server`, `cargo build -p slug`, focused Plan 61 label/repo-mapping
  guardrails (`6 passed, 68 deselected`), the full Plan 61 Python guardrail
  with `74 passed in 73.49s`, `cargo fmt --check`, and `git diff --check`.
  The four slugd processes left by the full guardrail were cleaned up
  afterward.
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
  injected-session lookup at runtime. Extension repo materialization also
  prefers that current repo-env key over serialized setup repo-env when a
  current DICE spoke value is unavailable.
- Registered toolchain and execution-platform facts now reach analysis through
  narrower DICE values, and the eager-load fast path is keyed by the
  DICE-derived registration signature, but the final `DeclaredToolchainInfo`
  registry remains process-global output plumbing rather than a DICE value.
  The legacy setup path also no longer scans `bazel-external` for
  diagnostic-only pending toolchain repo logs; removing that poll reduces
  incidental filesystem state without completing the final DICE-owned
  toolchain/materialization graph.
- Toolchain resolution now takes an explicit declared-toolchain snapshot from
  its caller instead of reading the process-global registry inside the pure
  resolver. The caller still snapshots transitional global state, so this only
  narrows the ownership boundary; it does not make registered toolchains
  DICE-owned. Validation passed with `cargo test -p slug_analysis
  test_resolve_toolchains_uses_explicit_declared_snapshot -- --nocapture`,
  `cargo test -p slug_analysis toolchain_resolution -- --nocapture` (`8
  passed`), `cargo check -p slug_analysis -p slug_server`, `cargo build -p
  slug`, and the full Plan 61 Python guardrail with `74 passed in 39.83s`.
  The slugd processes left by the full guardrail were cleaned up afterward.
- Toolchain `target_settings` enrichment now consumes the same explicit
  declared-toolchain snapshot as the resolver instead of reopening the
  process-global registry inside the helper, and the deferred-load retry
  refreshes both the declared-toolchain snapshot and derived target platform
  constraints before resolving again. The snapshot producer is still the
  transitional registry. Validation passed with `cargo test -p slug_analysis
  test_declared_toolchain_target_settings_use_explicit_snapshot --
  --nocapture`, `cargo test -p slug_analysis toolchain -- --nocapture` (`12
  passed`), the focused production guardrail
  `test_deferred_toolchain_retry_recomputes_target_settings` using Bazel-valid
  `config_setting(values = {"compilation_mode": "fastbuild"})` and
  `DefaultInfo(files = depset([out]))`, `cargo check -p slug_analysis -p
  slug_server`, `cargo build -p slug`, and the full Plan 61 Python guardrail
  with `75 passed in 39.94s`. The slugd processes left by the full guardrail
  were cleaned up afterward.
- Registered toolchain package loading now keeps label parsing syntactic and
  resolves the parsed repository name through the active `CellResolver`'s
  declared/runtime alias snapshot before falling back through legacy resolver
  lookup. Eager and deferred load-list construction build `PackageLabel`s from
  the resolved `CellInstance` name instead of pre-canonicalizing labels with the
  process-global dynamic alias helper. Validation passed with `cargo test -p
  slug_analysis
  registered_toolchain_package_label_prefers_runtime_aliases_before_globals --
  --nocapture` (including a resolvable conflicting process-global alias), `cargo
  test -p slug_analysis
  test_parse_registered_toolchain_label -- --nocapture`, `cargo test -p
  slug_analysis toolchain -- --nocapture` (`13 passed`), `cargo check -p
  slug_analysis -p slug_server`, `cargo build -p slug`, and the full Plan 61
  Python guardrail with `75 passed in 40.35s`. The slugd processes left by the
  full guardrail were cleaned up afterward.
- `config_setting(flag_values = ...)` build-setting lookup now normalizes bzlmod
  repo spellings through the active config-setting cell's declared/runtime alias
  resolver before falling back to the process-global dynamic alias helper. This
  lets resolver-owned runtime aliases explain transitioned build-setting labels
  without requiring a compatibility global. Validation passed with `cargo test
  -p slug_analysis build_setting_lookup_ -- --nocapture` (`2 passed`), `cargo
  test -p slug_analysis -- --nocapture` (`32 passed`), `cargo check -p
  slug_analysis -p slug_server`, `cargo build -p slug`, and the full Plan 61
  Python guardrail with `75 passed in 40.25s`. The slugd processes left by the
  full guardrail were cleaned up afterward.
- Build-setting label parsing now has a resolver-aware entry point used by
  config-setting lookup and CLI build-setting folding when those callers already
  carry an active `CellAliasResolver`. A resolver with a bzlmod runtime snapshot
  is authoritative, so stale process-global dynamic aliases cannot rewrite
  `@`/`@@` build-setting repo spellings or config-setting flag-value repo-name
  normalization ahead of the resolver-owned alias map.
  Validation passed with focused `cargo test -p slug_core build_setting_labels
  -- --nocapture`, `cargo test -p slug_configured
  target_platform_resolution::tests::cell_alias_is_canonicalized_at_storage_time
  -- --nocapture`, `cargo test -p slug_analysis build_setting_lookup_ --
  --nocapture` (`3 passed` after the config-setting normalization guardrail),
  `cargo check -p slug_core -p slug_analysis -p slug_configured`, `cargo build
  -p slug`, `cargo fmt --check`, and `git diff --check`.
- Metadata path canonicalization and toolchain implementation label parsing now
  also treat a resolver with a bzlmod runtime snapshot as authoritative on
  misses, so stale process-global dynamic aliases cannot fill an unowned repo
  spelling once the analysis caller has passed a runtime snapshot. Owner-scoped
  metadata alias canonicalization now uses a resolver-owned alias view for the
  owner cell and gates legacy scoped-alias globals the same way. Validation
  passed with `cargo test -p slug_analysis metadata_paths_ -- --nocapture` (`2
  passed`), `cargo test -p slug_analysis metadata_owner_scoped_alias --
  --nocapture` (`2 passed`), and `cargo test -p slug_analysis
  parse_impl_label_to_target_label -- --nocapture` (`3 passed`), `cargo check
  -p slug_analysis`, `cargo fmt --check`, and `git diff --check`.
- Starlark `Label("@repo//...")` explicit and owner-scoped repo
  canonicalization now carries the active `BuildContext` alias resolver into
  focused helpers, and a resolver with a bzlmod runtime snapshot is
  authoritative on misses before the transitional process-global dynamic/scoped
  alias maps. The explicit-repo helper no longer falls back to
  process-global dynamic or scoped aliases when no active resolver snapshot is
  available. A follow-up reviewer found the indirect no-runtime-snapshot bridge:
  the helper still called `CellAliasResolver::resolve(apparent_repo_name)`,
  whose legacy compatibility path can consult process-global aliases. That
  indirect bridge is now removed too: `Label()` uses
  `resolve_declared_or_runtime_alias(apparent_repo_name)`, so the DICE/Skyframe
  shaped owner remains the active `CellAliasResolver` from `BuildContext`,
  backed by bzlmod runtime alias snapshots and ultimately
  `BzlmodCellGraphKey`; no-snapshot resolvers can only use their declared alias
  map. Bridge burn-down before/after evidence: before, `rg -n
  "alias_resolver\\.resolve\\(apparent_repo_name\\)" app/slug_interpreter_for_build/src/interpreter/natives.rs`
  found the production helper call; after, it returns no hits and `rg -n
  "resolve_declared_or_runtime_alias\\(apparent_repo_name\\)|no_snapshot_resolver" app/slug_interpreter_for_build/src/interpreter/natives.rs`
  shows the owner-only helper plus no-snapshot stale-global regression coverage.
  Validation passed with `cargo test -p
  slug_interpreter_for_build label_context_explicit_repo -- --nocapture` (`2
  passed`), `cargo test -p slug_interpreter_for_build label_context_scoped_repo
  -- --nocapture` (`2 passed`), and reran with `cargo test -p
  slug_interpreter_for_build label_context_ -- --nocapture` (`9 passed`);
  `cargo check -p slug_interpreter_for_build`; `cargo build -p slug`; `cargo
  fmt --check`; `git diff --check`; and
  `TEST_EXECUTABLE=/var/mnt/dev/slug/target/debug/slug python -m pytest -q
  tests/core/bzlmod/test_plan61_guardrails.py -k
  'transitive_repo_name_aliases_are_scoped_to_declaring_module or
  root_repo_name_alias_does_not_leak_to_transitive_module or
  root_use_repo_alias_does_not_leak_to_transitive_module or
  inject_repo_keyword_alias_maps_generated_repo_and_replays' -rx --tb=short`
  (`4 passed, 151 deselected`).
- Bzlmod non-root `CellAliasResolverKey` now preserves the root resolver's
  runtime alias snapshot while still narrowing static aliases to canonical
  names, so non-root module resolvers can resolve DICE-owned generated-repo
  aliases without leaking root apparent names or consulting stale process-global
  aliases on runtime misses. Validation passed with `cargo test -p slug_common
  bzlmod_non_root_alias_resolver_preserves_runtime_snapshot -- --nocapture`.
- Registered toolchain loading's temporary process-global fast path now keys
  its loaded signature by the DICE-projected `WorkspaceId` plus registered
  toolchain list, so isolated output bases for the same project root cannot
  share a stale `DeclaredToolchainInfo` registry. If the DICE projection is
  unavailable, the fallback now clears the registry/deferred pool and leaves the
  loaded signature uncached instead of synthesizing a project-root-only
  workspace identity. Validation passed with
  `cargo test -p slug_analysis
  test_registered_toolchain_loading_records_dice_workspace_id -- --nocapture`,
  `cargo test -p slug_analysis
  test_toolchain_loading_signature_includes_workspace_id_and_registered_toolchains
  -- --nocapture` and `cargo test -p slug_analysis
  test_registered_toolchain_lookup_error_clears_loaded_signature_without_caching_fallback
  -- --nocapture`; the error-path regression now exercises the production
  `ensure_registered_toolchains_loaded` branch and checks the old signature is
  cleared, no requested/stale/project-root fallback signature is cached, and the
  declared registry, deferred pool, per-key deferred markers, and load-all
  deferred marker are cleared.
- Deferred registered-toolchain state now lives in one signature-scoped state
  object instead of three independent process globals. `ensure_deferred_toolchains_loaded`
  recomputes the current DICE registered-toolchain signature, requires it to
  match the eager loaded registry, and ignores mismatched deferred pool/marker
  state. This is still transitional process state rather than a DICE-owned
  registry, but it closes the concrete cross-workspace deferred-pool leak.
  Validation passed with `TMPDIR=/var/mnt/dev/.slug-tmp/plan61-toolchain-state
  cargo test -p slug_analysis toolchain_loading -- --nocapture` (`2 passed`),
  `cargo test -p slug_analysis test_deferred_toolchain_state_is_scoped_by_loading_signature
  -- --nocapture`, `cargo test -p slug_analysis
  test_registered_toolchain_lookup_error_clears_loaded_signature_without_caching_fallback
  -- --nocapture`, and `cargo test -p slug_analysis
  test_deferred_retry_ignores_optional_miss -- --nocapture`.
  Clean review of `ae098f62` found a stale-caller race where a request that
  computed signature A before waiting on the deferred-load lock could clear a
  newer signature B state after the eager loader installed B. The follow-up fix
  rechecks the loaded/deferred signature after acquiring the deferred-load lock
  and makes mismatched helper lookups return empty/false without clearing the
  current state. Follow-up validation reran the focused signature-scoping,
  lookup-error, and deferred-retry tests, plus `cargo check -p slug_analysis`,
  `cargo fmt --check`, and `git diff --check`.
- Extension repo file-ops no longer accepts a no-spec/no-spoke
  `.slug_repo_complete` marker as semantic authority. If setup and registered
  DICE spokes do not provide a current `RepoSpec`, the path now always enters
  the DICE spoke/use_repo_rule lookup and the repository execution key so reuse
  is owned by the materialization manifest. Validation passed with `cargo test
  -p slug_external_cells no_spec_complete_marker_does_not_skip_dice_execution
  -- --nocapture`, `cargo test -p slug_external_cells extension_repo::tests --
  --nocapture`, and `cargo check -p slug_external_cells -p slug_bzlmod`.
- Lockfile spoke pre-seeding now consumes the `TrackedExtensionBzlDigestKey`
  digest when bzlmod resolution is running with DICE inputs, so the pre-seed
  path no longer performs its own direct `.bzl` digest scan in that mode.
  Non-DICE bootstrap callers keep the old direct scanner until the remaining
  bridge is removed. Validation passed with `cargo test -p slug_bzlmod
  lockfile_preseed_uses_tracked_bzl_digest_when_provided -- --nocapture`,
  `cargo test -p slug_bzlmod lockfile_preseed_skips_stale_extension_cache --
  --nocapture`, and `cargo check -p slug_common -p slug_bzlmod`.
- Project-local missing `.bzl` load state in the tracked preseed digest path no
  longer performs a direct `std::fs` read just to obtain OS error text. The
  missing-file hash input is the deterministic
  `No such file or directory (os error 2)` state already expected by the
  guardrails. Non-DICE callers remain on the transitional direct scanner.
- `ExtensionBzlTransitiveDigestKey` now asks the interpreter-side module
  extension executor for the actual loaded-module graph digest before falling
  back to the transitional literal-load scanner. The loaded-graph path reads
  each loaded `.bzl` implementation file through `DiceFileComputations` and
  preserves the existing `bzl_transitive_v2` hash format, using the caller's
  replay extension id rather than any internal aggregation spelling and
  excluding Slug's implicit `@slug_builtins` autoload from Bazel lockfile input
  identity. Missing-load cases still fall back so existing lockfile replay can
  cover a missing helper until creation makes the graph loadable and stale.
  Validation passed with `cargo check -p slug_bzlmod -p
  slug_interpreter_for_build`, `cargo test -p slug_bzlmod
  bzl_transitive_digest -- --nocapture`, `cargo build -p slug`, focused Plan 61
  Python guardrails for project-local transitive edit, project-local missing
  transitive creation, and mapped external edit/create/delete replay rejection
  (`5 passed`), the eight replay/materialization guardrails exposed by the
  first full run (`8 passed` after the identity/autoload fix), and the full Plan
  61 Python guardrail (`119 passed in 132.46s`).
- Loaded-graph `ExtensionBzlTransitiveDigestKey` values now carry a tracked
  source bit: DICE-loaded graph digests are transaction-valid, while fallback
  literal-scanner digests remain invalid across transactions so missing-load
  creation can still be observed. Validation passed with `cargo test -p
  slug_bzlmod extension_spokes_lookup_keys_cache_after_digest_dependency --
  --nocapture`, `cargo check -p slug_bzlmod -p slug_interpreter_for_build`,
  `cargo build -p slug`, focused Plan 61 replay/warm-noop guardrails (`4
  passed`), and the full Plan 61 Python guardrail (`119 passed in 129.83s`).
- `ExtensionBzlTransitiveDigestKey` no longer silently uses the transitional
  literal scanner when the interpreter-side module extension executor is not
  registered. With an aggregation present, the DICE key now requires the
  executor-owned loaded-graph path and errors if that executor is unavailable;
  the remaining scanner fallback is limited to no-aggregation cases and
  non-DICE direct callers. Validation passed with `cargo test -p slug_bzlmod
  extension_bzl_digest_key_requires_executor_when_aggregation_exists --
  --nocapture`, `cargo test -p slug_bzlmod bzl_transitive_digest --
  --nocapture`, and `cargo test -p slug_bzlmod
  extension_spokes_lookup_keys_cache_after_digest_dependency -- --nocapture`,
  `cargo check -p slug_bzlmod -p slug_interpreter_for_build`, `cargo build -p
  slug`, and the focused Plan 61 replay subset for warm replay, transitive
  loaded `.bzl` edits, missing-load creation, and mapped external
  edit/create/delete replay rejection (`6 passed, 117 deselected`).
- DICE extension replay no longer falls back to the transitional literal-load
  scanner when the real executor cannot load the implementation graph. This
  matches Bazel's `SingleExtensionEvalFunction`: `RegularRunnableExtension.load`
  loads the extension `.bzl` before lockfile lookup, so a missing transitive
  load is a module-extension load error rather than a replay hit. Focused
  guardrails now prove a missing project-local or mapped external helper is not
  masked by lockfile replay, while creating the helper makes the loaded graph
  digest stale and runs the extension. Validation passed with `cargo check -p
  slug_bzlmod -p slug_interpreter_for_build`, `cargo test -p slug_bzlmod
  bzl_transitive_digest -- --nocapture`, `cargo test -p slug_bzlmod
  extension_bzl_digest_key_requires_executor_when_aggregation_exists --
  --nocapture`, `cargo test -p slug_bzlmod
  extension_spokes_lookup_keys_cache_after_digest_dependency -- --nocapture`,
  `cargo build -p slug`, and the focused extension `.bzl` replay subset
  selected by `-k 'extension_bzl_load or transitive_extension_bzl_load or
  mapped_external_extension_bzl_load'` (`8 passed, 117 deselected`), plus the
  full Plan 61 Python guardrail (`125 passed in 160.75s`); no stale `slugd`
  process remained after cleanup.
- Out-of-project bzlmod text reads used by module-file inputs, includes,
  registry-cache files, and hidden lockfiles now flow through a named
  `AbsoluteTextFileInputKey` child when the parent DICE computation reads the
  file. The key is still `validity=false` and the higher-level poll-digest
  construction still performs transitional direct reads, but parent bzlmod
  computations now have an auditable DICE dependency for those text-file
  contents. Validation passed with focused `cargo test -p slug_common
  absolute_text_file_input_key_tracks_polled_transitions -- --nocapture` and
  `cargo test -p slug_common
  out_of_project_module_include_reads_use_polled_text_key -- --nocapture`.
- Toolchain implementation labels, toolchain type alias chasing, C++ toolchain
  metadata labels, module-map metadata labels, and target-setting labels now
  parse through the active cell resolver's declared/runtime alias snapshot before
  falling back to process-global dynamic aliases. Validation passed with `cargo
  test -p slug_analysis parse_impl_label_to_target_label -- --nocapture` (`2
  passed`), `cargo test -p slug_analysis toolchain -- --nocapture` (`13
  passed`), `cargo test -p slug_analysis -- --nocapture` (`33 passed`), `cargo
  check -p slug_analysis -p slug_server`, `cargo build -p slug`, and the full
  Plan 61 Python guardrail with `75 passed in 47.69s`. The slugd processes left
  by the full guardrail were cleaned up afterward.
- C++ toolchain metadata and action-path helpers now carry a metadata label
  context seeded from the active `CellResolver`, so generated-repo path
  formatting, source-directory fallbacks, alias chasing, runtime library data,
  and feature/action-set data label expansion prefer resolver-owned
  declared/runtime aliases before consulting process-global dynamic aliases.
  Validation passed with `cargo test -p slug_analysis metadata_ -- --nocapture`
  (`10 passed`), `cargo test -p slug_analysis -- --nocapture` (`34 passed`),
  `cargo check -p slug_analysis -p slug_server`, `cargo build -p slug`, the full
  Plan 61 Python guardrail with `75 passed in 39.17s`, `cargo fmt --check`, and
  `git diff --check`. The slugd processes left by the full guardrail were
  cleaned up afterward.
- `module_ctx` and `repository_ctx` label filesystem resolution now treat an
  explicit resolver-owned cell path map as authoritative: missing external repos
  no longer fall through to process-global dynamic aliases, process-global
  project-root state, `bazel-external` directory scans, or a synthetic
  `repo/pkg/target` workspace path. Root labels still resolve from the active
  project root. Validation passed with focused `slug_interpreter_for_build`
  tests for missing resolver-owned label paths and conflicting globals, `cargo
  test -p slug_interpreter_for_build label_filesystem -- --nocapture` (`5
  passed`), `cargo test -p slug_interpreter_for_build module_ctx --
  --nocapture` (`30 passed`), and `cargo test -p slug_interpreter_for_build
  repository_context_ -- --nocapture` (`5 passed`), `cargo check -p
  slug_interpreter_for_build`, `TMPDIR=/var/mnt/dev/.slug-tmp/cargo-build
  cargo build -p slug`, the explicit-binary Plan 61 selector for module_ctx and
  repository_ctx Label paths (`15 passed, 140 deselected`), `cargo fmt
  --check`, and `git diff --check`.
- `module_ctx` Label-taking methods no longer fall back to the legacy
  repository-label resolver or a raw/synthetic Label string path when a
  `ModuleContext` was not seeded with resolver-owned label paths. Production
  module-extension execution already seeds those paths from the active
  `CellResolver`; callers missing that owner now fail with a resolver-owned
  bzlmod cell-path error instead of consulting process-global aliases,
  `bazel-external` scans, or extension-working-directory relative fallbacks.
  Bridge burn-down note: the production surface reduced is the `module_ctx`
  fallback through `resolve_label_to_path`, the `module_ctx.execute` raw-label
  fallback, and the shared `LabelFilesystemResolver` synthetic-miss path; the
  intended owner is `ModuleExtensionExecutionKey` constructing `ModuleContext`
  from the active cell resolver backed by
  `BzlmodCellGraphKey`/`BzlmodCellGraphDataKey`. The remaining bridge is still
  the legacy-produced cell graph. Before/after evidence:
  `rg -n "resolve_label_to_path|Fallback to legacy resolution if cell paths not available" app/slug_interpreter_for_build/src/module_ctx app/slug_interpreter_for_build/src/repository_ctx.rs`
  now returns no hits; the production resolver fallback search
  `rg -n "allow_legacy_fallbacks|without_legacy_fallbacks|scan_bazel_external_fallback|bazel_external_scan_dirs|PathBuf::from\\(repo\\)" app/slug_interpreter_for_build/src/label_filesystem.rs app/slug_interpreter_for_build/src/module_ctx app/slug_interpreter_for_build/src/repository_ctx.rs`
  now returns no hits. Validation passed with `cargo test -p
  slug_interpreter_for_build module_ctx -- --nocapture` (`30 passed`) and the
  explicit-binary Plan 61 selector for module_ctx and repository_ctx Label paths
  (`15 passed, 140 deselected`).
- `repository_ctx` Label path and lazy-materialization ownership now matches
  the same resolver-owned shape. `RepositoryContext` no longer carries a
  `resolver_owned_label_paths` mode bit: all path-like label resolution uses
  the context's explicit cell-path map, and resolver-owned misses fail instead
  of synthesizing workspace paths from the raw repo spelling. Missing canonical
  materialization names now stay as the apparent repo name instead of consulting
  process-global dynamic aliases/cells. Bridge burn-down note: the production
  surface reduced is the `RepositoryContext` branch that resolved unseeded or
  missing contexts through process-global aliases, `bazel-external` scans, and
  raw repo-name workspace paths; the intended owner is `StarlarkRepoRuleExecution`
  constructing `RepositoryContext` from the active `CellResolver` backed by
  `BzlmodCellGraphKey`/`BzlmodCellGraphDataKey`. The remaining bridge is still
  the legacy-produced cell graph. Before/after evidence:
  `rg -n "resolver_owned_label_paths|if self\\.resolver_owned_label_paths|resolve_dynamic_extension_cell_alias\\(repo\\)|get_dynamic_extension_cell\\(&resolved_repo\\)|resolver\\.resolve_label_string\\(label_str\\)" app/slug_interpreter_for_build/src/repository_ctx.rs`
  now returns no hits, and the old assertions that `@llvm-raw` or a missing
  apparent alias produced `workspace_root/<repo>/...` now assert the
  resolver-owned-path error. Validation passed with `cargo test -p
  slug_interpreter_for_build repository_context_ -- --nocapture` (`5 passed`),
  `cargo check -p slug_interpreter_for_build`, `TMPDIR=/var/mnt/dev/.slug-tmp/cargo-build
  cargo build -p slug`, `cargo fmt --check`, and `git diff --check`.
- Native repository-rule `build_file` and `patches` label resolution can now use
  resolver-owned bzlmod cell paths from the DICE cell graph during extension
  repository execution. That path prefers graph-owned aliases and cells over
  stale normal directories, rejects resolver-owned misses instead of reading
  source-tree or `bazel-external` collision paths, requires explicit graph
  aliases for apparent module names, and does not promote extension internal
  names as global aliases. Scoped aliases remain intentionally unflattened
  because the native executor does not yet carry the declaring-module owner
  context. Bzlmod load resolution now treats a runtime alias/cell snapshot as
  authoritative before consulting process-global dynamic aliases, scoped aliases,
  or cells, while canonical `module+` load paths can still resolve through an
  existing static module cell whose graph-owned path is
  `bazel-external/module+`. Validation passed with focused
  `cargo test -p slug_bzlmod resolve_build_file_label -- --nocapture` (`9
  passed`), `cargo test -p slug_bzlmod
  http_archive_build_file_uses_resolver_owned_label_path -- --nocapture` (`1
  passed`), `cargo test -p slug_core bzlmod_ -- --nocapture` (`18 passed`),
  `cargo test -p slug_interpreter_for_build load_ -- --nocapture
  --test-threads=1` (`9 passed`), `cargo test -p slug_analysis
  runtime_aliases_before_globals -- --nocapture` (`4 passed`), `cargo test -p
  slug_bzlmod -- --nocapture` (`322 passed`; doctest `1 passed, 4 ignored`),
  `cargo test -p slug_common bzlmod -- --nocapture` (`10 passed`), `cargo test
  -p slug_external_cells -- --nocapture` (`8 passed`), `cargo check -p
  slug_core -p slug_bzlmod -p slug_interpreter_for_build -p slug_analysis -p
  slug_server`, `cargo build -p slug`, the full Plan 61 Python guardrail with
  `75 passed in 45.34s`, `cargo fmt --check`, and `git diff --check`. The four
  slugd processes left by the full guardrail were cleaned up afterward.
- Native repository-rule label resolution no longer carries the production
  `bazel-external` directory-scan compatibility fallback. The remaining normal
  executor entrypoint requires a `RepositoryLabelResolution` value supplied from
  the bzlmod cell graph, while the test-only marker-shortcut helpers pass an
  explicit empty resolver map. Missing repositories now fail as
  resolver-owned graph misses instead of probing project-root or
  `bazel-external` collision paths. Bridge burn-down note: the production
  surface reduced is `scan_bazel_external_for_repository_executor` plus optional
  label-resolution entry into `execute_repository_rule_impl`; the intended
  owner is `RepositoryExecutionKey` consuming `RepositoryLabelResolution` from
  `BzlmodCellGraphKey`/`BzlmodCellGraphDataKey`. The remaining bridge is that
  the cell graph is still legacy-produced rather than a true DICE-derived
  value. Before/after evidence:
  `rg -n "scan_bazel_external_for_repository_executor|label_resolution: Option<&RepositoryLabelResolution>|execute_repository_rule_impl\\([^\\n]*(None|Some)|Falling back to bazel-external directory scanning for repository executor label|resolve_build_file_label\\([^\\n]*None|Some\\(&label_resolution\\)" app/slug_bzlmod/src/repository_executor.rs`
  now returns no hits. Validation passed with `cargo test -p slug_bzlmod
  resolve_build_file_label -- --nocapture`, `cargo test -p slug_bzlmod
  http_archive_build_file_uses_resolver_owned_label_path -- --nocapture`,
  `cargo check -p slug_bzlmod`, `cargo build -p slug`, `cargo fmt --check`,
  and `git diff --check`.
- Built-in `use_repo_rule()` materializations for Bazel's
  `local_repository` and `new_local_repository` now carry `RepoSpec.local =
  true`, matching Bazel's `tools/build_defs/repo/local.bzl` definitions, and
  local extension repo execution results now mark their DICE value
  non-cacheable instead of relying only on marker miss classification. Bazel
  anchors: `/var/mnt/dev/bazel/tools/build_defs/repo/local.bzl:66-81`,
  `/var/mnt/dev/bazel/tools/build_defs/repo/local.bzl:108-136`, and
  `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/bazel/repository/RepositoryFetchFunction.java:493-497`.
  Validation passed with focused `cargo test -p slug_bzlmod
  local_repos_local -- --nocapture`, focused `cargo test -p slug_bzlmod
  non_cacheable_results -- --nocapture`, and full `cargo test -p slug_bzlmod
  -- --nocapture` (`324 passed`; doctest `1 passed, 4 ignored`), `cargo
  check -p slug_bzlmod -p slug_server`, `cargo build -p slug`, `cargo fmt
  --check`, `git diff --check`, and the focused Plan 61 local-repository marker
  subset (`3 passed, 72 deselected`). No slugd processes remained after
  cleanup.
- Root-local custom `use_repo_rule("//:repo.bzl", "rule")` definitions now
  inspect the Starlark `repository_rule(local = ...)` bit during DICE bzlmod
  resolution before serializing precomputed `RepoSpec`s. The pre-cell-resolver
  path uses DICE-tracked project-file reads for root-local `.bzl` files and
  their root-local loads, then marks local repo specs non-cacheable through the
  existing repository-rule execution path. If that early probe cannot evaluate
  a valid `.bzl` module that needs the normal loader context, it no longer fails
  resolution; normal repository-rule execution still owns the eventual rule
  load. Bazel anchors:
  `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/bazel/repository/RepoRule.java:54-63`,
  `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/bazel/repository/starlark/StarlarkRepositoryModule.java:58-72`,
  and
  `/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/bazel/repository/RepositoryFetchFunction.java:493-497`.
  Validation passed with `cargo check -p slug_bzlmod -p slug_common -p
  slug_interpreter_for_build -p slug_server`, `cargo build -p slug`, and
  focused Plan 61 guardrail
  `test_custom_use_repo_rule_local_definition_reexecutes_after_input_edit` (`1
  passed, 75 deselected`), then the focused local-repository marker subset
  including that guardrail (`4 passed, 72 deselected`). The follow-up focused
  subset including
  `test_custom_use_repo_rule_local_probe_failure_does_not_block_execution`
  covers a root-local repo-rule module with an ordinary build `rule(...)`
  declaration that requires the normal `.bzl` loader context (`5 passed, 72
  deselected`). The normal repository-rule execution path now records the
  loaded rule's `local` bit in the repository materialization sidecar, so a
  valid root-local custom repo rule that the pre-cell-resolver probe cannot
  inspect is still treated as non-cacheable after the real loader runs; the same
  guardrail edits an unwatched input and proves rematerialization. Validation
  for that execution-time refresh passed with `cargo test -p slug_bzlmod
  materialization_manifest_treats_recorded_local_rule_as_non_cacheable --
  --nocapture`, `cargo check -p slug_bzlmod -p slug_interpreter_for_build`,
  `cargo build -p slug`, focused `test_custom_use_repo_rule_local_probe_failure_does_not_block_execution`
  (`1 passed, 76 deselected`), the focused local-repository marker subset (`5
  passed, 72 deselected`), `cargo fmt --check`, and `git diff --check`. No slugd
  processes remained after cleanup. The external-module guardrail
  `test_external_use_repo_rule_local_definition_reexecutes_after_input_edit`
  then proved the same execution-time local sidecar for an
  `@repo_rule_owner//:repo.bzl` custom repo rule (`1 passed, 77 deselected`).
- Repository-rule watched inputs are now captured in a sidecar, and root-file,
  recursive `watch_tree()`, and repo-env reads participate in same-daemon DICE
  invalidation. This is still marker/layout plumbing rather than a final
  DICE-owned repository materialization manifest.
- `repository_ctx.read(..., watch = "auto")` now mirrors Bazel 9's
  `StarlarkBaseExternalContext.readFile` path by recording a FILE input when
  the read path can be watched, while `watch = "no"` remains unrecorded and
  generated-repo working-directory reads remain skipped for `auto`. The new
  guardrail `test_repository_ctx_read_label_auto_watch_reexecutes_materialized_repo`
  first failed because no recorded-input sidecar was written, then passed after
  the fix. Validation also reran the adjacent repository watch/watch-tree
  guardrails (`3 passed` total) after `TMPDIR=/var/mnt/dev/.slug-tmp/plan61-read-watch
  cargo build -p slug`. Clean review of `06bafd5c` found no correctness issues
  and reran `git diff --check HEAD~1..HEAD`, the new guardrail (`1 passed`),
  the adjacent repository watch/read/watch-tree subset (`3 passed`), and
  `cargo check -p slug_interpreter_for_build`.
- `repository_ctx.template(..., watch_template = "auto")` now follows the same
  Bazel recorded-input path as `StarlarkRepositoryContext.createFileFromTemplate`:
  template labels and path objects resolve to filesystem paths, the template
  file is recorded before reading when the watch mode permits it, and generated
  repository working-directory templates remain skipped for `auto`. The new
  guardrail `test_repository_ctx_template_label_auto_watch_reexecutes_materialized_repo`
  first failed because no sidecar was written, then passed after the fix.
  Validation passed with `TMPDIR=/var/mnt/dev/.slug-tmp/plan61-template-watch
  cargo build -p slug`, the new guardrail (`1 passed`), and the adjacent
  repository watch/read/template/watch-tree subset (`4 passed, 144 deselected`).
  Clean review of `1cbd6341` found no correctness issues and reran
  `git diff --check HEAD~1..HEAD`, `cargo check -p slug_interpreter_for_build`,
  `cargo build -p slug`, and the focused four-test subset (`4 passed`).
- `repository_ctx.patch(..., watch_patch = "auto")` now mirrors Bazel's
  `StarlarkRepositoryContext.patch` path by recording the patch file before
  applying it when the patch path can be watched. The new guardrail
  `test_repository_ctx_patch_label_auto_watch_reexecutes_materialized_repo`
  first failed because no sidecar was written for a label patch file, then
  passed after the fix. Validation passed with `TMPDIR=/var/mnt/dev/.slug-tmp/plan61-patch-watch
  cargo build -p slug`, the new guardrail (`1 passed`), and the adjacent
  repository watch/read/template/patch/watch-tree subset (`5 passed, 144
  deselected`). Clean review of `df9b5501` found no correctness issues and
  reran `git diff --check HEAD~1..HEAD`, `cargo check -p
  slug_interpreter_for_build`, and the focused five-test subset (`5 passed,
  145 deselected`).
- `repository_ctx.extract(..., watch_archive = "auto")` now records the archive
  file before extracting it, resolves label and `RepositoryPath` archive inputs
  to filesystem paths, and keeps the DICE watch edge binary-safe by tracking
  source-file metadata/digest rather than UTF-8 file contents. The new guardrail
  `test_repository_ctx_extract_label_auto_watch_reexecutes_materialized_repo`
  first failed because `Label("//:watched.zip")` was treated as a repo-relative
  path, then failed again when single-file watch tracking attempted a UTF-8
  source read of the zip archive, and now passes. Validation passed with
  `TMPDIR=/var/mnt/dev/.slug-tmp/plan61-extract-watch cargo build -p slug`, the
  new extract guardrail (`1 passed`), and the adjacent
  repository watch/read/template/patch/extract/watch-tree subset (`6 passed,
  144 deselected`).
- Archive extraction `rename_files` is now parsed as a string-to-string dict
  and applied before `strip_prefix` for `repository_ctx.extract`,
  `repository_ctx.download_and_extract`, `module_ctx.extract`, and
  `module_ctx.download_and_extract`, matching Bazel's decompressor ordering in
  `StarlarkBaseExternalContext` plus `ZipDecompressor`/`CompressedTarFunction`.
  Focused guardrails cover `repository_ctx.download_and_extract` and
  `module_ctx.extract` with rename-before-strip zip entries and false
  prefix-match entries. Validation passed with
  `TMPDIR=/var/mnt/dev/.slug-tmp/plan61-rename-files cargo check -p
  slug_interpreter_for_build`, `TMPDIR=/var/mnt/dev/.slug-tmp/plan61-rename-files
  cargo build -p slug`, and explicit-binary pytest `-k
  'rename_files_before_strip'` (`2 passed, 152 deselected`).
- `repository_ctx.watch_tree(Label(...))` DICE watch tracking is now binary-safe:
  file leaves in watched trees depend on path metadata/digest instead of the
  UTF-8 source read path. The new guardrail
  `test_repository_ctx_watch_tree_binary_nested_edit_reexecutes_materialized_repo`
  first failed because the watched tree walker attempted
  `read_to_string_if_exists` on a binary leaf, then passed after the fix.
  Validation passed with `TMPDIR=/var/mnt/dev/.slug-tmp/plan61-watch-tree-binary
  cargo build -p slug`, the new binary watch-tree guardrail (`1 passed`), and
  the adjacent repository watch/read/template/patch/extract/watch-tree subset
  (`7 passed, 144 deselected`).
- Repository download cache hits now honor `canonical_id` for
  `repository_ctx.download`, `repository_ctx.download_and_extract`,
  `module_ctx.download`, `module_ctx.download_and_extract`, and native
  `http_archive`/`http_file`/`http_jar` execution. Bazel source anchors:
  `DownloadManager.downloadInExecutor` passes `canonicalId` into repository
  cache `get`/`put` (`/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/bazel/repository/downloader/DownloadManager.java:260-261,355-356`),
  and `DownloadCache.findCacheValue` rejects a hit when the requested
  non-empty canonical id was not associated with that checksum entry
  (`/var/mnt/dev/bazel/src/main/java/com/google/devtools/build/lib/bazel/repository/cache/DownloadCache.java:203-206`).
  The new guardrail
  `test_repository_ctx_download_canonical_id_restricts_cache_hits` primes a
  checksum cache entry with one canonical id, then proves a different canonical
  id cannot reuse it by expecting the second repository download to fail with a
  SHA256 mismatch. Validation passed with `cargo test -p slug_bzlmod
  test_download_canonical_id_restricts_cache_hits -- --nocapture`, `cargo check
  -p slug_interpreter_for_build`, `TMPDIR=/var/mnt/dev/.slug-tmp/plan61-download-canonical
  cargo build -p slug`, and the new Python guardrail (`1 passed`).
- Repository materialization recorded-input sidecars are now split into named
  manifest child keys: `RepoMaterializationRecordedInputsManifestContentKey`
  reads the sidecar content and `RepoMaterializationRecordedInputsValidationKey`
  validates the recorded FILE/DIRENTS/DIRTREE/ENV markers before the parent
  `RepoMaterializationManifestKey` accepts reuse. These keys still poll disk
  until the lower-level watched filesystem API is available in `slug_bzlmod`,
  but the manifest graph now has an auditable child dependency for the sidecar
  content and validation result. Focused validation passed with `cargo test -p
  slug_bzlmod
  materialization_manifest_key_observes_recorded_input_state_dependency --
  --nocapture`, `cargo test -p slug_bzlmod
  test_recorded_input_manifest_changes_materialization_manifest -- --nocapture`,
  and `cargo test -p slug_bzlmod
  materialization_manifest_key_observes_marker_state_dependency -- --nocapture`,
  plus `cargo check -p slug_bzlmod`, `cargo fmt --check`, and `git diff
  --check`.
- Repository materialization marker state is now split into named manifest child
  keys: `RepoMaterializationMarkerContentKey` reads the local-rule marker and
  `.slug_repo_complete` content, while `RepoMaterializationOutputDigestKey`
  computes the output-tree digest only when the complete marker names an
  expected digest. The exposed marker-state strings are unchanged, but the DICE
  graph now has an auditable child dependency for marker content and output
  integrity. A clean review of the previous recorded-input slice caught that
  the test-only direct manifest helper still referenced the deleted
  synchronous recorded-input helper; that direct helper is now restored as
  `#[cfg(test)]` code while the production manifest path remains child-keyed.
  Focused validation passed with `CARGO_TARGET_DIR=/var/mnt/dev/.slug-plan61-marker-target
  cargo test -p slug_bzlmod materialization_manifest_key_observes --
  --nocapture` (`3 passed, 328 filtered out`), `cargo test -p slug_bzlmod
  materialization_manifest -- --nocapture` (`10 passed, 321 filtered out`),
  `cargo test -p slug_bzlmod
  test_archive_repo_manifest_tracks_output_digest_marker_state -- --nocapture`
  (`1 passed, 330 filtered out`), `cargo check -p slug_bzlmod`, `cargo
  fmt --check`, and `git diff --check`.
- Repository materialization local-rule state is now split out of marker
  content into `RepoMaterializationRuleLocalStateKey`. Bridge surface reduced:
  `RepoMaterializationMarkerContentKey` no longer directly probes
  `.slug_repo_rule_local`; the parent marker state depends on a named child key
  for the bit that disables reuse for local repository rules. The intended
  owner remains `RepoMaterializationManifestKey` and, eventually, a watched
  filesystem-backed repository materialization manifest instead of polling
  child keys. Focused validation passed with `cargo test -p slug_bzlmod
  materialization_manifest_key_observes_rule_local_state_dependency --
  --nocapture` and `cargo test -p slug_bzlmod materialization_manifest --
  --nocapture`.
- Repository materialization layout state is now split into named manifest
  child keys too: `RepoMaterializationBuildFilePresenceKey`,
  `RepoMaterializationInvalidEmptyTargetLabelKey`,
  `RepoMaterializationForeignTopLevelSymlinkKey`, and
  `RepoMaterializationInvocationLayoutStateKey`. The parent layout key still
  exposes the same `layout-*` states, but BUILD-file presence, BUILD content
  scans, top-level symlink scans, and rule-specific layout validation are now
  auditable child dependencies instead of one direct helper. Focused validation
  passed with `CARGO_TARGET_DIR=/var/mnt/dev/.slug-plan61-marker-target cargo
  test -p slug_bzlmod materialization_manifest -- --nocapture` (`11 passed,
  321 filtered out`), `cargo check -p slug_bzlmod`, `cargo fmt --check`, and
  `git diff --check`.
- Extension repo file-ops no longer recomputes the repository output digest or
  rewrites `.slug_repo_complete` after `ExtensionRepoExecutionKey` succeeds;
  marker output-state writing is now left to the repository execution key that
  owns the materialization manifest. The no-spec fallback still enters DICE
  execution rather than trusting an existing marker. Focused validation passed
  with `TMPDIR=/var/mnt/dev/.slug-tmp CARGO_TARGET_DIR=/var/mnt/dev/.slug-plan61-external-target
  cargo test -p slug_external_cells complete_marker -- --nocapture` (`2
  passed, 6 filtered out`), `cargo check -p slug_external_cells`, `cargo
  fmt --check`, and `git diff --check`; an initial run without `TMPDIR` failed
  because `/tmp` was full.
- Stale public repository-materialization bypass APIs were removed from the
  `slug_bzlmod` crate root: recorded-input freshness, foreign symlink probing,
  and rule-specific layout probing are now internal manifest/executor helpers
  rather than exported APIs for external callers to consult outside
  `RepoMaterializationManifestKey`. Focused validation passed with
  `TMPDIR=/var/mnt/dev/.slug-tmp CARGO_TARGET_DIR=/var/mnt/dev/.slug-plan61-api-target
  cargo check -p slug_bzlmod`, `TMPDIR=/var/mnt/dev/.slug-tmp
  CARGO_TARGET_DIR=/var/mnt/dev/.slug-plan61-marker-target cargo test -p
  slug_bzlmod materialization_manifest -- --nocapture` (`11 passed, 321
  filtered out`), `cargo fmt --check`, and `git diff --check`.
- The unused repository-rule registry facade is no longer a public
  `slug_bzlmod` API. `RepositoryRegistry` is test-only and the crate-root
  re-export was removed, leaving production repository execution keyed through
  `RepositoryRuleExecutionKey` and `ExtensionRepoExecutionKey` instead of an
  externally usable transitional registry. Focused validation passed with
  `TMPDIR=/var/mnt/dev/.slug-tmp CARGO_TARGET_DIR=/var/mnt/dev/.slug-plan61-api-target
  cargo test -p slug_bzlmod test_repository_registry -- --nocapture` (`1
  passed, 331 filtered out`), `cargo check -p slug_bzlmod`,
  `cargo fmt --check`, and `git diff --check`.
- The thread-local repository-invocation registry facade is also no longer a
  public `slug_bzlmod` API. `RegistryGuard`, `RepositoryInvocationRegistry`,
  and their registry lifecycle helpers are test-only; production keeps the
  repository-rule hook `record_invocation` exported but no longer exposes the
  transitional capture mechanism as a caller-managed API. Focused validation
  passed with `TMPDIR=/var/mnt/dev/.slug-tmp CARGO_TARGET_DIR=/var/mnt/dev/.slug-plan61-api-target
  cargo test -p slug_bzlmod repository_invocations -- --nocapture` (`6
  passed, 326 filtered out`), `cargo check -p slug_bzlmod`,
  `cargo fmt --check`, and `git diff --check`.
- The unmapped best-effort project `.bzl` digest helper is no longer exported
  from `slug_bzlmod` and is test-only. The mapped transitional scanner remains
  available for bootstrap/preseed callers that still need repo mappings, while
  production callers cannot choose the weaker unmapped shortcut outside the
  keyed extension-spoke/replay paths. Focused validation passed with
  `TMPDIR=/var/mnt/dev/.slug-tmp CARGO_TARGET_DIR=/var/mnt/dev/.slug-plan61-bzl-digest-target
  cargo test -p slug_bzlmod project_bzl_digest -- --nocapture` (`6 passed,
  326 filtered out`), `cargo check -p slug_bzlmod`, `cargo fmt --check`, and
  `git diff --check`.
- Two more unused public wrappers were removed from `slug_bzlmod`'s crate
  root: the root-default extension aggregation wrapper and the explicit
  lockfile-path reader wrapper. Production callers still use the policy-aware
  aggregation and lockfile APIs, while the redundant direct wrappers no longer
  advertise transitional bypass surfaces. Focused validation passed with
  `TMPDIR=/var/mnt/dev/.slug-tmp CARGO_TARGET_DIR=/var/mnt/dev/.slug-plan61-wrapper-target
  cargo check -p slug_bzlmod`, `cargo test -p slug_bzlmod
  aggregate_extensions -- --nocapture` (`2 passed, 330 filtered out`), `cargo
  test -p slug_bzlmod lockfile_reader -- --nocapture` (`2 passed, 330
  filtered out`), `cargo test -p slug_bzlmod hidden_lockfile -- --nocapture`
  (`3 passed, 329 filtered out`), `cargo test -p slug_bzlmod
  malformed_lockfile -- --nocapture` (`3 passed, 329 filtered out`),
  `cargo fmt --check`, and `git diff --check`.
- `repo_spec_to_invocation` is no longer exported from the `slug_bzlmod` crate
  root and is now crate-private. Extension repository execution still uses it
  internally when converting a captured `RepoSpec` into the native repository
  executor invocation, but external callers cannot bypass the
  `ExtensionRepoExecutionKey` path by building invocations directly. Focused
  validation passed with `TMPDIR=/var/mnt/dev/.slug-tmp CARGO_TARGET_DIR=/var/mnt/dev/.slug-plan61-api-target
  cargo test -p slug_bzlmod repo_spec_to_invocation -- --nocapture` (`5
  passed, 327 filtered out`), `cargo check -p slug_bzlmod`,
  `cargo fmt --check`, and `git diff --check`.
- Repository execution implementation modules are no longer public crate
  modules. `repository_execution`, `repository_executor`, and
  `repository_invocations` are private behind the crate-root execution-facing
  exports, and the only downstream direct module import was moved to
  `slug_bzlmod::RepositoryInvocation`. The stale fresh-execution helper exposed
  by the now-private executor module is test-only, and unused test-only
  registry methods were removed. Focused validation passed with
  `TMPDIR=/var/mnt/dev/.slug-tmp CARGO_TARGET_DIR=/var/mnt/dev/.slug-plan61-module-visibility-target
  cargo check -p slug_bzlmod`, `cargo test -p slug_bzlmod
  fresh_repository_execution_bypasses_marker_shortcut -- --nocapture` (`1
  passed, 331 filtered out`), `cargo check -p slug_interpreter_for_build`,
  `cargo fmt --check`, and `git diff --check`.
- The disabled direct `RepositoryRuleExecutionKey` is no longer a public
  `slug_bzlmod` crate-root API and is compiled only for the unit tests that
  document the disabled path. Production repository execution remains routed
  through captured repository invocations and `ExtensionRepoExecutionKey`
  materialization instead of exposing a caller-built attrs-hash key. Focused
  validation passed with `TMPDIR=/var/mnt/dev/.slug-tmp
  CARGO_TARGET_DIR=/var/mnt/dev/.slug-plan61-direct-key-target cargo check -p
  slug_bzlmod` and `cargo test -p slug_bzlmod execution_key -- --nocapture`
  (`3 passed, 329 filtered out`).
- The production no-op `record_invocation` export was removed from
  `slug_bzlmod`; only the test registry hook remains. Non-extension
  `repository_rule()` invocation now explicitly does not write to the removed
  transitional hook, while MODULE.bazel repository directives continue to be
  captured through module globals. Focused validation passed with
  `TMPDIR=/var/mnt/dev/.slug-tmp
  CARGO_TARGET_DIR=/var/mnt/dev/.slug-plan61-record-hook-target cargo check -p
  slug_bzlmod -p slug_interpreter_for_build` and `cargo test -p slug_bzlmod
  repository_invocations -- --nocapture` (`6 passed, 326 filtered out`).
- The project-root-only `WorkspaceId::for_project_root` constructor is now
  test-only. Production fallback/session construction names the output base
  explicitly with `WorkspaceId::new(...)`, and cross-crate guardrail tests
  assert default-output-base identities with explicit construction instead of
  reusing the shorthand helper. Focused validation passed with
  `TMPDIR=/var/mnt/dev/.slug-tmp
  CARGO_TARGET_DIR=/var/mnt/dev/.slug-plan61-workspace-id-target cargo check -p
  slug_bzlmod -p slug_common -p slug_external_cells -p slug_analysis`, `cargo
  test -p slug_common cell_graph -- --nocapture` (`3 passed, 101 filtered
  out`), `cargo test -p slug_external_cells
  extension_spoke_lookup_uses_injected_workspace_identity -- --nocapture` (`1
  passed, 7 filtered out`), `cargo test -p slug_analysis toolchain_loading --
  --nocapture` (`2 passed, 39 filtered out`), `cargo fmt --check`, and `git
  diff --check`.
- `extension_execution_dice` is no longer a public crate module. Downstream
  crates continue to use the explicit crate-root APIs that are still required
  for extension execution and the transitional tracked `.bzl` digest bridge,
  while the unused `build_canonical_names` crate-root helper export was
  removed. Focused validation passed with `TMPDIR=/var/mnt/dev/.slug-tmp cargo
  check -p slug_bzlmod -p slug_common -p slug_interpreter_for_build -p
  slug_external_cells`, `cargo test -p slug_bzlmod build_canonical_names --
  --nocapture` (`1 passed, 331 filtered out`), `cargo fmt --check`, and `git
  diff --check`.
- Additional extension/repository implementation modules are private behind
  explicit crate-root exports: `module_extension_executor`, `repo_spec`,
  `spoke_materialization`, and `starlark_repo_rule_executor`. Downstream
  extension-interpreter code now uses `slug_bzlmod::ModuleExtensionMetadata`
  directly, and a scoped search found no remaining downstream references to
  those module paths. Focused validation passed with
  `TMPDIR=/var/mnt/dev/.slug-tmp cargo check -p slug_bzlmod -p slug_common -p
  slug_interpreter_for_build -p slug_external_cells`, `cargo fmt --check`, and
  `git diff --check`.
- Remaining data/helper modules that downstream callers reached directly are
  private behind explicit crate-root exports: `extensions`, `lockfile`,
  `parser`, `types`, and `version`. Downstream callers now use
  `slug_bzlmod::{...}` exports for extension data, recorded-input helpers,
  lockfile parsing/hashing, parse errors, and module-file data types instead of
  module-path shortcuts. Focused validation passed with
  `TMPDIR=/var/mnt/dev/.slug-tmp cargo check -p slug_bzlmod -p slug_common -p
  slug_interpreter_for_build -p slug_external_cells`, `cargo test -p
  slug_bzlmod recorded -- --nocapture` (`24 passed, 308 filtered out`), `cargo
  fmt --check`, `git diff --check`, and a scoped search showing no remaining
  `slug_bzlmod::{lockfile,parser,types,extensions,version}::` downstream
  references.
- The unused private `lockfile::compute_file_hash` helper was removed after the
  lockfile module was hidden behind crate-root helpers. Focused validation
  passed with `TMPDIR=/var/mnt/dev/.slug-tmp cargo check -p slug_bzlmod`,
  `cargo test -p slug_bzlmod recorded -- --nocapture` (`24 passed, 308
  filtered out`), `cargo fmt --check`, and `git diff --check`.
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
  exact lazy generated-repo lookup, dynamic/scoped alias resolution, and lazy
  generated-repo path classification can now use the resolver-local runtime
  snapshot before using process globals.
  `override_repo()` generated-repo aliases are projected into that snapshot as
  dynamic aliases.
  Resolver-local cells promoted from that snapshot are now graph-owned instead
  of root-scoped process-global cache entries. Resolver-local alias resolution
  now uses canonical generated repo names from that snapshot for direct cell
  existence and for same-extension sibling fallback before consulting
  process-global dynamic maps, so internal generated repo names are not made
  root-visible merely by snapshot membership. `CellResolver::get()` now follows
  that same boundary: runtime snapshot extension cells materialize only by
  canonical generated repo name, not by internal generated repo name. Validation
  passed with `TMPDIR=/var/mnt/dev/.slug-tmp/rustc-plan61-cell-resolver
  CARGO_TARGET_DIR=/var/mnt/dev/.slug-tmp/slug-plan61-cell-resolver-target
  cargo test -p slug_core bzlmod_resolver_uses_runtime_snapshot_for_lazy_extension_cell -- --nocapture`,
  `TMPDIR=/var/mnt/dev/.slug-tmp/rustc-plan61-cell-resolver
  CARGO_TARGET_DIR=/var/mnt/dev/.slug-tmp/slug-plan61-cell-resolver-target
  cargo test -p slug_core bzlmod_runtime_snapshot -- --nocapture`, and
  `TMPDIR=/var/mnt/dev/.slug-tmp/rustc-plan61-cell-resolver
  CARGO_TARGET_DIR=/var/mnt/dev/.slug-tmp/slug-plan61-cell-resolver-target
  cargo check -p slug_core`. Build-setting labels that arrive in Bazel canonical
  `@@repo//...` form now normalize to Slug's internal cell name after dynamic
  alias resolution. Extension repository execution constructors, bzlmod
  projection-key constructors, and materialization-manifest helper constructors
  that derive workspace identity from project root are test-only, so production
  execution/materialization code has to pass the explicit workspace identity.
  Generic empty-session construction from only a project root is also test-only;
  no-project interpreter setup uses a named sentinel. The graph is still
  legacy-produced, and alias compatibility plus runtime registration remain
  process-global transitional plumbing, so this does not yet make the runtime
  bzlmod cell graph DICE-owned.
- Bazel-visible analysis repository strings now carry the active
  `CellAliasResolver` into `AnalysisContext` when normal rule analysis prepares
  `ctx`. `ctx.label`, `ctx.workspace_name`, runfiles workspace names,
  `ctx.bin_dir`, `ctx.genfiles_dir`, and the `workspace_root_from_label` helper use
  resolver-owned bzlmod runtime aliases/cells before the legacy
  process-global canonical-name helper; contexts without a resolver snapshot
  keep the old fallback. A follow-up found that `ctx.label` was still built
  before the resolver-owned helper ran; `BazelLabel` construction now accepts
  the same resolver and has focused runtime-alias and runtime-miss coverage.
  Focused validation passed with
  `TMPDIR=/var/mnt/dev/.slug-tmp CARGO_TARGET_DIR=/tmp/slug-plan61-worker-analysis-context-target
  cargo test -p slug_build_api analysis_context_repo_name_ -- --nocapture`
  (`2 passed`) and `TMPDIR=/var/mnt/dev/.slug-tmp
  CARGO_TARGET_DIR=/tmp/slug-plan61-worker-analysis-context-target cargo check
  -p slug_build_api -p slug_analysis -p slug_action_impl -p slug_anon_target`.
  Follow-up validation passed with `TMPDIR=/var/mnt/dev/.slug-tmp/rustc-plan61-analysis-context
  CARGO_TARGET_DIR=/var/mnt/dev/.slug-tmp/slug-plan61-analysis-context-target
  cargo test -p slug_build_api runtime_alias_snapshot -- --nocapture` and
  `TMPDIR=/var/mnt/dev/.slug-tmp/rustc-plan61-analysis-context
  CARGO_TARGET_DIR=/var/mnt/dev/.slug-tmp/slug-plan61-analysis-context-target
  cargo test -p slug_build_api runtime_miss_is_authoritative -- --nocapture`.
  Clean review of `aecd85f8..c257fab9` found no issues and reran focused
  `slug_build_api` analysis-context/configured-label tests in an isolated
  target dir (`2 passed` and `3 passed`).
  The requested `/tmp/slug-plan61-worker-analysis-context-target` target path
  was a symlink to `/var/mnt/dev/slug-plan61-worker-analysis-context-target`
  because `/tmp` was full before validation.
- Starlark `Label` values produced from configured provider labels can now
  carry the same active `CellAliasResolver` used by analysis. This covers
  `ctx.label`, dependency labels, query-result dependencies, and direct
  configured-label attrs for Bazel-visible `workspace_name`, `repo_name`, and
  `workspace_root` strings while preserving the old resolverless fallback for
  dynamic/aspect/legacy callers. Focused validation passed with
  `TMPDIR=/var/mnt/dev/.slug-tmp/rustc-plan61-configured-label
  CARGO_TARGET_DIR=/var/mnt/dev/.slug-tmp/slug-plan61-configured-label-target
  cargo test -p slug_interpreter configured_label_ -- --nocapture`
  (`3 passed`) and `TMPDIR=/var/mnt/dev/.slug-tmp/rustc-plan61-configured-label
  CARGO_TARGET_DIR=/var/mnt/dev/.slug-tmp/slug-plan61-configured-label-target
  cargo check -p slug_interpreter -p slug_build_api -p slug_analysis`.
  Clean review of `834f7910` found follow-up gaps where public/derived label
  paths still dropped the resolver (`Dependency.label`, `SourceFileTarget.label`,
  `Label.relative`, `Label.same_package_label`, and `Dependency.sub_target`).
  Those paths now preserve the resolver, and `attrs.source()` source-file
  targets receive the active analysis resolver. Follow-up validation passed with
  `TMPDIR=/var/mnt/dev/.slug-tmp/rustc-plan61-followup cargo test -p
  slug_interpreter configured_label_ -- --nocapture` (`4 passed`),
  `TMPDIR=/var/mnt/dev/.slug-tmp/rustc-plan61-followup cargo test -p
  slug_build_api rule_defs::provider::dependency::tests -- --nocapture`
  (`3 passed`), `TMPDIR=/var/mnt/dev/.slug-tmp/rustc-plan61-followup cargo check
  -p slug_interpreter -p slug_build_api -p slug_analysis`, `cargo fmt --check`,
  and `git diff --check`. Clean review of `8ea30cce..a0cd7c58` found no issues
  and reran `git diff --check 8ea30cce..a0cd7c58`, the focused
  `slug_build_api` dependency tests (`3 passed`), the focused
  `slug_interpreter` configured-label tests (`4 passed`), and
  `cargo fmt --check`. Binary-build validation then caught remaining
  resolverless `RuleAnalysisAttrResolutionContext` initializers in anon target
  and BXL inspection paths; those legacy callers now pass `None` explicitly.
  Follow-up validation passed with `TMPDIR=/var/mnt/dev/.slug-tmp/rustc-plan61-smoke
  cargo build -p slug`, `cargo fmt --check`, and `git diff --check`.
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
   - Ensure graph identity includes every command policy value that Bazel uses:
     lockfile mode, repo env, nonstrict repo env, registry config, network
     policy, yanked-version allow-list, compatibility policy, and extension
     isolation.
   - Module-version consumers and current-workspace helpers still read the
     named cell graph where they need root or graph facts, and the persisted
     config-load key carries the daemon output base, including
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
     the server updater. `BzlmodProjectionData` remains as a transitional
     bridge payload assembled by the legacy resolver, but lockfile inputs,
     repo-env, resolution facts, repo mappings, registered toolchains,
     registered execution platforms, extension aggregations, and module
     versions have been split out to separate named injections; delete or rename
     the remaining graph-shaped projection API only after module graph and
     cell-graph facts have true DICE producers.
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
