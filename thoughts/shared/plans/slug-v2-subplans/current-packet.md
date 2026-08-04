# Current Slug V2 Packet

Packet: `WP-5-m1-direct-local-route-package-horizon-implementation`
Milestone: M1, one semantic loading spine
Owner: `slug-v2-subplans/05-bzlmod-checkpoint-evidence-3.md`
Role: private one-file route-aware package-horizon implementation
Evidence: accepted direct-local route/source/inspection owners in `e5e2c55d`
and `8aae11d6`; private `ExternalRepositoryPackageLookupKey` in `42ef64cd`;
accepted selected-source/loading migration in `9b5246af`; accepted root horizon
batching tests; and pinned Bazel 9.2 `ModuleFileFunction.advanceHorizon`. Add no
oracle or fixture.

Edit exactly `app/slug_bzlmod_v2/src/source_preparation.rs`. The formatted net
addition may not exceed **300 production lines, 650 test lines, or 950 total
lines**. Add only these private owners:

- `DirectLocalIncludePackageHorizonKey`;
- `DirectLocalIncludePackageHorizon`;
- `DirectLocalIncludePackageOccurrence`;
- `DirectLocalIncludePackageHorizonError`; and
- `DirectLocalIncludePackageFailure`.

The key identity is exactly `NormalizedAbsolutePath` plus nonroot
`ApparentRepoName`, matching `DirectLocalModuleInspectionKey`; reject the root
apparent name. Compute that accepted inspection key once and do not recompute
its route, source, or parser projection independently. Forward its
`SourcePreparationNeeds` unchanged. Keep outer inspection-compute failure
distinct from the typed inspection failure.

For an absent MODULE use an empty request slice. Parse every include occurrence
in source order through the existing `parse_root_include` seam before issuing
any package request. Return the first malformed label with its exact raw
`CompactString` and `LogicalSpan`. For every valid occurrence replace the
parser's root repository with `route.canonical_repo()`, retain its canonical
external `PackageIdentifier`, canonical `TargetName`, raw label, and span, and
preserve duplicates and source order.

Deduplicate only the package dependency keys in deterministic first-seen order
with the existing compact collections. Compute the private
`ExternalRepositoryPackageLookupKey` exactly once for every unique package in
one `compute_join` group. Preserve typed lookup-compute failures; do not panic
them away. Union every lookup Need with `SourcePreparationNeeds::try_union`;
same-route conflicts are an internal invariant.

After the whole group returns, rewalk the original occurrences in source order.
The first occurrence whose package is a complete `InvalidPackageName`,
`Deleted`, `NoBuildFile`, typed lookup error, or lookup-compute error returns
that terminal with the occurrence's raw label/span. The first unresolved
occurrence instead returns the union of **all** unresolved Needs from the
already-requested group. Therefore an earlier terminal beats a later Need and
an earlier Need beats a later terminal. This source-order mixed precedence is
the pinned Bazel 9.2 behavior; do not restore the rejected global-Need rule.
Only an all-successful group returns the ordered occurrence value.

The complete value retains exactly the full `RootRepositoryRoute` and one
`Arc<[DirectLocalIncludePackageOccurrence]>`. It does not retain MODULE bytes,
lookup results, duplicate dependency carriers, fragment paths or bytes, event
batches, evaluation state, or activation data. Absent MODULE and present empty
MODULE are intentionally equal when their route is equal; the inspection child
still recomputes, while the horizon prunes that semantically irrelevant
presence distinction. Include raw spelling/span and occurrence order remain
semantic value data for later diagnostics.

Errors distinguish `InspectionCompute`, typed `Inspection`, `BadLabel`, and
occurrence `Package`. Package failures distinguish `InvalidPackageName`,
`Deleted`, `NoBuildFile`, typed `Lookup`, and `LookupCompute`, and retain the
canonical package. Display the raw include and logical file/line/column context.
Expose typed inspection and operational lookup errors through `source()` where
their existing owners permit it; string compute and semantic lookup outcomes
have no source. Use complete-only equality and validity: every transient Need
is invalid and self-unequal.

Tests must discriminate parse-all-before-lookup; duplicate occurrences with one
lookup activation; preserved occurrence order; both mixed terminal/Need
directions; first complete error by source order independent of dependency
completion order; exact multi-kind Need union; every typed error mapping and
raw-label/span source chain; absent versus present-empty equality; include
add/edit/reorder/delete/recreate; canonical global deletion; route-local
REPO/ignore edits; BUILD-marker create/delete/recovery; route A-to-B-to-A; warm
reuse; and captured/uncaptured child REPO events with no horizon-local
evaluation data. Structurally prove the owner names no fragment source key,
derived fragment path, `FileBytes`, filesystem API, evaluator, event storage,
public export, or caller.

Use existing `Arc<[T]>`, `CompactString`, `LogicalSpan`, `SmallSet`/`SmallMap`,
`Dupe`, and `Allocative`. Add no `HashMap`, cache, interner, lock, mutable
carrier, direct IO, second inspection/route/policy/lookup graph, or duplicate
lookup-result storage. Do not modify or generalize the root horizon: its root
identity, `PathOutcome`, and selected-path result are different owners.

Stops: any second Rust file, cap breach, visibility widening/public export,
fragment read, recursive occurrence closure or cycle behavior, evaluator or
compilation work, empty-key defaults, contextual mappings, registry/MVS/JVM
transport, public activation, fixture/oracle, direct filesystem IO, or root
horizon semantic change requires `REPLAN`. Run only focused serial Cargo tests,
formatting, GNU-Windows no-run check, archive/scope/cap/diff gates, and an
independent latest-diff review. Clean stale `slugd` processes before and after
daemon-sensitive tests.
