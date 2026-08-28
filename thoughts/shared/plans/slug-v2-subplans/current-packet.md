# Current Slug V2 Packet

Packet: `WP-4-5-7A-target-platform-and-exec-configuration-prerequisite-r7`

Milestone: M7A command/ruleset bootstrap closure feeding ordinary M8 Stage
10.3 analysis.

Base: `c2ec8481e`.

Result: retain the independently reviewed R4 target-platform candidate and R5
graph-only Bzlmod mapping architecture; derive root and non-root module-
extension repository mappings once from their complete graph-level inputs,
share that projection with ordinary extension ownership, admit already-typed
aliases through the generic external-package gate, and prove the default host
platform end to end. Do not implement toolchain selection.

R5 design commit `c2ec8481e` and independent Sol review: `ACCEPT`. R6
implementation review: `REPLAN`. R7 design commit `dfb56b9b5` and independent
Sol review: `ACCEPT`.

## R4, R5 and design replan record

Independent terminal review returned `REPLAN`. The R4 candidate embeds exact
Bazel 9.2 `tools/BUILD` and `tools/build_defs.bzl`, but
`HostCanonicalRepositoryRoute::builtin()` publishes an empty repository
mapping. `tools/build_defs.bzl` eagerly loads
`@platforms//host:constraints.bzl`, so Slug cannot evaluate the builtin package
far enough to publish `host_platform -> @@platforms//host:host`. Focused tests
proved the fallback option label, local alias normalization and exact source
bytes separately; they did not prove their required composition. The failed
Slug smoke confirmed the missing mapping boundary. The pinned Bazel oracle
proves Bazel behavior, not Slug realization.

R4 stopped on any further material correction, so none of its Rust is accepted
under that packet. R5 explicitly retains the validated candidate under this
corrected owner/proof contract; do not silently narrow the exact claim.

The first design proposed projecting the builtin mapping from
`HostSelectedModuleRoutesKey`. Independent pre-review returned `REPLAN` because
that owner also awaits every selected registry RepoSpec and its source
metadata. A mapping-only request would inherit unrelated materialization
Needs/errors and an overbroad observed frontier. R2 instead extracts the
mapping computation directly over the selected module graph, matching Bazel's
`BazelDepGraphValue` dependency boundary.

R5 implementation stopped at its mandated exact `tools/BUILD` composition
proof. The selected dependency portion correctly published `platforms` without
RepoSpec/source metadata, but the graph-only owner omitted non-root MODULE
`use_repo` imports already present in the selected graph, so the exact BUILD
could not resolve `@buildozer_binary`. The generic external package gate then
rejected the now-typed `alias` target even though loading and configured
analysis represent it. R6 attempted to correct those two category-wide
boundaries without a dependency-specific mapping or a C++ rule path.

Independent R6 review returned `REPLAN`. Root extension usages are retained by
`RootModuleFiles`, not `HostSelectedModuleGraph`. The provisional candidate
therefore projected non-root usages once with an empty root usage set and the
existing extension owner would later project the combined root/non-root set a
second time. Their shared collision namespace can choose different suffixes,
which would change canonical generated repositories and invalidate already-
published mappings. R7 removes that shortcut and freezes one complete
projection consumed by both mapping and extension owners. The review also
found a predecessor-order regression: semantic mapping failure must not outrank
an outstanding RepoSpec Need that the former selected-route owner completed
first. R7 preserves that order explicitly.

## Learned facts and authority

Pinned Bazel 9.2 commit
`8220c6198837d5c13d53fea211cf3282aa12408a` is behavior authority:

- `RepositoryMappingFunction#computeForBazelModuleRepo` publishes a module
  repository's full mapping, including module-extension imports, from the
  selected dependency graph;
- `BzlLoadFunction#getRepositoryMapping` consumes that full mapping for normal
  `@bazel_tools` `.bzl` and BUILD evaluation after bootstrap;
- `RepositoryMappingFunctionTest` covers selected dependency mappings; and
- the existing `toolchain-resolution-first-platform` oracle records
  `@bazel_tools//tools:host_platform -> @@platforms//host:host`.

Slug already computes the required semantic fact inside
`selected_routes_with_canonicals`: the selected module graph gives
`bazel_tools` its exact special canonical name and complete
apparent-to-canonical mapping, including `platforms -> platforms`. That
calculation is currently fused with selected registry RepoSpec consumption in
`HostSelectedModuleRoutesKey`. The selected-definition API deliberately returns
`BuiltinDeferred`, while the loading-owned canonical-route key bypasses the
mapping and constructs a static builtin route with an empty mapping. The fused
owner and shortcut are the defects.

