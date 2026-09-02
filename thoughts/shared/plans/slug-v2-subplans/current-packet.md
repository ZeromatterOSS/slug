# Current Slug V2 Packet

Packet: WP-4-5-7A-repository-source-glob-routing-category-implementation-r1

Milestone: M7A bootstrap-critical generic Starlark/loading and repository
closure. Make every admitted repository package source kind feed the same
Bazel 9.2 BUILD `glob()` evaluator through its natural directory-fact owner.

Status: initial DICE/ownership review returned `REVISE` on integrated catalog-
boundary and fail-closed projection proof plus one overbroad equivalence claim.
The corrected contract requires those discriminators and narrows the claim;
focused rereview returns `ACCEPT`. Rust is authorized only within this packet.
The predecessor complete package-context dependency-label category is
terminally accepted in `5f9f9a98a`.

The unrelated dirty
`app/slug_loading_v2/src/registration_expansion_tests.rs` proof remains parked
at SHA-256
`36c937d49369ac57e51defe2b17d4a53636a815ec0b2d407f7bd1a664c4d816a`.
Do not edit or stage it.

## Trigger and learned facts

The fresh authenticated rules_rust 0.73 replay clears the package-context
label category and stops while loading verbatim
`@@bazel_tools//tools/res:BUILD`: both `glob(["**"])` and `glob(["*.bzl"])`
reach `RepositoryPackageLoadErrorInner::GlobUnsupported`. This is not a
rules_rust, rules_cc, C++, `cc_common`, `cc_internal` or `tools/res` semantic
gap. `RepositoryPackageSourceAddress::Host` already enters the complete Host
glob attempt driver; `BuiltinCatalog` evaluates once with an empty prepared
request map and turns the first pending request into that unsupported error.

The complete recursive BUILD-glob category is already exact and accepted in
`cfe83834d`. Reuse its pinned Bazel 9.2.0 commit
`8220c6198837d5c13d53fea211cf3282aa12408a` and source/test basis:

- `StarlarkNativeModuleApi.java`, `StarlarkNativeModule.java`,
  `GlobberUtils.java` and `BuildLanguageOptions.java` own the callable,
  include/exclude and empty-result contract;
- `GlobComputationProducer.java`, `FragmentProducer.java`,
  `DirectoryDirentProducer.java`, `GlobFunctionWithMultipleRecursiveFunctions.java`,
  `GlobValue.java`, `GlobsValue.java`, `GlobCache.java`, `UnixGlob.java`,
  `PackageLookupFunction.java` and `IgnoredSubdirectoriesFunction.java` own
  traversal, directory membership and package/ignore boundaries; and
- `GlobTestBase`, `GlobCacheTest`, `GlobTest`, `PackageFunctionTest` and the
  existing `glob-package-boundaries`, `glob-callable-contract`,
  `glob-directory-invalidation` and `glob-raw-name-pattern-lazy` fixtures
  discriminate the admitted behavior. No new oracle or fixture is needed.

Pinned Bazel packaging and Slug's verbatim catalog supply the source facts.
`src/create_embedded_tools.py` defines embedded path selection; the catalog
pins `tools/res/BUILD` at
`bef477365d864eab46fcfe73c635bafd11a7300e4e47c158abe20d269e07e8ac`,
its three `.bzl` children, and the recursive `src/tools/launcher` tree plus
`util/BUILD` package boundary. `BuiltinBazelToolsDirectoryListingKey` validates
the complete manifest before returning deterministic immediate file/directory
entries. `HostRepositoryDirectoryListing{,Observation}Key` already routes a
built-in repository identity to that catalog key with an empty Host
observation epoch. `HostExternalPackageBoundary{,Observation}Key` already uses
the same route to detect catalog BUILD boundaries.

## Decision and natural ownership

Retain one `GlobPattern`, `HostGlobLoadingRequest`, attempt/retry loop,
traversal state machine and `PackageRecorder::host_glob` output projection.
Extend the existing traversal scope with a distinct catalog-external variant:

1. root and materialized/Host external scopes keep the current
   `HostGlobSegmentCandidates{,Observation}Key` path resolution, raw-name and
   symlink behavior unchanged;
2. catalog-external scope filters each requested segment from
   `HostRepositoryDirectoryListing{,Observation}Key` for the traversal state's
   repository-relative `PackagePath`;
3. both external variants use the same
   `HostExternalPackageBoundary{,Observation}Key` and exact recursive traversal;
