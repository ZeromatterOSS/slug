# Current Slug V2 Packet

Packet: `WP-2A-m1-external-bzl-module-evaluation-observation-implementation`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Rust base: `ac7b8bdf`
Accepted design: `b82496b6`
Result: implement and validate the private recursive external `.bzl`
observation sibling and matching cycle-family seam.

## Authority and caps

Write exactly:

- `app/slug_loading_v2/src/bzl_module.rs`: at most 400 production net lines
  and 6,595 physical lines;
- `app/slug_loading_v2/src/cycle_detector.rs`: at most 160 production/test net
  lines and 758 physical lines;
- `app/slug_loading_v2/src/host_package_load_tests.rs`: at most 560 test net
  lines and 2,891 physical lines.

Aggregate semantic growth is at most 1,120 lines and combined physical size at
most 10,244 lines against `ac7b8bdf`. `bzl_module.rs` is the accepted cohesive
large owner; touched helpers remain below 200 lines.

## Required implementation

Add private structural `ExternalBzlModuleObservationKey` and
`ObservedExternalBzlModule` retaining exactly one local external-module Result
Arc plus one compact complete epoch. Refactor the current evaluator into one
Legacy/Observed driver. Legacy selects only legacy Host source and recursive
external children and moves the exact local Result Arc. Observed selects only
`HostRepositorySourceFileObservationKey` and observed recursive children.
Neither sibling computes the other; add no export or caller.

Preserve source first, complete load-label prevalidation, then sequential AST-
order children, evaluation and freeze. Observed unions every Complete source/
child epoch left-first before semantic inspection using shared Result Arcs.
Equal duplicates keep the first Arc; conflict/operation mismatch is typed
outer. SourceCompute has empty prior prefix; source/Absent/encoding/parse/load-
label keeps source; child DICE error keeps prior; child semantic keeps merged;
evaluation/freeze/success keeps full reached epoch. Need/outer is immediate and
carrierless with no later child. There is no Need union.

Extend only the existing request-scoped cycle detector. Add compact
`ExternalBzlCycleIdentity` route+label, separate external legacy/observed nodes,
and one mode-aware guard that records only matching child edges and rejects
mixed families. Preserve one sequential waiter, poison and existing detector
task/state. On observed cycle, rotate through other cycle identities in exact
cycle-key order, compute accepted observed source carriers, and union before
semantic inspection. Need/outer is carrierless; a changed source terminal
outranks stale Cycle with its reached prefix; otherwise Cycle carries the
complete source epoch. Propagating parents union that carrier before Child.

Each sibling stores exactly its own local semantic-Complete `.bzl` batch,
including empty/error cases. Need/outer/cancellation stores none. Recursive
child batches precede parent and package BUILD/query/build remain dormant.
Need is invalid/self-unequal; Complete outer equality is by outer value and
Complete carrier equality by semantic Result+epoch.

Retain only the existing frozen module graph/closure inside the local Result
Arc plus compact epoch. Source/child carriers and Arcs, load/AST/evaluator/
event/union/cycle-source scratch remain compute-local or existing detector
state. Add no collection/store/cache/interner/lock/task, direct Host read,
revision, certificate or event owner.

## Proof, compatibility and terminal

Prove identity/Display and compact cycle identity; exact legacy Arc/result/
cycle/event parity and observed semantic parity; exact source/recursive epoch
Arcs, duplicate first Arc, conflict/mismatch; every frozen prefix and source/
first-middle-last child Need/outer/semantic position; later-child suppression;
recursive diamond reuse; self/multi-node cycle epoch completion, mixed-family
rejection, poison and fresh-detector recovery; exact child-before-parent local
batch text/order including empty/error, warm suppression and cancel; both family
directions with concurrent roots; zero package-load/query/build activation;
poll-drop recovery; edit/delete/recreate/A-B-A; manifest/bytes lifetime;
Allocative/retention and AI cleanup.

Exact: external `.bzl` values/errors/load order/manifest/cycle/events and legacy
Arc behavior. Slug-native: sibling/carrier/outer/epoch and detector family
split. Deferred: package load/BUILD batch, loading query/build publication and
identity bytes.

Run focused external-Bzl tests, full loading unit/integration suites, direct
bzlmod/query dependents, fmt, diff-check and exact cap accounting serially.
Require retention/cleanup and independent implementation review. STOP on any
other file/export/caller, package-load or upper activation, mixed family,
incomplete cycle carrier, semantic/event/cycle drift, new retained state,
helper over 200 lines, cap excess, multiple successors or M1 closure. REPLAN if
the accepted cycle association cannot remain in the existing detector. After
ACCEPT, commit and return only to a docs-only package-load/upper-owner audit.
