# Current Slug V2 Packet

Packet: `WP-5-host-nonregistry-module-closure-implementation-r2`
Milestone: cross-stage M7 prerequisite implementation
Owner: `slug-v2-subplans/05-bzlmod-and-repository-graph.md`
Result: implement the accepted route-independent nonregistry MODULE/include
closure owner without activating evaluation or a graph consumer.

## Accepted predecessor boundary

Commit `054844d4` records the first closure design and its exact `REPLAN`:
root and fragment bytes already had a route-independent owner, but include
package preflight still required `RootRepositoryRoute` plus apparent and
canonical repository identity.

Commit `411af144` removes that blocker. It adds callerless crate-private
nonregistry REPO, ignore, and package-preflight keys over normalized workspace,
exact `NonrootModuleKey`, and `PackagePath`. They reuse only
`RepositorySourceFileKey`, preserve local/immutable invalidation and
REPO/.bazelignore/BUILD-marker order, and fail closed before source work for
every nonempty canonical deleted-package set. No MODULE closure, evaluator,
graph, or consumer was activated.

The remaining live route-bound owners are
`DirectLocalModuleInspectionKey`,
`DirectLocalIncludePackageHorizonKey`, and
`DirectLocalModulePreparationKey`. Their inspection, breadth-first include,
repeated-label, cycle-capability, fragment validation, Need-union, and error
ordering are accepted semantics, but their retained `RootRepositoryRoute`
and canonical `PackageIdentifier` cannot identify a transitive nonregistry
override. The legacy supplied-file `ResolvedGraph` is still forbidden as a
second production graph.

## Accepted design review

Commit `5757ea1d` and independent latest-diff review accept the resumed
architecture: one route-independent closure key, a shared parsing/BFS core,
the two-file allowlist, 420/480/900 caps, lifecycle/error/order proof, and
fail-closed evaluation/graph boundaries.

## Accepted cap correction

The first implementation attempt ended REPLAN solely because exact section
accounting measured 587 formatted net production lines and 468 test lines,
1,055 total, beyond the frozen 420/480/900 limits. Independent latest-diff
review found no semantic, identity, ownership, or error-order blocker and found
that forcing the shared Host/direct adapter algebra under the original
production cap would create riskier abstraction. Revision two preserves the
complete diff and every scope, behavior, proof, and terminal stop, changing
only the bounds to 620/500/1,120. The margin grants no authority for another
owner, diagnostic, behavior, evaluator, graph consumer, or public surface.

## Active implementation contract

Implement one crate-private
`HostNonregistryModuleClosureKey { workspace, module: NonrootModuleKey }`.
It computes `RootModuleFilesKey` first, requires exactly the named
`RootModuleOverride::NonRegistry(RepoSpec)`, projects a route-free semantic
source identity from the existing `RepositoryMaterializationKey`, and reads
the root `MODULE.bazel` plus every included fragment only through
`RepositorySourceFileKey`.

The retained complete value must include the exact module key, RepoSpec/source
category identity, exact root and fragment bytes, logical source identifiers,
validated inspections, include labels and spans in occurrence order, and
either a supported closure or the existing explicit unsupported cycle
capability. Physical local/generation roots, observation instances, apparent
names, canonical repository names, and request generations are operational
and must not enter semantic closure equality. Immutable source identity and
all exact content must remain structural.

Use a shared route-independent preparation core rather than a second BFS.
It must:

1. inspect and validate the root source before include work;
2. parse every occurrence in source order without manufacturing repository
   identity;
3. deduplicate package computations while running the complete horizon through
   `HostNonregistryPackagePreflightKey`;
4. union all horizon Needs, but select the first source-order terminal error;
5. only after the horizon succeeds, deduplicate fragment paths and read them
   through the sole source key;
6. union fragment Needs and preserve first-occurrence source/error order;
7. validate the complete horizon, append repeated occurrences without
   deduplicating closure order, and derive the next breadth-first horizon; and
8. retain the first cycle capability while completing all otherwise reachable
   preparation work allowed by the accepted direct-local semantics.

Keep the current direct-local keys as adapters over the shared pure parsing,
relative-path, ancestry/cycle, and preparation logic so their existing Host
query/package behavior and diagnostics do not change. A tiny crate-private
parser projection in `module_eval.rs` may expose `PackagePath` plus
`TargetName` directly if that is required to remove the current transient
root-canonical wrapper; it may not widen accepted include syntax or public API.

Edit only
`app/slug_bzlmod_v2/src/source_preparation.rs` and
`app/slug_bzlmod_v2/src/module_eval.rs`. The latter is limited to the
crate-private route-free include parser projection and its colocated tests;
all DICE ownership and preparation remain in `source_preparation.rs`. Cap
formatted net growth at 620 production lines, 500 test lines, and 1,120 total.
Account production and tests exactly by splitting each file at its main
cfg(test) module boundary; the corrected margin may cover only this retained
implementation and focused corrections.
Add no file, public export, Cargo/BUILD metadata, dependency, fixture, asset,
cache, lock, interner, process-global state, or direct filesystem path.

Focused real-DICE proof must cover local content A/B/A, immutable
content/generation A/B/A, local/immutable category A/B/A,
RepoSpec/source-identity changes, root missing/wrong-kind/recovery, breadth-
first horizon ordering, package preflight before fragment reads, repeated
labels, fragment missing/wrong-kind/Need union, cycle capability, cold/warm
reuse, and unchanged direct-local behavior. Structural scans must prove the
new owner contains no route/apparent/canonical keys, raw Host file/path keys,
direct filesystem IO, evaluation, graph, or consumer edge. Run focused closure
and direct-local tests, the full `slug_bzlmod_v2` suite, downstream
`slug_loading_v2` and `slug_core_v2` checks, formatting, diff/scope/caps,
and independent latest-diff implementation review.

Return `REPLAN` if the shared core cannot preserve accepted direct-local
behavior, if source identity would require physical roots in semantic
equality, if package preflight cannot be composed without duplicate policy or
IO ownership, or if preparation would need evaluation/MVS/mapping state.

## Compatibility

Exact: Bazel 9.2 root-local include parsing, breadth-first closure preparation,
package preflight, fragment ordering, and typed source behavior for admitted
root `local_repository` and immutable archive/Git RepoSpecs with empty
canonical deleted-package policy. Slug-native: DICE/type/diagnostic names,
logical source namespace, Host observation framing, compact storage, and
non-Bazel identity bytes. Unsupported/deferred: nonempty deleted policy,
command overrides, registry/built-in closure through this owner, MODULE
evaluation/discovery consumption, recursion/MVS, mappings/extensions/
registrations, package/BUILD/Bzl loading, toolchains, Test, execution/results/
BEP/coverage, unadmitted RepoSpecs, Windows, JVM/Java, and exact Bazel identity
bytes.

## Terminal stops

Stop with `REPLAN` on route/apparent/canonical identity in the new owner,
duplicate materialization/source/package ownership, direct filesystem IO,
lost RepoSpec/category/content/include identity, changed direct-local
semantics, lock-held compute, evaluation/graph/consumer activation, second
graph, JVM/Java, third file, or cap excess.