4. `RepositoryPackageSourceAddress` selects Host versus catalog scope before
   evaluation, and both sources use `evaluate_host_package_attempts_driver`;
   and
5. delete the now-unreachable `GlobUnsupported` branch rather than preserving
   a fallback.

The catalog listing key is the sole directory-membership producer. The
external boundary key is the sole subpackage producer. The existing traversal
key remains the sole pattern/operation/package/source-route result owner, and
the loaded package remains the sole final glob consumer. Do not copy the
catalog into another tree, synthesize a filesystem root, materialize built-in
files, scan `CATALOG` from loading, or add a key, cache, interner, registry,
lock, task or evaluator-retained value.

The traversal core becomes platform-independent for catalog scope. Host
segment computation remains at its existing Unix gate and existing
unsupported result elsewhere. Convert one catalog component exactly by first
requiring a valid Rust-Unicode `OsStr`, then mapping each scalar U+0000..U+00FF
to the equal single byte used by the existing Bazel-internal matcher; reject an
invalid-Unicode OS name or any scalar above U+00FF. `PathDirectoryName` already
forbids empty, dot, separator and multi-component names. A future catalog
containing either rejected component form, a symlink or an unknown entry kind
returns a typed traversal error before any match slice is published.

## Compatibility classification

Admit as **exact** for the named Bazel 9.2 successful surface:

- all already-accepted include/exclude, recursive `**`, files versus
  files-and-directories, ordering, deduplication and empty-result behavior over
  pinned built-in catalog entries;
- immediate file/directory membership from the exact catalog manifest;
- BUILD/BUILD.bazel package-boundary pruning through the existing external
  package lookup; and
- exact frozen target labels and filegroup membership for the named catalog
  packages and patterns.

Keep **Slug-native**:

- the manifest-digest route identity, Rust DICE key/value layout, synthetic
  logical path used only in internal diagnostics, immutable `Arc` scratch and
  complete-only DICE equality cutoff; and
- fail-closed catalog-name/entry-kind diagnostics rather than Bazel internal
  Java error text for a state absent from the pinned manifest.

Keep **unsupported/deferred**:

- mutable or user-authored built-in catalogs, catalog symlinks/unknown entry
  kinds/non-Latin-1 names, exact impossible-state diagnostic wording, and
  catalog content not present in the pinned Bazel 9.2 manifest;
- non-Unix Host filesystem glob traversal, whose existing boundary is not
  widened by platform-independent catalog traversal; and
- ruleset, toolchain, configured analysis, action/execution or
  `cc_common`/`cc_internal` behavior beyond ordinary downstream consumption.

## DICE identity, revision and memory

The traversal key already hashes/equates workspace, scope, logical root,
package, parsed pattern and operation. The new scope discriminant plus existing
`HostRepositorySourceRoute` prevents Host/catalog or cross-route collisions;
the route already contains the built-in snapshot and exact manifest digest.
Every listing and boundary is computed through the caller's DICE context, so
dependencies, Need propagation, cancellation and equality cutoff remain
producer-owned. Hold no lock across a compute.

The built-in snapshot is a closed single-variant input, so a fabricated
catalog A/B/A mutation is inapplicable. Prove instead that Host and catalog
scope keys are unequal, catalog traversal directly depends on the route-owned
listing/boundary keys, repeated calculation reuses the complete result, and no
Host path-observation or repository-materialization key activates. A later
real snapshot variant must change `BuiltinBazelToolsRouteIdentity` and will
therefore invalidate the existing dependency chain without this packet adding
revision state.

No retained collection is added. The scope adds one enum discriminant around
the already-retained route. Catalog candidate vectors/arcs are traversal-phase
scratch and are dropped after the existing compact sorted match slice is
published. Existing catalog listings, glob patterns, routes and package values
retain their current DICE lifetimes and release on invalidation/eviction or
service shutdown. There is no command, transfer-owned async or service-cache
memory.

## Buck2/starlark-rust and Zabel guidance

No V1 or Buck2 extraction is required. Preserve the accepted Buck2-derived
`Arc` slices, `Dupe`, `Allocative`, compact `SmallSet` traversal state and
complete-only DICE equality. Do not replace them with owned graph `String`/
`Vec`, `HashMap`, a second listing tree or an interner. Record this no-extraction
decision in Stage 9.

Clean Zabel `0795445f3ab60f4e49070bdd0b94425c5610f73a` is peer guidance only.
Its `session_package_glob_computation.zig` keys one package glob by canonical
package/source identity and visits source-owned directory facts; its
`session_source_directory_key.zig` keeps a source root and relative directory
in the directory key. Adopt only that one-producer/source-routed-listing
lesson. Copy no Zig key, packed allocator, matcher, scheduler, cache, error,
order, symlink limit or behavior claim. Bazel 9.2 remains sole authority.

