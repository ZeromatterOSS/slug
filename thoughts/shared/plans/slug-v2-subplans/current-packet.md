# Current Slug V2 Packet

Packet: `WP-2A-m1-loading-query-observed-publication-implementation-retry`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Rust base: `a9270586`
Accepted query design: `44c1b444`
Accepted proof correction: `e22404a8`
Accepted repository-selection correction: `1f2fb3f6`
Accepted event-epoch correction: `533fe50f`
Result: finish, validate and accept only the retained observed loading-query
candidate with native accepted-event reconciliation.

## Exact authority and caps

Write exactly evaluator/loading_environment/graph/lib and new
`observed_loading_query.rs` in `slug_query_v2`; core `runtime/dice.rs`,
`runtime/events.rs` and new `runtime/tests/query_command_tests.rs`; and
loading `host_package_load_tests.rs`. No tenth file.

Preserve existing caps 170+20/417, 360+60/2,346, 520+100/3,771, 4/81,
760/780, 100+12/11,000, relocation+372/1,132, and loading proof +4/3,442.
Allow events only +80 production/+100 colocated tests, <=1,800 physical.
Aggregate semantic caps are +1,234 production/+1,428 tests/+2,662. The primary
physical envelope plus events is <=21,331; loading proof remains separately
<=3,442. Base core lines 7,318-8,036 relocate exactly; only the three accepted
stable-parent replacements may differ.

## Frozen query and selection implementation

Preserve the structural observed query root, private observed graph/subtree
siblings, matching Legacy/Observed drivers, compute-local environment,
anchor/evaluator order, left-first union-before-semantic exact Arcs, immediate
sequential terminals and full subtree-batch outer > compatible Need union >
semantic > success. REPLAN rather than inventing a QueryError for Need union.

Carriers remain one natural Result Arc plus compact path epoch with
`Allocative` and `Dupe`; root retains no child carrier. Environment/arena/
graph/traversal/listing/union scratch remains compute-local. Child keys alone
produce local batches; Need/outer/cancel publishes none. Add no Host read,
revision, certificate, producer, cache, interner, store, lock or task.

Retain the exact loading query-positive/core-negative assertion and three
distinct crate-target `tempdir_in` parents from `e22404a8`. Preserve the
private typed `NativeCommandRoot` selection policy from `1f2fb3f6`: strict
path-only by default; closure-selected repository sidecars only for observed
query. Keep `selected_snapshot` and materializer acceptance as sole repository
owners and unconditionally compare the complete selected path epoch by length,
demand, value and `Arc::ptr_eq`. Add no repository state to the query carrier.

## Accepted-event epoch correction

Each command has a fresh event tracker and DICE reused activations expose no
evaluation data. Make terminal selection return exact closure order plus, per
node, either a known current `Option<EventBatch>` transition or no transition.
Known Some includes empty batches; known None removes a batch. Selection state
is scratch and must not enter the accepted snapshot.

Retain exactly one private `Arc<[(DiceNodeId, EventBatch)]>` event epoch in
`AcceptedNativeDemandSnapshot`, with cheap clone and memory accounting. Fold
the selected closure against the prior epoch in closure order: current Some
replaces and emits only when exact batch value changed and is nonempty; current
None removes; no-transition carries a matching prior entry without emission;
prior nodes absent from closure drop. Retain Some(empty) distinctly. Reordering
alone emits nothing but replaces retained order. Use only compute-local
`SmallMap`/vectors; no retained map, deep clone or event Arc-identity check.

Prepare filtered output and next epoch locally. Replace event/path/repository
accepted state together only at the existing post-materializer native snapshot
boundary. Need, outer, selection/validation/materializer failure, cancellation
and restorable abort preserve the prior epoch; post-irreversible failure stays
fail-closed. Public `AcceptedCommand` moves only filtered batches and semantic
terminal. Add no lock across DICE and do not change path/carrier equality,
activation closure, child batch ownership or repository association.

## Compatibility, proof and terminal

Exact public query values/errors/order/events/materialization, loading proof and
all legacy/build/direct APIs remain exact. Private observation, selection and
accepted-event association plus stable test parents are Slug-native. One-shot
query, external exported source, multi-build, unsupported breadth and exact
identity bytes remain deferred.

Prove accepted Some(A) -> next-command no-transition carry -> evaluated Some(A)
with no output. Prove accepted Some(A) -> evaluated known None removal/no output
-> no-transition no resurrection -> evaluated Some(A) new/emitted. Prove
Some(empty) retained distinctly; changed batches replay in current closure
order; absent/reordered/cross-command state is exact; failure and cancel roll
back. The external sibling-file mutation must suppress, while BUILD and `.bzl`
A/B/delete/recreate/A replay changed local batches and suppress after restore.

Retain external nonempty repository request+validation acceptance, root-empty
and strict-root rejection proof, complete selected path/result Arc identity,
family/event/lifecycle parity and zero upper activation. Run corrected tests
isolated, then default-parallel core, full query/loading/bzlmod, fmt, diff-check,
exact relocation/accounting, archive status, Buck2 retention/AI cleanup and
independent final review.

STOP on a tenth file/root opt-in, unrestricted boolean, weakened validation,
path/carrier/key equality drift, retained map/cache/interner, deep-cloned
events, child event-owner change, non-atomic accepted replacement, lost changed
event replay, body drift beyond accepted exceptions, cap excess, caller/public
expansion or M1 closure. REPLAN on another material miss. After ACCEPT commit
and return to exactly one docs-only next-owner audit.
