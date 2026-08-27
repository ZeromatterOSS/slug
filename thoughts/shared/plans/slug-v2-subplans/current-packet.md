# Current Slug V2 Packet

Packet: `WP-4-5-7A-routed-repository-directory-listing-owner-implementation`

Milestone: M7A command/ruleset bootstrap closure feeding ordinary M8 Stage
10.3 analysis.

Base: `ecf1f76e8`.

Result: implement one policy-free bzlmod DICE owner for direct directory
entries below an authenticated repository route. Cover direct-local,
selected-registry, generated and built-in sources through one public loading
contract. Activate no recursive traversal, target-pattern expansion or
registration.

## Frozen architecture

The accepted design REPLAN is recorded in
`06-analysis-toolchains-and-actions.md`. `RootRepositoryRoute` plus a validated
root-capable `PackagePath` is the semantic key. Retain the complete route;
never infer a source from apparent/canonical repository text or reconstruct a
physical repository root.

Implement doc-hidden legacy and observed
`HostRepositoryDirectoryListingKey` siblings in bzlmod source preparation.
Their semantic value distinguishes `Present(PathDirectoryEntries)` from
`Missing`. Reuse the workspace types' sorted immutable `Arc` entry slice,
names and kinds. Return only repository-relative entries and typed projected
errors; expose no observation namespace, materialization/generation root,
resolved/real path, resolver chain or catalog internals.

For direct-local, selected-registry and generated routes, privately consume
the existing `RepositoryMaterializationResultKey`, select its owned Host or
Materialization observation namespace, and compute
`PathDirectoryListingKey`/`PathDirectoryListingObservationKey`. Translate
physical-path errors to repository-relative semantic errors before crossing
the bzlmod boundary.

For built-in `@bazel_tools`, add a snapshot-keyed catalog directory-listing
projection. It accepts the root and nested package paths, validates catalog
identity, returns missing for an absent prefix and wrong-kind for a catalog
file, and produces sorted **unique** direct children. Explicitly coalesce
repeated prefixes such as `tools` before `PathDirectoryEntries::new`, which
sorts but does not deduplicate. Never invent a filesystem root for the built-in
snapshot.

The listing key owns no package policy. `--deleted_packages` suppresses only a
current package and repository-ignore matches prune subtrees; the existing
point external lookup collapses them to `Deleted`. Do not consume, alter or
copy that policy here. A later selected-external subtree packet will add one
typed bzlmod package-boundary projection before loading traversal.

## DICE and retained-state contract

Legacy returns `SourcePreparationOutcome<Result<Value, Error>>`. Observed also
retains `PathObservationEpoch` and admits `ObservedPathFrontierError`. Preserve
observed outer error before Need before terminal precedence, complete-only
equality/validity and ordinary cancellation. A materialization or path Need is
not a complete cache value. No lock may span a DICE compute.

The key accepts an already authenticated route. Its observed epoch owns only
directory resolution/listing facts; a caller of
`RootRepositoryRouteObservationKey` must later merge the route epoch. Built-in
catalog listings add no path observation because snapshot/manifest identity is
structural in the route.

Reuse `PathDirectoryEntries`, `PathDirectoryEntry`, `PathDirectoryName`,
`PathDirectoryEntryKind`, immutable `Arc` slices, `Dupe` and `Allocative`.
Add no alternate entry rows, interner, global cache, mapping/source copy,
manual lock or utility-ledger row.

## Compatibility

- **Exact:** no new named Bazel surface is activated. Repository
  content/catalog integrity remains exact for Slug's actual graph.
- **Slug-native:** Rust/DICE key and observation carrier shape, projected
  repository-relative errors, materialization namespaces and retained entry
  representation.
- **Unsupported/deferred:** selected-external recursive package membership,
  ignore/deleted boundary projection, target-pattern expansion, wildcard-name
  conflict lookup, family filtering/dedupe, registration activation,
  configured validation, options, rule implementations and actions.

This is general Starlark/loading infrastructure. Bazel 9 BCR Starlark remains
the source of rules including `cc_internal`; `cc_common` is only a later host-
capability consumer. Zabel's authenticated-source/loading-owner separation is
peer guidance only. Copy no Zabel type, session store, allocator, diagnostic or
compatibility claim.

## Allowlist and caps

Change only:

- `app/slug_bzlmod_v2/src/builtin_repository.rs`;
- `app/slug_bzlmod_v2/src/source_preparation.rs`;
- `app/slug_bzlmod_v2/src/lib.rs`; and
- `app/slug_bzlmod_v2/src/source_preparation_observation_tests.rs`.

Caps are 560 production and 700 proof additions. Add no dependency, fixture or
oracle. The large source-preparation owner remains cohesive because its private
materialization result and physical-root projection must not be widened into a
new module.

## Required proof

Direct tests must cover:

- route/package key/hash A/B/A discrimination and root identity;
- root and nested present, missing and wrong-kind listings;
- lexical direct-child order and exact unique built-in root/nested child sets,
  including repeated-prefix coalescing and manifest/catalog identity;
- direct-local create/delete/recreate and symlink retarget observation;
- selected-registry immutable generation/source identity and generated file
  effects;
- all four route dispositions, materialization/path Need replay, observed outer
  error, complete-only equality/validity and route A/B/A; and
- absence of a public physical root/namespace and absence of package-policy or
  traversal ownership in the new key.

Run formatting, focused new tests, full `cargo test -p slug_bzlmod_v2`,
downstream `cargo check`/focused tests for `slug_loading_v2`, locked core checks
and `cargo build -p slug_cli_v2`. Cargo commands sharing the target directory
must be serial. Run scope, cap, dependency, helper, no-lock, diff and archive
gates. No new Bazel oracle is required because no recursive or public behavior
is activated.

Require independent retained-DICE/source-boundary review before terminal
`ACCEPT`.

## Stops

STOP and `REPLAN` for cap or allowlist expansion, another retained entry
representation, any exposed/reconstructed physical root or namespace, copied
mapping/source tree, package-policy change, traversal/registration activation,
new exact claim, dependency, global state, lock across DICE compute, or a route
source kind that cannot use this single contract.

## Immediate predecessor

Commit `ecf1f76e8` accepts the docs-only selected-external subtree owner design
as a `REPLAN` to this missing routed listing primitive. Independent review
accepted the root-capable `PackagePath`, built-in prefix deduplication,
deleted-versus-ignore separation and implementation limits.
