# Current Slug V2 Packet

Packet: `WP-4-5-7A-repository-qualified-external-bzl-load-route-design`

Milestone: M7A registered-toolchain closure prerequisite.

Base: accepted complete direct `tools/build_defs/cc` catalog `84190d95c`,
accepted external-`.bzl` source-observation cutover `1b997c5ef`, accepted
TestingBootstrap ABI `ecee4aca5`, and accepted selected-BCR realization
`1599d730c`. The proof-only registration and selected-context candidates
remain dirty, parked, and read-only.

Independent architecture review returned `ACCEPT` after requiring explicit
ambiguity and unchanged-Root-dependency proofs. This reviewed design now
authorizes only the bounded implementation below.

## Observable boundary

Two fresh-workspace/fresh-output-root rules_rust cqueries clear the complete
pinned Bazel 9.2 `tools/build_defs/cc` package and stop identically at:

```text
loading `@rules_cc//cc/common:cc_common.bzl`:
resolving a load in @@rules_cc+//cc/common:cc_common.bzl:
repository-qualified external load is deferred:
@cc_compatibility_proxy//:symbols.bzl
```

This is a generic repository-qualified external-`.bzl` route boundary. The
evaluator already parses and freezes the exact public rules_cc facade and
compatibility-proxy provider graph in repository-owned tests. It is not a
`cc_common`, `cc_internal`, parser, `set`, provider-shape, C++ rule-engine,
or source-catalog failure. Bazel 9/BCR Starlark remains the complete rule and
control-flow owner.

## Learned facts and authority

Bazel 9.2 and selected BCR bytes are sole semantic authority:

- `BzlLoadFunction.computeInternalWithCompiledBzl` resolves every direct load
  relative to the current file repository's mapping before constructing child
  load keys;
- `BzlLoadFunction.getRepositoryMapping` obtains the full mapping from
  `RepositoryMappingFunction` for ordinary BUILD and Bzlmod `.bzl` loads;
- `BazelModuleContext.repoMapping` retains the mapping applicable to the
  repository containing the `.bzl` file;
- `BzlLoadFunctionTest.testLoadBzlFileFromBzlmod` proves a nonroot module's
  apparent alias resolves to the dependency canonical repository and records
  that exact mapping use; and
- `ModuleExtensionRepoMappingEntriesFunction` states that a generated
  repository sees sibling generated repositories and the repositories visible
  to the module hosting the extension.

The real rules_cc 0.2.17 route supplies the discriminating extension-generated
case without another oracle fixture:

- `MODULE.bazel` declares
  `compat = use_extension("//cc:extensions.bzl", "compatibility_proxy")` and
  `use_repo(compat, "cc_compatibility_proxy")`;
- exact `cc/common/cc_common.bzl` hash
  `65e91cf0fa7ebb1c8efc84bbf6b1c4ec4db46f5e5ed4606759aa4a45a23b4063`
  loads `@cc_compatibility_proxy//:symbols.bzl`;
- the final mapping retains
  `rules_cc+,cc_compatibility_proxy ->`
  `rules_cc++compatibility_proxy+cc_compatibility_proxy`; and
- the generated repository mapping retains `rules_cc -> rules_cc+`.

The read-only Slug owner trace is conclusive:

1. `HostSelectedExtensionMappingProjection` includes nonroot extension usages
   and merges their `use_repo()` imports into each selected owner mapping.
2. `HostCanonicalSelectedModuleDefinitionView::mapping` and
   `RootRepositoryRoute::bzl_repository_mapping` retain that final mapping.
   `selected_registration_patterns_borrow_final_generated_mapping` already
   proves the retained generated target and predecessor identity.
3. `HostCanonicalSelectedModuleDefinition::mapped_bzl_load` first finds the
   exact generated canonical target, then collapses it to `None` because
   `find_canonical_route_ordinal` searches selected module rows only.
4. Consequently the observed `ExternalBzlModuleError::LoadLabel` occurs before
   any canonical child-route compute. A canonical route failure would instead
   be `ExternalBzlModuleError::Route`.
5. `HostCanonicalRepositoryLoadRouteKey` already accepts a canonical name,
   tries selected then generated route ownership, executes a generated effect
   when present, constructs the existing source input, and has Legacy and
   Observed variants. Existing recursive-route tests prove generated-parent to
   generated-sibling routing, Need/error disposition, and exact Observed
   route/effect frontier union.

Clean Zabel commit `0795445f3ab60f4e49070bdd0b94425c5610f73a` is
**concept/test only** peer guidance:

