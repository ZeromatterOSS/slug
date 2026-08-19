# Current Slug V2 Packet

Packet: `WP-2A-m1-loading-query-event-acceptance-epoch-correction-design`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Scheduling base: `0568f845`
Rust base: `a9270586`
Retained query candidate authority: `44c1b444`, `e22404a8`, `1f2fb3f6`
Result: freeze one native-command accepted event epoch before resuming the same
loading-query implementation retry.

## Exact docs-only authority

Write only canonical Live Status, this manifest, Stage 2 and the Slug routing
log, within 40/220/180/30 net lines and 470 aggregate. The existing eight-file
Rust candidate remains retained but non-writable. Stop Cargo, BUILD, fixture,
oracle, Rust, public/caller activation and M1 closure.

## Learned failure and natural owner

The stable-parent external query now passes repository selection, but creating
an unrelated `missing_input.txt` beside an unchanged external BUILD graph
recomputes exact observed children and replays their equal local event batches.
The semantic query result and exact path epoch are correct; weakening either
the epoch or child-key equality would retain stale carriers.

`SealedCommandAttempt::select` owns the exact terminal activation closure and
currently projects only nonempty `EventBatch` values, discarding each
`DiceNodeId`. `AcceptedNativeDemandSnapshot` owns the prior successfully
accepted command state but retains no event association. The uniquely smallest
owner is therefore native-command event acceptance, not a query carrier or a
lower producer. Bazel 9.2 public query values/event order and the already
accepted cold/warm/edit tests remain the exact oracle; this is a regression of
that evidence and needs no new fixture. DICE activation-closure selection and
the retained runtime transaction remain the Stage 2/Buck2 evidence basis.

## Frozen design

Each command has a fresh event tracker, and DICE reused activations expose no
evaluation data. Event selection must therefore return the exact ordered
closure as a compute-local node stream that distinguishes a known current
transition `Option<EventBatch>` from a node with no current transition. A
known `Some` includes empty batches; known `None` removes a prior batch. Add
one private accepted event epoch backed by
`Arc<[(DiceNodeId, EventBatch)]>` with cheap clone and memory accounting.
`EventBatch` stays the existing immutable Arc-backed `Dupe` value. Reuse the
V2/Buck2 compact immutable-slice pattern; add no retained map, interner, cache
or event producer. Closure nodes, a `SmallMap` and emitted-batch `Vec` are
permitted only as selection/acceptance scratch.

Fold the selected closure against the prior accepted epoch in closure order.
A known current `Some(batch)` replaces that node and emits only when the exact
batch differs and is nonempty. Known `None` removes it. A node with no current
transition carries a matching prior entry without emission; this preserves a
warm reused closure instead of clearing the epoch. Prior nodes absent from the
current closure are dropped, so a later evaluated reappearance is new and
replays. Reordering alone emits nothing but replaces retained order. Do not
compare event Arc identity and do not change child batch ownership, activation
closure, exact path observations or repository selection.

Store the next event epoch in `AcceptedNativeDemandSnapshot`; prepare the
filtered output and next epoch locally, then replace path/repository/event
accepted state together at the existing post-materializer acceptance boundary.
Need, typed outer, selection/validation/materializer failure, cancellation and
restorable abort leave the prior epoch untouched. Existing post-irreversible
failure remains fail-closed. The public `AcceptedCommand` moves only filtered
output batches and the semantic terminal; it retains no epoch. Cross-command
replacement uses the serialized native lease and adds no lock across DICE.

## Future implementation authority and proof

After independent design ACCEPT, retry may write the existing eight files plus
`app/slug_core_v2/src/runtime/events.rs`. Preserve every existing per-file cap
and the three relocation exceptions. Allow events only +80 production/+100
colocated tests, <=1,800 physical; aggregate semantic becomes +1,234
production/+1,428 tests/+2,662. The prior 19,531 primary envelope plus the
full events file is <=21,331; loading proof remains separately <=3,442.

Prove ordered selection distinguishes known Some/None from no-transition nodes,
includes empty owners and exact node identity, and carries warm reused entries.
Discriminate accepted Some(A) -> next-command no-transition carry -> evaluated
Some(A) with no output. Separately prove accepted Some(A) -> evaluated known
None removal with no output -> no-transition does not resurrect it -> evaluated
Some(A) is new and emits. Keep Some(empty) as a distinct retained entry.
Changed batches replay in closure order; removal/evaluated-reappearance and
cross-command replacement behave exactly; no retained map or deep clone
survives acceptance.
Prove sibling source-file churn suppresses,
while BUILD and `.bzl` A/B/delete/recreate/A events replay changed local
batches and suppress after restoration. Cover failed selection/validation/
materializer, strict-root mismatch, cancellation and abort rollback; root/
external query, legacy/build policy, repository sidecars, exact selected path/
result Arcs, family isolation and warm behavior remain unchanged.

Exact public query values/errors/order/events/materialization and all existing
legacy/build behavior remain exact. The private accepted event epoch and
deduplication association are Slug-native. One-shot query, external exported
source, multi-build, unsupported breadth and identity bytes remain deferred.

STOP on Rust now; later STOP any tenth file, path/carrier/equality weakening,
child event-owner change, retained map/interner/cache, non-atomic epoch update,
lost changed-event replay, cap excess, public/caller expansion or M1 closure.
REPLAN if node identity is not stable for one workspace runtime, acceptance
cannot replace the epoch atomically, or the bounded representation cannot
preserve exact event behavior. Schedule exactly the same implementation retry
after independent ACCEPT; no other successor.
