# Current Slug V2 Packet

Packet: `WP-2A-m1-external-singleton-observed-build-implementation-retry-3`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Scheduling base: `340159c0`
Accepted Rust base: `a4dd40d6`
Accepted external-build design: `1a217e2a`
Accepted loading-proof correction: `ce110d9a`
Accepted revision-event design: `5dabd4bf`
Accepted root-association correction: `340159c0`
Result: publish one nonroot exported-source build through the observed root
without replaying equal child events across certificate revision retries.

## Exact Rust authority and caps

Write exactly:

1. `app/slug_core_v2/src/runtime/dice.rs`: <=340 production net and <=11,350
   physical;
2. `app/slug_core_v2/src/runtime/tests/build_command_tests.rs`: <=440 test net
   and <=3,450 physical;
3. `app/slug_loading_v2/src/host_package_load_tests.rs`: only the accepted
   line-neutral assertion change, zero net and <=3,439 physical; and
4. `app/slug_core_v2/src/runtime/events.rs`: <=100 production plus <=160 test
   net and <=2,050 physical.

Aggregate semantic <=1,040 and combined physical <=20,289 against
`a4dd40d6`. No other loading byte or file. Planning docs, Cargo, BUILD,
fixtures, oracles, generated evidence, exports, callers and server-test edits
are forbidden. Large owners remain cohesive exceptions; touched helpers stay
below 200 lines.

## External owner, order and terminal algebra

Keep structural `BuildCommandRootObservationKey` admission for root PackageAll
and every syntactic nonroot Single. External wrong-kind targets classify only
after observed package load. Root Single, multi-target and every other identity
retain neutral/legacy routing and observed -> neutral -> legacy constructor
order.

One private mode-aware external driver selects only matching legacy or observed
route/package/source children and preserves exact legacy infrastructure
projection/post-join precedence. Observed order is anchor -> route ->
repository package -> ExportedFile classification -> RequestRevisionKey ->
source. Missing/wrong-kind targets activate neither revision nor source.

Union every Complete observed epoch left-first before semantic inspection;
equal duplicates keep the first exact Arc, conflict/operation mismatch is typed
outer, and Need/outer is immediate and carrierless. Preserve
empty/anchor/anchor+route/anchor+route+package/full-source prefixes. Present,
Absent, source semantic, accepted directory WrongKind and success retain the
full source child epoch as exact SourceCertificate; earlier terminals retain
none. Only external observed Single initializes revision and selects
ClosureRepositories; PackageAll remains strict-empty. Full selected path
value/Arc validation and entire-certificate materializer reobservation remain
unconditional.

The root owns no event batch. Matching anchor/module/package/source keys remain
sole owners. Preserve exact cold/error text/order, warm suppression, changed
BUILD/`.bzl` replay, equal sibling-source churn suppression and external ->
root PackageAll no replay.

## Accepted roots and revision-event carry

`AcceptedEventEpoch` retains exactly an ordered
`Arc<[DiceNodeId]>` of accepted closure roots plus its Some-only ordered event
entries. `SelectedEventState` captures exact ordered closure roots. The
command-local `ProvisionalEventEpoch` retains the same roots plus ordered
`(DiceNodeId, Option<EventBatch>)` entries; Some is effective, None is an
explicit tombstone, and absence alone permits fallback.

Normal reconciliation drops nodes absent from the current closure. On the
first source-certificate revision retry, seed missing true-prior entries only
when the current ordered roots exactly equal prior accepted roots and every
matched root transition is NoTransition/reused. Any root/order/length mismatch,
unavailable root, or root Known(Some/None) uses ordinary closure reconciliation
and seeds nothing.

After seeding, preserve carry through later Needs for the same fixed roots.
Fold retries left-first: final Known(Some) replaces, Known(None) tombstones,
NoTransition uses carry then true prior, retry-only nodes retain first retry
order, and final-only nodes append in final closure order. Diff only the final
effective epoch against true prior and emit changed nonempty Some batches.
Accept filtered Some entries and current roots only after materializer
acceptance. Outer/cancel/abort/selection/revision/materializer failure drops
carry and changes no accepted state.

Retain only one build Result Arc, compact full/certificate path epochs, compact
accepted/provisional event root and entry slices. Child carriers/outcomes,
selected paths, closure, dependency graph, event/union maps/Vecs and repository
sidecars stay compute-local or dependency-owned. Add no retained map/cache/
store/interner/lock/task, Host read, child carrier, event owner or historical
snapshot; hold no lock across DICE.

## Loading proof, compatibility, proof and STOP

In `host_package_load_tests.rs`, only replace the stale core-negative
`RepositoryPackageLoadObservationKey` assertion with the accepted positive
assertion. Preserve every other byte.

Exact: public build values/errors/classification, child event text/order,
PackageAll/root Single/multi-target and every legacy/direct API. Slug-native:
observed carrier/certificate/repository, accepted root IDs and provisional
retry-event association. Unsupported/deferred: multi-build, one-shot, broader
actions, external globs and exact Bazel identity bytes.

Preserve all existing routing/family/prefix/Arc/certificate/repository/
lifecycle/cancellation/rollback proof. Add reuse-only root association proof:
same roots plus root NoTransition seeds/suppresses hidden equal child; same root
plus root Known(Some/None) does not seed and removes absent owners; different
and reordered roots do not seed; simultaneous BUILD or `.bzl` plus source
change replays changed/removes absent batches. Cover revision -> Need,
final Some/None/NoTransition, Some(empty), multiple retry order, tombstone
reappearance and all failure atomicity. The public source edit/delete/directory/
recreate lifecycle emits no package replay, external -> root stays silent, and
the unchanged server lifecycle passes except separately recorded inherited
query baselines.

Require focused event/build, 33/33 build group, loading 138/138, full bzlmod,
documented core/query/server baselines, fmt/diff, exact caps, Buck2 retention,
AI cleanup and independent final review.

STOP on every other file/loading byte, child filtering, path/key equality
weakening, seeding an evaluated/mismatched root, retaining a closure/map,
behavior/family/order drift, cap excess, broader activation or M1 closure.
REPLAN on any new blocker. After ACCEPT return only to one docs-only remaining
M1 owner audit.
