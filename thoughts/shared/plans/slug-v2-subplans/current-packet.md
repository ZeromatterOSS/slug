# Current Slug V2 Packet

Packet: `WP-6-7A-host-root-apparent-repository-source-input-observation-implementation`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Design and Rust base: `400b819e` / `b61f8f1a`

## Goal and authority

Implement only the accepted private observation owner for
`HostRootApparentRepositorySourceInputKey`. Freeze one Legacy/Observed driver
over the now-nameable observed route child, exact source-input projection and
terminal semantics, child-epoch forwarding and proof without activating the
sole source-path consumer or any later/public branch.

Rust authority is exactly
`app/slug_core_v2/src/runtime/root_apparent_repository_source_input.rs`.
Every second Rust/test/API/export/caller/fixture/oracle/Cargo/BUILD file and all
orchestration docs are read-only during implementation.

## Audited frontier

Accepted `b61f8f1a` exposes exactly the crate-internal observed route key/new,
carrier borrowed Result-Arc/epoch accessors and opaque outer. Source input can
name its complete associated Value without seeing the route Definition
terminal. No visibility or lower-owner prerequisite remains.

Live source input has one DICE child. It imports legacy
`HostRootApparentRepositoryRouteKey` at line 24 and requests it at line 186.
Need returns immediately; child DICE failure becomes semantic `Compute`; a
completed route error becomes `Route`; a successful route without a source or
with an invalid certificate becomes `InvalidRoute`; `host_repository_source_input`
projection failure becomes `Projection`; Main or accepted Builtin/spec-backed
input completes successfully. Every completed semantic terminal retains the
exact route Result Arc when one exists. There is no second child or merge.

The legacy source-input key has exactly one production consumer: source-path
input imports it at its line 27 and computes it at line 234. Source path has
one production consumer, host source observation at its line 234; that key has
zero production callers. Public command analysis instead computes the
parallel Bzlmod route/observed-route branch at `runtime/dice.rs:4476-4494`.
Root bootstrap remains imperative and dormant. These upper/parallel layers are
not prerequisites and stay inactive.

Committed design `400b819e` selects exactly this one-file private owner, not a
visibility prerequisite.

## Frozen implementation contract

Add private nominal
`HostRootApparentRepositorySourceInputObservationKey(HostRootApparentRepositorySourceInputKey)`
with the exact existing two-argument `Option<Self>` construction, root-name
rejection and Display `observed-{legacy Display}`. `/workspace`, `@first`
renders exactly:

```text
observed-HostRootApparentRepositorySourceInputKey { workspace: NormalizedAbsolutePath { path: "/workspace" }, apparent_repo: ApparentRepoName("first") }
```

Add private `ObservedHostRootApparentRepositorySourceInput` retaining only
`Arc<HostRootApparentRepositorySourceInputResult>` plus
`PathObservationEpoch`, with private borrowed `result` and `observations`
accessors. Derive Debug/Clone/PartialEq/Eq/Allocative/Dupe.

Add private typed outer
`HostRootApparentRepositorySourceInputObservationError::Route(
HostRootApparentRepositoryRouteObservationError)` with matching
Debug/Clone/PartialEq/Eq/Allocative and manual Dupe. It has no carrier or epoch
and no Merge/OperationMismatch variant.

Factor only existing compute into
`RootApparentRepositorySourceInputMode::{Legacy, Observed}`, one private
`compute_root_apparent_repository_source_input` driver and exactly one pure
`finish_root_apparent_repository_source_input`. Driver Value is
`SourcePreparationOutcome<Result<(Arc<HostRootApparentRepositorySourceInputResult>,
PathObservationEpoch), HostRootApparentRepositorySourceInputObservationError>>`.
The legacy Key projects the Result Arc and asserts an empty epoch; the observed
Key publishes the carrier. Both use complete_eq equality and complete-only
validity.

Legacy requests exactly the legacy route and supplies an empty epoch. Observed
requests exactly `HostRootApparentRepositoryRouteObservationKey`: Need is
immediate; its opaque child outer maps directly to carrierless parent Route
outer; child success supplies the original Result Arc and cloned epoch. A DICE
compute error remains the same semantic source-input Compute terminal with an
empty epoch. Do not inspect or construct the opaque child outer/private inner.

Run the existing projection once after child completion. Route semantic error
maps to `Route`; absent source or failed association maps to `InvalidRoute`;
`host_repository_source_input` failure maps to `Projection`; Main and valid
Builtin/spec-backed dispositions succeed. Preserve exact workspace/apparent/
canonical/capability/policy association and one projection call. Every success
or semantic terminal forwards the child epoch unchanged and retains the exact
route Result Arc where legacy does. With one child there is no prefix union,
epoch rebuild, join, fallback or parent mismatch algebra.

