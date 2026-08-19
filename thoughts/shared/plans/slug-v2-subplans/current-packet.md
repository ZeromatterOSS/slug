# Current Slug V2 Packet

Packet: `WP-2A-m1-external-singleton-observed-build-implementation-retry-4`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Scheduling base: `56ed9923`
Accepted Rust base: `a4dd40d6`
Accepted external-build design: `1a217e2a`
Accepted loading-proof correction: `ce110d9a`
Accepted revision-event design: `5dabd4bf`
Accepted current-closure correction: `56ed9923`
Result: publish one nonroot exported-source build through the observed root
with exact current-closure event reconciliation across revision retries.

## Exact Rust authority and caps

Write exactly:

1. `app/slug_core_v2/src/runtime/dice.rs`: <=340 production net and <=11,350
   physical;
2. `app/slug_core_v2/src/runtime/tests/build_command_tests.rs`: <=440 test net
   and <=3,450 physical;
3. `app/slug_loading_v2/src/host_package_load_tests.rs`: only the accepted
   line-neutral assertion, zero net and <=3,439 physical;
4. `app/slug_core_v2/src/runtime/events.rs`: <=100 production plus <=160 tests
   and <=2,050 physical.

Aggregate semantic <=1,040 and combined physical <=20,289 against
`a4dd40d6`. No other file/loading byte. Planning docs, Cargo, BUILD, fixtures,
oracles, generated evidence, exports, callers and server-test edits are
forbidden. Remove all temporary trace logging. Touched helpers stay below 200.

## Frozen implementation contract

Preserve structural observed admission, exact matching-family legacy/observed
external driver, anchor -> route -> package -> ExportedFile classification ->
RequestRevisionKey -> source order, union-before-semantic left-first epochs,
all prefixes, exact source-child certificate, external-only repository
selection, full selected value/Arc validation and exact legacy infrastructure
projection. Need/typed outer is carrierless; every failure preserves accepted
state. The root owns no batch; matching child keys remain sole owners.

`AcceptedEventEpoch` retains exact ordered roots plus Some-only event entries;
`SelectedEventState` captures current roots/transitions; the command-local
provisional slice retains roots plus optional-batch entries. Normal commands
retain ordinary closure reconciliation.

On the first source-certificate revision retry, require exact ordered root
equality; root mismatch seeds nothing. With matching roots, iterate exact
current closure order:

1. prior event node + current Known(Some), including empty, uses current;
2. prior event node + current Known(None) or NoTransition carries prior Some;
3. prior event node absent from current closure drops;
4. new current node contributes only Known(Some), in current order.

This relies on and must prove the admitted producer invariant: every
semantic-Complete event-owning child stores Some(batch), including Some(empty);
Need/outer stores none and cannot be accepted. Therefore first-revision
Known(None) on a present prior node is transient lineage and exact closure
absence is semantic removal.

After first carry, final Known(Some/None) overrides, NoTransition uses carry
then true prior, retry-only/current-only order remains exact, later Needs retain
the fixed-root carry, multiple retries keep first order/latest transition, and
only final changed nonempty Some batches emit. Replace accepted roots/Some-only
entries only after materializer acceptance. Outer/cancel/abort/selection/
revision/materializer failure drops carry and changes no accepted state.

Retain only one build Result Arc, compact path/certificate epochs and compact
Dupe/Allocative accepted/provisional root/entry slices. Closure/dependency
graph, child carriers, selected paths, event/union maps/Vecs and repository
sidecars stay compute-local or dependency-owned. Add no retained map/cache/
store/interner/lock/task, Host read, child carrier, event owner or snapshot.

## Loading, compatibility, proof and STOP

In `host_package_load_tests.rs`, preserve the sole accepted line-neutral
core-positive `RepositoryPackageLoadObservationKey` assertion and every other
byte.

Exact: public build values/errors/classification, child event text/order and all
legacy/root routes. Slug-native: observed carrier/certificate/repository,
accepted root IDs and current-closure retry association. Unsupported/deferred:
multi-build, one-shot, broader actions, external globs and exact Bazel identity
bytes.

Preserve all routing/family/prefix/Arc/certificate/repository/lifecycle/
cancellation/rollback proof. Add a mixed current-closure reducer table covering
prior KnownNone/NoTransition carry, changed Some, Some(empty), absent prior drop
and new Some in exact current order; root mismatch and reorder; simultaneous
`.bzl` removal plus source change; changed/reordered BUILD; final transitions,
revision->Need, multiple retries and failure atomicity. The accepted epoch must
still contain the package event after source edit/delete/directory/recreate;
changed BUILD/`.bzl` replays, external->root suppresses, and the unchanged
server lifecycle passes except recorded inherited query baselines.

Run focused event/build, 33/33 build, loading 138/138, full bzlmod, documented
core/query/server baselines, fmt/diff, exact caps, Buck2 retention, AI cleanup
and independent final review.

STOP on every other file/loading byte, child filtering, path/key equality
weakening, carrying an absent prior node, treating first-revision present-prior
KnownNone as removal, prior-order replay, producer-invariant failure, retained
closure/map, behavior/family/order drift, cap excess, broader activation or M1
closure. REPLAN on any new blocker. After ACCEPT return only to one docs-only
remaining M1 owner audit.
