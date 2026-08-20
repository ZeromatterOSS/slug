# Current Slug V2 Packet

Packet: `WP-6-7A-host-nonregistry-module-closure-observation-design`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Scheduling and Rust base: `18166691`

## Objective and docs-only authority

Freeze the uniquely smallest complete observed owner for nonregistry module
closure preparation. `HostNonregistryModuleClosureKey` already owns the exact
effective override -> materialization -> root MODULE source -> validation ->
repeated package horizon -> fragment-source frontier and final cycle/closure
semantics. Every mutable child now has an accepted observed carrier; the
horizon and fragment reducers are private closure mechanisms with no separate
consumer. Keep `HostDiscoveredModuleKey` and every upper owner inactive.

Write only canonical/current/Stage 6/routing at <=40/<=220/<=180/<=30 net
lines and <=470 aggregate. Rust, Cargo, BUILD, fixtures, oracles, callers and
public files are read-only during design.

After independent design ACCEPT, future exact Rust authority is only:

- `app/slug_bzlmod_v2/src/source_preparation.rs`, baseline 15,611 physical,
  <=520 production and <=16,250 physical;
- `app/slug_bzlmod_v2/src/source_preparation_observation_tests.rs`, baseline
  4,230 physical, <=1,200 proof and <=5,550 physical.

Aggregate growth is <=1,800 semantic and <=21,800 physical. Touched helpers
remain below 200 lines; the two large owner/proof files are cohesive exceptions.

## Frozen owner and carrier

Add private `HostNonregistryModuleClosureObservationKey` structurally wrapping
`HostNonregistryModuleClosureKey`, plus
`ObservedHostNonregistryModuleClosure`. Its Value is exactly
`SourcePreparationOutcome<Result<ObservedHostNonregistryModuleClosure,
HostNonregistryModuleClosureObservationError>>`. The carrier retains one local
`Arc<Result<HostNonregistryModuleClosure,
HostNonregistryModuleClosureError>>` plus one cumulative compact
`PathObservationEpoch`. Require `Dupe`/`Allocative`, borrowed accessors and no
export or caller activation. Legacy projection moves the exact local Result Arc.

Use one Legacy/Observed closure driver. Preserve exact order: effective
override, repository materialization, root `MODULE.bazel` source, pure root
validation, repeated BFS package-horizon batches, fragment-source batches,
cycle detection and final closure. Legacy selects only legacy effective,
materialization, source and preflight families. Observed selects only their
accepted observed siblings. Neither key computes the other family.

## Prefix and terminal algebra

For every Complete child, merge the accumulated earlier prefix left-first with
the child's epoch before inspecting semantics. Explicit materialization is
earlier than duplicate materialization demands reached through the root,
package and fragment carriers; equal duplicates therefore retain the first
exact materialization Arc. Conflict or operation mismatch is typed outer.

Effective, materialization and root-source Need or typed outer returns
immediately carrierless and suppresses later work. DICE compute failures are
explicit: Legacy effective failure remains the existing invariant panic and
produces no Value; Observed effective failure is typed outer with empty prefix.
Materialization compute failure in either mode is semantic
`MaterializationCompute` with the accepted effective prefix. Root-source compute
failure is semantic `RootSourceCompute` with effective+materialization prefix.
At a package occurrence, compute failure is semantic `Package(Compute)` with the
prefix through earlier successful occurrences unless an earlier occurrence
already selected the full compatible Need. At a fragment occurrence, source
compute failure is semantic `Fragment(SourceCompute)` with the prefix through
earlier successful fragments unless an earlier Need selects the full Need union.
Observed child frontier errors and epoch conflicts are typed outer and
carrierless; Legacy has no sibling frontier outer.

Semantic effective, materialization, source, root-validation, absence and cycle
terminals retain the full prefix reached before the decision. Complete carrier
equality is semantic Result plus epoch; Complete outer equality is outer by
value; Need is invalid and self-unequal.

