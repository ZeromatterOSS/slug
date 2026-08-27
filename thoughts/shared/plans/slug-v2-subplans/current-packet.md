# Current Slug V2 Packet

Packet: `WP-4-5-7A-selected-external-package-boundary-projection-design`

Milestone: M7A command/ruleset bootstrap closure feeding ordinary M8 Stage
10.3 analysis.

Base: `0055c653b`.

Result: freeze one public, route-owned external package-boundary projection
which distinguishes ignore pruning from current-package deletion before any
selected-external traversal is implemented. This packet is docs-only.

## Learned facts and source basis

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
remains semantic authority. `PackageLookupFunction` validates a package name,
checks `--deleted_packages`, then for an external repository obtains its
repository and ignore inputs before probing `BUILD.bazel` then `BUILD`.
`ProcessPackageDirectory` obtains package existence and the directory listing
as sibling dependencies. Its child recursion is filtered by the recursive
key's `IgnoredSubdirectories`; a deleted current package does not remove child
dependencies. `RecursivePkgKey` structurally includes repository, rooted path
and excluded paths, and `RecursivePkgFunction` aggregates child package sets.

Slug's live `ExternalRepositoryPackageLookupKey` already owns the corresponding
route, deleted-package, routed-ignore and marker facts, but currently collapses
deleted policy and ignore policy into one `Deleted` terminal. The accepted
`HostRepositoryDirectoryListingKey` owns direct entries without package policy.
`HostRootPackageBoundaryKey` is the root-repository analogue of the missing
public decision shape.

Buck2 DICE guidance in `docs/developers/dice.md` requires immutable complete
values, semantic dependency ownership, complete-only validity for transient
Need, no lock across compute and observation release with dependent graph
versions. No new oracle is needed: this design changes no public command
behavior and reuses accepted marker-priority, deleted-package, repository-ignore
and recursive-pattern evidence.

Zabel's authenticated-source recursive discovery is concept/test guidance for
producer/consumer separation only. Do not copy its session store, source IDs,
allocator, diagnostics or compatibility claims. Bazel 9.2 is the behavior
authority.

## Decision and non-decisions

Add a doc-hidden `HostExternalPackageBoundaryKey` and observed sibling in a
small bzlmod projection module. The key is the complete authenticated
`RootRepositoryRoute` plus validated root-capable `PackagePath`. It consumes,
but does not reimplement, the existing private external package lookup.

First split that lookup's conflated terminal into its existing `Deleted` policy
terminal and a new `IgnoredDirectory` terminal at the points where their
already-owned predecessors decide them. Preserve the private `Deleted` spelling
to minimize consumer churn. Preserve invalid-name validation, policy/ignore
ordering, marker priority, Need and observed outer-error precedence. The public
projection reports exactly:

- `InvalidPackageName`;
- `DeletedPackage`, for which traversal may still inspect descendants;
- `IgnoredDirectory`, which prunes the candidate subtree;
- `Package`, with only the selected `BUILD.bazel` or `BUILD` spelling; and
- `NoPackage`.

This is a projection, not a second lookup. It must not directly compute deleted
policy, repository ignore, marker paths, directory listings or source files.
The private lookup remains the natural owner and existing package-source
consumers continue to use its rich internal result. Public boundary errors are
repository-relative semantic tags only; their retained state, `Debug`,
`Display` and `source()` expose no physical path, observation namespace,
materialization root, compute message or private lookup error.

Do not implement recursive traversal, target-pattern expansion, explicit-name
conflict resolution, family filtering, registration activation, package
loading, configured validation, rules or actions. Do not alter BCR source
authentication or define any BCR rule in Rust. Bazel 9 BCR Starlark remains the
source of rules including `cc_internal`; `cc_common` is only a later generic
host-capability consumer.

## DICE, request and retained-state contract

The route and package path are the complete semantic key. The route retains
workspace, apparent/canonical repository, module and full source identity,
including selected mapping or generated file-effect plan. Never project key
identity to display repository text or reconstruct a physical root.

Legacy returns `SourcePreparationOutcome<Arc<Result<Value, Error>>>`. Observed
also carries the exact private lookup `PathObservationEpoch` and admits
`ObservedPathFrontierError`. Both use complete-only equality and validity.
Observed outer error precedes Need, which precedes a semantic terminal; no Need
is cached as complete. Ordinary DICE cancellation applies and no lock spans a
compute.

