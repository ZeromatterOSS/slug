# Current Slug V2 Packet

Packet: `WP-4-5-7A-repository-qualified-external-bzl-load-route-design`

Milestone: M7A registered-toolchain closure prerequisite.

Base: accepted complete direct `tools/build_defs/cc` catalog `84190d95c`,
accepted external-.bzl source-observation cutover `1b997c5ef`, accepted
TestingBootstrap ABI `ecee4aca5`, and accepted selected-BCR realization
`1599d730c`. The proof-only registration and selected-context candidates
remain dirty, parked, and read-only.

## Observable boundary

The catalog packet imports the complete pinned Bazel 9.2 direct package
`tools/build_defs/cc/{BUILD,action_names.bzl,cc_import.bzl}`. Exact
bytes/modes/listing/manifest/package-set tests, complete Bzlmod/loading suites,
CLI rebuild, two cold real replays, and independent terminal review pass.

Both fresh-workspace/fresh-output-root rules_rust cqueries clear
`action_names.bzl` and stop identically at:

```text
loading `@rules_cc//cc/common:cc_common.bzl`:
resolving a load in @@rules_cc+//cc/common:cc_common.bzl:
repository-qualified external load is deferred:
@cc_compatibility_proxy//:symbols.bzl
```

This is a generic repository-qualified external-`.bzl` route/mapping
boundary. The evaluator already parses and freezes the exact rules_cc public
facade and compatibility-proxy provider graph in repository-owned tests. It is
not a `cc_common` parser, `set`, C++ rule-engine, provider-shape, or source
catalog failure.

## Established facts and authority

Bazel 9.2 and the selected BCR source are sole semantic authority:

- rules_cc 0.2.17's `MODULE.bazel` declares
  `compat = use_extension("//cc:extensions.bzl", "compatibility_proxy")` and
  `use_repo(compat, "cc_compatibility_proxy")`;
- its exact `cc/common/cc_common.bzl` loads
  `@cc_compatibility_proxy//:symbols.bzl`; the accepted source hash is
  `65e91cf0fa7ebb1c8efc84bbf6b1c4ec4db46f5e5ed4606759aa4a45a23b4063`;
- Bazel's final repository mapping contains
  `rules_cc+ : cc_compatibility_proxy ->`
  `rules_cc++compatibility_proxy+cc_compatibility_proxy`, and the generated
  repository maps `rules_cc -> rules_cc+`;
- repository-qualified loads resolve in the defining `.bzl` repository's
  final mapping, then load the resolved canonical repository through ordinary
  package/source owners; and
- the BCR Starlark files remain the complete rules/control-flow owners,
  including `cc_internal`.

The live Slug architecture already contains candidate generic owners:

- `HostSelectedExtensionMappingProjection` merges root and nonroot
  `use_extension`/`use_repo` projections into selected repository
  mappings;
- `HostCanonicalSelectedModuleDefinitionView::mapping`,
  `HostCanonicalRepositoryRoute::mapping_target`, and
  `RootRepositoryRoute::selected_bzl_load_route` expose typed mapping
  projections;
- `HostCanonicalRepositoryLoadRouteKey` and its observed sibling own
  canonical selected/generated child routes;
- `ExternalBzlModuleEvalKey` retains the complete
  `HostRepositorySourceRoute` and already routes same-repository and admitted
  cross-repository children without a second evaluator; and
- repository-owned loading tests already prove the exact
  `cc_compatibility_proxy` canonical identity, source presentation,
  repository mapping, child provider identities, and public re-export
  behavior when supplied a resolved child.

The unresolved fact is precise: determine whether the real rules_cc route
loses the generated mapping before `resolve_canonical_external_bzl_load_label`,
or retains the mapping but fails while projecting the canonical child route.
Do not select an implementation owner before that trace.

Clean `../zabel` commit
`0795445f3ab60f4e49070bdd0b94425c5610f73a` is peer architecture and
optimization guidance only. Audit its repository-mapping/source-session
separation for a useful ownership lesson, but do not copy its representation,
scheduler, or compatibility claims.

## Compatibility classification

- **Exact:** apparent repository names in `load()` resolve in the defining
  module's Bazel 9 final repository mapping; the resolved canonical label,
  missing-repository diagnostic/order, recursive child graph, source
  presentation, Legacy/Observed outcomes, and repository/provider identities
  match admitted Bazel behavior.
