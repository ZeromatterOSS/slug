# Current Slug V2 Packet

Packet: `WP-4-6-7A-transitive-runfiles-package-mapping-owner-design-r3`

Milestone: M7A generic Starlark/ruleset closure; Stage 4 configured analysis
and Stage 6 runfiles-support prerequisites.

Status: third corrected design draft; zero Rust. Base `f346c209a`. The first
support-action draft was rejected because Bazel 9 Bzlmod always registers a
preceding repository-mapping manifest. The first prerequisite draft was then
rejected because it did not route the complete selected root mapping and
assumed that every published or semantic configured dependency was still
available at node finalization. R2 then over-included resolution-only requested
toolchain/candidate-platform inputs and flattened selector packages into the
parent. R3 limits rows to Bazel package-contributing prerequisites and makes
each selector condition one configured child. Do not begin Rust before
independent `ACCEPT`.

The unrelated dirty
`app/slug_loading_v2/src/registration_expansion_tests.rs` proof remains parked;
do not edit or stage it.

## Result and stop boundary

Design one Bazel-shaped transitive package collector for the admitted
aspect-free configured graph. Every completed configured result retains one
typed dense package/repository-mapping closure. The collector starts with the
current loaded package, records any additional package metadata read directly
for semantic analysis, and records the complete closures of configured child
results. Direct packages and configured children remain separate until one
deterministic final composition.

Root package loading must obtain its complete selected repository mapping from
the existing `HostRootRepositoryMappingKey` or observation sibling before
`PackageRecorder` finishes. External packages continue to use their already-
selected repository-source route. A mapping-only transition therefore changes
`LoadedPackage`, invalidates configured analysis, and changes the retained
package closure without a BUILD-byte change.

This packet creates no Artifact or action and leaves every executable
`FilesToRunProvider` incomplete. After the owner is accepted and implemented,
the support successor registers four actions in Bazel order:

1. `RepoMappingManifest`;
2. `SourceSymlinkManifest`;
3. `SymlinkTree`; and
4. `RunfilesTree`.

Spawn expansion remains the following successor. The packet is generic graph
and provider infrastructure. It adds no `cc_common`, `cc_internal`, rules_cc,
parser, evaluator builtin, C++ rule, or BCR special case. Bazel 9 rule bodies
remain BCR Starlark and Buck2-derived starlark-rust remains the sole
parser/evaluator/`set` owner.

