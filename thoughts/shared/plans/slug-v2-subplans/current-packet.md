# Current Slug V2 Packet

Packet: `WP-2A-m1-direct-local-preparation-observation-design`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Scheduling base: `b7110fe5`
Rust base: `79248832`
Result: freeze only the private observed direct-local preparation sibling at
the accepted natural owner.

## Authority

Write only:

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`
- `thoughts/shared/plans/slug-v2-subplans/current-packet.md`
- `thoughts/shared/plans/slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`

Docs caps against `b7110fe5`: 40 net canonical, 220 net manifest, 180 net
Stage 2, and 440 aggregate. Write no Rust, Cargo, BUILD, fixture, oracle, or
generated artifact.

## Audit result and evidence

The audit accepts `DirectLocalModulePreparationKey` as the uniquely smallest
complete owner. It owns root inspection/validation, the recursive breadth
frontier, include ancestry and first-cycle capability, horizon selection,
fragment-source reads and validation, and the final ordered closure. The
fragment read helper owns no reusable semantic fact; a new fragment DICE key
would duplicate the accepted per-file source producer or retain evolving
frontier scratch.

Bazel 9.2 `ModuleFileFunction.advanceHorizon` owns package lookup, fragment
reads/parsing, and next-horizon creation
(`ModuleFileFunction.java:343-359,382-509`), with discriminating include
coverage in `ModuleFileFunctionTest`. Buck2 DICE dependency/equality and
cancellation behavior is represented by `docs/developers/dice.md`. Reuse
existing source-preparation/loading/query tests; add no fixture. Donor
classification is concept/test only; no donor scheduler, cache, or side store.

## Frozen design

Add one private structural
`DirectLocalModulePreparationObservationKey(DirectLocalModulePreparationKey)`
and `ObservedDirectLocalModulePreparation` retaining exactly one local
semantic preparation Result Arc plus one cumulative `PathObservationEpoch`.
One mode-aware preparation driver preserves the legacy key/value and projects
the exact local Result Arc; neither key computes the other family.

Observed order is:

1. accepted observed direct-local inspection;
2. root validation;
3. for each BFS level, the accepted shared horizon/package driver over that
   level's current request slice and accumulated prefix;
4. unique fragment sources in first-occurrence order through
   `HostRepositorySourceFileObservationKey`;
5. fragment validation, ancestry/cycle handling, and next frontier.

Do not compute the standalone horizon observation key inside preparation: its
identity re-inspects the root request set and is not the recursive frontier.
Pass each current request slice and cumulative prefix to the accepted shared
horizon driver. Host nonregistry preparation remains legacy-only.

Merge every Complete child epoch left-first before semantic inspection.
InspectionCompute has an empty epoch; inspection semantic and root validation
retain the inspection prefix. Horizon terminals retain their accepted prefix.
For each fragment batch, SourceCompute uses the prior prefix; source semantic,
Absent, and validation use the merged child prefix. Cycle-capability and normal
success retain the full reached epoch.

For a joined fragment batch, identify the first semantic terminal in
first-occurrence order. Through that decisive child, first typed outer/epoch
union error wins, then an earlier Need returns the deterministic union of all
batch Needs, then the semantic with its snapshot; later outcomes remain
dependency-owned. With no semantic, inspect the full batch: first typed
outer/union error wins over the combined Need, otherwise Need, otherwise
success. Need/typed outer carry no carrier. Complete equality/validity is:
Need invalid/self-unequal, typed outer valid/equal by outer value, carrier
valid/equal by semantic Result plus epoch.

Preparation remains eventless. Inspection/root-module/routing/source children
remain their existing event authorities; `DirectLocalModuleEvaluationKey`
remains the sole direct evaluation batch owner. Do not activate evaluation,
`direct_local_module_support`, `RepositoryPackageSourceKey`,
`ExternalBzlModuleEvalKey`, `RepositoryPackageLoadKey`, query, build, or a
public caller.

The semantic Result may retain its existing root/fragment closure, bytes,
inspections, ancestry-derived cycle capability, and Arc-backed collections.
Retain no additional collection outside that Result Arc and epoch, no child
semantic Result Arc, and no frontier/outcome map/Need accumulator/prefix
snapshot/parser scratch after compute. Add no store, cache/interner, lock,
task, direct Host read, request revision, source certificate, or event owner.
Cancellation drops only compute-local state and publishes no parent batch.

The existing 13,133-line source module remains a cohesive exception because it
already owns the private preparation types and shared recursive driver; a
production split would expose or duplicate that ownership. Keep new proof in
the existing observation test file and split helpers so no touched function
exceeds 200 lines. Future Rust authority is exactly:

- `app/slug_bzlmod_v2/src/source_preparation.rs`: at most 430 production net,
  at most 13,600 physical lines;
- `app/slug_bzlmod_v2/src/source_preparation_observation_tests.rs`: at most
  550 test net, at most 2,130 physical lines;
- at most 980 aggregate semantic and 15,730 combined physical lines against
  `79248832`.

## Compatibility and proof

Exact: MODULE/include bytes, parsing, labels, BFS/fragment/cycle order,
preparation values/errors, child source selection, legacy behavior and child
events. Slug-native: the structural sibling/carrier, typed observed outer,
epoch order, complete-only equality and retry/cancellation mechanics.
Unsupported/deferred: evaluation, upper source/load, public query/build
publication, other repository families and exact identity bytes.

Require discriminating proof for key identity/Display; exact legacy semantic
and Result-Arc projection; empty/single/duplicate/nested horizons; first cycle;
root and fragment validation; every inspection/horizon/fragment
compute/Need/typed-outer/semantic position; earlier semantic plus later
Need/outer, earlier Need plus semantic plus later outer, no-semantic
outer-over-Need and full Need union; exact inspection->level horizon->fragment
epoch membership/order/first Arc; equal duplicate/conflict/operation mismatch;
complete validity/equality; both family directions and Host-family isolation;
zero evaluation/support/source/load/query activation; child-only event order
and warm suppression; real poll-drop-successor cancellation; create/edit/
delete/recreate and A/B/A; Allocative/retention scan and cleanup categories.

After implementation, run focused tests, full bzlmod/loading/query, established
core baselines, fmt/check/diff/accounting, Clippy/archive disposition, Buck2
retention and AI cleanup, then one independent latest-diff review.

## STOP / REPLAN

STOP on any non-doc write now, a future third Rust file/export/caller,
standalone-horizon recomputation, host-family change, mixed families, moved or
duplicate events, partial/rebuilt epochs, retained traversal scratch/state,
upper activation, cap excess, multiple successors, or M1 closure. `REPLAN`
if preparation cannot reuse the accepted horizon/source producers, exact child
Arcs cannot survive, the recursive algebra needs another retained owner, the
two-file caps cannot hold, or legacy semantics/event authority must change.
After independent design ACCEPT, schedule exactly one bounded implementation;
otherwise record the blocking evidence.
