# Current Slug V2 Packet

Packet: `WP-2A-m1-direct-local-module-inspection-observation-design`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Scheduling/Rust base: `99d78875`
Result: design only the smallest complete observed direct-local MODULE
inspection producer after the accepted file carrier.

## Authority

Write only:

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`
- `thoughts/shared/plans/slug-v2-subplans/current-packet.md`
- `thoughts/shared/plans/slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
- `.codex/skills/slug-agent-orchestration/references/routing-log.md`

Docs caps against `99d78875`: 40 canonical, 180 current, 220 Stage 2, 30
routing and 470 aggregate net lines. Rust, Cargo, fixtures and oracles are
read-only.

## Required design

Audit the live `DirectLocalModuleInspectionKey` boundary and freeze one
structurally distinct crate-private observed sibling/carrier only if it is the
first complete natural owner. The legacy key must continue to compute only
`DirectLocalModuleFileKey`; the observed key must compute only
`DirectLocalModuleFileObservationKey`. Use one mode-aware inspection driver
and do not activate horizon, preparation, evaluation, source/load or query.

The observed carrier should retain exactly one local semantic inspection Result
Arc plus the child's unchanged `PathObservationEpoch`. Inspection adds no path
read: forward the complete file epoch without rebuilding or unioning it.
File Need and typed outer return immediately with no carrier. InputCompute has
an empty epoch; file semantic error, inspection parse error, Absent and Present
retain the complete file epoch. Preserve legacy error projection and
Complete-only equality/validity: Need invalid/self-unequal, typed outer by outer
error, carrier by semantic result plus epoch.

The inspection parent remains eventless. Root-module and Host-source child event
ownership remains unchanged. Retain no child semantic Arc or additional
collection outside the exact semantic Result Arc; AST/parser scratch remains
compute-local. Add no store/cache/interner, lock/task, direct Host read, request
revision or source certificate. Keep the sibling private to
`source_preparation.rs`.

## Compatibility

MODULE parsing, Absent/Present inspection values, semantic errors and child
events remain exact Bazel 9 admitted behavior. The structural sibling/carrier
and typed outer frontier are Slug-native. Include horizon, preparation,
evaluation, upper source/load/query/publication and identity bytes remain
unsupported/deferred.

## Proof and validation

Freeze proof for identity/Display, exact legacy parity, valid and invalid MODULE
inspection, Absent/Present, InputCompute-empty and every file
Need/outer/semantic prefix, exact epoch/result Arcs, both family directions,
zero horizon/preparation/evaluation/source/load/query activation, child-only
events and warm suppression, real polled cancellation/recovery, and
edit/delete/recreate plus A/B/A. Require a pointer-discriminating projection
test rather than comparing one cache hit with itself.

If accepted, future Rust may write only
`app/slug_bzlmod_v2/src/source_preparation.rs` and
`app/slug_bzlmod_v2/src/source_preparation_observation_tests.rs`. Proposed
caps against `99d78875`: 140 production and 12,900 physical lines in the
owner, 300 tests and 1,175 physical lines in the proof file, 440 aggregate
semantic and 14,075 combined physical. Schedule exactly one implementation,
then return to a docs-only include-horizon/preparation/evaluation and upper
source/load audit.

## STOP / REPLAN

STOP on Rust/Cargo/BUILD/fixture/oracle writes, implementation, public export or
caller, upper activation, mixed families, rebuilt/partial epochs, moved events,
retained scratch/state, direct Host reads, multiple successors or M1 closure.
`REPLAN` if inspection is not the first complete natural owner, it performs a
new path observation, exact child Arcs cannot survive unchanged, another file
is required, or legacy semantics/event ownership must change.