Clean `../zabel` commit `0795445f3ab60f4e49070bdd0b94425c5610f73a`
is concept/test guidance only: Bzlmod workspace/repository-view producers own
selected mappings, and generic package/`.bzl` evaluators consume immutable
mapping contexts without selecting or inferring them. No Zig type, scheduler,
identity or behavior claim is imported. Buck2-derived `SmallMap`, immutable
`Arc` carriers, `Dupe` and `Allocative` remain the preferred compact substrate.

## Decision

Extract one graph-only Bzlmod mapping owner, one graph-level root-extension-
usage projection, and one shared complete extension-mapping projection; then
add one public, hidden builtin projection family:

- `HostRootExtensionUsagesKey(workspace)` consumes the existing
  `RootModuleFilesKey` and retains only its immutable ordered
  `RootExtensionUsage` slice. Its observed sibling forwards exactly the root-
  module-files frontier. It does not retain the rest of `RootModuleFiles`;
- `HostSelectedRepositoryMappingsKey(workspace)` consumes the selected module
  graph and root-extension-usage projection. It owns canonical-name derivation,
  selected-dependency mappings, and exactly one invocation of the existing
  extension namespace/import/override semantics over the complete ordered root
  plus non-root usage set. It retains one internal compact projection containing
  usage/override/base/final mapping facts needed by the existing extension
  owner. It does not evaluate extensions or request registry RepoSpecs, source
  metadata, repository materialization, root visibility or packages;
- its observed sibling consumes the observed selected graph and observed root-
  usage projection and merges exactly those two frontiers;
- `HostSelectedModuleRoutesKey` consumes that mapping owner plus its existing
  RepoSpec owner, joins registry RepoSpecs to the already-selected mapping
  rows, and preserves its current result, errors, order and observations. It
  requests and finishes RepoSpecs before publishing a completed mapping-
  semantic error, so a competing RepoSpec Need retains predecessor precedence;
- `HostSelectedExtensionMappingsKey` consumes selected routes plus the same
  retained complete extension projection. It attaches the route predecessor
  and publishes the existing usage/override/base/final fields without assigning
  names or applying mappings again;
- `HostBuiltinBazelToolsRepositoryMappingKey(workspace)` consumes only the
  graph-only mapping owner and selects the unique `bazel_tools` row;
- its observed sibling consumes the observed graph-only mapping key and
  forwards only that mapping frontier;
- the retained value clones only the existing compact mapping owner
  (`context_repo`, ordered apparent names and shared entry map), not the full
  selected graph, evaluator values or source bytes; and
- missing, duplicate, wrong-context or non-builtin routes fail closed with a
  typed semantic error. Need, outer error and cancellation publish nothing.

The loading-owned canonical-route key must consume that projection for the
`bazel_tools` canonical identity in both legacy and observed modes. The builtin
route retains its pinned snapshot identity plus the selected mapping. Its
existing `mapping_target()` and `bzl_repository_mapping()` methods then serve
generic label resolution and evaluator construction exactly like other route
families. Route equality and hashing include the mapping; no consumer rereads
MODULE, registry, lockfile or source state.

The canonical external-package loaded-target gate must admit `alias` and
`config_setting`, which are already typed loading values with generic
configured consumers, just as it already admits filegroups and typed native
platform/toolchain declarations. This is a representation-gate correction,
not rule-specific behavior; unsupported target kinds remain fail-closed.

This is the full builtin module mapping, not a hard-coded `platforms` pair.
That keeps `tools/build_defs.bzl`, later autoloaded rule modules and all other
BCR dependencies on one architecture and prevents one dependency-specific
repair from becoming permanent.

The complete root/non-root usage ordering and collision namespace are semantic
inputs to one producer, never two staged mutation passes. Ordinary selected
routes and the builtin projection consume its final mappings; extension
evaluation consumes its retained definition facts. No consumer is permitted to
rerun namespace allocation.

## Non-decisions and compatibility

- **Exact:** selected full repository mapping for builtin `bazel_tools` on the
  admitted Bzlmod graph, including graph-declared `use_repo` imports; generic
  eager `@platforms` load resolution; exact
  `host_platform -> @@platforms//host:host` alias realization.
- **Slug-native:** Rust/DICE keys, compact retained carriers, observed-frontier
  error wording and structural route identity.
