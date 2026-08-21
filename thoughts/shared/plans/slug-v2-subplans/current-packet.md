# Current Slug V2 Packet

Packet: `WP-6-7A-host-canonical-repository-apparent-mapping-observation-implementation`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Design and Rust base: pending docs commit / `e27974c8`

## Goal and exact authority

Add the private callerless observation sibling of
`HostCanonicalRepositoryApparentMappingKey` in its existing core owner. Share
the exact root/nonroot branch selection and mapping reducer between Legacy and
Observed modes. Publish one apparent-mapping Result Arc plus exactly the chosen
child epoch; activate no root-definition or upper consumer.

Write only
`app/slug_core_v2/src/runtime/generated_repository_definition.rs`, baseline
3,328 physical lines with `#[cfg(test)]` at 978 and SHA-256
`06eb23895ba637cc9146f968974c9eee626a4ccbdf02a5bbbc8a5fea26ecd268`.
Every other Rust/test, fixture, oracle, Cargo/BUILD target, API, export, caller
and plan is read-only.

## Exact nominal surface and shared driver

Add private `HostCanonicalRepositoryApparentMappingObservationKey` as a newtype
over the legacy key, with private three-argument `new` and Display
`observed-{legacy}`. Add private
`CanonicalRepositoryApparentMappingResult =
Arc<Result<HostCanonicalRepositoryApparentMapping,
HostCanonicalRepositoryApparentMappingError>>` while leaving the legacy
outcome's concrete spelling unchanged.

Add private `ObservedHostCanonicalRepositoryApparentMapping` containing exactly
that Result Arc and `PathObservationEpoch`, with private borrowed `result()` and
`observations()` accessors. Add private typed outer
`HostCanonicalRepositoryApparentMappingObservationError` with exactly:

- `RootMapping(HostRootRepositoryMappingObservationError)`; and
- `Definition(HostCanonicalRepositoryDefinitionObservationError)`.

Key Value is
`SourcePreparationOutcome<Result<ObservedHostCanonicalRepositoryApparentMapping,
HostCanonicalRepositoryApparentMappingObservationError>>`. Use matching Debug/
Clone/PartialEq/Eq/Hash/Allocative derives on the key and Debug/Clone/PartialEq/
Eq/Allocative/`Dupe` on carrier and outer. Add no visibility, export, alias,
adapter or caller.

Add private `CanonicalRepositoryApparentMappingMode::{Legacy, Observed}` and a
single private child outcome that distinguishes Need, typed Outer, and Complete
with either a branch-specific semantic error kind or
`ApparentMappingPredecessor` plus one epoch. Factor only the current compute
through:

- `complete_canonical_apparent_mapping_driver`;
- root-mapping and canonical-definition child adapters;
- `finish_canonical_repository_apparent_mapping`;
- `compute_canonical_repository_apparent_mapping`; and
- `project_legacy_canonical_repository_apparent_mapping`.

Both Key implementations call this one driver. Preserve existing
`mapping_lookup_status`, `ApparentMappingPredecessor`, value/error/view and
`resolved_target()` logic.

## Branches, terminals and epochs

The nonroot-context/root-apparent preflight remains first. It returns semantic
`RootApparent` with empty epoch and computes no child. Otherwise exactly one
branch runs:

- root context: Legacy computes only `HostRootRepositoryMappingKey`; Observed
  computes only `HostRootRepositoryMappingObservationKey`;
- nonroot context: Legacy computes only
  `HostCanonicalRepositoryDefinitionKey`; Observed computes only the private
  same-file `HostCanonicalRepositoryDefinitionObservationKey`.

DICE child compute failure remains semantic `RootMappingCompute` or
`DefinitionCompute` with empty epoch. Need returns immediately with no carrier.
Observed child outer maps carrierlessly to typed `RootMapping` or `Definition`.
Complete child semantic error remains semantic `RootMapping` or `Definition`
and retains that child's exact epoch. Success converts the child value to the
same `ApparentMappingPredecessor` and retains the exact epoch.

After child success, preserve contexts-before-target order. Missing views or
published/context mismatch remain `ContextMismatch { predecessor }`; a valid
context with no apparent target remains `Missing { predecessor }`; success
retains the predecessor and apparent name. Each terminal/success retains the
chosen child epoch unchanged. There is never a second child, epoch merge,
rebuild, union, validation or empty-epoch substitution after child completion.
The legacy projection moves the exact local Result Arc and discards only its
necessarily empty epoch.

The observed key uses `complete_eq` and Complete-only validity: Need is invalid
and self-unequal; Complete carrier/outer compares structurally. Result equality
alone cannot cut off an epoch-only change.

## Events, retention and lifecycle

