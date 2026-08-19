# Current Slug V2 Packet

Packet: `WP-2A-m1-external-singleton-event-current-closure-correction-design`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Scheduling base: `98b6d787`
Accepted Rust base: `a4dd40d6`
Accepted external-build design: `1a217e2a`
Accepted loading-proof correction: `ce110d9a`
Accepted revision-event design: `5dabd4bf`
Superseded root-association design: `340159c0`
Result: freeze the exact current-closure reconciliation that distinguishes a
transient retry `None` from a removed child event owner.

## Exact docs authority and measured stop

Write exactly canonical/current/Stage/routing under 40/220/180/30 net and 470
aggregate against `98b6d787`. Retain but do not write the dirty four-file Rust
candidate in `runtime/{dice,events}.rs`,
`runtime/tests/build_command_tests.rs`, and
`slug_loading_v2/src/host_package_load_tests.rs`. Cargo, BUILD, fixtures,
oracles, server tests, generated evidence, callers, exports and public behavior
are forbidden.

Exact retry-3 instrumentation disproves the accepted root-transition premise.
Cold, warm and source-edit attempts have the same ordered root, but that root is
always `Known(None)`, never `NoTransition`. On source edit all prior event
nodes remain in the current closure: unchanged nodes are `NoTransition`,
while the equal package BUILD owner is `Known(None)`. Normal reconciliation
tombstones it and drops the accepted entry before the later delete replay.

This is command-local event lineage at native revision acceptance, not a
build-root, child-owner, path-epoch or key-equality defect. Root IDs remain the
smallest cross-command association, but the first revision carry must classify
each prior event node by current closure membership and transition.

## Frozen current-closure carry contract

Keep compact ordered accepted roots plus Some-only accepted entries, exact
selected closure roots/transitions, and the command-local optional-batch
provisional slice. Normal commands remain unchanged and drop absent nodes.

On the first source-certificate revision retry:

1. if exact ordered current roots differ from accepted roots, use ordinary
   reconciliation and seed nothing;
2. if roots match, iterate nodes in exact current closure order;
3. a prior event node with current `Known(Some(batch))`, including empty, uses
   the current batch;
4. a prior event node with current `Known(None)` or `NoTransition` carries
   its prior Some batch;
5. a prior event node absent from the current closure is dropped; and
6. a new current node contributes only `Known(Some(batch))), in current
   closure order.

Do not preserve prior order when the current closure reorders event owners.
After the first carry, retain the frozen retry algebra: later final Known(Some)
or Known(None) overrides, final NoTransition uses carry then true prior,
retry-only nodes retain retry order, final-only nodes append, Needs preserve
the fixed-root carry, multiple retries keep first order/latest transition, and
only the final effective nonempty delta emits. Accepted roots/Some-only entries
replace atomically only after materializer acceptance; every failure drops
command-local state.

Freeze the producer invariant for admitted source-certified roots: every
semantic-Complete event-owning child stores `Some(EventBatch)`, including
`Some(empty)`; Need/typed outer stores none and cannot be accepted. Therefore
a present prior event node's first-revision `Known(None)` is transient
lineage, while semantic removal is represented by exact absence from the
current closure. STOP/REPLAN if any admitted child violates this invariant.

Retain only Dupe/Allocative ordered root and entry Arc slices. Current-order
maps/Vecs are compute-local. Add no retained closure/dependency graph/map,
cache, store, interner, lock, task, Host read, child carrier, event owner or
historical snapshot, and hold no lock across DICE.

## Retry authority, proof and compatibility

After independent design ACCEPT, schedule exactly
`WP-2A-m1-external-singleton-observed-build-implementation-retry-4` with the
same four Rust files and unchanged corrected caps: DICE +340/11,350, build proof
+440/3,450, loading zero/3,439, events +100 production/+160 tests/2,050;
aggregate <=1,040 semantic and <=20,289 physical against `a4dd40d6`.

Preserve every external owner/order/prefix/certificate/repository/family/event,
legacy infrastructure, tombstone/Need/multiple-retry/atomicity and compatibility
contract from `1a217e2a`, `ce110d9a`, and `5dabd4bf`. Remove temporary
trace logging during implementation.

Required proof: one mixed current-closure table with prior+KnownNone carry,
prior+NoTransition carry, prior+changed Some, prior+Some(empty), absent prior
drop and new Some, asserting exact current order; root mismatch; reordered
same-root nodes; simultaneous `.bzl` removal plus source change drops the
removed owner while retaining equal siblings; changed/reordered BUILD emits in
current order; final Some/None/NoTransition, revision->Need, multiple retries
and failure atomicity; accepted epoch membership after source edit; changed
BUILD/`.bzl` replay, root switch and unchanged server lifecycle.

Exact: public build values/errors, child event text/order and every legacy/root
route. Slug-native: observed carrier/certificate/repository, accepted root IDs
and current-closure retry-event association. Unsupported/deferred: multi-build,
one-shot, broader actions, external globs and exact Bazel identity bytes.

STOP now on Rust or any file outside the four docs, child filtering,
path/key-equality weakening, treating initial KnownNone as removal for a present
prior source-certified event node, carrying an absent prior node, prior-order
replay, retained closure/map, behavior/family/order drift, cap excess, broader
activation or M1 closure. The design may schedule only the one bounded retry.
