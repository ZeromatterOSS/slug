# Current Slug V2 Packet

Packet: `WP-2A-m1-direct-local-module-inspection-observation-implementation`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Scheduling/design base: `e7b705a9`
Rust base: `99d78875`
Result: implement only the accepted observed direct-local MODULE inspection
producer.

## Authority

Write only:

- `app/slug_bzlmod_v2/src/source_preparation.rs`
- `app/slug_bzlmod_v2/src/source_preparation_observation_tests.rs`

Caps against Rust base `99d78875`: 140 production and 12,900 physical lines
in `source_preparation.rs`; 300 tests and 1,175 physical lines in the proof
file; 440 aggregate semantic and 14,075 combined physical.

## Required implementation

Keep `DirectLocalModuleInspectionKey` and its Value exact. Add one
structurally distinct crate-private observed sibling/carrier and one mode-aware
inspection driver. Legacy computes only `DirectLocalModuleFileKey`; observed
computes only `DirectLocalModuleFileObservationKey`. Do not add a caller or
activate horizon, preparation, evaluation, source/load or query.

The observed carrier retains exactly one local semantic inspection Result
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

Extend only the existing proof file for identity/Display, exact legacy parity,
valid and invalid MODULE inspection, Absent/Present, InputCompute-empty and every file
Need/outer/semantic prefix, exact epoch/result Arcs, both family directions,
zero horizon/preparation/evaluation/source/load/query activation, child-only
events and warm suppression, real polled cancellation/recovery, and
edit/delete/recreate plus A/B/A. Require a pointer-discriminating projection
test rather than comparing one cache hit with itself.

Run focused tests, full bzlmod/loading/query, established core baselines,
fmt/check/diff/accounting, Buck2 retention and AI cleanup, then one independent
latest-diff review. After ACCEPT, commit and return only to a docs-only
include-horizon/preparation/evaluation and upper source/load audit.

## STOP / REPLAN

STOP on every other file, Cargo/BUILD/fixture/oracle writes, public export or
caller, upper activation, mixed families, rebuilt/partial epochs, moved events,
additional retained collection/state, direct Host reads, cap excess, multiple
successors or M1 closure. `REPLAN` if inspection performs a new path
observation, exact child Arcs cannot survive unchanged, another file is
required, or legacy semantics/event ownership must change.
