# Current Slug V2 Packet

Packet: `WP-4-5-7A-canonical-external-package-loading-adapter-design`

Milestone: M7A command/ruleset bootstrap closure feeding ordinary M8 Stage
10.3 analysis.

Base: `85593f300`.

Result: freeze two bounded implementation packets that let a selected
canonical repository reach package policy, subtree discovery, external `.bzl`
evaluation and package loading without a root-visible alias. This packet is
docs-only and activates no behavior.

## Learned facts and non-decisions

Commit `85593f300` accepts one apparent-free canonical source input, shared
canonical source/listing projections and a loading-owned canonical load route.
Its root-unmapped transitive registry proof succeeds by canonical name and its
selected source specification and final mapping invalidate independently.
Both address wrappers converge on the existing built-in catalog,
materialization-result, path-resolution and directory-listing owners.

The caller chain cannot be migrated by changing only
`HostExternalPackageBoundaryKey`. Its private package lookup computes REPO and
repository-ignore policy before package markers; those owners and package
source still retain `RootRepositoryRoute`. Loading's recursive external
subtree, external `.bzl` keys/cycle identity and repository package load also
retain that full root route. A selected canonical repository may have no
root-visible alias, and fabricating one would invent mapping state.

Do not retype `RootRepositoryRoute`, make its apparent name optional, bypass
REPO/ignore policy, copy a selected mapping/specification, infer a physical
root, or move source IO into loading. Do not combine both crate migrations in
one implementation packet. Existing core/query/module-extension root callers
remain exact and unchanged through this design.

## Accepted two-stage architecture

### Stage A: Bzlmod source and policy convergence

Add one compact source-route carrier with exactly two variants:

- `Root(RootRepositoryRoute)`, retaining existing root identity and behavior;
- `Canonical(HostCanonicalRepositorySourceInput)`, retaining the accepted
  apparent-free route plus materialization disposition.

Generalize the existing source-file, direct-directory-listing, REPO,
repository-ignore, private package lookup, public external package boundary
and repository-package-source owners around that carrier. Root constructors
remain exact adapters. Canonical constructors accept only the already-complete
source input; they do not compute loading keys or perform IO. Both variants
must converge on the accepted catalog/materialization/path owners.

After every canonical source/policy caller uses the generalized owners, delete
the four temporary canonical source/listing legacy/observed key wrappers from
`canonical_repository_source.rs`; retain only the source input/carrier logic.
Do not delete existing root constructors or alter their outputs, errors,
dependency order or display identity.

### Stage B: loading and package adaptation

Add one loading request-address carrier with two variants: the existing full
root route, or apparent-free workspace plus canonical repository. Root
constructors and current callers remain unchanged. Canonical variants compute
`HostCanonicalRepositoryLoadRouteKey` or its observed sibling first and pass
the resulting generalized source route to Stage A owners.

Use that address for external subtree, external `.bzl` evaluation/cycle
identity and repository package load. A canonical external load resolves its
apparent repository through the current canonical route's full mapping, then
computes the child's canonical load route. It must merge child route/effect
observations before child source/module observations. It must not synthesize a
`RootRepositoryRoute` or root alias.

Canonical cycle identity is workspace plus canonical repository plus
`RepositoryBzlLabel`; mapping and specification remain structural
dependencies of the canonical route owner. Root cycle identity stays the
existing full root route plus label. Root-only direct-local MODULE support is
not called for an alias-free canonical route; proof must show the selected
canonical definition already owns the necessary module-evaluation predecessor.

## DICE, ordering and lifetime

Follow `docs/developers/dice.md`. Keys retain complete semantic route carriers,
never command scratch. Observed order is route, generated effect, REPO/ignore,
boundary, BUILD source, then recursive `.bzl` children. Outer error precedes
Need, which precedes semantic terminal. Route or child-route failure/Need
activates no downstream source. Merge epochs in that same order and publish no
complete value on Need or cancellation. Equality/validity remain complete-only
and no lock spans a DICE compute.

The Stage A carrier is one compact enum over existing retained values. Stage B
retains either the existing root route or workspace/canonical address and, in
complete results, the accepted canonical load route. Add no mapping/spec/source
copy, interner, global cache, side table, manual eviction or dependency. Reuse
`Arc`, `Dupe`, existing structural hashes and accepted compact route state.

## Bazel, Buck2 and Zabel basis

