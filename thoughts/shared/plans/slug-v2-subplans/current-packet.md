# Current Slug V2 Packet

Packet: `WP-6-7A-host-root-apparent-repository-route-observation-implementation`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Design and Rust base: pending docs commit / `16a3d80a`

## Goal and authority

Implement one private observation owner for the accepted root apparent-
repository route. Share the existing route semantics between Legacy and
Observed modes, consume exactly the now-nameable observed root-definition
child, and forward that child's epoch unchanged. Do not activate source input
or any later consumer.

Authority is exactly
`app/slug_core_v2/src/runtime/root_apparent_repository_route.rs`, baseline
1,130 physical/tests 374, SHA-256
`e70226c7dd65eae174022ed37949056df2c0086d610d8a16ea01f30aa8231bb7`.
Every second Rust/API/export/caller/fixture/oracle file is read-only.

## Audited frontier and owner decision

Accepted visibility commit `16a3d80a` makes
`HostRootApparentRepositoryDefinitionObservationKey`, its carrier and opaque
outer nameable in the route sibling. The legacy route at production lines
296-372 computes exactly one child,
`HostRootApparentRepositoryDefinitionKey`, at lines 301-324 and has no other
semantic input. Thus no visibility, merge or evidence prerequisite remains.

The legacy route key has exactly one production consumer:
`HostRootApparentRepositorySourceInputKey` imports it and computes it at
source-input lines 24/186. Source input then has one source-path consumer;
source path has one source-observation consumer; that source observation has
no production caller. Public command analysis instead consumes Bzlmod
`RootRepositoryRouteKey`/`RootRepositoryRouteObservationKey` at
`dice.rs:4476-4494`, and root bootstrap remains a dormant imperative owner.
Those are later or parallel branches, not route-owner prerequisites.

## Frozen owner and driver

Add private `HostRootApparentRepositoryRouteObservationKey` as a nominal
wrapper over `HostRootApparentRepositoryRouteKey`. Its exact two-argument
`Option<Self>` constructor delegates to the legacy constructor and preserves
root-name rejection. Display is exactly `observed-{legacy Display}`; for
`/workspace` and `@first` it is:

```text
observed-HostRootApparentRepositoryRouteKey { workspace: NormalizedAbsolutePath { path: "/workspace" }, apparent_repo: ApparentRepoName("first") }
```

Add private `ObservedHostRootApparentRepositoryRoute` with exactly private
`Arc<HostRootApparentRepositoryRouteResult>` and `PathObservationEpoch` fields
and borrowed `result`/`observations` accessors. Add private, typed:

```rust
enum HostRootApparentRepositoryRouteObservationError {
    Definition(HostRootApparentRepositoryDefinitionObservationError),
}
```

Use existing matching Debug/Clone/PartialEq/Eq/Allocative and Dupe conventions.
The Key Value is
`SourcePreparationOutcome<Result<ObservedHostRootApparentRepositoryRoute,
HostRootApparentRepositoryRouteObservationError>>`. Equality remains
`complete_eq`; validity remains `is_complete`.

Factor only the existing compute into one private
`RootApparentRepositoryRouteMode::{Legacy, Observed}` driver and, if needed,
one pure finisher over the legacy key, predecessor Result Arc and epoch. The
driver outcome is
`SourcePreparationOutcome<Result<(Arc<HostRootApparentRepositoryRouteResult>,
PathObservationEpoch), HostRootApparentRepositoryRouteObservationError>>`.

Legacy computes exactly the legacy definition child and pairs Complete with
an empty epoch. Observed computes exactly
`HostRootApparentRepositoryDefinitionObservationKey`; Need returns immediately,
the opaque child outer becomes carrierless `Definition(error)`, and a carrier
supplies its original Result Arc and cloned epoch. A child DICE failure keeps
the existing semantic route `Compute` error with an empty epoch. No fallback,
second child, join, merge, union, epoch reconstruction or direct Host read is
allowed.

For every completed child Result, run the existing route algebra unchanged:
ordinary nondeferred child failure becomes `Predecessor`; a successful or
deferred child without a valid view becomes `InvalidPredecessor`; request/view
inconsistency also becomes `InvalidPredecessor`; and a consistent generated,
selected-registry, selected-nonregistry, Main or Builtin view produces the
same route certificate. Main/Builtin remain successful route projections of
the exact deferred definition errors. Every semantic success/error retains the
original predecessor Arc and forwards the child epoch unchanged. There is no
epoch merge and no parent OperationMismatch.

Legacy projection asserts the epoch is empty and returns the original route
outcome. Observed success publishes the local Result Arc plus epoch; its typed
outer publishes no carrier or epoch.

## Events, retention and lifecycle proof