The retained public value is one small enum plus the selected marker spelling
when present. The observed carrier adds one existing immutable epoch and `Arc`
result. It retains no route/source/mapping copy, package tree, entry slice,
physical path, evaluator heap, command scratch, global cache or manual
interner. DICE invalidation and equality cutoff own publication and release;
there is no service/async-transfer lifetime or fallback.

The key accepts an already-authenticated route. A later traversal caller must
merge any predecessor `RootRepositoryRouteObservationKey` epoch before
publishing a larger observed result. Overlapping requests share only immutable
DICE values and retain independent injected request revisions.

## Compatibility

- **Exact:** no new named Bazel surface is activated. Existing external marker
  priority, deleted-package and repository-ignore semantics remain exact for
  the already-admitted point lookup.
- **Slug-native:** the public boundary enum, Rust/DICE key, opaque projected
  errors and observation carrier.
- **Unsupported/deferred:** selected-external recursive membership and error
  aggregation, target-pattern expansion, wildcard-name conflict lookup,
  family policy/dedupe, registration activation, configured validation,
  options, rules and actions.

## Docs-only allowlist and review

This design packet may change only:

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`;
- `thoughts/shared/plans/slug-v2-subplans/06-analysis-toolchains-and-actions.md`;
- `thoughts/shared/plans/slug-v2-subplans/current-packet.md`.

Caps are 50, 260 and 190 net lines respectively. Run source-anchor, structure,
scope, archive-baseline and diff checks. Require independent architecture
review before selecting implementation because this freezes a new public DICE
boundary.

## Successor implementation bounds

If accepted, select
`WP-4-5-7A-selected-external-package-boundary-projection-implementation` with
this implementation allowlist:

- `app/slug_bzlmod_v2/src/host_package.rs` only to split the existing private
  lookup's deleted and ignored terminals and preserve its current consumers;
- `app/slug_bzlmod_v2/src/host_external_package_boundary/mod.rs` for the new
  public projection;
- `app/slug_bzlmod_v2/src/host_external_package_boundary/tests.rs` for direct
  boundary and lifecycle proof; and
- `app/slug_bzlmod_v2/src/host_package_observation_tests.rs` to discriminate
  the private deleted-policy and ignored-directory terminals;
- `app/slug_bzlmod_v2/src/source_preparation.rs` only to map both split private
  terminals to the existing direct-include `Deleted` failure;
- `app/slug_bzlmod_v2/src/source_preparation_observation_tests.rs` only for
  mechanical private-terminal proof updates if the split requires them; and
- `app/slug_bzlmod_v2/src/lib.rs` for doc-hidden exports and module wiring.

Implementation caps are 560 production and 900 proof additions, with no
dependency, fixture or oracle. `host_package.rs` exceeds the complexity trigger
but remains the cohesive private lookup owner; the new public projection is
split into its own small module so no new responsibility is added there.

Proof must cover all five terminals, exact marker priority, route/package
key-hash A/B/A, direct-local, immutable/selected-registry, generated and built-
in routes through the shared private lookup, deleted-versus-ignore descendant
semantics, path/materialization Need, observed outer precedence, exact epoch
forwarding, complete-only equality/validity, policy/marker and source/generation
A/B/A restoration, and public error/debug redaction. A structural regression
must prove the projection has no direct policy, ignore, marker-path, listing or
source-file dependency.

Existing repository-package source and direct-local include-horizon consumers
must continue to treat both private split terminals as their accepted `Deleted`
failure. The split is observable only through the new projection; it must not
change point package loading or include-horizon terminal ordering/equality.

Run focused and full bzlmod tests, downstream loading tests/checks, locked core
check and rebuilt locked CLI, all serially. Formatting, scope, cap, dependency,
no-lock, archive-baseline and diff gates remain mandatory. Require independent
DICE/source-boundary review before terminal `ACCEPT`.

STOP and `REPLAN` for another policy computation, a physical root/namespace in
the public result, a copied package tree or route mapping, a source disposition
that cannot use the private lookup, cap/allowlist expansion, traversal or
registration activation, a new exact claim, dependency, global state or lock
across DICE compute.

## Immediate predecessor

Commit `0055c653b` accepts the generic routed repository directory-listing
owner for direct-local, selected-registry, generated and built-in sources. It
activates no rule semantics: Bazel 9 BCR Starlark remains the rule source, and
Zabel remains peer guidance only.