Pinned Bazel 9.2 package lookup applies deletion and repository policy before
marker selection; recursive traversal composes lookup with listing; Bzl load
resolution applies the declaring repository's mapping before creating a child
load key. Existing accepted Bazel regressions cover root behavior; the new
canonical path requires structural and selected-registry regressions, not a
new named-surface oracle.

Buck2 DICE ownership/cancellation guidance supports one semantic owner and
ordered observed composition. Zabel is peer guidance for separating
authenticated source, mapping, package discovery and consumers, and for
compact retained carriers only. Bazel 9.2 remains behavioral authority.

This is generic BCR Starlark loading architecture. Bazel 9 owns rule
definitions and control flow, including `cc_internal`; `cc_common` is only a
demanding consumer of the generic host-builtin ABI. Builtins remain planned by
reusable capability category.

## Compatibility

- **Exact:** existing root source/policy/subtree/`.bzl`/package-load results,
  diagnostics, dependency order, cycle behavior and observations; canonical
  label mapping follows Bazel 9.2 semantics.
- **Slug-native:** carrier enum layout, key names, structural hashes, observed
  carriers and retained-memory accounting.
- **Unsupported/deferred:** BUILD repository-qualified loads beyond the
  admitted loader, target-pattern expansion, toolchain/execution-platform
  registration, configured semantics, rules, actions and exact output identity.

## Implementation packets to freeze

Stage A becomes
`WP-4-5-7A-canonical-source-policy-convergence-implementation` with only:

- `app/slug_bzlmod_v2/src/source_preparation/canonical_repository_source.rs`;
- `source_preparation.rs`, `lib.rs`, `repo_file.rs`,
  `repository_ignore.rs`, `host_package.rs`;
- `host_package_observation_tests.rs`,
  `source_preparation_observation_tests.rs`; and
- `host_external_package_boundary/mod.rs` and `tests.rs`.

Cap Stage A at 1,200 production and 1,500 proof lines, functions at 120 lines,
and require a bounded split for any touched production file already over the
complexity trigger. Deletions count separately and do not buy unrelated scope.

Stage B becomes
`WP-4-5-7A-canonical-loading-package-adapter-implementation` with only:

- a new `app/slug_loading_v2/src/external_repository_load_route.rs`;
- loading `lib.rs`, `external_subtree_package_set.rs` and its tests;
- `bzl_module.rs`, `cycle_detector.rs`, and `host_package_load_tests.rs`.

Cap Stage B at 1,300 production and 1,800 proof lines, the new module below 300
lines and functions at 120 lines. `bzl_module.rs` and its 30k-line test file are
above complexity triggers: permit only route plumbing, bounded helper
extraction and focused proof; no unrelated cleanup.

## Required proof and validation

Stage A must prove all root outputs/dependency order unchanged; root-unmapped
selected-registry REPO/ignore/boundary/package-source success; exact deepest
catalog/materialization/path owners; deletion of temporary canonical wrappers;
complete-only lifecycle; carrier A/B/A identity/hash and compact size.

Stage B must prove root constructors/callers unchanged; alias-free canonical
subtree and package load; same-repository and mapped child `.bzl` loads;
mapping A/B/A child identity; exact route/effect/policy/source/recursive epoch
order; route/child-route Need and failure short-circuit; cycle identity;
cancellation; and no fabricated alias or duplicate source owner.

Each implementation runs focused owners, full bzlmod/loading, named core/query
dependents, locked CLI build, formatting, diff/scope/cap/duplicate-owner/no-lock
guards and the archive checker. Each requires independent DICE/public-boundary
terminal review. Stage B starts only after Stage A is accepted.

## Stops

STOP and `REPLAN` for a fabricated apparent alias; optional apparent state
inside `RootRepositoryRoute`; copied mapping/spec/source bytes; a second
effect/materialization/path owner; loading-layer direct filesystem/catalog
access; bypassed REPO/ignore policy; changed root caller behavior; missing
child-route epoch; lock across compute; dependency, cap or allowlist expansion;
or activation of registration, configured semantics, rules or actions.

## Immediate predecessor and successor

Commit `85593f300` is the accepted predecessor. Independent caller-chain audit
requires this docs-only design before Rust. If independent design review
accepts the split, activate only Stage A. Stage B follows only after Stage A;
the one shared toolchain/execution-platform registration expander follows only
after Stage B.
