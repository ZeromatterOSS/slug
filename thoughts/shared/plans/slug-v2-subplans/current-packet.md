# Current Slug V2 Packet

Packet: `WP-5-host-selected-module-route-owner-implementation`
Milestone: cross-stage M7 prerequisite implementation
Owner: `slug-v2-subplans/05-bzlmod-and-repository-graph.md`
Result: implement the independently accepted callerless selected module route
owner without activating any consumer.

## Active implementation contract

Implement exactly the independently accepted selected-route design below. This
packet may edit only:

- `app/slug_bzlmod_v2/src/selected_repo_spec.rs`; and
- `app/slug_bzlmod_v2/src/source_preparation.rs` for the sole crate-private
  borrowed nonregistry RepoSpec projection and focused assertion.

Cap formatted net growth at 420 production lines, 700 test lines, and 1,120
total. Complete the frozen pure and real-DICE proof matrix, full owner/loading
validation, compact-representation and AI-cleanup audits, structural scans,
and independent implementation review.

No third file, public export, predecessor key/value mutation, second graph/I/O/
override owner, legacy graph/catalog, raw I/O, extension mapping/route,
materialization, lockfile/final-module publication, loading, command, analysis,
execution, or consumer activation is authorized. Return `REPLAN` on any stop
or cap excess; `REVISE` on one bounded implementation defect; a second
material correction is `REPLAN`.

## Accepted design contract

This accepted design is historical context for the active implementation and
grants no separate file, action, cap, or scheduling authority.

Perform one read-only owner audit over the accepted
`HostSelectedModuleGraphKey`, `HostSelectedRegistryRepoSpecsKey`, built-in
identity, and nonregistry provenance. The audit must pin Bazel 9.2 ordering and:

- derive root, well-known, single-version, and multiple-version canonical module
  repository names from the selected graph, including collision terminals;
- derive the root/self/resolved-ordinary-dependency contextual mapping while
  keeping extension imports, extension repositories, and overrides additive and
  deferred;
- compose exactly one selected module route algebra across root, built-in
  `bazel_tools`, nonregistry RepoSpecs, and selected registry RepoSpecs without
  re-reading registry files, re-merging overrides, or activating the legacy
  supplied-file graph;
- decide whether accepted `RootRepositoryRoute` can be reused or whether one
  new private selected-route value is required, and identify the sole DICE
  owner, structural equality/validity, Need/error ordering, and consumer-neutral
  boundary;
- freeze an auditable implementation successor allowlist, production/test/total
  caps, discriminating lifecycle/collision/mapping proof, and terminal stops; or
  return `REPLAN` at the first missing semantic leaf; and
- classify exact, Slug-native, and unsupported/deferred surfaces explicitly.

The prior docs-only three-file allowlist and 300/280/45/625 caps are closed.
The accepted audit established this implementation seam before activation.

## Accepted predecessor evidence

This section is historical evidence only and grants no file, action, cap, or
scheduling authority.

Commit `e8ad58dd` accepts the private callerless selected registry RepoSpec
owner at 1,010 production, 844 tests, and 1,854 total formatted lines, within
the corrected 1,020/1,050/2,070 caps. It borrows the accepted selected graph,
Host registry policy, registry-file observations, winning MODULE provenance,
effective override, and compact RepoSpec algebra. It fetches only selected
registry source state and performs no registry work for root, built-in, or
nonregistry selected entries.

The exact admitted projection covers archive, local_path, and git_repository,
mirror order/deduplication, blank registry JSON, decoded file-registry anchoring
and lexical path normalization, MODULE SRI, and RegistrySingle patch fields.
All semantic inputs remain structural; Need is invalid; completed typed errors
beat compatible Need. Ten pure and five real aggregate DICE tests prove
selected/unselected I/O, root/built-in/nonregistry zero work, source/policy/
MODULE/override A/B/A restoration, warm reuse, Need validity, and typed-error
precedence. The full owner and loading suites, formatting/diff/scope scans,
AI-cleanup review, and independent implementation review pass.
## Completed owner audit

Pinned Bazel 9.2.0 at `8220c6198837d5c13d53fea211cf3282aa12408a`
confirms the composition order. `BazelDepGraphFunction` receives the
post-selection BFS graph, counts selected keys by module name, constructs one
canonical-name bi-map, and only then derives each module's Bazel-dependency
repository mapping. `ModuleKey` maps root to main, `bazel_tools` and
`platforms` to their well-known unversioned names, a uniquely selected module
to `<name>+`, and every version of an MVO name to
`<name>+<normalized-version>`. Bi-map collisions are terminal.

`Module#getRepoMappingWithBazelDepsOnly` maps the empty apparent name to main
for root, a nonempty module `repo_name` to its own canonical identity, and
each resolved ordinary dependency's apparent name to the dependency's selected
canonical identity. Slug's accepted `HostSelectedModuleGraph.resolved` is
already roots-first BFS and retains exactly those transformed ordinary edges,
their apparent names, each root/nonroot self repo name, and the complete source
provenance. Nodep-only reachability is absent from this slice.