Source input is eventless. The observed route child owns every lower event
batch and its ordering. Parent execution adds no event on Need, outer, compute,
finisher terminal or success; direct-parent event rows equal direct-child rows,
and every warm activation is batchless. Dependency vectors are exactly legacy
source input -> legacy route and observed source input -> observed route.

Retain only the source-input Result Arc plus compact child epoch. The existing
Result success/error already retains its required route Arc and projected
input/request Arc. Child carrier, mode, source disposition scratch and closure
die before publication; add no cache/store/interner/task/lock or command
borrow. DICE owns serialization. Poll-drop publishes no parent activation,
dependency, carrier or event; recovery recomputes lawfully.

## Exact proof

Add exactly three tests:

- `observed_root_apparent_repository_source_input_identity_finisher_and_terminal_algebra`;
- `observed_root_apparent_repository_source_input_real_families_events_and_parity`;
- `observed_root_apparent_repository_source_input_lifecycle_cancellation_and_nonactivation`.

The first proves key/root/Display/hash/accessors/equality/validity, Need,
exact child edge, finisher disposition/error/Arc/epoch algebra and the opaque-
outer source mapping without private injection. `InvalidRoute` remains bounded
pure defensive algebra because lawful accepted routes supply a source.

The real proof covers Generated projection error, SelectedNonregistry Need then
WorkspaceRelative and CommandAbsolute success, mapping/missing Route terminal,
Main and Builtin success: exact legacy Result semantic parity, exact route Arc
retention, unchanged child epoch, direct-child lower event vectors and warm
batchlessness. Reuse accepted route selected-registry static/pure evidence and
accepted `host_repository_source_input` policy tests; do not fabricate a real
private-mirror registry row.

Lifecycle holds parent and observed-route child carriers through mapping,
definition and local-policy A-B-A, verifies semantic restoration, neutral same-
Result/different-epoch invalidation, parent epoch exactly child and a subset of
the same transaction-global epoch, Arc identity only on Reused, cancellation/
recovery and zero source-path/source-observation/public-command/bootstrap
activation. No cross-transaction child pairing or stale epoch claim.

Preserve the accepted route-surface sibling smoke byte-for-byte. Extend only
shared tracker/helpers and update existing
`production_edge_is_only_route_then_pure_projection` to require exactly one
legacy route request, one observed route request, one pure projection and no
other predecessor. Preserve all existing legacy assertions.

## Authority, caps and validation

Implementation authority is exactly
`app/slug_core_v2/src/runtime/root_apparent_repository_source_input.rs`,
baseline 856 physical/cfg(test) line 271, SHA-256
`940bcf9fe00782fba10dff5c1084525675a3b47c0471a1b15a18408f5c5249f2`.
Every second file is read-only.

Caps are <=240 production, <=620 proof, <=860 aggregate additions and <=1,750
physical. Add at most six production/eight test helpers and exactly three
tests; driver below 150 and every helper/test below 200. The file remains the
cohesive source-input value/view/error/key/driver/fixture owner below the
2,000-line trigger. No hot-path/retained-representation review applies.

Validate the three exact tests; protected legacy source-input and accepted
route-observation tests/smoke; full core; direct commands check; formatting;
exact one-file allowlist/SHA/accounting/physical/helper/test/driver/source-
shape and diff hygiene serially. Reuse accepted Bazel 9.2
`BazelDepGraphFunction.computeCanonicalRepoNameLookup`,
`BazelDepGraphValue.getRepositoryMapping`, repository source-capability tests
and original owner `e4292de7`; Buck2 DICE lifecycle is concept/test evidence.
Add no fixture or oracle.

## Compatibility and stops

Source-input Main/Builtin/spec projection, Need and terminal order, retained
route identity, policies, errors, equality/invalidation and lower events remain
**exact** Bazel 9 compatibility. The private Result-Arc+transaction-local epoch
carrier/outer is **Slug-native**. Source-path/source observation, public
command/bootstrap activation and exact identity bytes remain
**unsupported/deferred**.

STOP second file/key/child/owner/adapter, visibility/export/caller, source-path
or upper activation, semantic/projection/policy/order/error/event/equality/
retention drift, epoch merge/rebuild, parent OperationMismatch, retained child/
scratch/task/lock, private/malformed injection, fixture/oracle, cap/helper/test/
format waiver, Cargo/BUILD, milestone closure, M8/M7B or exact identity work.
REPLAN before widening or hash drift.

## Terminal

ACCEPT returns only to docs-only source-input carrier visibility/source-path
consumer audit. M7 remains partial and M7A -> M8 -> M7B remains.

## Immediate predecessor

Committed design `400b819e` is docs-only +284/-165 and accepts the complete
one-file contract above. Accepted visibility `b61f8f1a` remains the Rust base;
source input is 856 physical and its observed route child is nameable.
