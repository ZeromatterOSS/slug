# Current Slug V2 Packet

Packet: `WP-2A-m1-root-repository-route-observation-design`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Scheduling base: `b7d3405c`
Rust base: `03f2db3e`
Result: freeze the uniquely smaller observed root-repository-route producer
required before public loading-query publication can remain family-isolated.

## Authority and caps

Write only the canonical plan, this manifest, Stage 2 and the orchestration
routing log. Net caps are 40 canonical, 180 Stage 2, 180 manifest, 30 routing
and 390 aggregate lines.

## Required implementation

Freeze one private `RootRepositoryRouteObservationKey` structurally distinct
from `RootRepositoryRouteKey`. Both keys must use one route-projection driver;
legacy computes only `HostRootModuleFileKey`, observed computes only
`HostRootModuleFileObservationKey`. The observed terminal retains exactly one
semantic `Arc<Result<RootRepositoryRoute, RootRepositoryRouteError>>` and one
Arc-backed `PathObservationEpoch`.

Preserve Need immediately. Preserve a completed observed path-frontier failure
as typed outer, and preserve unknown repository, root-module semantic failure,
builtin `bazel_tools` and direct-local route results as semantic Complete
values. Complete-only equality and validity must match the accepted observed
key pattern. The observed Host root-module child remains the sole event owner;
the route key publishes no local event.

Freeze the implementation allowlist to
`app/slug_bzlmod_v2/src/host_module.rs` and
`app/slug_bzlmod_v2/src/lib.rs`. Against `03f2db3e`, cap host-module growth
at 140 production, 240 test and 380 aggregate semantic lines with 4,578
physical lines; cap lib growth at 8 production lines and 405 physical lines;
cap aggregate semantic growth at 388. Keep the cohesive host-module owner
unless exact sizing proves a split mandatory, in which case `REPLAN`.

## Compatibility and proof

Route values, errors, canonical repository names, builtin identity and local
override semantics remain exact. The observed sibling, typed outer and
carrier association are Slug-native. Broader query publication, multi-build
certificate aggregation, one-shot migration and exact identity bytes remain
deferred.

Require parity and exact carrier Arc proof for builtin, local override, unknown
repo and root-module error; observed/legacy family nonactivation; one shared
observed module child and one cold MODULE event when anchor plus route are
requested together; warm suppression; Need, mismatch/conflict outer,
cancellation/recovery, edit/delete/recreate and A/B/A; exact demand membership
and first-Arc union; complete-only equality/validity; retained-state accounting;
focused/full bzlmod and loading/core checks; formatting/diff/archive gates;
Buck2 retention and AI cleanup scans; and independent review.

## STOP / REPLAN

STOP on Rust, Cargo, BUILD, fixture, oracle or generated-file writes; public
activation; query-crate changes; another route/store/cache/event owner; legacy
and observed module-family cross-activation; semantic/error/event drift; a
retained collection/lock/task/direct Host read; cap increase; implementation
claim; or M1 closure. `REPLAN` if the two keys cannot share one projection,
the carrier cannot preserve exact child Result Arcs, another Rust file is
required, or the cohesive physical cap cannot hold.

## Immediate predecessor

The audit at `b7d3405c` found `RootQueryCommandKey` to be the next complete
public owner only after one prerequisite. Direct or transitive external labels
compute `RootRepositoryRouteKey`, which activates legacy
`HostRootModuleFileKey` beside the query root's future observed anchor and
would violate family/event isolation. Constructor syntax cannot exclude that
path. The route producer is the uniquely smaller prerequisite; accepted
observed anchor/package/path children already cover every other query edge.
Multi-build still needs aggregate source-certificate/revision design, and the
one-shot evaluator remains outside native publication.
