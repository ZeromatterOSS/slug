# Current Slug V2 Packet

Packet: `WP-5-m1-direct-local-support-gated-acyclic-closure-implementation`
Milestone: M1, one semantic loading spine
Owner: `slug-v2-subplans/05-bzlmod-checkpoint-evidence-3.md`
Role: private one-file support-gated occurrence preparation
Evidence: accepted source/inspection/package horizon and shared preflight in
`34a2340e`; pinned Bazel 9.2 breadth-first acquisition; accepted repeated/nested
include oracles; root closure regressions; and the accepted private Slug cycle
capability boundary. Add no oracle or fixture.

Edit exactly `app/slug_bzlmod_v2/src/source_preparation.rs`. The formatted net
addition may not exceed **600 production lines, 1,650 test lines, or 2,250 total
lines**. Add only private, callerless owners:

- `DirectLocalModulePreparationKey(NormalizedAbsolutePath, ApparentRepoName)`;
- `DirectLocalModulePreparation::{Supported, Unsupported}`;
- `DirectLocalModuleClosure`;
- `DirectLocalIncludeFragment`;
- `DirectLocalIncludeCycleCapability`;
- `DirectLocalModulePreparationError`; and
- `DirectLocalIncludeFragmentFailure`.

The key identity is workspace plus nonroot apparent repository and it computes
`DirectLocalModuleInspectionKey` once. Forward its Needs unchanged; distinguish
typed inspection from inspection-compute failure. Preserve root absent versus
present state in the supported closure through the accepted inspection carrier.
For a present root, invoke `validate_root_module_source` on its logical path and
bytes before seeding the first package horizon; map failure to a distinct root-
validation error and request no package or fragment dependency. Root absence
remains supported without validation. Use complete-only equality/validity:
every Need is invalid and self-unequal.

The supported closure retains the accepted root inspection plus
`Arc<[DirectLocalIncludeFragment]>` in breadth-first occurrence order. Each
fragment retains canonical package/target, raw label/`LogicalSpan`, the
route-derived requested logical path, shared bytes, and
`NonrootModuleFileInspection`. Preserve every occurrence, including identical
raw labels, diamonds, siblings, and distinct labels for one canonical path.
Dependency dedupe is horizon-local only and never deduplicates occurrence
carriers or occurrence compilation.

For every frontier call the accepted
`preflight_direct_local_include_package_horizon(ctx, route, requests)` and finish
it before any fragment demand. Derive normalized repository-relative fragment
paths from canonical package plus target. Request every first-seen
`HostRepositorySourceFileKey` in one group, union all
`SourcePreparationNeeds::try_union` results, then rewalk occurrences in source
order. An earlier complete terminal beats a later Need; an earlier Need returns
the full group union and beats every later terminal. Preserve outer source-
compute failure separately from typed source failure and Absent. Use the
existing `validate_root_module_source` seam for UTF-8, restricted syntax,
MODULE/include inspection, and Starlark prepare/identifier validation. Invoke it
for every successful occurrence, even when its source dependency was deduped,
so occurrence compile order remains exact.

Every successful non-backedge occurrence appends its nested requests to the next
horizon in occurrence order with an extended active ancestry. Active identity
is route-canonical package plus canonical target. Only a repeat on that
occurrence's active ancestry is a cycle candidate; never use a global visited
set. Retain the first breadth-first candidate's repeated raw label/span and the
first matching ancestor's raw label/span as private capability metadata.

A cycle candidate is pending, not terminal. Finish its current horizon normally,
do not enqueue only that repeated occurrence's outgoing requests, and continue
all other queued occurrences and descendants breadth-first. At every later
horizon, real Needs and terminals retain normal precedence. This cycle-pruned
capability analysis is not supported-closure occurrence truncation: it is used
only to prove the unsupported domain. Return `Unsupported` only after the entire
remaining cycle-pruned reachable worklist succeeds and exhausts. This ensures a
cycle plus a finite side-branch terminal or Need returns the Bazel result rather
than hiding it. No capability value may be returned merely because its first
current horizon succeeded.

Errors distinguish inspection/compute, root validation, package preflight,
fragment source-compute, typed source, Absent, and fragment validation/compile
failures, restoring root logical path or occurrence raw label/span and
repository-relative/requested logical path. Typed inspection, package, and
source errors expose their existing source chains where available; string
compute, Absent, validation, and capability variants do not masquerade as Bazel
errors. `Supported` equality includes root state and ordered fragment identities/
bytes/inspections but excludes transient ancestry. `Unsupported` equality
includes only typed capability provenance. The key owns no event batch and does
not copy or replay routed-REPO child events.

Tests must discriminate breadth-first versus depth-first compile order; package
barrier before fragments; horizon-local path dedupe versus repeated occurrence
compilation; both mixed terminal/Need directions; exact multi-kind Need union;
all typed failures and raw-label/span sources; siblings, diamonds, same canonical
path under distinct labels, and finite later reuse; self and multi-file cycles;
same-horizon cycle candidate versus terminal/Need; cycle in H plus sibling H+1
terminal and Need in both orders; a side branch emitted by the cyclic ancestor;
deterministic first pending-cycle provenance after full pruned traversal; root
absence; a root prepare/identifier failure with zero include-package lookup or
fragment-source activations beyond root inspection; fragment and nested-include
add/edit/delete/reorder/recreate; route A-to-B-to-A;
warm reuse/downstream pruning; complete-only equality; and captured/uncaptured
child events with no preparation-local data or warm replay.

Use existing `Arc<[u8]>`, `Arc<[T]>`, `CompactString`, `SmallMap`/`SmallSet`,
`Dupe`, and `Allocative`. Stops: no second file, public export/caller/activation/
publication, Bazel-like cycle diagnostic, cycle variant in the supported closure,
global visited set, cross-horizon dependency dedupe, recursive DICE, intentional
hang, hard depth/node limit, evaluator execution, default/validation/print
change, event storage, direct IO, lock/interner/cache, fixture/oracle, or cap
breach. `REPLAN` on any such expansion. Run focused serial tests, formatting,
GNU-Windows no-run, archive/scope/cap/diff gates, and independent latest-diff
review; do not run Bazel.
