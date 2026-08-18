# Current Slug V2 Packet

Packet: `WP-2A-m1-external-bzl-module-evaluation-observation-design`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Scheduling base: `69e4fa43`
Rust base: `ac7b8bdf`
Result: freeze the recursive external `.bzl` observation owner without
activating package load or an upper caller.

## Authority and measured future envelope

Write only canonical, this manifest, Stage 2 and
`.codex/skills/slug-agent-orchestration/references/routing-log.md`. Against
`69e4fa43`, caps are 40/180/120/30 net lines respectively and 370 aggregate.
Rust, Cargo/BUILD, fixtures, oracles, generated artifacts and caller/public
activation are forbidden.

After independent design ACCEPT, future Rust authority is exactly:

- `app/slug_loading_v2/src/bzl_module.rs`: at most 400 production net lines
  and 6,595 physical lines;
- `app/slug_loading_v2/src/cycle_detector.rs`: at most 160 production/test net
  lines and 758 physical lines;
- `app/slug_loading_v2/src/host_package_load_tests.rs`: at most 560 test net
  lines and 2,891 physical lines.

Aggregate semantic growth is at most 1,120 lines and combined physical size at
most 10,244 lines, measured against `ac7b8bdf` baselines 6,195/598/2,331.
`bzl_module.rs` is the cohesive recursive loading/event owner; touched helpers
must remain below 200 lines.

## Natural owner and structure

Freeze a private structural `ExternalBzlModuleObservationKey` wrapping the
same route+label identity as `ExternalBzlModuleEvalKey`, plus
`ObservedExternalBzlModule` containing exactly one local
`Arc<Result<FrozenBzlModule, ExternalBzlModuleError>>` and one complete
`PathObservationEpoch`. One Legacy/Observed driver owns source acquisition,
load-label validation, sequential recursive children, evaluation, freezing and
the matching key's local Complete event batch. Legacy selects only
`HostRepositorySourceFileKey` and legacy external children and projects the
exact local Result Arc. Observed selects only
`HostRepositorySourceFileObservationKey` and observed external children.
Neither sibling computes the other. Keep the observed key/carrier crate-private
for the later same-crate package-load owner; add no export or caller.

Extend the existing request-scoped cycle detector in the same packet. Represent
external legacy and observed nodes as separate variants around one compact
`ExternalBzlCycleIdentity` carrying route+label, as the accepted Host-Bzl
detector already does. Project `ExternalBzlLoadCycle` path/keys to compact Arc
slices of that identity while preserving their exact label/order behavior. A
mode-aware external guard records only matching-family edges and rejects mixed
families. Preserve sequential single-waiter locking,
cycle poison and request-local task/state; add no second detector or retained
collection. Bazel 9.2 `BzlLoadFunction` likewise gives each recursively loaded
`.bzl` its own cached node and insertion-ordered load-stack cycle identity.

## Order and terminal algebra

Observed order is the current source first, then resolved direct loads in AST
order, recursively. Union every Complete source/child epoch left-first before
semantic inspection with shared Result Arcs. Equal duplicates retain the first
Arc; conflict or operation mismatch is a typed outer. Canonical epoch demand
order remains the compact `SortedMap` order, distinct from recursive execution
order.

Source DICE compute failure remains semantic `SourceCompute` with the prior
empty prefix. Source semantic/Absent, encoding, parse and load-label terminals
retain the source prefix. For each child, DICE compute failure retains the
prior prefix as the existing contextual `Child(SourceCompute)` semantic;
child semantic failure retains the merged prefix. Need or typed outer from the
source or any child returns immediately with no carrier or local batch and does
not activate later children. Success/evaluation/freeze retains the full reached
epoch. This sequential owner has no joined Need union.

On an observed cycle, preserve the current prefix, then re-request the accepted
observed source carrier for each other cycle identity in cycle-key order after
the current identity, unioning before semantic inspection. Need/typed outer is
carrierless; a changed semantic source supersedes the stale cycle with its
exact reached prefix; otherwise the semantic `Cycle` retains the complete
cycle-source epoch. Propagating parents merge that carrier before contextual
`Child` inspection. Do not retain cycle-source carriers or prefix snapshots.

Need is invalid/self-unequal. Complete typed outer is valid/equal by outer
value. Complete carrier is valid/equal by local semantic Result plus complete
epoch. Every semantic Complete, including error/empty-event cases, stores
exactly the matching external key's existing local batch; Need, outer and
cancellation store none. Source/routed children keep their batches, recursive
children publish before parents, and package BUILD/query/build batches remain
dormant.

## Retention, compatibility and proof

Retain only the existing frozen module graph/child closure inside the one local
semantic Result Arc plus the compact epoch. Source/child semantic Arcs,
resolved-load vectors, evaluator/loader/event staging, cycle-source carriers,
union state and prefix snapshots are compute-local or existing request-scoped
detector state. Add no store/cache/interner/lock/task beyond the existing
detector, direct Host read, revision, certificate or event owner.

Exact: external `.bzl` values/errors, label resolution, direct-load order,
manifest/retained closure, legacy Result Arc and cycle/event behavior.
Slug-native: structural sibling/carrier, typed outer, complete epoch and
legacy/observed detector-node split. Deferred: observed package load and BUILD
batch, loading query/build publication and exact identity bytes.

Proof must discriminate distinct key/Display and common cycle identity; exact
legacy Arc/result/event parity; observed semantic parity; exact source then
recursive-child epoch membership with per-demand `Arc::ptr_eq`; stable duplicate
first Arc, conflict and operation mismatch; every source/parse/load-label/
child/evaluation/freeze prefix; source and first/middle/last-child Need/outer,
validity/equality/no carrier and later-child nonactivation; DICE-compute
semantic polarity; recursive diamond order/reuse; self and multi-node cycle
epoch completion, mixed-family rejection, release and fresh-detector recovery;
exact child-before-parent local batch text/order including empty/error batches
and warm suppression; both family directions and zero package-load/query/build
activation; real poll-drop cancellation/recovery; edit/delete/recreate and
A-B-A; manifest/bytes Arc lifetime; Allocative/retention and cleanup scans.

STOP on any other file, export/caller, package-load or upper activation, mixed
family, incomplete cycle carrier, reconstructed Result Arc, changed semantic/
event/cycle behavior, new retained state, helper over 200 lines, cap excess,
multiple successors or M1 closure. REPLAN if matching-family cycle detection
cannot remain bounded in the existing detector. After independent design
ACCEPT, schedule exactly one implementation; after implementation ACCEPT,
return only to a docs-only package-load/upper-owner audit.
