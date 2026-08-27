# Current Slug V2 Packet

Packet: `WP-4-5-7A-external-subtree-package-set-owner-design-r2`

Milestone: M7A command/ruleset bootstrap closure feeding ordinary M8 Stage
10.3 analysis.

Base: `ee20d5c7c`.

Result: freeze the loading-owned recursive package-set producer for an already
authenticated external repository route. This packet is docs-only.

## Learned facts and source basis

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
remains semantic authority. `PackageLookupFunction` distinguishes deleted
current packages from repository-ignore matches. `ProcessPackageDirectory`
obtains package existence and direct directory entries as sibling facts;
`RecursivePkgFunction` aggregates child recursive values under a
repository-scoped `RecursivePkgKey`. Ignore policy prunes a subtree, while
`--deleted_packages` suppresses only the current package and leaves descendants
eligible.

Slug now has both missing source-neutral bzlmod inputs: the accepted
`HostRepositoryDirectoryListingKey` owns direct entries for direct-local,
selected-registry, generated and built-in routes, and
`HostExternalPackageBoundaryKey` owns invalid/deleted/ignored/package/no-package
policy without exposing source paths. Loading's accepted
`RootSubtreePackageSetKey` supplies the deterministic DFS, compact package-set
and lifecycle precedent for the root repository, but it cannot be reused by
projecting an external route to a filesystem root.

`docs/developers/dice.md` and Buck2 DICE ownership guidance require immutable
complete values, dependencies computed by their natural owners, transient
Needs, complete-only equality/validity, cancellation without partial
publication and no lock across compute. The matching V1 extraction-ledger row
retains package discovery in demand-driven DICE and query graphs request-
locally; it explicitly excludes external-repository behavior from the older
root packet.

Zabel's `load/session_recursive_package_discovery.zig` is concept/test guidance
for one authenticated-source producer with sorted child names and thin
consumers. Do not copy its session store, source identifiers, allocator,
diagnostics, scheduler or compatibility claims. Bazel 9.2 remains behavior
authority.

## Decision and non-decisions

Add a loading-owned `ExternalSubtreePackageSetKey` and observed sibling. The
semantic key is the complete authenticated `RootRepositoryRoute` plus one
root-capable `PackagePath` prefix. The producer performs one deterministic
repository-relative DFS. For each candidate it consumes only these bzlmod
owners in natural order:

- `HostExternalPackageBoundaryKey`, which decides current-package membership
  and whether ignore policy prunes descendants; and
- `HostRepositoryDirectoryListingKey`, which supplies sorted direct entries
  without exposing its source disposition or physical root.

The boundary is computed first. `IgnoredDirectory` records no package and
terminates that candidate before any listing dependency; this mirrors Bazel's
parent recursion filtering ignored children before `ProcessPackageDirectory`
work. Every other successful boundary then computes the listing. The observed
form merges epochs in boundary-then-listing order. Each dependency preserves
outer failure before Need before its semantic terminal, and the boundary
terminal precedes any listing terminal. A missing listing contributes no
children. `DeletedPackage`, `InvalidPackageName` and `NoPackage` record no
current package but retain directory traversal; `Package` records the package
and retains traversal. A package terminal paired with a missing candidate
directory is a typed fail-closed inconsistency.

Directory-valued direct entries become children. A file entry is not a child.
Any symlink entry fails closed with a typed repository-relative error until a
route-owned followed-directory/cycle boundary is designed; Bazel 9.2 follows
such entries unless its no-follow sentinel applies, so silently skipping them
would produce an incomplete set. `Unknown` likewise fails closed rather than
guessing. A non-Unicode directory name returns a typed redacted error carrying
only its valid parent `PackagePath`, never lossy text or raw OS bytes.

Push admitted children in reverse lexical order for deterministic DFS, then
lexically sort and deduplicate the final package names. That order is
Slug-native: Bazel's stable-order nested set preserves deterministic traversal
but does not promise a public lexical order. The result is one
`Arc<[CompactString]>`, matching the accepted root producer. DFS stacks and
temporary package vectors are compute scratch and never retained.

This packet creates no second package policy, marker lookup, directory owner,
source tree or query traversal. It does not load BUILD files, expand target
patterns, resolve wildcard-name conflicts, filter registration families,
activate registrations, evaluate rules, configure targets or create actions.
Bazel 9 BCR Starlark remains the source of rules including `cc_internal`;
`cc_common` is only a demanding consumer of the generic host-builtin ABI.

## DICE, request and lifetime contract

The route remains intact in key equality/hash: workspace, apparent/canonical
repository, module, source disposition, selected mapping or generated effect
plan all participate structurally. The prefix is the only subtree selector.
Never reconstruct or retain a materialization root, observation namespace or
display-only repository identity.

