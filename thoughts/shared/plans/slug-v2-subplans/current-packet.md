# Current Slug V2 Packet

Packet: `WP-5-host-selected-module-route-owner-design-r2`
Milestone: cross-stage M7 prerequisite design
Owner: `slug-v2-subplans/05-bzlmod-and-repository-graph.md`
Result: audit the smallest post-selection canonical identity, contextual mapping,
and repository-route composition owner now that selected registry RepoSpecs exist.

## Active design contract

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

This packet may edit only:

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`;
- this manifest; and
- `thoughts/shared/plans/slug-v2-subplans/05-bzlmod-and-repository-graph.md`.

The root may inspect pinned Bazel 9.2 source and live Rust owners read-only.
Cap net growth at 300 manifest lines, 280 owner-plan lines, 45 canonical lines,
and 625 total. Obtain fresh independent reserved-architecture review.

No Rust, Cargo/BUILD, public API, legacy graph/catalog activation, registry I/O,
RepoSpec projection, materialization, loading, lockfile publication, command,
analysis, execution, mapping consumer, route consumer, or JVM/Java work is
authorized. Return `REPLAN` on a missing selected RepoSpec/source/collision
owner, a second graph or observation edge, an implementation successor beyond
three Rust files, or inability to freeze an explicit allowlist/caps/stops.
Return `REVISE` on one bounded design correction; a second material correction
is `REPLAN`. No production representation may begin before independent
`ACCEPT` and explicit implementation activation.

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