- `selected_extension_import_repo_views.zig` completes owner mappings with
  graph-assigned generated imports before execution;
- `session_natural_bzl_repository_source.zig` joins a producer-owned complete
  mapping with the selected materialized source root; and
- `session_bzl_module_source_computation.zig` borrows the retained mapping into
  evaluation scratch and releases temporary resolution rows before child
  demand.

Do not reuse Zabel code or representation. Avoid its scheduler, registration
model, stores, mapping digest, and physical-source machinery. The useful lesson
is only the same ownership split already natural in Slug: Bzlmod owns final
mapping/route identity; loading owns source observation and evaluation.

## Decision and compatibility

Add one doc-hidden typed Bzlmod projection:

```text
RootRepositoryBzlLoadRoute
  Root(RootRepositoryRoute)
  Canonical(CanonicalRepoName)
```

`RootRepositoryRoute::selected_bzl_load_route` continues to return the exact
existing `Root` route for a uniquely selected registry module or built-in
`bazel_tools`. When the final mapping contains a canonical target without such
a Root representation, it returns `Canonical`; this includes extension-
generated repositories and uses no spelling/source-kind special case. An
absent apparent mapping or ambiguous selected-route representation remains
`None` and fails closed before source access.

Loading consumes the projection as follows:

- `Root` uses the accepted synchronous path and exact Root child identity;
- `Canonical` calls the existing
  `compute_canonical_external_child_input`, selecting
  `HostCanonicalRepositoryLoadRouteKey` in Legacy mode or
  `HostCanonicalRepositoryLoadRouteObservationKey` in Observed mode;
- the returned existing `HostCanonicalRepositorySourceInput` becomes the child
  `HostRepositorySourceRoute::Canonical`; and
- recursive evaluation, cycles, source presentation, providers, and child
  error wrapping continue through the single existing evaluator.

Compatibility classes:

- **Exact:** apparent repository lookup in the defining repository's final
  Bazel 9 mapping; resolved canonical label and repository identity; absent
  mapping failure before source access; mapped canonical route/effect failure
  ordering; Legacy result; Observed route/effect/source frontier union; child
  graph, cycles, and provider identity; unchanged selected-module and built-in
  Root paths.
- **Slug-native:** the Rust projection enum, DICE key/type names, structural
  equality/hashing, `Arc` layout, and diagnostic carrier internals.
- **Unsupported/deferred:** names absent from the defining mapping, physical
  materialization beyond existing repository owners, unrelated repository
  breadth not reached by the proof, C++ action semantics, configured
  testing/coverage invocation, exact Java/HotSpot state, and later action
  families.

Non-decisions: no parser or builtin work; no Rust-defined rule semantics; no
second mapping graph; no apparent-to-canonical DICE key; no loading-side
RepoSpec/source/path synthesis; no physical fallback; no copied source bytes;
no extension/repository/ruleset special case; and no change to the existing
canonical load-route or source-input representation.

## DICE, request, and lifetime contract

No DICE key is added or changed. The new generated-target dependency row is:

```text
ExternalBzlModuleEvalKey (Root defining route)
  -> HostCanonicalRepositoryLoadRouteKey
       -> HostCanonicalRepositoryRouteKey
            -> selected definition, then generated definition when absent
       -> HostSelectedRepositoryFileEffectKey only for a generated route
  -> ExternalBzlModuleEvalKey (Canonical child route)
       -> existing canonical source owner
```

Observed mode substitutes the observation siblings and unions their exact
frontiers before child source demand. Same-repository, selected-registry, and
built-in loads retain their current direct dependency rows. `Need` remains
non-cache-valid; complete equality remains structural through existing route,
mapping, RepoSpec/effect, request, and source-input carriers.

The defining `RootRepositoryRoute` already structurally retains the complete
selected definition and final mapping. A mapping revision therefore changes
the parent evaluator key; canonical route/effect/source revisions invalidate
through their existing dependencies. A/B/A restoration must reproduce route
and result equality. Overlapping requests deduplicate through DICE; no mutex,
side cache, global registry, task, or await-under-lock is introduced.
Cancellation and cycle behavior remain owned by existing DICE computations and
the existing `ExternalBzlLoadCycleGuard`; incomplete work is not published.

The projection enum and duplicated parse result for the exceptional deferred
branch are evaluation scratch. Existing selected definitions, mappings,
canonical source inputs, frozen modules, and observation frontiers keep their
current DICE-retained lifetimes. No retained value borrows evaluator scratch;
no new service cache, eviction, shutdown, or transfer-owned memory exists.