- **Unsupported/deferred:** live network BCR materialization in direct CLI
  query/cquery, missing builtin packages outside the catalog, command platform
  flags, platform mappings, toolchain selection, providers and implementation
  analysis.

Do not change builtin source bytes, infer mappings from source labels, inject a
root mapping, special-case `platforms` in loading/evaluation, widen selected
registry materialization, or turn `cc_common`/`cc_internal` into Rust rule
control flow. Buck2-derived Rust remains the generic Starlark evaluator and
compact utility substrate. BCR Starlark owns every rule definition and control
path including `cc_internal`; `cc_common` is only a demanding generic host-ABI
consumer. Zabel is peer guidance, never authority.

## Request, revision and lifetime

The key identity is the immutable workspace path. The selected module graph
owns root and builtin MODULE inputs, version selection, canonical identities,
dependency edges and their observed path frontier. RepoSpec/source metadata is
not a dependency of the mapping owner. Concurrent requests deduplicate through
DICE. A mapping-changing revision invalidates the builtin projection and
route; equal selected mappings cut off downstream recomputation. A/B/A, cold
cancellation and same-graph repair must work without manual locks or
process-global state.

The mapping is DICE-retained semantic state. Ordered names and entries retain
their existing compact shared allocations; route consumers borrow or clone
only immutable `Arc` state. Lookup scratch is compute-local. No evaluator heap,
command scratch, cache, interner, task or filesystem handle is retained. No
lock may cross a DICE computation.

## Corrected successor proof contract

The implementation packet must prove the composition, not its parts:

1. Graph-only mapping tests publish complete ordered mappings for root,
   registry, nonregistry and builtin rows, including builtin self and
   `platforms` plus graph-declared `use_repo` imports from both root and non-root
   modules; force a root/non-root namespace collision and prove both the builtin
   and existing extension owners retain the same canonical generated names;
   reject missing, duplicate and wrong-context rows; and prove
   missing/erroring registry RepoSpec/source metadata cannot block mapping
   publication or enter its observed frontier.
2. Legacy and observed mapping owners merge selected-graph and root-module-file
   frontiers exactly, preserve A/B/A, cancellation and same-graph repair, and
   retain no full root-files/graph predecessor at the public builtin boundary.
   A mapping-semantic error competing with a RepoSpec Need must publish the Need
   first, matching the predecessor route owner.
3. Legacy and observed canonical routes retain that exact mapping, mapping
   changes affect structural identity, equal mappings reuse results, and
   Need/outer/cancellation/lifecycle behavior is preserved.
4. One Slug loading proof evaluates exact builtin `tools/BUILD` and its eager
   `tools/build_defs.bzl` load using a provenance-pinned selected `platforms`
   source, then observes alias actual `@@platforms//host:host`.
5. One configured-analysis proof starts from the default structural
   `host_platform` option and reaches the same actual configured platform
   through the generic route/package/alias/platform keys.
6. Existing selected, generated and root mapping suites remain unchanged;
   exact builtin hashes/modes and all R4 platform/cycle/action proofs remain.

Retained R4 proofs include visible/non-visible option projection; first
platform/fallback and Target/Exec shape checks; per-platform exec identity and
equal-input `Arc` reuse; direct/multi-hop aliases for platform/value/setting;
wrong kinds, cycles, defaults and duplicate settings; Target/Exec platform
facts and A/B/A/cancellation repair; constraint match/no-match/extra-setting
and competing outer/Need/semantic errors; exact `.bzl` detector diagnostics;
and locked scans for one alias recursion, one condition owner, no package read
in platform consumers, no retained standard collection/cache/interner, no
toolchain/provider/ruleset specialization and no lock across DICE.

Reuse the existing Bazel oracle and the provenance-pinned local `platforms`
module fixture; add no oracle fixture unless the implementation audit proves a
real evidence gap. The local fixture authenticates Slug composition but does
not support a claim of live network BCR materialization.

## Retained R4 implementation contract

The retained candidate must remain exactly within these owners:

1. Project visible `ResolvedOptionLabel` values to canonical labels without
   stringification; select the first native `platforms` entry or
   `host_platform` fallback; and derive exec configurations by installing the
   selected actual platform in the existing native row before the existing
   Starlark exec projection.
2. Keep the two pinned upstream builtin files byte-for-byte and `100644`, with
   catalog/listing/snapshot identity updated. Retain
   `constraint_setting.default_constraint_value` only to reject defaults at
   this admitted slice.
