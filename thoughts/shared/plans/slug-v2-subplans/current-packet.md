# Current Slug V2 Packet

Packet: `WP-4-5-7A-repository-aware-loading-package-carrier`

Milestone: M7A command/ruleset bootstrap closure feeding ordinary M8 Stage
10.3 analysis.

Base: `104291321`.

Result: add one loading-owned Root/Canonical package carrier keyed by workspace
and full `PackageIdentifier`, plus an observed sibling. Compose only accepted
route/package owners, retain their exact result `Arc`s, and leave analysis and
registration expansion behavior unchanged.

## Accepted architecture

Commit `104291321` freezes the configured-consumer sequence. The live analysis
path reparses root-only exact registrations, keys native packages only by
`PackagePath`, rejects canonical native references and loads configured rule
packages through a root-only input. A direct consumer cutover would collapse
repository identity or introduce analysis-side source routing.

This packet implements only sequence step 1. Later packets converge configured
package identity and then consume both expansion families. The exact bypass
predicate remains
`!has_toolchain_requirement && local_declarations.is_empty()`; no analysis code
changes here.

## Authority and peer guidance

Pinned Bazel 9.2 package/repository and configured-toolchain sources remain
behavioral authority. This carrier adds no new Bazel-visible behavior: it
composes Slug's accepted Root package and canonical route/inventory owners.

Clean Zabel `0795445f3ab60f4e49070bdd0b94425c5610f73a` remains peer guidance
only. Its configured registration graph and loaded-native-toolchain input
projection support keeping repository-aware loading facts separate from
configuration/type-keyed selection. Copy no Zig type, store ID, registration
family, allocator pattern, diagnostic or compatibility claim.

BCR Starlark continues to own every rule definition and control-flow surface,
including `cc_internal`. `cc_common` remains a generic evaluator/host-ABI
consumer. This packet contains no builtin, C++ rule parser or configured rule
logic.

## Public key and retained carrier

Add `HostPackageInventoryKey` with structural identity:

1. `NormalizedAbsolutePath workspace`; and
2. full `PackageIdentifier`, including canonical repository and package path.

Provide an observed sibling. Root and canonical package identities must never
compare equal merely because package paths match. Display text is Slug-native
and includes both semantic fields.

The terminal carrier is a small typed enum retaining one existing child result
`Arc`:

- Root retains `Arc<Result<LoadedPackage, RootPackageLoadError>>` from
  `RootPackageLoadKey`; or
- Canonical retains
  `Arc<Result<LoadedPackage, RepositoryPackageLoadError>>` from the
  crate-private general `RepositoryPackageInventoryKey`.

A canonical route semantic failure is a distinct typed carrier terminal that
retains the route result `Arc`; it is not flattened into a package error.
Expose borrowed access to the successful `LoadedPackage` and typed terminal
inspection without cloning the package or evaluator-owned values. The carrier
retains no workspace mapping, source address, route success, registration
label, event batch or scratch cache.

The exact public names of carrier variants/accessors are Slug-native, but the
Root/Canonical and route/package distinctions are mandatory. Every retained
type derives the applicable `Allocative`, `Dupe`, equality and debug traits.

## One shared legacy/observed driver

Both key forms use one private driver parameterized by legacy versus observed
mode.

For a root package:

1. compute `RootPackageLoadKey` or its observed sibling;
2. forward `Need` before any terminal;
3. retain the exact child result `Arc`; and
4. in observed mode retain the exact child epoch result `Arc`s.

For a canonical package:

1. compute `HostCanonicalRepositoryLoadRouteKey` or its observed sibling;
2. forward route `Need` or observed outer before package inventory;
3. retain a semantic route error as the decisive carrier terminal;
4. on route success, construct `HostRepositorySourceRoute::canonical` only
   from the route's accepted input;
5. compute `RepositoryPackageInventoryKey` or observed sibling; and
6. retain the exact inventory result `Arc` and, observed, merge route then
   inventory epochs without replacing any per-demand result `Arc`.

Do not use `RepositoryPackageLoadKey`, the old external consumer-policy
adapter. Do not read a path, BUILD file, repository mapping or source directly.
The composed wrapper stores no `EventBatch`; route/root/inventory children
remain sole event owners. Cache hits are DICE reuse, not a process cache.