## Errors and proof matrix

- Missing apparent mapping: existing `ExternalBzlModuleError::LoadLabel` and
  zero canonical child-route/source activation.
- Ambiguous selected-route representation: same fail-closed boundary and zero
  source activation.
- Mapped target with missing/invalid canonical route or failed generated
  effect: existing `ExternalBzlModuleError::Route`, preserving the route/effect
  Observed prefix and performing no child source read.
- Mapped target with a valid generated route: exact canonical child label,
  source presentation, provider/re-export result, and canonical source owner.
- Child parse/evaluation/freeze error: existing `Child` wrapping and canonical
  identity.
- Cycle: existing structural route/label cycle identity and poison behavior.

Repository-owned tests must prove:

1. the Bzlmod projection preserves selected-module and `bazel_tools` Root rows,
   returns `Canonical` for a nonroot extension-generated final mapping, returns
   `None` for absent mapping and a duplicate-canonical selected-row
   representation, and is structural under mapping A/B/A;
2. a selected-registry `.bzl` parent loads one extension-generated sibling in
   Legacy and Observed modes through the canonical load-route owner;
3. exact child canonical identity, provider value, route/source dependency
   rows, and Observed frontier union;
4. absent mapping and duplicate-canonical selected rows activate neither a
   canonical load-route key nor any canonical child source owner;
5. generated effect failure is `Route` and preserves its observed prefix; and
6. dependency traces for same-repository, selected-registry dependency, and
   built-in loads retain their direct Root child keys and activate no canonical
   load-route key; accepted cycle, overlapping-request, Need, and error tests
   remain green.

Use existing synthetic registry, extension, materialization, dependency-trace,
and observation-epoch scaffolding. No new copied fixture subtree or external
fixture file is admitted, so `fixture.toml` provenance is inapplicable. Reuse
the accepted two cold real rules_rust replays as the exact integration oracle;
add no Bazel oracle unless implementation exposes a behavior not covered by
the pinned source/test anchors and real mapping evidence above.

## Allowlist, caps, validation, and stops

Frozen source blobs:

- `app/slug_bzlmod_v2/src/host_module.rs`
  `504885b531d6deb2874102aac4b125a3dbfe2ba0`;
- `app/slug_bzlmod_v2/src/selected_repo_spec.rs`
  `05387c9c888118421f1aa087eb8ada006a3a32e6`;
- `app/slug_bzlmod_v2/src/lib.rs`
  `f258307feea1ef0b4a5352071994f54be7999eb8`;
- `app/slug_loading_v2/src/bzl_module.rs`
  `bd7b919bca8ed905f9da88f0705825c608fceb2d`; and
- `app/slug_loading_v2/src/canonical_repository_load_route_tests.rs`
  `6f4be47551650325525c07217a3c9672ce12047c`.

Only those five Rust files may change after review. Cap production growth at
80 lines total and test growth at 320 lines. No new crate, dependency, key,
fixture file, unsafe code, background task, lock, cache, fallback, or public
stability shim.

The three touched production owner files exceed the 2,000-line review trigger.
Keep the change colocated because each edit is a bounded extension of its
existing cohesive mapping projection or recursive-load resolver; splitting
would separate a variant from the sole matcher or a route branch from the
single evaluator. Do not add another responsibility or touch a function over
150 lines. The surface is not a demonstrated hot-path or retained-
representation change, so performance measurement and Buck2 utility adoption
are inapplicable.

Validation after implementation:

1. targeted Bzlmod projection tests;
2. targeted canonical load-route/external-module tests in both modes;
3. complete `slug_bzlmod_v2` and `slug_loading_v2` suites;
4. one direct compile dependent and `cargo build -p slug_cli_v2`;
5. clean stale `slugd`, then two fresh-workspace/fresh-output-root real
   rules_rust cqueries with identical outcome and no
   `@cc_compatibility_proxy` route failure;
6. `cargo fmt --check`, `git diff --check`, cap/allowlist/dirty-isolation
   checks; and
7. independent terminal patch review.

`REPLAN` for a `cc_common`, rules_cc, compatibility-proxy, extension-name,
repository-name, canonical-string, or path special case; parser/`set` changes;
Rust-defined rule semantics; a new key or mapping graph; loading-owned RepoSpec
or source synthesis; physical materialization work; copied bytes; weakened
missing/ambiguous mapping failure; changed selected/built-in Root dependencies;
work in parked dirty files; cap/allowlist overflow; or a second material
architecture correction.