3. Publish terminal `actual_configured_target` identity from the existing
   configured alias recursion; direct nodes publish self, aliases preserve the
   requested node and `AliasActual` edge, and null nodes publish none. Admit
   direct native toolchain declarations only as provider-empty terminal nodes.
4. Analyze native platform, constraint value and setting nodes in Target and
   Exec configurations. Resolve value/setting aliases through terminal actual
   identity, preserve original ordered edges, reject wrong kinds, defaults and
   duplicate actual settings.
5. `ConfiguredPlatformKey` consumes only configured nodes/edges and publishes
   requested/actual keys, the existing platform fact and an immutable actual
   value/setting constraint slice. `ConfiguredTargetPlatformKey` consumes only
   structural configuration selection plus that platform key.
6. The sole `ConfiguredConditionKey` matches every requested actual constraint
   value by actual setting/value identity on the target platform while
   preserving native/define/flag behavior and outer-before-Need-before-semantic
   precedence.
7. The request-scoped configured-analysis cycle detector guards only the
   existing alias-child await and composes losslessly with the `.bzl` detector.
   Preserve concrete `.bzl` guards/downcasts, finish events, cancellation and
   same-graph recovery; retain no cycle state in DICE values and hold no lock
   across a computation.

Exact behavior is the admitted target-platform/constraint/alias surface above.
Rust structural configuration bytes, DICE layout and diagnostic wording are
Slug-native. Command platform flags/mappings/default-constraint semantics,
converged registered execution-platform aliases, toolchain selection,
providers and implementation analysis remain unsupported/deferred.

## Exact allowlist and caps

Every retained R4 baseline remains `cf91fe8de`; `ce38f0373`, `959cbd889` and
`c2ec8481e` changed plans only.

| Path | Baseline blob / lines | Maximum physical growth |
|---|---:|---:|
| `app/slug_identity_v2/src/label.rs` | `081bbb5b49238d361a83c437dbebd29b543334f4` / 537 | +30 |
| `app/slug_configuration_v2/src/native/configuration.rs` | `12b7e78d753633a42f0a5fc1ebdb4be0fdfe2536` / 1,540 | +90 |
| `app/slug_configuration_v2/src/native/tests.rs` | `4f9b01a779a6ebd5518c46728954348512987c8c` / 3,529 | +90 |
| `app/slug_bzlmod_v2/src/builtin_repository.rs` | `28819e3b37b6be21f1d855bbf68d9de6a37f4d44` / 889 | +20 |
| `app/slug_bzlmod_v2/src/host_module.rs` | `28c78c310ab6804da7824829efcc2c06f9d5bca8` / 5,349 | +4 |
| `app/slug_bzlmod_v2/tests/builtin_bazel_tools.rs` | `3002f00320df7540b4c4905610f11e42534b4f7b` / 149 | +35 |
| `app/slug_loading_v2/src/package.rs` | `bfc62b265d336a57a612e2f50def2ce3da587a2e` / 6,852 | +50 |
| `app/slug_loading_v2/tests/build_file_loading.rs` | `fa35fbbedc839f49b701ffc98810554349d28629` / 3,559 | +55 |
| `app/slug_loading_v2/src/external_subtree_package_set_tests.rs` | `d8e7477ae4f33c13e83c7edbadceaa85d6d0cbed` / 838 | +4; replace two rows with the graph-backed builtin route fixture |
| `app/slug_analysis_v2/src/dice.rs` | `08711874e49e37b297b8a7eb989ba7a1c60d70e1` / 3,748 | +340 |
| `app/slug_analysis_v2/src/result.rs` | `2d5fb57083c522ea5229610e1c033371065ad790` / 668 | +100 |
| `app/slug_analysis_v2/src/lib.rs` | `777f01622c2051a3b54c2a697173e136072ac792` / 77 | +15 |
| `app/slug_analysis_v2/tests/starlark_rule.rs` | `5fba7dd923011f724073ac8b6674b1ce4d283db9` / 6,304 | +450 |
| `app/slug_analysis_v2/tests/root_analysis.rs` | `b2fd28f8fda584b50ec597eb21018a24461b8167` / 1,123 | +100 |
| `app/slug_core_v2/src/runtime/dice.rs` | `e0bf2cb329b63089ca51c039e82881c3188c8655` / 12,008 | +20 |
| `app/slug_core_v2/src/runtime/tests/build_command_tests.rs` | `fd3f417977f417a0098decd36c34097d1d50d391` / 4,056 | +0; replace one token |
| `app/slug_core_v2/src/runtime/tests/query_command_tests.rs` | `2017bbf52cd19967a8450450bc55a08603de4ecf` / 860 | +0; replace four visibility diagnostics and one named-repository error reached after generic alias/config-setting admission |
| `app/slug_analysis_v2/Cargo.toml` | `36cd3ffd8e681d998d6f1bcd47f493e2496484e6` / 31 | +0; move Tokio row |