Legacy returns `SourcePreparationOutcome<Arc<Result<Value, Error>>>`. Observed
adds the complete merged `PathObservationEpoch` and admits
`ObservedPathFrontierError`. Both use complete-only equality and validity. No
Need is cached as complete. DICE cancellation owns abandoned compute release,
and equality cutoff owns reuse of successful immutable results. Overlapping
requests share only immutable completed graph values and retain independent
injected request revisions.

The retained value owns one compact immutable slice. Each package string is
stored once as `CompactString`; no `String`, `HashMap`, `HashSet`, interner,
global cache, route/mapping copy or manual lock is added. The existing Buck2-
derived `CompactString`, `Arc` slice, `Dupe` and `Allocative` patterns are
sufficient, so no utility import or Stage 9 ledger update is needed. There is
no command, service-cache, async-transfer or shutdown lifetime.

## Compatibility

- **Exact:** no named command surface is activated. Within Unicode,
  symlink-free admitted trees, package membership, marker priority inherited
  from the boundary, ignore-versus-delete traversal and prefix containment
  follow Bazel 9.2.
- **Slug-native:** lexical package order, the Rust/DICE key, compact immutable
  carrier, observation epoch, typed redacted error and source-neutral route
  projection.
- **Unsupported/deferred:** target-pattern expansion and wildcard conflicts,
  followed-symlink external traversal/cycle policy, family filtering/dedupe,
  registration activation, package loading from this producer, configured
  validation, options, rules and actions. Non-Unicode and unknown-kind entries
  are explicit fail-closed errors rather than admitted membership.

## Docs-only scope and review

This design packet may change only:

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`;
- `thoughts/shared/plans/slug-v2-subplans/06-analysis-toolchains-and-actions.md`;
- `thoughts/shared/plans/slug-v2-subplans/current-packet.md`.

Caps are 60, 260 and 210 net lines respectively. Run source-anchor, structure,
scope, archive-baseline and diff checks. Require independent DICE/ownership and
retained-representation review before selecting implementation.

## Successor implementation bounds

If accepted, select
`WP-4-5-7A-external-subtree-package-set-owner-implementation-r2` with this
allowlist:

- `app/slug_loading_v2/src/external_subtree_package_set.rs` for the value,
  error, legacy/observed keys and sole DFS producer;
- `app/slug_loading_v2/src/external_subtree_package_set_tests.rs` for focused
  semantic, DICE and lifecycle proof; and
- `app/slug_loading_v2/src/lib.rs` for doc-hidden module wiring and exports.

Caps are 560 production and 900 proof additions, with no dependency, fixture,
oracle or bzlmod edit. A new loading module is required because the root module
already combines root-specific multi-package-root and non-UTF-8 Host-path
policy; adding route-owned external policy there would cross its cohesion
boundary. The new module may mechanically reuse its compact carrier and
reducer patterns, not its filesystem traversal.

Proof must cover root and nested prefixes; all four admitted route
dispositions by composition through the two sole bzlmod dependencies; package,
no-package, invalid, deleted and ignored terminals; ignored pruning versus
deleted descendant retention; BUILD marker spelling invariance; missing,
wrong-kind and inconsistent listings; lexical/deduplicated Slug-native output;
typed/redacted symlink, unknown-kind and non-Unicode failures; boundary-
before-listing activation, outer/Need/error precedence and proof that ignored
candidates never request a listing; exact epoch merge and pointer sharing;
complete-only equality/validity; route/source/prefix key A/B/A;
create/delete/recreate, ignore/unignore and generated source/generation
restoration; cancellation nonpublication; public error/debug redaction; and a
structural sole-dependency guard.

Reuse accepted root-subtree, routed-listing and external-boundary evidence; add
no new oracle because the producer activates no command surface. Run focused
and full loading tests, direct bzlmod/loading checks, locked query/core checks
and rebuilt locked CLI serially. Formatting, scope, cap, dependency, no-lock,
archive-baseline and diff gates remain mandatory. Require independent terminal
DICE/source-boundary review.

STOP and `REPLAN` for following a symlink without a separately reviewed routed
owner/cycle contract, a physical root/namespace, direct filesystem/catalog
read, another policy or marker computation, route projection/copy, second
retained tree, unsupported source disposition, new dependency, cap/allowlist
expansion, command activation, traversal outside the authenticated route,
global state or lock across compute.

## Immediate predecessor

Commit `ee20d5c7c` accepts the branch-free public external package boundary at
363 production and 408 proof additions. Its independent review confirms that
existing four-disposition private-lookup coverage composes with the sole-
dependency projection. Together with `0055c653b`, loading now has every
source-neutral fact required to freeze this producer without widening BCR,
Starlark builtin or rule semantics.