- **Slug-native:** Rust route/mapping carrier types, DICE key names, structural
  hashing, and immutable `Arc` layout.
- **Unsupported/deferred:** repositories absent from the defining mapping,
  unrelated extension/generated-repository breadth, physical materialization,
  C++ action semantics, configured testing/coverage invocation, exact
  Java/HotSpot state, and later action families.

## Design audit

Run a read-only owner trace before freezing implementation:

1. Record the real rules_cc Legacy and Observed external-module route kinds,
   full mapping rows, and exact child-resolution stop. Discriminate mapping
   absence from canonical child-route failure.
2. Trace rules_cc's parsed nonroot extension usage through
   `HostSelectedExtensionMappingProjection`, selected route publication,
   canonical route/load-route publication, and the evaluator's retained
   `BzlModuleIdentity.repository_mapping`.
3. Prove the generated canonical repository
   `rules_cc++compatibility_proxy+cc_compatibility_proxy` has one existing
   generic selected/generated load route or identify the smallest missing
   owner. Do not synthesize a RepoSpec or source path in loading.
4. Record exact before/after dependency rows for same-repository loads,
   selected module-dependency loads, built-in loads, extension-generated loads,
   missing mappings, Legacy mode, Observed frontier union, cycles, and
   overlapping requests.
5. Audit pinned Bazel 9 repository-mapping/load resolution and existing tests.
   Add an oracle only for a demonstrated evidence gap.
6. Inspect clean Zabel only for mapping/source ownership and retained-memory
   ideas; Bazel remains authority.

## Candidate architecture constraints

The eventual design must keep mapping ownership in Bzlmod and source/evaluator
ownership in loading:

- one typed route-owned operation resolves an apparent child repository to a
  canonical child route;
- loading consumes that projection without inspecting `rules_cc`,
  `cc_compatibility_proxy`, source kind, module-extension name, or canonical
  spelling;
- the defining source identity retains its complete final mapping for Label
  construction and evaluator calls;
- Legacy and Observed modes use the same semantic child route, with Observed
  mode additionally unioning the existing route/source frontier;
- missing or ambiguous mappings fail closed before source access;
- child source bytes remain owned by existing canonical source-observation
  keys with no copy or filesystem fallback; and
- Root-request dependency identities accepted in `1b997c5ef` remain exact.

Prefer convergence on the existing canonical load-route owner if the trace
proves it already represents extension-generated repositories. If it does not,
`REPLAN` to the natural Bzlmod mapping/route producer rather than adding a
loading-side lookup table.

## Frozen design inputs

No Rust edit is authorized in this design packet. Audit these current blobs:

- `app/slug_loading_v2/src/bzl_module.rs`
  `bd7b919bca8ed905f9da88f0705825c608fceb2d`;
- `app/slug_bzlmod_v2/src/canonical_repository_route.rs`
  `42458a059436e9920948263314ddc03b5406e084`;
- `app/slug_bzlmod_v2/src/selected_repo_spec.rs`
  `05387c9c888118421f1aa087eb8ada006a3a32e6`;
- `app/slug_bzlmod_v2/src/host_module.rs`
  `504885b531d6deb2874102aac4b125a3dbfe2ba0`;
- `app/slug_loading_v2/src/host_package_load_tests.rs`
  `df4b09edc826607f0b8de9fe4cf944430bc6f015`; and
- `app/slug_loading_v2/src/canonical_repository_load_route_tests.rs`
  `6f4be47551650325525c07217a3c9672ce12047c`.

The terminal design must name exact owners/blobs, public/private boundary,
DICE dependencies/order, revision/cancellation behavior, error projection,
memory lifetimes, proofs, caps, and deletions. Independent architecture review
is mandatory before implementation because this crosses Bzlmod route identity
and recursive loading.

## Stops

`REPLAN` for a `cc_common`, rules_cc, compatibility-proxy, extension-name,
repository-name, canonical-string, or path special case; parser/`set`
changes; Rust-defined rule semantics; a second mapping graph; loading-owned
RepoSpec synthesis; physical materialization; copied source bytes; weakening
missing/ambiguous mapping errors; changed exact Root-request children; work in
parked dirty files; or implementation before reviewed architecture.