R7 uses these `c2ec8481e` mapping-owner baselines:

| Path | Baseline blob / lines | Maximum physical growth |
|---|---:|---:|
| `app/slug_bzlmod_v2/src/selected_repo_spec.rs` | `286d9e1042f76fef1ca6f50c8c6df92c516f4352` / 14,538 | +850 |
| `app/slug_bzlmod_v2/src/selected_repo_spec/selected_extension_demand.rs` | `ece6a03a72a120dc531fcf1d2ce5df1f02d4c7b0` / 1,195 | +4; initialize the retained projection in test-only constructors |
| `app/slug_bzlmod_v2/src/host_external_package_boundary/tests.rs` | `4db50c6d185b3238c67fda5f3dbc818e4504efc4` / 501 | +4; initialize the builtin mapping in test-only constructors |
| `app/slug_bzlmod_v2/src/lib.rs` | `279b4d8d98a8c534eca9a7112a57788e2c3f8326` / 539 | +20 |
| `app/slug_bzlmod_v2/src/canonical_repository_route.rs` | `9aa6bc6ad89b754c23e5d0897a15011a07d3ffcd` / 415 | +80 |
| `app/slug_loading_v2/src/canonical_repository_route.rs` | `86cd5e194fe4ce37fe5677a2ef7190472a081c68` / 326 | +160 |
| `app/slug_loading_v2/src/canonical_repository_route_tests.rs` | `90c8c212ac33dfd6755fb907054d4bd413916b64` / 3,047 | +250 |
| `app/slug_loading_v2/src/canonical_repository_load_route_tests.rs` | `81a0d1ef364f40ecdb9da5c5a150361c5fe876a0` / 2,668 | +250 |
| `app/slug_loading_v2/src/bzl_module.rs` | `f8f8182b2e3e62c834120fc610b0d186c93e16ef` / 10,576 | +20 |
| `app/slug_loading_v2/src/host_package_load_tests.rs` | `7ef16a76ee16aa9b8156e859646e86f9a66f6eac` / 35,108 | +0; replace three stale alias-rejection fixtures with unsupported generic target kinds |

The only new non-plan files remain the exact 50-line builtin `tools/BUILD`,
the exact 106-line `tools/build_defs.bzl`, and the at-most-350-line configured
cycle detector. R4 caps remain 950 production, 700 proof and 1,750 total added
Rust lines. Mapping-owner and loaded-target-gate additions have separate caps
of 1,150 production, 700 proof and 1,850 total; combined R7 caps are
2,100/1,400/3,600. The large
`selected_repo_spec.rs` owner remains cohesive because R5 extracts an existing
mapping calculation beside its selected-graph/route/extension keys, and R7
shares their existing private extension projection without exposing graph
internals across a crate boundary.

No files beyond both tables, the three named new files and writable plans may
change. No new fixture, lockfile, sync script, CLI/core production,
registry-materialization or Cargo change beyond the retained Tokio move is
allowed.

## Validation and stops

The implementation closes only after independent Sol review returns `ACCEPT`.
Run focused identity/configuration, Bzlmod route, loading and analysis suites;
full affected crate suites serially; direct dependents; rustfmt;
`git diff --check`; source/hash/mode and cap audits; packet/canonical matching;
and `scripts/v2_archive_status.sh`. Rebuild `slug_cli_v2` before any smoke and
report unsupported CLI materialization honestly.

STOP and `REPLAN` for a selected-route/package-load DICE cycle; any RepoSpec,
source-metadata or materialization dependency in the mapping owner; a mapping
derived from source spelling or root visibility; `platforms`-only injection;
staged or repeated root/non-root namespace assignment; a mapping-semantic error
outranking the predecessor RepoSpec Need; copied full graph/root-files/source/
evaluator state at the builtin boundary; lost observations; changed
root/selected/generated mapping semantics; an unguarded lock across DICE;
live-registry or catalog expansion; Rust BCR/ruleset control flow;
`cc_common` specialization; Zabel authority; cap breach; or any further
material implementation correction.