The route owner is eventless. The sole observed definition child owns every
load/invocation batch and exact event order; route and all warm rows are
batchless. Need, child outer, child compute, Predecessor, InvalidPredecessor and
success introduce no parent event or replay. Dependency rows are exactly
legacy route -> legacy definition and observed route -> observed definition.

The carrier retains only one route Result Arc plus the compact epoch. The route
Result retains its already-required predecessor Arc. Child carrier, mode,
views, disposition booleans, closure and evaluator/event scratch die before
publication. Add no cache/store/interner/task/lock or command borrow. DICE owns
serialization; cancellation publishes no parent activation/dependency/carrier
or event, and same-DICE recovery recomputes lawfully.

Add exactly three tests:

- `observed_root_apparent_repository_route_identity_finisher_and_terminal_algebra`;
- `observed_root_apparent_repository_route_real_families_events_and_parity`;
- `observed_root_apparent_repository_route_lifecycle_cancellation_and_nonactivation`.

They prove key identity/root rejection/exact Display, accessors, equality/
validity, Need and exact completed-disposition/finisher terminals; source shape
with exactly one observed child and one typed outer mapping; exact real
generated, selected-nonregistry, mapping-failure, Main and Builtin legacy
semantic parity, one-child dependency rows, unchanged child epoch, lower-owned
event vectors and all warm rows batchless; plus held child/parent semantic
A-B-A through mapping and generated-definition changes, equal Result with a
metadata-only changed epoch, parent epoch equal to child and a subset of that
transaction's global epoch, Arc identity only on Reused, poll-drop recovery,
and legacy/source-input/source-path/source-observation/Bzlmod-route/public-
command/bootstrap nonactivation.

Reuse accepted
`observed_root_apparent_repository_definition_real_order_events_and_parity`
for the real-family child proof, plus its identity test's accepted selected-
registry source/policy/forwarding chain. Require static route projection
evidence rather than private mirror injection. Construct no opaque child outer,
malformed epoch, private state hook or synthetic keyed mismatch.

## Caps and validation

Caps are <=240 production, <=620 proof and <=860 aggregate semantic additions,
with physical <=1,990. Add at most six production and six test helpers, exactly
three tests, driver below 150 and every helper/test below 200. The file remains
cohesive because it already owns the legacy route value/error/views, sole-child
projection, trackers and real fixtures; the cap remains below the 2,000-line
complexity trigger. This is not a demonstrated hot path and changes no retained
representation beyond the bounded carrier.

Run serially:

1. `cargo test -p slug_core_v2 observed_root_apparent_repository_route_ --lib`;
2. protected sibling-surface, `consistency_is_fail_closed`,
   `generated_route_borrows_original_definition`,
   `selected_nonregistry_route_retains_original_spec` and
   `main_deferred_is_promoted_without_fallback` tests;
3. protected observed root-definition tests;
4. full `cargo test -p slug_core_v2`;
5. `cargo check -p slug_commands_v2`;
6. `cargo fmt --all -- --check`; and
7. exact one-file allowlist, baseline SHA/accounting/physical/helper/test-size/
   driver/dependency/event/retention/nonactivation/source-shape checks plus
   `git diff --check`.

Reuse accepted Bazel 9.2 `BazelDepGraphFunction.computeCanonicalRepoNameLookup`,
`BazelDepGraphValue.getRepositoryMapping`, `ModuleKey` canonical naming and
the accepted route/source-capability tests. Buck2 DICE incrementality,
cancellation and activation-tracker tests remain concept/test evidence. Add no
fixture or oracle because the owner introduces no new Bazel-visible behavior.

## Compatibility and stops

Route values, five-family projection, predecessor/view/source-capability
semantics, errors, order, equality/invalidation and lower events remain
**exact** Bazel 9 compatibility. The private Result-Arc+transaction-local epoch
carrier and typed outer are **Slug-native**. Carrier visibility, source-input/
source-path/source observation, public-command/bootstrap activation and exact
Bazel configuration/output/ActionKey bytes remain **unsupported/deferred**.

STOP on a second file/key/child/owner/adapter; visibility/export/caller/API;
source-input or upper activation; semantic/view/policy/order/error/event/
equality/retention drift; epoch merge/rebuild; parent OperationMismatch;
retained child/scratch/task/lock; private/malformed injection; fixture/oracle;
cap/helper/test/format waiver; Cargo/BUILD; milestone closure, M8/M7B or exact
identity work. REPLAN before widening or on baseline hash drift.

## Terminal

ACCEPT returns only to a docs-only root-route carrier-visibility and sole
source-input consumer audit. M7 remains partial and M7A -> M8 -> M7B remains.

## Immediate predecessor

Accepted `16a3d80a` promoted only the root-definition observation handoff and
its route sibling smoke at +75/-17 across two files. It activated no caller.