## DICE and lifecycle contract

Use complete-only equality and validity. Root/canonical identity, package
identity, child terminal kind and child result semantics participate. `Need`
is transient and self-unequal. Observed outer errors remain complete and typed.

Prove direct dependency order and family isolation:

- root carrier -> observed root package only;
- canonical carrier -> observed canonical route, then observed inventory only;
- a root request never activates canonical route/inventory;
- a canonical request never activates the root package key; and
- the wrapper never owns or replays an event batch.

Prove exact child result and observation `Arc` identity for nonempty root and
canonical epochs, semantic route/root-package/canonical-package terminals,
Need and decisive-prefix behavior, cancellation nonpublication and recovery,
warm reuse, same-path/different-repository key inequality, and A/B/A result
restoration. Conflicting observed epochs remain a typed frontier outer error.

No lock may cross a DICE compute. The retained enum and child `Arc` are
semantic DICE memory. Epoch merge buffers are bounded compute scratch and are
dropped at completion or cancellation.

## Compatibility classification

- **Exact:** no new Bazel-visible surface; full canonical package identity and
  accepted child dependency/terminal order are preserved exactly for the
  composed Slug graph.
- **Slug-native:** public key/carrier/error names, enum layout, display/error
  wording, structural hash, observation transport and memory accounting.
- **Unsupported/deferred:** configured consumers, alias/provider/settings
  semantics, option precedence, general external configured graphs, new
  builtins/rules/actions and exact configuration/output bytes.

## Exact allowlist and caps

Production files:

1. `app/slug_loading_v2/src/host_package_inventory.rs` (new)
2. `app/slug_loading_v2/src/lib.rs`
3. `app/slug_loading_v2/src/bzl_module.rs` (crate-private constructor/accessor
   wiring and stale next-packet annotations only if required)

Proof files:

4. `app/slug_loading_v2/src/host_package_inventory_tests.rs` (new)

No Cargo manifest/lockfile, Bzlmod, identity, analysis, query, core, BUILD,
fixture, oracle, Zabel or plan file is admitted after this scheduling commit.
`bzl_module.rs` may not change package evaluation or the old policy adapter.

Caps: at most 850 net production lines, 1,150 net proof lines and 2,000 net
total lines; at most 760 lines in the new production module, 1,100 in its test
file, 35 additions in `lib.rs`, 45 in `bzl_module.rs`, and 120 lines per new or
touched function. The new module triggers complexity review at 650 physical
lines. Add no dependency, `HashMap`, `HashSet`, interner or global cache.

## Validation

Run serially:

1. focused key/carrier root/canonical success and typed terminal tests;
2. observed dependency, exact `Arc`, event nonownership, Need, cancellation,
   warm reuse and A/B/A matrices;
3. complete `slug_loading_v2` tests;
4. direct read-only `slug_analysis_v2` and `slug_query_v2` dependents;
5. focused Bzlmod canonical-route and identity package tests if unchanged APIs
   do not already have current coverage;
6. `cargo fmt --all --check`, allowlist/cap/function checks,
   `git diff --check`, packet/canonical ID agreement and
   `scripts/v2_archive_status.sh` against its recorded three-file baseline.

No new Bazel oracle is planned because this is a composition-only Slug-native
carrier over accepted exact owners. Require independent terminal review of
identity, dependency/terminal order, zero-copy and epoch/event ownership,
scratch lifetime, utility reuse, scope and caps before acceptance.

## Stops

STOP and `REPLAN` for analysis or query edits; a path-only key; root/canonical
identity collapse; direct route/source/path/BUILD/mapping discovery; use of the
old public external policy adapter; a second package evaluator; cloning a
`LoadedPackage` or child error instead of retaining its result `Arc`; retaining
a successful route/source/mapping; wrapper-owned events; loss or replacement
of observed result `Arc`s; a lock across DICE; process/global cache or interner;
configured activation; alias/provider/settings or option-precedence behavior;
Rust ownership of BCR rules or `cc_internal`; a C++ parser/rule engine; cap or
allowlist overflow; or treating Zabel as compatibility authority.