For each package horizon, parse the whole ordered request slice before any
package compute. The first bad label is semantic with the incoming prefix and
activates zero package children. Deduplicate packages by first occurrence,
compute the unique package set as one input-ordered batch, and reduce in original
occurrence order. Precompute the deterministic full compatible Need union, but
the first occurrence terminal wins exactly as Legacy: outer/conflict, Need, or
semantic at that occurrence stops the scan; a Need returns the full union.
Merge each successful Complete epoch before inspecting its semantic result and
before advancing. Incompatible Need triggers REPLAN rather than inventing an
error. Later batch outcomes were computed but are not merged or retained after
the first terminal.

Preserve `finish_nonregistry_fragment_batch` semantics. Reduce occurrence
order while merging every Complete epoch. A semantic before any Need returns
that semantic with its reached prefix. An earlier Need followed by semantic
returns the full compatible Need union. Typed outer or union conflict before a
decisive semantic wins, including after an earlier Need. With no semantic,
outer/conflict wins, then full Need, then ordered success. Preserve duplicate
includes, BFS order, cycle identity and the complete accumulated epoch. Do not
invent a second Need algebra or DICE owner.

## Events, families and retention

The closure sibling is eventless. Reached root MODULE and REPO descendants
remain sole owners of their exact local batches and public order; effective,
materialization, source, preflight, horizon and fragment orchestration remain
eventless. Need, typed outer and cancellation publish no parent state. Warm
reuse emits no new batch. Do not activate `HostDiscoveredModuleKey`, selected
graph, registry preparation, extensions or public callers.

Retain only the local closure semantic Result Arc and compact epoch. Child
carrier Arcs, package/fragment outcomes, BFS frontier, ancestry/cycle scratch,
Need union, temporary labels, event staging and union maps/vectors stay
compute-local or dependency-owned. Add no extra collection, cache, interner,
store, lock, task, direct Host read, revision, certificate or event state.

## Required proof

Discriminate key identity/hash/Display, accessors, `Dupe`/`Allocative`, outer
validity/equality, Need invalid/self-unequal, observed result parity and exact
legacy Result-Arc projection.

Cover effective/materialization/root-source Need, typed outer, DICE compute and
semantic terminals with exact prefixes and later suppression. Assert exact
cumulative epoch iteration and per-demand `Arc::ptr_eq`, including effective
and duplicate materialization demands; prove earliest equal Arc, conflict and
operation mismatch.

For every horizon slot, prove whole-slice bad-label prevalidation, first-seen
deduplication, input-order batch reduction, every-position Need/outer/semantic,
full compatible Need union, incompatible-Need REPLAN boundary, first semantic,
later activation without later result retention and exact prefix. For fragments,
table semantic-before-Need, Need-before-semantic, outer/conflict after Need,
all-Need union, no-semantic outer precedence, duplicate include and success.

Exercise multi-level BFS, duplicate includes, complete success, every root,
package, fragment, absence and cycle semantic failure. Run local and immutable
A -> B -> absent -> directory -> A lifecycles, retain Result/epoch handles
through churn, and compare restored semantics and child-parent exact Arcs. Prove
real poll-drop cancellation and same-DICE recovery.

Assert exact observed and legacy direct dependency rows and reverse isolation;
exact child-owned ROOT -> REPO batch text/order, parent silence, warm suppression
and cancellation silence; zero discovered/selected/registry/extension/public
activation. Run focused closure proof, full bzlmod and affected loading/query/core
baselines, fmt, diff-check, exact accounting and AI-cleanup/Buck2 retention review.

## Compatibility and STOP

Exact: current nonregistry closure values/errors, BFS/include/cycle order,
legacy Result Arc and lower child events. Slug-native: private sibling,
Result-Arc+epoch carrier and typed outer. Unsupported/deferred: discovered and
selected module graph publication, registry source preparation/patches,
extension-generated repositories, M8/M7B and exact identity bytes.

STOP Rust during design. STOP a third file, export/caller, direct discovery or
upper activation, legacy/order/event/family drift, incompatible-Need coercion,
extra retained state, cap excess or milestone closure. If the frozen algebra
cannot be implemented within caps, REPLAN.

After independent design ACCEPT schedule exactly one bounded successor:
`WP-6-7A-host-nonregistry-module-closure-observation-implementation`. After its
independent ACCEPT, return only to the docs-only selected-module-graph frontier
audit; do not activate discovery directly.