## Authority and authenticated evidence

Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a` is the sole
semantic authority. Pinned sources are:

- `Package.java` SHA-256
  `23a35af653e4e6d807f8b5e94f7085ccc0109e0d04316c7a45a66c631096638c`;
- `TransitiveDependencyState.java` SHA-256
  `9321dc738cc8346570af3495a1d6efa99997c7798934d84eb2b1c0809717f8fd`;
- `ConfiguredTargetFunction.java` SHA-256
  `8c05a1be0f3049811c957667544ad7d52b03350b1a0226039a2d5c781e668682`;
- `DependencyResolver.java` SHA-256
  `60f470668b8b2828c3253cc4e4f45bcf12514d7e1635e288cb7668a824ef86f5`;
- `DependencyResolutionHelpers.java` SHA-256
  `9736c25c376501594d9a123132ae25e3d8fe842cde3e33cb50eb39a62897aa97`;
- `TargetProducer.java` SHA-256
  `cb3d2851c64b978ecff9fb602351e31cfc3843897e3f56dd04c5f6b4e2f5c912`;
- `ConfiguredTargetAndDataProducer.java` SHA-256
  `eccb7fcd55955c40dfe7d88cb959164082db8603ec70bfb2bab258b8935741c0`;
- `ConfigConditionsProducer.java` SHA-256
  `42d72a2123f04c2925750ccc001d64acfb37d58a7dafc490fddb0af30bca564d`;
- `ToolchainRule.java` SHA-256
  `b9a3772db2ffacb005b051bd72a2fe7322cd27e9ffcb45e2293e87b04fb244d1`;
- `ConfigRuleClasses.java` SHA-256
  `c9f58ee6f8e657fe041ebfebc5289adc2d5eb07c9fecad7d7fd8dd1c804d6ba5`;
- `SkyframeExecutor.java` SHA-256
  `7d53250e5db42a98930ea3941362faaf42fab790d9d2efb5c3db9fe8c2d23867`;
- `BazelRepositoryModule.java` SHA-256
  `5d08f39631f3e656eb637c63c82d7c7894d06501e5a0664c02d49b6b5fb42782`;
- `RepoMappingManifestAction.java` SHA-256
  `e8663c7ed8a341ae3337386a82ce29dfb2e35daca3bba211409a920e5b1ad23a`;
  and
- `RunfilesRepoMappingManifestTest.java` SHA-256
  `8df1c7f6cc4558fe35405f43e7130ffc4f0588f41e75f18709adf520146545df`.

`BazelRepositoryModule.workspaceInit` enables external repositories.
`SkyframeExecutor.shouldStoreTransitivePackagesInLoadingAndAnalysis` therefore
keeps package tracking enabled for the Bzlmod/BCR server lifetime.
`TargetProducer` adds the metadata of each target package read directly.
`ConfiguredTargetAndDataProducer` adds the complete package nested set of each
configured prerequisite. `ConfigConditionsProducer` and dependency-map
construction share the same `TransitiveDependencyState`; package ownership is
broader than Starlark-visible attributes or Slug's current `computed` map.
`DependencyResolutionHelpers.addToolchainDeps` adds selected implementation
labels to that dependency map. Requested toolchain types and candidate
execution platforms remain inputs to separate toolchain-resolution SkyKeys and
are not configured prerequisites in the parent collector.

`PackageCollector.buildSet` sorts direct metadata by `PackageIdentifier`, then
adds configured-child nested sets in configured-target-key order, then applied
aspects. `RepoMappingManifestAction` fingerprints that nested set and later
derives a canonical-repository-sorted mapping table. This packet owns the
aspect-free direct and configured inputs, not action bytes.

The root mapping authority is the existing `HostRootRepositoryMappingKey`.
Its `HostRootRepositoryMapping::view().mapping()` exposes the selected ordered
mapping after extension and innate `use_repo_rule` usages have been applied.
`RootModuleGraph.repository_mapping`/`root_mapping()` is only the earlier
direct-module view and is forbidden as the retained root package mapping.

Zabel commit `0795445f3ab60f4e49070bdd0b94425c5610f73a` is peer
architecture and optimization guidance only. Authenticated peer sources are:

- `session_configured_transitive_package_repositories.zig` SHA-256
  `4d1e884ea1b9daad77ac71a2f7dff7c1bead7b9e8fad0057cfe5530b2bec2ec2`;
- `session_nonconfigurable_transitive_package_repositories.zig` SHA-256
  `8665924bc4032dadbf976f754ba11a7f9731d792387f963e50743bc70bbd528c`;
  and
- `analysis_py_internal_create_repo_mapping_manifest_bazel_fixture_test.sh`
  SHA-256
  `fac48e707b856be8fc11e7e7259f2310e7ae09ecf1e721b79db444d7e5469cbf`.

Zabel usefully separates configured package closure, repository-view-owned
mappings, and later action projection, and preserves requested/final child
ownership. Slug adopts those ownership and compactness lessons only. Copy no
Zig code, IDs, stores, exact-fingerprint nodes, digests, scheduler, cache,
layout, errors, action keys, or behavior.

## Compatibility classification

**Exact:** canonical package/repository identities; complete selected ordered
apparent-to-canonical mappings; root mapping after selected module-extension
and innate usages; one direct current-package contribution; every directly
read semantic package contribution; every admitted configured prerequisite's
complete package closure; direct-package sorting by `PackageIdentifier`;
canonical-label-first configured-child ordering; duplicate package
elimination; generated mapping cohort identity; mapping-only and child-only
invalidation; and non-null package tracking under Bzlmod.

**Slug-native:** structural configuration comparison after canonical labels
where Bazel orders by execution-platform label and options checksum; compact
Rust storage; structural DICE identity; and the opaque Rust spelling of a
generated mapping cohort. These divergences may change exact nested-set bytes
but may not omit a package, mapping, or dependency.

**Unsupported/deferred:** aspects and applied-aspect package closure; exact
Bazel package-root/build-filename metadata unused by mapping projection; exact
NestedSet interning/fingerprint bytes; manifest content and ISO-8859-1 writing;
the action's Bazel ActionKey; all four support actions; physical
materialization; aquery/execution/REAPI; Windows; and nondefault compact-
manifest mode. No deferred boundary permits a `None`, empty, digest-only,
repository-only, edge-only, or root-only substitute.

## Frozen retained model

Add one neutral build-API owner:

```text
RunfilesRepositoryMapping = {
  entries: Arc<[(ApparentRepoName, CanonicalRepoName)]>,
  compact_group: Option<CompactString>,
}
RunfilesPackageMetadata = {
  package: PackageIdentifier,
  mapping: Arc<RunfilesRepositoryMapping>,
}
RunfilesPackageDepset = Depset<Arc<RunfilesPackageMetadata>>
```

The configured-analysis crate owns its phase-local row so the neutral build API
does not depend upward on analysis identity:

```text
RunfilesPackageClosureRow = {
  key: ConfiguredNodeKey,
  packages: RunfilesPackageDepset,
}
```

`compact_group` is present only for repositories generated by one selected
module-extension or innate owner and uses that owner's existing collision-free
unique name. Root, selected modules, built-in `bazel_tools`, and ordinary local
routes use `None`. Equal mapping bytes from different owners remain identity-
distinct; repositories from one generated owner share a group. This preserves
the identity partition needed by later compact serialization without making
the group a repository identity or public name.

`RunfilesPackageMetadata::PartialEq` compares package, mapping entries, and
compact group. Its `Hash` intentionally hashes only `PackageIdentifier`, like
Bazel's cheap `Package.Metadata.hashCode`; full equality resolves lawful
collisions. All retained types implement `Allocative` and use cheap `Arc`
clones.

Use the accepted Buck2-derived dense depset and iterative traversal. Add no
retained standard hash collection, flattened repository list, exact-fingerprint
side graph, digest surrogate, interner, cache, global state, task, or lock.

## Complete root mapping route

`HostRepositorySourceRoute` exposes the generated-owner compact group as a
read-only projection of its already-selected route. It performs no repository
selection, source read, or DICE computation. External `PackageRecorder`
construction receives the selected mapping entries and group together.

Add one private mode-aware root-mapping child in `slug_loading_v2::bzl_module`:

- Legacy computes `HostRootRepositoryMappingKey`;
- Observed computes `HostRootRepositoryMappingObservationKey`;
- `Need` propagates unchanged;
- observed path frontiers remain outer outcomes;
- infrastructure/semantic mapping failures become typed root-package load
  errors without panics; and
- observed mapping epochs merge through the existing root-package observation
  union before publication.

Invoke this child after the existing root source and direct-load preparation
has succeeded and immediately before package evaluation. This preserves the
accepted source/load error frontier while ensuring even a root BUILD with no
external `load()` depends on the complete selected mapping. Collect
`HostRootRepositoryMapping::view().mapping()` in its owned order and pass it to
`evaluate_host_package_attempts_driver` instead of `Arc::from([])`. Root uses
`compact_group = None`.

`PackageRecorder::finish` moves package identity, mapping entries, and compact
group into one `RunfilesPackageMetadata` retained by `LoadedPackage`.
`LoadedPackage::PartialEq` includes it. Analysis must consume this retained
metadata and must not reconstruct mappings from labels, command mappings, or
module graphs.

Required root proof includes a no-external-load package whose BUILD bytes stay
constant while an extension/innate root mapping changes A/B/A. Both Legacy and
Observed roots must invalidate/restore, Observed must retain exact epoch/event
algebra, and the old empty/direct-module mapping path must be absent.

## Bazel-shaped analysis collector

Add one phase-local `RunfilesPackageCollector` inside configured analysis. It
owns two scratch sequences:

```text
direct: RunfilesPackageMetadata
configured: RunfilesPackageClosureRow
```

`add_direct` accepts metadata only from a `LoadedPackage` whose target read
corresponds to a Bazel `TargetProducer` direct contribution in the current
semantic computation; incidental implementation reads do not silently widen
membership. `add_configured` accepts only a completed child result or an
authenticated prepared carrier derived from one. Before
publication, sort/deduplicate direct values by package identifier and
configured rows by `ConfiguredNodeKey`; equal keys with unequal closures are an
internal error, never first-wins. Compose direct leaves followed by dense
transitive children without flattening.

Every `ConfiguredNodeResult` constructor requires the completed depset; there
is no default-empty path. A single finalization helper receives the node's
edges, closure rows, and extra semantic rows. Every Bazel package-contributing
edge target must have a matching closure row. Extra semantic configured
dependencies may contribute a row without becoming a public query edge.
`ToolchainRequirement` and `CandidateExecutionPlatform` are explicitly
noncontributing topology edges: pinned Bazel resolves them in separate
toolchain SkyKeys, while only selected implementation labels enter
`DependencyResolutionHelpers.addToolchainDeps` and the parent's dependency
map. No other edge kind may omit a row. The helper performs no DICE compute,
await, source read, mapping lookup, or lock acquisition.

### Nonconfigured and native nodes

- source/exported file: direct current package only;
- package group: compute every included null-config package group under the
  existing configured-analysis cycle guard, publish include edges from those
  results, and add each closure;
- constraint setting/toolchain type: direct current package only;
- constraint value: add the already-computed setting child closure;
- platform: add every already-computed constraint-value child closure;
- alias: add the requested alias child's closure, which already contains its
  requested package and finalized actual closure;
- generated file: add the generating rule's closure; and
- native toolchain declaration: compute and publish configured children for
  `toolchain_type`, `exec_compatible_with`, `target_compatible_with`, and
  selected `target_settings` dependencies in deterministic attribute/index
  order. Bazel's `toolchain` attribute is `NODEP_LABEL`; the implementation
  label is deliberately not a declaration child and enters only when selected
  for a consuming rule.

No native branch may publish an edge before its child result is available.
Package-group and native-toolchain cycles use the existing analysis cycle
guard and fail; they never publish a partial closure.

### Starlark rule preparation

Replace the assumption that `computed` is complete with typed carriers from
each semantic preparation stage:

- each configured selector condition exposes one configured-child row keyed by
  its `config_setting`; that row's dense closure contains the condition
  target's package as its direct leaf and the closures of configured flag and
  constraint attribute prerequisites. The parent adds only this configured
  row, never a flattened direct condition-package leaf. Target-platform lookup
  used only to test constraint matching is not a package contribution;
- declared, transitioned, hidden, and subrule attributes contribute the child
  closures already held in `computed`;
- declaring visibility labels are explicitly computed as null-config children
  under the same cycle guard before `finish_analysis` and contribute their
  closures;
- configured execution-platform and toolchain-resolution results remain typed
  topology/selection inputs but do not enter the parent's package collector;
  the candidate and requested-type edges are noncontributing for the pinned
  reason above; and
- `PreparedToolchain` retains closure rows for selected implementation results
  beside the action context instead of discarding them.

The parent collector merges selector-condition, declared/hidden, visibility,
and selected-implementation rows. Duplicate rows deduplicate by configured key
only after full closure equality. Public edge construction and package closure
construction consume the same prepared rows for every contributing edge;
selected implementation and visibility edges cannot drift from closure
ownership. Candidate-platform and requested-type changes still invalidate
toolchain topology through their existing DICE keys but do not change the
package depset unless they change the selected implementation.

Prepared carriers retain only immutable keys and dense package depsets, not
full child results or evaluator values. They are `Allocative`, cheap to clone,
and remain inside existing analysis/result owners. No new DICE key is added:
existing loading, condition, platform, resolution, and configured-analysis
keys remain the invalidation edges; their values gain only the semantic package
projection already computed below them.

## Bounded implementation succession

After design `ACCEPT`, land two independently reviewed implementation commits:

1. **Loading/metadata owner:** retained build-API types, generated-owner group
   projection, complete Legacy/Observed root mapping route, external mapping
   forwarding, `LoadedPackage` equality, and focused loading/Bzlmod proofs.
   This commit changes no configured result or action.
2. **Configured collector:** mandatory `ConfiguredNodeResult` closure,
   phase-local collector, native/null/Starlark preparation carriers, complete
   edge coverage, DICE invalidation, and retained-size proofs. This commit
   changes no provider public field or action count.

Only after both commits are terminally accepted may the four-action runfiles
support packet activate. It must share one typed support object across all
recipes and the completed `FilesToRunProvider`, and the private FilesToRun
occurrence carrier becomes mandatory after builtin identity validation. There
is no supportless compatibility fallback.

## Allowlist and caps

Design/ledger files may update this manifest, the canonical plan, Stage 6, and
Stage 9.

Loading/metadata production allowlist:

- new `app/slug_build_api_v2/src/runfiles_packages.rs`;
- `app/slug_build_api_v2/src/lib.rs`;
- `app/slug_bzlmod_v2/src/{canonical_repository_route.rs,selected_repo_spec/selected_extension_demand.rs}`
  only for the read-only generated-owner projection;
- `app/slug_bzlmod_v2/src/source_preparation/canonical_repository_source.rs`;
- `app/slug_loading_v2/src/{bzl_module.rs,package.rs}`; and
- compiler-required loading constructor call sites only.

Configured-collector production allowlist:

- `app/slug_analysis_v2/src/{result.rs,dice.rs,starlark_rule.rs}`;
- `app/slug_analysis_v2/src/configured_target.rs` only if existing edge kinds
  cannot name native toolchain attributes; and
- compiler-required `ConfiguredNodeResult` constructor call sites in
  `app/slug_analysis_v2/src` only.

Proof allowlist:

- new `app/slug_build_api_v2/tests/runfiles_packages.rs`;
- colocated Bzlmod projection tests;
- `app/slug_loading_v2/src/host_package_load_tests.rs`;
- `app/slug_analysis_v2/tests/{configured_target.rs,starlark_rule.rs}`; and
- focused private DICE tests colocated in `app/slug_analysis_v2/src/dice.rs`.

Across both implementation commits: at most 1,050 net / 1,300 gross
production Rust, 700 net / 850 gross proof Rust, and 1,750 net / 2,150 gross
total Rust. The retained build-API owner stays below 260 physical lines and
each new helper below 160 lines. No touched production file may newly cross
2,000 lines. Existing oversized loading/DICE owners receive bounded fields,
projections, carriers, and helpers only. `REPLAN` before structural refactoring,
a new crate/dependency/key, or cap excess.

Add no parser/provider public field/action/executor file, action kind,
filesystem observation, ruleset branch, V1 extraction, exact-fingerprint node,
or second retained graph.

## Required proof

1. full metadata equality distinguishes mapping entries and compact groups
   while the package-only hash remains lawful;
2. dense composition preserves sorted direct leaves, sorted configured
   children, exact duplicate elimination, shared diamonds, alias requested/
   final ownership, and depth-3,500 safety;
3. root/selected/builtin mappings are identity-distinct, multiple repositories
   from one ordinary module-extension owner share its group, an innate owner
   independently projects its own group, and equal bytes from different owners
   or owner kinds do not share;
4. Legacy and Observed root packages consume the complete selected mapping and
   restore on mapping-only A/B/A with unchanged BUILD bytes and exact observed
   frontier/event behavior;
5. source, package-group, alias, generated, constraint, platform, native
   toolchain, selector-condition, visibility, hidden/subrule, ordinary
   Starlark, and selected-implementation package-contributing paths publish
   complete closures;
6. a mechanical assertion proves every package-contributing edge has a
   matching child closure, while selector conditions remain retained without
   inventing query edges; a pinned Bazel oracle proves candidate-platform and
   requested-toolchain topology alone do not enter the package closure;
7. warm same-DICE A/B/A direct-package, child-package, mapping, candidate,
   visibility, condition, and selected-toolchain changes invalidate then
   restore without replaying unrelated siblings; candidate/requested changes
   that preserve selection restore the identical package closure;
8. Bazel 9.2 oracle fixtures discriminate ordinary, visibility/package-group,
   select/config-setting, alias/final, generated, platform/native toolchain,
   selected-toolchain-runfiles, external mapping, and shared-diamond package
   membership for the admitted aspect-free graph;
9. every executable FilesToRun remains incomplete and action count, public
   provider fields, query formatting, and execution behavior do not change;
   and
10. retained-size accounting proves Arc/dense sharing and absence of a flat
    repository list, full-child retention, or parallel graph.

Run serial focused and full `slug_build_api_v2`, `slug_bzlmod_v2`,
`slug_loading_v2`, and `slug_analysis_v2` suites plus `cargo check -p
slug_core_v2`. Finish each implementation commit with `cargo fmt --all --
--check`, metadata, archive status, `git diff --check`, cap/physical-size
accounting, independent terminal review, and parked-file SHA-256 verification.

## Review gate

Independent review must return `ACCEPT` or `REPLAN` on complete root mapping
ownership, Bazel direct/configured contribution fidelity, every admitted
semantic and published edge, requested/final alias handling, dense topology,
DICE invalidation, natural owners, absence of fallback/parallel state,
successor sufficiency, proof, and caps. Commit the accepted zero-Rust design
before implementation.
