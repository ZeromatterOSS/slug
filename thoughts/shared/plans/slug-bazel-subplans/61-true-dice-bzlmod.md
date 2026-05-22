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
  to project-local literal loads. Focused Rust coverage and the Plan 61 Python
  replay guardrails pass for this transitional digest behavior.
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
- The legacy bzlmod repo-env process global was removed. The server now threads
  the effective command repo environment through config overrides as
  `bzlmod.repo_env_json`, and bzlmod resolution/replay digests consume that
  explicit option. Focused `slug_common` coverage verifies repo-env option
  parsing, and the full 50-test Plan 61 guardrail target passed after the
  change.

## Consolidated Learnings

What worked:

- Root `MODULE.bazel` and visible/hidden lockfile reads were moved behind DICE
  keys as a bridge. Those keys are intentionally non-cacheable until file reads
  are backed by tracked DICE filesystem inputs.
- Extension evaluation and extension repository execution now run through DICE
  keys instead of immediate startup-side materialization.
- Lockfile replay reads, facts validation, registry checksum policy,
  yanked-version policy, include invalidation, and local override input
  invalidation gained focused guardrails.
- Repository materialization no longer blindly trusts stale marker files. The
  marker path distinguishes known repo-spec/output-state cases and rejects old
  incomplete layouts.
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
- The resolved graph, repo mappings, cell graph, and registered toolchain and
  execution platform facts are still assembled during legacy cell setup, then
  injected as `BzlmodSessionData`.
- Hidden lockfile identity is included in the transitional bridge key equality
  and hashing path, and hidden replay has same-daemon edit coverage. Broader
  hidden lockfile replay/fail-open behavior now has stronger guardrails, but
  lockfile replay still depends on the transitional graph and is not complete.
- Extension `.bzl` transitive digests are still best-effort. Project-local
  literal loads and existing external files under `bazel-external/<repo>` are
  hashed, but repo mappings at load sites, load failures, deleted files, and
  the full interpreter load graph are not replay-complete.
- Extension spoke registration and seeded-extension tracking still use
  process-global state. The command repo-env global has been removed, but the
  remaining spoke/seed reset hooks do not provide a replay-pure dependency
  model.
- Some Bazel 9 semantics are parsed but not fully modeled, including
  `bazel_dep(max_compatibility_level)`, registry selection on overrides, and
  remaining command policy around non-root dev dependencies.
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
   - Prove warm reuse by DICE cutoffs, not by a process-global bridge cache.
     The process-global fast path is removed, but the transitional key still
     wraps the legacy resolver.

2. Make root, included, local override, registry, git, and archive module files
   true DICE inputs.
   - Replace direct `std::fs` validity hacks with tracked filesystem
     dependencies or equivalent DICE input nodes.
   - Include create/delete transitions, parse failures, include cycles, and
     UTF-8 failures.
   - Model registry selection and source metadata for overrides.

3. Make lockfile replay complete.
   - Maintain visible and hidden lockfile identity in every key that can
     consume their contents.
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
   - Keep the current external `bazel-external/<repo>` digest coverage while
     adding repo mappings at load sites, file digest changes from the actual
     loader graph, load failures, and deleted files.
   - Reject replay when any loaded implementation file changes, not only
     literal loads that the transitional scanner can find.

5. Move extension spoke and generated repo registration out of process globals.
   - Represent generated repo specs, sibling spokes, seeded cells, and
     materialization state as DICE values.
   - Remove `SPOKE_REGISTRY` / `SEEDED_EXTENSIONS` as semantic state. Temporary
     instrumentation may remain only if guardrails prove it cannot affect
     correctness.
   - Ensure two workspaces and two command policies cannot share generated repo
     state by accident.

6. Complete Bazel 9 directive semantics.
   - Implement or explicitly Bazel-ground the behavior for
     `max_compatibility_level`, remaining `dev_dependency` surfaces,
     `single_version_override(registry/patches)`,
     `multiple_version_override(registry)`, `archive_override`, `git_override`,
     `override_repo`, `inject_repo`, `isolate`, and `bazel_compatibility`.
   - Preserve root `bazel_dep(dev_dependency=True)` default inclusion and
     `--ignore_dev_dependency` exclusion while moving command policy out of the
     transitional resolver.
   - Add negative tests where Bazel 9 fails.

7. Make repository execution replay-correct.
   - Track `repository_ctx.watch`, `watch_tree`, environment reads, repo mapping
     reads, label paths, downloads, archive/git source identity, patches,
     overlays, and generated files.
   - Replace marker-file trust with a manifest value that proves the current
     repo spec and observed output tree are compatible.
   - Ensure local repository rules are non-cacheable where Bazel does not reuse
     cached local repository contents.

8. Make the bzlmod cell graph a DICE value.
   - Derive module cells, extension-generated cells, aliases, scoped mappings,
     external symlinks, and bundled repos from DICE values.
   - Ensure cell graph changes invalidate analysis and package loading
     correctly in the same daemon.
   - Prove apparent aliases do not leak across module scopes.

9. Delete transitional APIs.
   - Remove `BzlmodSessionData` fields as the authority for graph semantics.
   - Legacy command repo-env global state is removed; keep command repo-env
     threaded through explicit DICE/key inputs as the remaining graph migrates.
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