Apparent mapping owns no event batch. Exact fresh observed dependency rows are
`[]` for RootApparent preflight,
`[observed-host-root-repository-mapping]` for root context, and
`[observed-host-canonical-repository-definition]` for nonroot context. Legacy
rows use exactly the corresponding legacy child. Matching-family exclusion is
mandatory: no canonical-definition child on root, no root-mapping child on
nonroot, and no child on preflight. Accepted lower event owners/payloads remain
unchanged; parent and every warm/Reused row are batchless and never replay or
move lower prints. Need, child outer and cancellation publish no parent carrier
or batch.

Each carrier retains only the new apparent-mapping Result Arc and compact chosen
child epoch. Existing success, ContextMismatch and Missing values retain the
same cloned predecessor value; semantic child errors retain only the same
cloned child error. Child carrier/Result Arc, unchosen branch, view/iterator/
lookup, mode, event/tracker, cache, task and lock scratch die before
publication. DICE alone serializes compute; no lock or task crosses an await.

Add exactly:

- `observed_canonical_repository_apparent_mapping_identity_branch_and_terminal_algebra`;
- `observed_canonical_repository_apparent_mapping_real_branches_events_and_parity`;
- `observed_canonical_repository_apparent_mapping_lifecycle_cancellation_and_nonactivation`.

The first proves key equality/hash/Display/accessors, Need/Complete/outer
equality/validity, RootApparent/no-child, both compute and semantic-error
families, Definition outer, ContextMismatch/Missing/success, exact Result/epoch
forwarding, branch polarity and absence of merge. Reuse accepted Bzlmod proof
for the opaque root-child outer, and add only parent source/dependency mapping
evidence; do not access its private field or inject malformed DICE state.

The real proof covers root and nonroot success/error parity with legacy, exact
one-child rows and matching-family exclusion, borrowed target/order behavior,
lower event owner/payload equality, batchless parent/warm behavior and no
replay. Reuse current real apparent-mapping fixtures and accepted observed
root/canonical child proof.

The lifecycle proof holds Result and epoch handles through independent root and
nonroot semantic A-B-A changes plus comment-only equal-Result/different-epoch
changes; checks every epoch against its own transaction global, Arc identity
only on a proven Reused value, poll-drop/no publication and same-DICE recovery
for both branches. Deny legacy apparent mapping, the unchosen child family,
root apparent definition, root route/source input/source observation/source
path, repository route/source/file, materialization and public command/
bootstrap activation. Use real trackers/source direction only; add no hook,
fresh-graph bypass or malformed runtime injection.

## Caps, validation and compatibility

Caps are <=260 production, <=720 proof, <=980 aggregate semantic and <=4,310
physical; at most six new production/seven test helpers, exactly three tests,
shared driver below 140 and every helper/test below 200. The large file remains
cohesive because it already owns both observed children, mapping predecessor/
value/error/reducer, trackers and real fixtures; splitting would expose private
canonical state. This is not a demonstrated hot path and adds no retained
container.

Run serially:

- `cargo test -p slug_core_v2 observed_canonical_repository_apparent_mapping_ --lib`;
- protected `cargo test -p slug_core_v2 observed_canonical_repository_definition_ --lib`;
- protected apparent-mapping/root-mapping tests in `slug_core_v2`;
- full `cargo test -p slug_core_v2`;
- direct dependent `cargo check -p slug_commands_v2`;
- protected `cargo test -p slug_bzlmod_v2 root_repository_mapping_observation_surface_is_cross_crate_usable --test root_repository_mapping_observation_api`;
- `cargo fmt --all -- --check`; and
- exact one-file allowlist/SHA/accounting/physical/helper/test checks plus
  `git diff --check`.

Reuse Bazel 9.2 `BazelDepGraphFunction.computeCanonicalRepoNameLookup`,
`BazelDepGraphValue.getRepositoryMapping` and
`BazelDepGraphFunctionTest`; Buck2 DICE dependency/invalidation/cancellation
evidence remains concept/test only. Add no fixture or oracle.

Apparent-mapping branch order, targets, errors, predecessor/order retention,
equality/invalidation and lower events remain **exact** Bazel 9 compatibility.
The private observation key/carrier/typed outer and Result-Arc transaction-local
epoch association are **Slug-native**. Promotion/caller, root apparent
definition/route/source, public command/bootstrap observation and exact Bazel
configuration/output/ActionKey bytes remain **unsupported/deferred**.

## Terminal

ACCEPT returns only to a docs-only canonical apparent-mapping carrier-
visibility audit. STOP a second file/key/owner/adapter, visibility/export/
caller, root-definition or upper compute, branch/preflight/terminal/order/
semantic/event/equality/retention drift, second-child or epoch merge, private
opaque-root-outer access, malformed injection, retained scratch/task/lock,
fixture/oracle, cap/helper/test/format waiver, milestone closure, M8/M7B or
exact identity work. REPLAN before widening or baseline-hash drift. M7 remains
partial and M7A -> M8 -> M7B remains.

## Immediate predecessor

Accepted promotion `e27974c8` exposes exactly the observed root-mapping child
without activating core. The nonroot observed canonical child was already
private in this same source file; no visibility or evidence prerequisite
remains before this owner.
