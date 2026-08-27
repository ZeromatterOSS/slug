# Current Slug V2 Packet

Packet: `WP-4-5-7A-selected-external-package-boundary-projection-implementation-r2`

Milestone: M7A command/ruleset bootstrap closure feeding ordinary M8 Stage
10.3 analysis.

Base: `18cd8f35b`.

Result: resume the accepted public, route-owned external package-boundary
projection now that built-in optional metadata and BUILD-marker presence reach
the existing private lookup.

## Learned facts and source basis

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
remains semantic authority. `PackageLookupFunction` validates a package name,
checks `--deleted_packages`, then obtains repository-ignore state before
probing `BUILD.bazel` then `BUILD`. `ProcessPackageDirectory` obtains package
existence and directory entries as sibling dependencies; ignore policy prunes
child recursion, whereas deleting the current package does not remove child
dependencies.

Slug's private `ExternalRepositoryPackageLookupKey` already owns route,
deleted-package, routed-ignore and marker facts, but collapses deleted policy
and ignore policy into one `Deleted` terminal. The accepted routed listing and
commit `18cd8f35b` now let direct-local, selected-registry, generated and
built-in routes all reach that private owner without weakening catalog
integrity or fabricating a physical root.

Buck2 DICE guidance in `docs/developers/dice.md` requires immutable complete
values, semantic dependency ownership, complete-only validity for transient
Need, no lock across compute and observation release with dependent graph
versions. Zabel's authenticated-source recursive discovery remains peer
architecture/test guidance only; do not copy its session store, source IDs,
allocator, diagnostics or compatibility claims.

## Accepted decision

Add a doc-hidden `HostExternalPackageBoundaryKey` and observed sibling in a
small bzlmod projection module. The key is the complete authenticated
`RootRepositoryRoute` plus validated root-capable `PackagePath`. It consumes,
but does not reimplement, the existing private external package lookup.

Split that lookup's conflated terminal into its existing `Deleted` policy
terminal and a new `IgnoredDirectory` terminal at the already-owned decision
sites. Preserve invalid-name validation, policy/ignore ordering, marker
priority, Need and observed outer-error precedence. The public projection
reports exactly:

- `InvalidPackageName`;
- `DeletedPackage`, whose descendants may still be inspected;
- `IgnoredDirectory`, which prunes the candidate subtree;
- `Package`, carrying only the selected `BUILD.bazel` or `BUILD` spelling; or
- `NoPackage`.

This is a projection, not a second lookup. It must not directly compute deleted
policy, repository ignore, marker paths, directory listings or source files.
Existing package-source and include-horizon consumers continue to use the rich
private result and map both split policy terminals to their accepted `Deleted`
failure.

Do not implement recursion, target-pattern expansion, conflict resolution,
family filtering, registration activation, package loading, configured
validation, rules or actions. Bazel 9 BCR Starlark remains the source of rule
definitions including `cc_internal`; `cc_common` is only a demanding consumer
of the generic Rust host-builtin ABI. No C++ rule implementation belongs in
Rust.

## DICE and retained-state contract

The route and package path are the complete semantic key. Never project key
identity to display repository text or reconstruct a physical root. Legacy
returns `SourcePreparationOutcome<Arc<Result<Value, Error>>>`; observed also
carries the exact private lookup `PathObservationEpoch` and admits
`ObservedPathFrontierError`. Observed outer error precedes Need, which precedes
a semantic terminal. Both forms use complete-only equality and validity.

The retained public value is one small enum plus selected marker spelling. The
observed carrier adds one existing immutable epoch and `Arc` result. It retains
no route/source/mapping copy, package tree, entry slice, physical path,
evaluator heap, command scratch, cache, interner or manual lock. A later
traversal caller must merge its route predecessor epoch.

## Compatibility

- **Exact:** existing external marker priority, deleted-package and
  repository-ignore semantics remain exact for the admitted point lookup; no
  new named Bazel surface is activated.
- **Slug-native:** the public boundary enum, Rust/DICE key, opaque projected
  errors and observation carrier.
- **Unsupported/deferred:** selected-external recursive membership and error
  aggregation, target-pattern expansion, wildcard conflicts, family policy,
  registration, configured validation, options, rules and actions.

## Active allowlist and bounds

This implementation packet may change only:

- `app/slug_bzlmod_v2/src/host_package.rs` to split the private terminals;
- `app/slug_bzlmod_v2/src/host_external_package_boundary/mod.rs` for the public
  projection;
- `app/slug_bzlmod_v2/src/host_external_package_boundary/tests.rs` for direct
  boundary and lifecycle proof;
- `app/slug_bzlmod_v2/src/host_package_observation_tests.rs` for private
  terminal proof;
- `app/slug_bzlmod_v2/src/source_preparation.rs` only to map both split private
  terminals to the accepted direct-include `Deleted` failure;
- `app/slug_bzlmod_v2/src/source_preparation_observation_tests.rs` only for
  mechanical private-terminal proof updates; and
- `app/slug_bzlmod_v2/src/lib.rs` for doc-hidden module wiring and exports.

Caps remain 560 production and 900 proof additions, with no dependency,
fixture or oracle. `host_package.rs` remains its existing cohesive private
owner; the new projection stays in its own small module.

Proof must cover all five terminals, marker priority, route/package key-hash
A/B/A, all four admitted route dispositions, deleted-versus-ignore descendant
semantics, path/materialization Need, observed outer precedence, exact epoch
forwarding, complete-only equality/validity, policy/marker and
source/generation restoration, and public error/debug redaction. Structural
proof must show the projection has no direct policy, ignore, marker-path,
listing or source-file dependency.

Run focused/full bzlmod, downstream loading tests/checks, locked core check and
rebuilt locked CLI serially. Formatting, scope, cap, dependency, no-lock,
archive-baseline and diff gates remain mandatory. Require independent
DICE/source-boundary review before terminal acceptance.

STOP and `REPLAN` for another policy computation, a physical root/namespace in
the public result, copied package tree/route mapping, an unsupported source
disposition, cap/allowlist expansion, traversal or registration activation, a
new exact claim, dependency, global state or lock across DICE compute.

## Immediate predecessors

Commit `5ec7f3c79` freezes this boundary design after independent review.
Commit `18cd8f35b` accepts the prerequisite built-in optional-input projection:
normal missing built-in metadata and markers now use the shared routed listing,
while materialized routes and BCR authentication remain unchanged. Independent
terminal review accepted the corrected natural dependency order and 252/261
production/proof result. Resume only this previously reviewed boundary.
