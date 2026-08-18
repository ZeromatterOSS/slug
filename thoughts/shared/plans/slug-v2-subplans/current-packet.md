# Current Slug V2 Packet

Packet: `WP-2A-m1-loading-query-observed-publication-implementation`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Rust base: `a9270586`
Accepted design: `44c1b444`
Result: implement and validate only observed native loading-query publication.

## Authority and caps

Write exactly:

1. `app/slug_query_v2/src/evaluator.rs`: +170 production/+20 colocated proof,
   <=417 physical;
2. `app/slug_query_v2/src/loading_environment.rs`: +360/+60, <=2,346;
3. `app/slug_query_v2/src/graph.rs`: +520/+100, <=3,771;
4. `app/slug_query_v2/src/lib.rs`: +4, <=81;
5. new `app/slug_query_v2/tests/observed_loading_query.rs`: +760 tests,
   <=780;
6. `app/slug_core_v2/src/runtime/dice.rs`: +100 production/+12 test glue,
   <=11,000 after exact relocation of base lines 7,318-8,036; and
7. new `app/slug_core_v2/src/runtime/tests/query_command_tests.rs`: exact
   719-line relocation plus <=360 proof, <=1,120 physical.

Caps against `a9270586` are +1,154 production, +1,312 tests and +2,466
aggregate semantic lines, 19,515 combined physical. Replace the contiguous
query-test range only with
`mod query_command_tests { include!("tests/query_command_tests.rs"); }`; the
included file starts with `use super::*;` and all relocated bodies remain byte
exact. Existing large query/graph/core files are cohesive owner exceptions;
touched helpers stay below 200 lines.

## Required implementation

Add doc-hidden public structural `RootQueryCommandObservationKey` and
`ObservedRootQueryCommand`, plus private observed root/external graph and
root-subtree siblings. Use shared Legacy/Observed root, graph and subtree
drivers and one mode-aware ephemeral `LoadingQueryEnvironment`. Direct and
one-shot query APIs remain legacy; only the existing native public query
constructor selects the observed root.

The root Value is
`LoadingPreparationOutcome<Result<ObservedRootQueryCommand,
ObservedPathFrontierError>>`; its carrier retains the exact existing query
Result Arc plus compact epoch. Each graph/subtree DICE sibling retains exactly
one natural local Result Arc plus epoch. Root retains no child carrier Arc; all
carriers implement `Allocative` and `Dupe`. Environment, arena, resolved graph,
traversal/listing vectors and event/union scratch stay compute-local.

Preserve anchor-first and exact evaluator/callback order. Observed mode selects
only accepted anchor/route/root-or-repository-load/boundary/listing/resolution
siblings. Merge each Complete child epoch left-first before semantic
inspection; equal duplicates keep the first exact Arc and conflict/mismatch is
typed outer. Sequential first Need/outer/semantic stops immediately; semantic
error keeps the reached prefix. Reuse private Need and outer sentinel channels.
Issued subtree joins scan the full input order and choose first outer/epoch
error > combined compatible Need > first semantic > success.

Root/graph/subtree/environment siblings are eventless; child keys remain sole
batch owners. Need/outer/cancel publishes none and warm reuse is suppressed.
Implement the observed `NativeCommandRoot`, expose its epoch to existing exact
selected-snapshot validation, and consume/project through
`AcceptedCommand::map_terminal` without changing the public query Result Arc or
event buffer. Add no revision, certificate, cache/store/lock/task, Host read or
second event/publication owner, and no interner.

## Proof and terminal

Prove identity/equality/validity/Allocative/Dupe and exact semantic Arc;
anchor and every environment terminal prefix; root/external graph projection;
every subtree boundary/marker/listing batch position, Need union, first Arc,
conflict/mismatch and BFS order; direct/external/recursive/visibility/
buildfiles/loadfiles/generated queries; exact selected epoch/Arc identity;
public Result-Arc identity; exact cold/error child events and warm suppression;
both family directions and concurrent isolation; cancellation/recovery;
root/external BUILD and `.bzl` A-B-delete-recreate-A; retained lifetimes; and
zero upper-build activation.

Exact public query behavior and all legacy/direct APIs remain exact. Private
observation/outer/selected association is Slug-native. One-shot workspace
evaluation, external exported-source publication, multi-build aggregation,
unsupported query breadth and exact identity bytes remain deferred.

Run focused proof, full query/loading/bzlmod/core validation, fmt, diff-check,
exact accounting, retention/cleanup and independent review serially. STOP on
any other file/caller, semantic/order/family/event drift, public API expansion,
retained scratch, relocation/body drift, cap excess or M1 closure. REPLAN if
the exact epoch needs another owner/state, private outer cannot remain private,
existing Need kinds cannot union without inventing a new query error, or the
bounded scope cannot hold. After ACCEPT commit and return to exactly one
docs-only next-owner audit; do not close M1.
