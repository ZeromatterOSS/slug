# Current Slug V2 Packet

Packet: `WP-2A-m1-direct-local-preparation-observation-implementation`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Scheduling/design base: `9cdcf9e0`
Rust base: `79248832`
Result: implement only the accepted private observed direct-local preparation
sibling.

## Authority and caps

Write only:

- `app/slug_bzlmod_v2/src/source_preparation.rs`
- `app/slug_bzlmod_v2/src/source_preparation_observation_tests.rs`

Against `79248832`: at most 430 production net and 13,600 physical lines in
the source owner; 550 test net and 2,130 physical lines in the proof file; 980
aggregate semantic and 15,730 combined physical.

## Required implementation

Keep `DirectLocalModulePreparationKey` and its Value exact. Add one private
structural `DirectLocalModulePreparationObservationKey` and carrier retaining
exactly one local semantic preparation Result Arc plus one cumulative
`PathObservationEpoch`. Use one mode-aware preparation driver; legacy
projects the exact local Arc and neither key computes the other family.

Observed order is accepted inspection, root validation, then at each BFS level
the shared horizon/package driver over that level's current request slice and
accumulated prefix, followed by unique
`HostRepositorySourceFileObservationKey` children in first-occurrence order,
fragment validation, cycle handling, and next-frontier construction. Do not
compute the standalone horizon observation key inside preparation. Keep Host
nonregistry preparation legacy-only.

Merge every Complete child epoch left-first before semantic inspection.
InspectionCompute is empty; inspection semantic/root validation use inspection
prefix; horizon uses its accepted prefixes; SourceCompute uses the prior
prefix; source semantic/Absent/validation use the merged child prefix; cycle
and success use the full reached epoch.

For a joined source batch, bound choice through the first semantic child:
first typed outer/union error in that prefix, then an earlier Need with the
deterministic union of all batch Needs, then the semantic snapshot. Later
outcomes stay dependency-owned. Without semantic, full-batch outer/union wins
over combined Need, otherwise Need, otherwise success. Need/outer has no
carrier. Need is
invalid/self-unequal; Complete outer compares by outer value; Complete carrier
compares semantic Result plus epoch.

Preparation stays eventless and cancellation-local. Existing children retain
event ownership; evaluation/support/RepositoryPackageSource/
ExternalBzlModuleEval/RepositoryPackageLoad/query/build remain dormant. Retain
only the existing semantic closure within the Result Arc plus the compact
epoch. Frontier, ancestry, outcome maps, Need union, snapshots and parser
scratch remain compute-local. Add no child Result Arc, state/store/cache,
interner, lock/task, direct Host read, request revision, source certificate,
export, caller, or event owner.

Keep the cohesive source owner and existing proof file; split bounded helpers
so no touched function exceeds 200 lines.

## Compatibility and proof

Exact: MODULE/include bytes, parsing, labels, BFS/fragment/cycle order,
preparation values/errors, source selection, legacy behavior and child events.
Slug-native: sibling/carrier, typed outer, epoch order and retry/cancellation.
Deferred: evaluation, upper source/load, public query/build, other families and
exact identity bytes.

Prove identity/Display; exact legacy semantic/Arc projection; empty/single/
duplicate/nested/cycle and root/fragment validation; every
inspection/horizon/source compute/Need/outer/semantic position; the three mixed
batch precedence cases and full Need union; exact inspection->horizon->source
epoch order/membership/first Arc; equal duplicate/conflict/operation mismatch;
complete equality/validity; both family directions and Host isolation; zero
upper activation; child-only event order/warm suppression; real polled
cancellation/recovery; edit/delete/recreate and A/B/A; compact Allocative
retention and cleanup categories.

Run focused tests, full bzlmod/loading/query, established core baselines,
fmt/check/diff/accounting, Clippy/archive disposition, Buck2 retention and AI
cleanup, then one independent latest-diff review. After ACCEPT, commit and
return only to a docs-only evaluation/upper-source publication owner audit.

## STOP / REPLAN

STOP on every other file, Cargo/BUILD/fixture/oracle write, a third
file/export/caller, standalone-horizon recomputation, Host-family change, mixed
families, moved/duplicate events, partial/rebuilt epochs, retained traversal
state, upper activation, cap excess, multiple successors or M1 closure.
`REPLAN` if accepted producers cannot be reused, exact Arcs cannot survive,
another retained owner/file is required, caps cannot hold, or legacy
semantics/event authority must change.
