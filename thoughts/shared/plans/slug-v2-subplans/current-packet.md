# Current Slug V2 Packet

Packet: `WP-2A-m1-loading-query-observed-publication-design-resume`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Scheduling base: `9d17ca1b`
Rust base: `a9270586`
Result: freeze the complete observed native loading-query publication boundary;
do not implement it.

## Authority and caps

Write exactly:

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`: <=40 net;
- this manifest: <=220 net;
- `thoughts/shared/plans/slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`:
  <=200 net;
- `.codex/skills/slug-agent-orchestration/references/routing-log.md`: <=30 net.

Aggregate docs growth is <=490 net lines against `9d17ca1b`. Do not write
Rust, Cargo/BUILD metadata, fixtures, oracles or generated artifacts. Do not
activate a caller, change public behavior, or close M1.

## Natural owner and boundary

Freeze `RootQueryCommandKey` as the sole complete native query publication
owner. It owns anchor -> parsed-query evaluation -> graph/output completion and
one semantic query Result Arc; its `LoadingQueryEnvironment` is the sole
aggregation point for root/external graph requests, package provenance,
recursive subtree expansion and BUILD companion lookup. Generic
`drive_command` remains the only retry, selected-snapshot and publication
owner.

Add a doc-hidden public structural `RootQueryCommandObservationKey` and
`ObservedRootQueryCommand`. Add private observed siblings only for
`RootUnconfiguredPackageGraphKey`, `ExternalUnconfiguredPackageGraphKey` and
`RootSubtreePackageSetKey`. These graph/subtree siblings and one mode-aware
query environment are cohesive mechanisms of the root packet: none covers all
query edges or publishes a command, and splitting them would leave callerless
partial carriers around the same ephemeral environment.

The accepted anchor, route, root/repository package load, package boundary,
directory-listing and resolved-path siblings now cover every Host edge. The
external source/load gap that forced the earlier REPLAN is closed through
`a9270586`. Keep one-shot/direct query APIs legacy.

Core external exported-source build remains separate and deferred. Its owner
performs route -> package -> selected source, has additional FileBytes,
target-kind and certificate/revision concerns, and currently returns no source
certificate. Do not combine it with certificate-free query publication.

## Carrier, order and terminal algebra

The observed root Value is exactly
`LoadingPreparationOutcome<Result<ObservedRootQueryCommand,
ObservedPathFrontierError>>`. The carrier retains the existing exact
`Arc<Result<QueryOutput, QueryError>>` plus one compact
`PathObservationEpoch`. Private graph/package-set carriers likewise retain
only their natural local semantic Result Arc plus one epoch.

Use shared Legacy/Observed drivers for root command, graph projection and
subtree traversal. Legacy selects only current keys. Observed selects only the
accepted observed anchor/route/load/boundary/listing/resolution families.
Order is anchor first, then the unchanged parsed-query evaluator/callback
order. At every route, package, graph, provenance, boundary, marker and listing
completion, merge the Complete child epoch left-first before semantic
inspection. Equal duplicates keep the first exact Arc; conflict or operation
mismatch is typed outer.

Sequential work stops at the first Need, typed outer or semantic terminal in
existing evaluator order. Need/outer is carrierless and discards the attempted
epoch; semantic query error retains the reached prefix. Reuse the private
restart sentinel for compatible Need aggregation and a parallel private outer
sentinel/channel; neither may escape as public `QueryError`.

For an already-issued subtree marker/listing `compute_join`, inspect the full
input-ordered batch, merge every Complete epoch, then choose first typed outer
or epoch error > combined compatible Need > first semantic > success. Preserve
BFS/root/basename/child ordering. REPLAN rather than inventing a new query
error if existing Need kinds cannot union.

## Events, publication and retention

Root/graph/subtree/environment siblings are eventless. Anchor, package, Bzl and
BUILD children remain sole local batch owners. Semantic completion preserves
their exact public order; Need/outer/cancellation publishes none and warm reuse
emits none.

Implement `NativeCommandRoot` only for the observed sibling, expose its epoch
through `observations()`, use existing selected-epoch exact-Arc validation, and
consume/project through existing `AcceptedCommand::map_terminal`. The public
`AcceptedCommand<Arc<Result<QueryOutput, QueryError>>>` and event buffer remain
unchanged. Query initializes no request revision and owns no source
certificate.

Retain only the semantic query Result Arc plus compact epoch at the root; it
retains no child carrier Arc. Each graph/subtree DICE sibling retains exactly
its one natural local semantic Result Arc plus compact epoch. Only the
ephemeral environment, candidate arena, resolved graph, traversal/listing
vectors and event/union scratch stay compute-local. All root and private DICE
carriers implement `Allocative` and `Dupe`. Add no cache, interner, store,
lock, task, Host read or second event/publication owner.

## Future implementation boundary

Against Rust base `a9270586`, future Rust is exactly:

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

Replace that contiguous query-test range with only
`mod query_command_tests { include!("tests/query_command_tests.rs"); }`; the
included file begins with `use super::*;`. Relocated bodies remain byte exact.
Caps are +1,154 production, +1,312 tests, +2,466 aggregate semantic lines and
19,515 combined physical lines. Existing large query/graph/core files are
cohesive owner exceptions; touched helpers remain below 200 lines.

## Proof, compatibility and terminal

Require identity/Display/equality/validity/Allocative and exact semantic Arc
projection; anchor and every environment edge Need/outer/semantic prefix;
root/external graph projection; every subtree boundary/marker/listing batch
position, Need union, first Arc/conflict/mismatch and BFS order; direct,
external, recursive, visibility, buildfiles/loadfiles and generated-file
queries; exact selected epoch values and per-demand `Arc::ptr_eq`; public
Result-Arc identity; cold anchor/Bzl/BUILD event order, semantic-error batches
and warm suppression; both family directions plus concurrent isolation;
poll-drop/no-publication/recovery; root/external BUILD and `.bzl`
edit/delete/recreate A-B-A; retained bytes/manifest/result lifetimes; zero
upper-build activation; exact caps, retention, cleanup and full dependent
validation.

Exact compatibility is public query values/errors/order/events and all legacy
direct APIs. Sibling/carrier/epoch/typed-outer/selected validation is
Slug-native. One-shot `evaluate_workspace_targets`, external exported-source
publication/certificates, multi-build aggregation, unsupported query breadth
and exact identity bytes remain unsupported/deferred.

STOP on any Rust now; another file/caller/public owner; semantic/order/family/
event drift; retained scratch; cap excess; or M1 closure. REPLAN if exact epoch
association needs another owner/state, outer cannot remain private, relocation
is not exact/buildable, or the bounded caps cannot hold. After independent
design ACCEPT, schedule exactly one implementation successor and nothing else.
