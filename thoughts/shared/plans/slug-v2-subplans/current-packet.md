# Current Slug V2 Packet

Packet: `WP-2A-m1-root-repository-route-observation-implementation`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Rust base: `03f2db3e`
Frozen design: `1ce16378`
Result: implement the observed root-repository-route prerequisite without
activating public loading query.

## Authority and caps

Write only:

- `app/slug_bzlmod_v2/src/host_module.rs`; and
- `app/slug_bzlmod_v2/src/lib.rs`.

Against `03f2db3e`, cap host-module growth at 140 production, 240 test and
380 aggregate semantic lines with 4,578 physical lines; cap lib growth at 8
production lines and 405 physical lines; cap aggregate semantic growth at 388.

## Required implementation

Add one doc-hidden `RootRepositoryRouteObservationKey` newtype around
`RootRepositoryRouteKey`, plus a doc-hidden `ObservedRootRepositoryRoute`.
Export both only for the later cross-crate query consumer. Both route keys call
one pure projection from the existing root-module semantic carrier; legacy
computes only `HostRootModuleFileKey`, observed computes only
`HostRootModuleFileObservationKey`.

The observed value owns exactly the projected
`Arc<Result<RootRepositoryRoute, RootRepositoryRouteError>>` and the child
`PathObservationEpoch`. It forwards that epoch unchanged, including every
exact child Result Arc; no epoch union or rebuilt Result Arc is permitted.
Root-module semantic carrier state remains dependency-owned after projection.

The value algebra is
`SourcePreparationOutcome<Result<ObservedRootRepositoryRoute,
ObservedPathFrontierError>>`. Preserve Need immediately; preserve a completed
observed path-frontier failure as typed outer; keep root-module semantic error,
unknown repository, unsupported nonlocal override, builtin `bazel_tools` and
direct-local route results inside the semantic Result Arc. Complete-only
equality and validity match the accepted observed-key pattern.

The observed Host root-module child remains the sole event owner. The route
key stores no local `EventBatch`; computing observed anchor then observed route
in one transaction reuses the same structural observed module child and cannot
replay its batch.

Keep the cohesive host-module owner. Do not split or move existing tests.

## Compatibility and proof

Route values, errors, canonical repository names, builtin identity and local
override semantics remain exact. The observed sibling, typed outer and
carrier association are Slug-native. Broader query publication, multi-build
certificate aggregation, one-shot migration and exact identity bytes remain
deferred.

Require parity and exact projected semantic Arc/forwarded epoch Arc proof for
builtin, local override, unknown repo, unsupported override and root-module
error; observed/legacy family nonactivation; one shared observed module child
and one cold MODULE event when anchor plus route are requested together; warm
suppression; Need, mismatch/conflict outer, cancellation/recovery,
edit/delete/recreate and A/B/A; exact demand membership; complete-only
equality/validity; and post-return proof that only the observed route Result Arc
plus epoch remain in the parent value.

Run focused route/module tests, full bzlmod, loading and core checks,
formatting/diff/archive gates, exact accounting, Buck2 retention and AI cleanup
scans, and independent review.

## STOP / REPLAN

STOP on any other file; Cargo, BUILD, fixture, oracle or generated-file write;
public activation; query-crate changes; another route/store/cache/event owner;
legacy and observed module-family cross-activation; semantic/error/event drift;
epoch union or Result-Arc reconstruction; a retained collection/lock/task/
direct Host read; cap excess; or M1 closure. `REPLAN`
if the two keys cannot share one projection, the carrier cannot forward exact
child Result Arcs, another Rust file is required, or the cohesive physical cap
cannot hold.

## Immediate predecessor

`1ce16378` freezes the observed route design selected by the audit at
`b7d3405c`. It is the only prerequisite before returning directly to
loading-query publication design.
