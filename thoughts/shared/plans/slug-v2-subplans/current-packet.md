# Current Slug V2 Packet

Packet: `WP-2A-m1-external-singleton-event-root-association-correction-design`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Scheduling base: `d63e1718`
Accepted Rust base: `a4dd40d6`
Accepted external-build design: `1a217e2a`
Accepted loading-proof correction: `ce110d9a`
Accepted revision-event design: `5dabd4bf`
Result: freeze the smallest accepted-root association required to carry exact
child event state across source-certificate revision retries.

## Exact docs authority and measured stop

Write exactly:

1. `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`: <=40 net;
2. this manifest: <=220 net;
3. `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`: <=180 net;
4. `.codex/skills/slug-agent-orchestration/references/routing-log.md`: <=30 net.

Aggregate docs net <=470 against `d63e1718`. Retain but do not write the dirty
four-file Rust candidate in `runtime/{dice,events}.rs`,
`runtime/tests/build_command_tests.rs`, and
`slug_loading_v2/src/host_package_load_tests.rs`. Cargo, BUILD, fixtures,
oracles, server tests, generated evidence, callers, exports and public behavior
are forbidden.

The first command-local carry implementation compiles, but exact accepted-epoch
proof shows the cold package event node is absent immediately after an
event-silent source edit. The pre-revision terminal root is equality-reused, so
its exact selected closure omits the previously accepted package descendant;
the provisional retry slice therefore never sees the node. Seeding every prior
entry is unsound when a different or reevaluated root genuinely removes event
owners. This is an accepted-event association gap, not a child-event,
build-root, path-epoch or key-equality defect.

## Frozen accepted-root and retry-carry contract

Keep `AcceptedEventEpoch` as compact accepted state, but extend it with exactly
one ordered `Arc<[DiceNodeId]>` of the accepted closure roots beside its
existing Some-only `Arc<[(DiceNodeId, EventBatch)]>`. Capture the exact ordered
closure roots in `SelectedEventState`. The command-local
`ProvisionalEventEpoch` retains the same roots plus its existing ordered
`Arc<[(DiceNodeId, Option<EventBatch>)]>`; Some is an effective batch, None a
known-removal tombstone, and absence alone permits fallback.

Normal command reconciliation remains unchanged: nodes absent from the current
closure are dropped, and the current ordered roots replace the accepted roots
only after successful materializer acceptance.

On the first source-certificate revision retry, seed missing prior event entries
only when both conditions hold:

1. the current ordered closure roots exactly equal the prior accepted roots; and
2. every matched current root transition is `NoTransition` (reused).

Any root/order/length mismatch, unavailable root, or root
`Known(Some/None)` uses ordinary current-closure reconciliation and seeds
nothing. Thus an equality-reused fixed root can recover its hidden accepted
descendants, while a reevaluated same root or a different/reordered root cannot
retain genuinely removed owners.

After seeding, preserve the provisional carry through later Needs for the same
fixed roots. Fold multiple revision retries left-first: final Known(Some)
replaces, final Known(None) tombstones, final NoTransition uses carry then true
prior, retry-only nodes retain first retry order, and final-only nodes append in
final closure order. Diff only the final effective epoch against the true prior;
emit only changed nonempty Some batches. Filter Some entries into accepted
state. Typed outer, cancellation, abort and every selection/revision/
materializer failure drop command-local carry and leave accepted roots/events
unchanged.

Use only compute-local SmallMap/Vec merge scratch. Both accepted/provisional
root and entry slices are `Dupe`/`Allocative`. Add no retained closure,
dependency graph, map, cache, store, interner, lock, task, Host read, child
carrier, event owner or historical snapshot, and hold no lock across DICE.

## Retry authority, proof and compatibility

After independent design ACCEPT, schedule exactly
`WP-2A-m1-external-singleton-observed-build-implementation-retry-3` with the
same four Rust files only:

1. `runtime/dice.rs`: <=340 production net, <=11,350 physical;
2. `runtime/tests/build_command_tests.rs`: <=440 test net, <=3,450 physical;
3. `slug_loading_v2/src/host_package_load_tests.rs`: only the accepted
   line-neutral assertion, zero net, <=3,439 physical;
4. `runtime/events.rs`: <=100 production plus <=160 tests, <=2,050 physical.

Aggregate semantic <=1,040 and combined physical <=20,289 against
`a4dd40d6`. Preserve every external owner/order/prefix/certificate/repository/
family/event and legacy infrastructure contract from `1a217e2a`,
`ce110d9a` and `5dabd4bf`.

Required discriminators: same ordered roots plus root NoTransition seeds and
suppresses a hidden equal child; same root plus root Known(Some) or Known(None)
does not seed and drops removed owners; different and reordered roots do not
seed; simultaneous BUILD or `.bzl` plus source change replays the changed
batch and removes absent owners; revision -> Need preserves seeded carry;
final Some/None/NoTransition, empty, multiple-retry order and tombstone
reappearance remain exact; cancel/abort/selection/revision/materializer failure
is atomic. The public source edit/delete/directory/recreate lifecycle emits no
equal package replay, changed BUILD/`.bzl` still replays, external -> root
PackageAll suppresses prior events, and the unchanged server lifecycle passes
apart from its separately recorded inherited query baselines.

Exact: public build values/errors, child event text/order and every legacy/root
route. Slug-native: observed carrier/certificate/repository association,
accepted root IDs and provisional retry-event association.
Unsupported/deferred: multi-build, one-shot, broader actions, external globs
and exact Bazel identity bytes.

STOP now on Rust or any file outside the docs allowlist, weakening path/key
equality, filtering a child batch, seeding on an evaluated/mismatched root,
retaining a closure/map, behavior/family/order drift, cap excess, broader
activation or M1 closure. REPLAN if compact ordered roots cannot discriminate
the frozen cases. The design may schedule only the one bounded retry above.