The accepted `HostSelectedRegistryRepoSpecsKey` now closes the only missing
source leaf. It owns one exact selected registry entry for every resolved
registry module and none for root, built-in, or nonregistry entries. Accepted
nonregistry provenance owns `HostNonregistryPreparedClosure`, whose source
identity already contains the exact effective RepoSpec. Built-in provenance
owns `BuiltinBazelToolsRouteIdentity`. No registry file, override map, or
materialization result needs to be read again.

The existing public `RootRepositoryRoute` is not the selected seam: its source
algebra is only `DirectLocal|BuiltinBazelTools`, its key is root-context-only,
and it hardcodes `<name>+` before MVO/collision selection. The public
`RootModuleGraph.repository_mapping` is likewise a preselection root adapter
with a synthetic mapping id. Both remain unchanged for their accepted current
consumers. Widening either would mix predecessor and selected ownership.

## Proposed selected-route owner

Add a callerless crate-private `HostSelectedModuleRoutesKey { workspace }` in
the existing private `selected_repo_spec.rs`. It computes
`HostSelectedModuleGraphKey` first and, only after a complete successful
graph, `HostSelectedRegistryRepoSpecsKey`. It holds no lock across DICE
computation and performs no I/O itself.

The retained value is one Arc-backed BFS slice of
`HostSelectedModuleRoute` entries. Each entry owns:

- the accepted shallow-cloned `HostSelectedModuleEntry`, retaining its key,
  source/provenance, and ordered transformed/original/nodep edges;
- the derived `CanonicalRepoName`;
- one private `HostSelectedRepositoryMapping { context_repo, entries }` whose
  compact ordered map contains exactly root-empty, nonempty self, and resolved
  ordinary-dependency mappings; and
- an optional exact `HostSelectedRegistryRepoSpec`, present iff the selected
  source provenance is registry.

Transient compact maps may count names, detect canonical collisions, resolve
dependency keys, and match registry entries, but no scratch index is retained.
The selected registry entry remains whole rather than reducing it to RepoSpec,
so registry policy, observations, MODULE provenance, override provenance, and
final RepoSpec remain structural. The selected graph entry likewise retains
built-in and nonregistry provenance without duplicating those owners.

Add only a crate-private
`HostNonregistryPreparedClosure::repo_spec(&self) -> &RepoSpec` projection in
`source_preparation.rs`. It selects the RepoSpec already retained in either
Local or Immutable source identity. It performs no classification, computation,
observation, cloning policy, or materialization. Future route consumers can use
this projection without rereading `HostEffectiveModuleOverrideKey`.

The route key returns typed graph, selected-RepoSpec, canonical-name,
collision, source/provenance, missing/extra-registry-entry, and mapping errors.
Graph completion precedes selected source projection exactly as in Bazel.
Slug's Need remains invalid/non-self-equal; complete errors are stable. Within
pure derivation, first failure follows the retained BFS and dependency order.
This deterministic completed-error choice and Rust error wording are
Slug-native; the accepted canonical identities, collision terminal, mapping
contents/context, source category, RepoSpec, and BFS route order are exact.

Extension imports, unique extension names, repo overrides, extension-generated
routes, post-selection validation policy, final-module/lockfile publication,
materialization, public `RootRepositoryRouteKey` replacement, loading, and all
command/analysis/execution consumers remain unsupported/deferred in this
packet.

## Frozen implementation successor

After independent design acceptance, activate only
`WP-5-host-selected-module-route-owner-implementation` in:

- `app/slug_bzlmod_v2/src/selected_repo_spec.rs` for the private selected-route
  key/value/error, pure derivation, and colocated tests; and
- `app/slug_bzlmod_v2/src/source_preparation.rs` for the single crate-private
  borrowed nonregistry RepoSpec projection and its focused assertion.

Cap formatted net growth at 420 production lines, 700 test lines, and 1,120
total. The increase grants no third file, public API, new owner, source family,
consumer, or behavior family.

Required proof:

- reuse the checked-in Bazel 9.2 `repo-mapping-canonical-names` oracle; add no
  fixture unless a demonstrated missing source discriminator forces REPLAN;
- pure tables for root, well-known names, unique versions, normalized MVO
  versions, empty nonregistry versions, canonical collisions, root/self/alias
  mappings, context identity, MVO context differences, and roots-first BFS;
- real-DICE root/built-in/nonregistry/registry composition, exact
  selected-registry matching and zero extra registry work, source/mapping/
  version/alias/RepoSpec A/B/A restoration, and cold/warm equality/reuse;
- complete typed graph/source/collision/missing-extra errors, Need
  invalidity/non-self-equality, and graph-before-source precedence;
- full `slug_bzlmod_v2` and `slug_loading_v2` suites, formatting/diff/cap
  checks, and structural scans proving no legacy resolution/catalog,
  `RootRepositoryRouteKey`, raw I/O, override-map merge, materializer,
  loading, lockfile, or consumer edge; and
- fresh AI-cleanup, Buck2 compact-representation, and independent
  implementation review.

Return `REPLAN` on a third file, public export, change to either predecessor
key/value, second graph/I/O/override owner, inability to borrow the exact
nonregistry RepoSpec, extension mapping/route need, materialization or consumer
activation, cap excess, or any missing exact canonical/mapping/source identity.
A single bounded implementation defect is `REVISE`; a second material
correction is `REPLAN`.
