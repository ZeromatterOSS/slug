# Current Slug V2 Packet

Packet: `WP-2A-m1-direct-local-module-file-observation-implementation`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Scheduling/design base: `707eb1b5`
Rust base: `33717f27`
Result: implement only the accepted direct-local `MODULE.bazel` file
observation prerequisite.

## Authority

Write only:

- `app/slug_bzlmod_v2/src/source_preparation.rs`
- `app/slug_bzlmod_v2/src/source_preparation_observation_tests.rs`

Caps against Rust base `33717f27` are 160 production and 12,750 physical
lines in `source_preparation.rs`; 360 tests and 880 physical lines in the
existing proof file; 520 aggregate semantic and 13,630 combined physical.

## Required implementation

Keep `DirectLocalModuleFileKey` and its Value exact. Add one structurally
distinct crate-private `DirectLocalModuleFileObservationKey`, one private
`ObservedDirectLocalModuleFile` retaining exactly one semantic Result Arc
plus one `PathObservationEpoch`, and one mode-aware driver.

Legacy selects only `RootRepositoryRouteKey` then
`HostRepositorySourceFileKey(MODULE.bazel)`. Observed selects only
`RootRepositoryRouteObservationKey` then
`HostRepositorySourceFileObservationKey(MODULE.bazel)`. Route is first and
source activates only after route semantic success. For observed children,
union the complete route epoch before route semantic inspection, then the
complete source epoch before source semantic inspection with stable left-first
`PathObservationEpoch::from_shared`. Preserve exact child Result Arcs and
surface duplicate conflict/operation mismatch as typed outer errors.

Route/source Need returns no carrier; typed child outer remains outer with no
carrier. RouteCompute retains an empty epoch; route semantic error retains the
route prefix; SourceCompute retains route-only; source semantic error, Absent
and Present retain route+source. Preserve existing semantic values and error
projections. Need is invalid/self-unequal; typed outer Complete is valid/equal
by outer error; carrier Complete is valid/equal by semantic result plus epoch.

The file parent remains eventless. The root-module child remains the sole MODULE
batch owner; Host source stays eventless; evaluation events stay deferred. Add
no child semantic Arc retention, collection, store/cache/interner, lock/task,
direct Host read, request revision, source certificate, export or caller.

## Compatibility

Route/MODULE bytes, source kind, semantic errors/values, and child events remain
exact Bazel 9 admitted behavior. The structural sibling/carrier and typed outer
frontier are Slug-native. Inspection/include/preparation/evaluation, upper
source/load/query/publication, and identity bytes remain unsupported/deferred.

## Proof and validation

Extend only the existing observation proof file. Cover identity/Display, parity,
Present/Absent and semantic errors, both child Need/outer positions,
RouteCompute-empty, route-semantic route-only, SourceCompute route-only, and
source-child route+source prefixes; route-terminal source suppression; exact
ordered values and `Arc::ptr_eq`; stable equal-duplicate first Arc, conflict,
and operation-mismatch outer; full equality/validity; both family directions;
zero inspection/horizon/preparation/evaluation/source/load/query activation;
child-only events/warm suppression; real polled cancellation and successor
recovery; and route/source edit/delete/recreate plus A/B/A.

Run focused tests, full bzlmod, loading and query, established core baselines,
fmt/check/diff/accounting, compact-retention and AI-cleanup review, then one
independent latest-diff review. After ACCEPT, commit and schedule only a
docs-only observed direct-local inspection/preparation/evaluation and upper
source/load audit.

## STOP / REPLAN

STOP on every other file, Cargo/BUILD/fixture/oracle write, public export or
caller, upper activation, mixed families, rebuilt/partial epoch, moved event,
retained scratch/state owner, direct Host read, cap excess, fallback or M1
closure. `REPLAN` if exact child Arcs cannot survive in this owner/two-file
scope or legacy semantics/event ownership must change.
