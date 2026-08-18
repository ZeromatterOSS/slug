# Current Slug V2 Packet

Packet: `WP-2A-m1-direct-local-module-file-observation-design`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Scheduling base: `ba35d0f6`
Rust base: `33717f27`
Result: freeze the uniquely smaller direct-local `MODULE.bazel` file
observation prerequisite selected by the resumed external source/load audit.

## Authority

Write only canonical, this manifest, Stage 2, and the orchestration routing
log. Do not write Rust, Cargo, BUILD files, fixtures, oracles, generated files,
exports, or callers. Docs caps are 40 net canonical, 180 Stage 2, 180 manifest,
30 routing, and 430 aggregate.

## REPLAN evidence

`RepositoryPackageSourceKey` first calls `direct_local_module_support`, whose
private chain is evaluation -> preparation -> inspection ->
`DirectLocalModuleFileKey`. The file key at
`source_preparation.rs:1052-1147` independently selects legacy
`RootRepositoryRouteKey`, then legacy
`HostRepositorySourceFileKey(MODULE.bazel)`. Selecting observed package
lookup, BUILD source, or recursive `.bzl` inputs only in the upper source/load
owner would therefore retain this second legacy route/source family.

`DirectLocalModuleFileKey` is the uniquely smaller prerequisite. Inspection
consumes it at lines 1184-1213. Preparation separately calls the include
package preflight directly at lines 1958-1965, while the standalone horizon
key calls the same helper at lines 1348-1380; that later helper/key decision is
not part of this leaf. Evaluation owns its own event boundary at 2426-2527.

## Frozen design

Keep `DirectLocalModuleFileKey` and its legacy value exact. Add one
structurally distinct crate-private
`DirectLocalModuleFileObservationKey` and one private carrier. One
mode-aware driver serves both siblings and selects only matching route and
Host-source families:

- legacy computes `RootRepositoryRouteKey`, then
  `HostRepositorySourceFileKey(MODULE.bazel)` and returns the unchanged value;
- observed computes `RootRepositoryRouteObservationKey`, then
  `HostRepositorySourceFileObservationKey(MODULE.bazel)` and returns
  `SourcePreparationOutcome<Result<ObservedDirectLocalModuleFile,
  ObservedPathFrontierError>>`;
- the observed carrier retains exactly one local semantic
  `Arc<Result<DirectLocalModuleFile, DirectLocalModuleFileError>>` plus one
  compact `PathObservationEpoch`. Retain no child semantic Arc or collection.

Route is first and source activates only after route semantic success. Validate
and union the complete route epoch before inspecting route semantics, then
validate and union the complete source epoch before inspecting source
semantics. Stable left-first `PathObservationEpoch::from_shared` preserves
the route's first equal Result Arc when the local override aliases the root
`MODULE.bazel` demand. A conflict or operation mismatch is typed outer at the
exact source step. Do not reconstruct child path Results.

Route/source Need returns immediately without a carrier. A typed observed
child outer error remains outer and carries no carrier. Preserve existing DICE
compute failures as semantic projections with the only possible prefix:
`RouteCompute` has an empty epoch; route semantic error retains the route
prefix; `SourceCompute` retains the route-only prefix; child source semantic
error, Absent, and Present retain route then source. Constructor rejection
remains outside DICE.

Need is invalid and self-unequal. Every typed outer `Complete` is valid and
compares by its outer error. Every carrier `Complete` is valid and compares
by semantic result plus complete epoch.

The file parent remains eventless. The observed root-module child remains the
sole MODULE batch owner; Host source remains eventless; evaluation events stay
deferred. Cancellation publishes no parent state or event and leaves completed
child DICE values dependency-owned. Route/source carriers and union scratch
are compute-local or child-owned. Add no collection, store, cache, interner,
lock, task, direct Host read, request revision or certificate.

## Compatibility and source

Existing route, MODULE source-kind/bytes, errors, values and child events remain
exact Bazel 9 admitted behavior. The structural sibling/carrier and typed outer
frontier are Slug-native. Inspection/include/preparation/evaluation observation,
upper external package source/load, RootQuery, multi-build, one-shot
publication and identity bytes remain unsupported/deferred in this packet.

Reuse the accepted Bazel 9.2 FileFunction/source-kind and root-module routing
evidence; no new oracle is authorized. DICE equality, cancellation and child
dependency ownership follow `docs/developers/dice.md`; retained epochs keep
the accepted compact Buck2-derived sorted-map representation.

## Future implementation envelope

After independent design acceptance, schedule exactly one implementation from
Rust base `33717f27` and the accepted design commit. Write only
`app/slug_bzlmod_v2/src/source_preparation.rs` and existing
`app/slug_bzlmod_v2/src/source_preparation_observation_tests.rs`. Reuse its
existing nested `include!`; move no existing test body or fixture.

Caps against `33717f27` are 160 production and 12,750 physical lines in
`source_preparation.rs`; 360 tests and 880 physical lines in the existing
test file; 520 aggregate semantic and 13,630 combined physical lines. The large
production owner remains cohesive because the sibling shares its exact route
and source driver; the separate proof file keeps the test tail bounded.

Proof must discriminate key identity/Display, legacy parity, Present/Absent
and route/source semantic errors, Need and typed outer at both children,
route-terminal source suppression, exact ordered demand/value/`Arc::ptr_eq`,
stable duplicate first Arc and conflict/mismatch outer error, Complete
carrier/outer equality and validity plus Need invalidity, both family
directions, zero inspection/horizon/preparation/
evaluation/source/load/query activation, child-only MODULE event order and warm
suppression, genuinely polled cancellation/recovery, and route/source
edit/delete/recreate plus A/B/A. Include compact-retention/Allocative and AI
cleanup reviews.

Run focused tests, full `slug_bzlmod_v2`, loading and query suites, established
core baselines, fmt/check/diff/accounting, compact-retention review and
independent latest-diff review serially. After implementation ACCEPT, return
only to the docs-only observed inspection/preparation/evaluation and upper
source/load owner audit; do not activate query or close M1.

## STOP / REPLAN

STOP on every other file, public export/caller, inspection/include/preparation/
evaluation or upper source/load/query activation, mixed route/source families,
partial/rebuilt epochs, moved/duplicate events, retained scratch, new state
owner, direct Host read, cap excess, fallback or M1 closure. `REPLAN` if the
file sibling cannot preserve exact child Arcs with the two-file envelope, needs
another owner/API, or changes legacy semantics or event ownership.