## Evidence and proof

Add focused proof that:

- otherwise-identical Host-external and catalog-external traversal scopes are
  structurally unequal while restored catalog keys compare equal;
- loading `@@bazel_tools//tools/res` evaluates both `glob(["**"])` and
  `glob(["*.bzl"])` to the exact three catalog `.bzl` labels;
- the observed load depends on built-in catalog directory-listing keys,
  carries no Host observations, activates no path-listing/materialization key,
  and reuses its warm complete value;
- an integrated test-only catalog traversal over
  `@@bazel_tools//src/tools/launcher:**` activates the external-boundary key for
  `src/tools/launcher/util`, excludes `util/BUILD` and every path beneath that
  subpackage, and uses no stub/copy of the catalog or BCR source;
- pure catalog-listing projection rejects invalid-Unicode and above-U+00FF
  components plus `Symlink` and `Unknown` entry kinds, with no partial matches;
  and
- Host root/external raw-name, symlink, recursive, Need, error-order and A/B/A
  tests remain unchanged.

The authentic rules_rust replay must clear `@@bazel_tools//tools/res` and stop
at the next honest generic boundary. It is acceptance evidence, not authority
for a ruleset-specific branch.

## Allowlist, caps and complexity

Production Rust may change only:

- `app/slug_loading_v2/src/bzl_module.rs`;
- `app/slug_loading_v2/src/host_glob/mod.rs`; and
- `app/slug_loading_v2/src/host_glob/traversal.rs`.

Proof may change only those files' existing test modules plus:

- `app/slug_loading_v2/src/host_glob/traversal_tests.rs`; and
- `app/slug_loading_v2/src/canonical_repository_load_route_tests.rs`.

Scheduling records may change only the canonical plan, owner plans 04/05/06,
Stage 9 and this manifest. Caps are 300 gross added production Rust lines, 420
proof lines and 720 total. No new function may exceed 150 lines.

`bzl_module.rs` and `canonical_repository_load_route_tests.rs` exceed the
2,000-line trigger. The production file may only replace its source-kind glob
branch and remove the obsolete error; the test file already owns built-in route
and listing proof. `host_glob/{mod,traversal}.rs` remain the cohesive segment
and traversal owners; do not move unrelated loading behavior into them. No
demonstrated hot-path or retained-memory growth warrants a benchmark: catalog
membership is a fixed small manifest, every listing is already DICE-cached,
and the packet adds no retained collection.

## Validation and stops

Run serially:

- focused traversal identity and built-in catalog package-load tests;
- `cargo test -p slug_loading_v2 --lib -q` and all loading integration tests;
- `cargo test -p slug_bzlmod_v2 --lib -q` for the consumed listing/boundary
  owners;
- `cargo test -p slug_query_v2 --lib -q`;
- `cargo check -p slug_loading_v2 --target x86_64-pc-windows-gnu -q`;
- `cargo build -p slug_cli_v2 -q` before authentic replay;
- authentic rules_rust configured-query replay with stale `slugd` cleanup
  before and after;
- `cargo fmt --check`, `git diff --check`, archive checker and parked-proof
  SHA-256 verification.

Return `REPLAN` before or during Rust if:

- catalog matching needs a second glob parser/traversal, copied inventory,
  materialized filesystem root, new DICE key, fallback scan or direct catalog
  access from loading;
- Host scope changes its path/symlink/observation behavior, or catalog scope
  lacks its route/listing/boundary dependency in structural identity;
- a catalog result can publish after Need, cancellation, listing/boundary
  failure, either invalid component class or either unsupported entry kind;
- exact behavior requires a new oracle/fixture or unpinned `@bazel_tools`
  content;
- any `tools/res`, rules_rust, rules_cc, toolchain, C++, `cc_common` or
  `cc_internal` specialization appears;
- cross-target catalog compilation fails without a bounded source-neutral
  correction; or
- production/proof caps or file allowlists are exceeded.

Rust begins only after independent review accepts the source-scope identity,
existing-key dependency graph, platform split, proof matrix and bounds.

Focused independent correction rereview returns `ACCEPT`. The integrated
catalog-boundary proof, exact component lifting and four fail-closed negative
rows, narrowed named-catalog output claim, existing-key ownership, platform
split, allowlist, caps and stops are accepted.
