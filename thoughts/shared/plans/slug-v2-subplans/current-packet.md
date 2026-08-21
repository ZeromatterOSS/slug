# Current Slug V2 Packet

Packet: `WP-6-7A-host-canonical-repository-definition-observation-implementation`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Design and Rust base: pending docs commit / `66a669cc`

## Goal and authority

Add the private callerless observation sibling of
`HostCanonicalRepositoryDefinitionKey` in its existing core owner. Share the
exact selected-first/generated-only-on-semantic-Missing composition between
Legacy and Observed modes. Publish one local canonical Result Arc plus its
transaction-local observation epoch; do not activate an upper consumer.

Write only
`app/slug_core_v2/src/runtime/generated_repository_definition.rs`, baseline
2,902 physical lines with `#[cfg(test)]` at line 872. Every other Rust file,
test, fixture, oracle, Cargo/BUILD target, API, reexport, caller and plan is
read-only.

## Frozen owner and types

Add private `HostCanonicalRepositoryDefinitionObservationKey` as a newtype over
the legacy key with the same two-argument constructor and Display
`observed-{legacy}`. Add private `ObservedHostCanonicalRepositoryDefinition`
holding exactly
`Arc<Result<HostCanonicalRepositoryDefinition,
HostCanonicalRepositoryDefinitionError>>` and `PathObservationEpoch`, with
borrowed accessors. Its Key Value is
`SourcePreparationOutcome<Result<carrier, outer>>`; Complete equality and
validity match accepted observation siblings.

Add only the private typed outer
`HostCanonicalRepositoryDefinitionObservationError` with:

- `Selected(HostCanonicalSelectedModuleDefinitionObservationError)`;
- `Generated { selected_missing:
  HostCanonicalSelectedModuleDefinitionError, error:
  HostGeneratedRepositoryDefinitionObservationError }`; and
- `Merge { selected_missing:
  HostCanonicalSelectedModuleDefinitionError, error:
  ObservedPathFrontierError }`.

No stage enum is needed because there is exactly one merge site, after the
generated child. Add no export, alias, adapter, caller, public field or outer
inspector.

## Exact shared driver and terminal algebra

Factor only the existing canonical compute into private
`CanonicalRepositoryDefinitionMode::{Legacy, Observed}`, one canonical Result
alias, one driver-outcome alias and bounded child/finisher helpers. Legacy
computes only the legacy selected/generated keys and uses empty epochs.
Observed computes only the accepted selected/generated observation keys.

The first child is always selected. Selected Need returns Need. A selected DICE
compute failure remains `SelectedCompute(message)` with an empty epoch. The
observed selected outer becomes carrierless `Selected`. Selected success,
non-Missing semantic failure and their complete epochs are final: success maps
to canonical Selected, a non-Missing error maps to canonical `Selected`, and
generated is not requested. Only an exact semantic error whose disposition is
`Missing` retains that error plus the selected epoch as the generated-stage
prefix.

Only then request generated. Generated Need returns Need without publishing the
stored prefix. Generated DICE compute failure remains semantic
`GeneratedCompute { selected_missing, message }` and carries the selected
prefix epoch. The observed generated outer becomes carrierless `Generated {
selected_missing, error }`. On a complete generated carrier, merge selected
prefix then generated epoch left-first with `PathObservationEpoch::from_shared`.
Equal duplicate demands retain the selected prefix entry/Arc; a differing
result for the same demand becomes carrierless `Merge { selected_missing,
error: ObservedPathFrontierError::Epoch(ConflictingDemand) }`. Valid epochs
cannot create an operation mismatch at this parent merge. OperationMismatch is
proved only through accepted lower typed `Selected` or `Generated` outers; the
latter retains selected Missing. Merge happens before generated semantic
projection. Generated success maps to canonical Generated; generated Missing
maps to canonical Missing retaining both errors; every other generated
semantic error maps to canonical Generated retaining selected Missing. Each
carries the merged epoch.

Need/outer/merge never publishes a carrier. No full scan, Need union, direct
Host read, legacy fallback, epoch reconstruction or runtime injection is
allowed. The shared driver must preserve the exact current Result values,
error fields, selected-first order and generated-on-Missing polarity.

## Events, retention and lifecycle proof

The canonical parent owns no event batch. Selected/generated observed children
retain all lower batches; the parent neither moves nor replays them. Selected
terminal rows have exactly one child edge. Missing-fallback rows have ordered
children `[observed selected, observed generated]`. Parent, instantiation,
validation, generated and every warm/Reused row remain batchless. Later-child
Need/outer/compute/semantic/merge terminals do not replay legacy invocation
prints; cancellation publishes no parent carrier or batch.

Each successful observed carrier retains only the canonical Result Arc and
compact epoch. A carrierless outer retains only its named child outer plus
selected Missing where specified, and no epoch. The child carriers, generated
Result, prefix tuple, merge iterators, mode, event/evaluator state and all
task/lock scratch die before publication. Existing canonical values/errors
retain only their established selected or generated certificates/errors. DICE
owns serialization; add no manual lock, task, cache, store or retained Starlark
heap.

Add exactly:

- `observed_canonical_repository_definition_identity_staging_and_terminal_algebra`;
- `observed_canonical_repository_definition_real_order_events_and_parity`; and
- `observed_canonical_repository_definition_lifecycle_cancellation_and_nonactivation`.

Together prove key identity/hash/Display/accessors/equality/validity; exact
selected success/Need/outer/compute/non-Missing/Missing staging; generated
Need/outer/compute/success/Missing/other error; equal-left merge and conflicting
merge; accepted lower outer/mismatch evidence plus bounded parent mapping,
source and dependency evidence, exact legacy parity and ordered dependency
rows; real selected and
generated success/failure families, lower print order, parent batchlessness,
warm silence and no later invocation prints; held Result/epoch handles through
selected and generated semantic A-B-A, metadata-only equal Result with changed
epoch, each carrier epoch as a subset of its own transaction global, Arc
identity only on proven Reused, poll-drop of real selected-terminal and
Missing-fallback requests plus same-DICE recovery. Prove zero legacy canonical,
apparent/root mapping, root apparent definition, route/source, public command
or bootstrap activation. Use pure finishers only for valid equal/conflicting
epochs and real graph inputs; forbid a malformed epoch, production/test hook or
local synthetic operation mismatch.

## Caps, validation and compatibility

Permit <=320 production, <=740 proof and <=1,060 aggregate semantic net lines,
with <=3,975 physical lines. Add at most eight production and eight test
helpers, exactly three tests, keep the shared driver below 140 lines and every
helper/test below 200. The >2,000-line owner remains cohesive because it
already contains the generated child observation, canonical reducer/value/
error/view, both production consumers, tracker and real fixtures beside the
imported selected handoff; splitting would expose private representations.
This is not a demonstrated hot path and changes no retained representation
beyond the standard Result-Arc+epoch carrier.

Run serially:

- `cargo test -p slug_core_v2 observed_canonical_repository_definition_ --lib`;
- `cargo test -p slug_core_v2 canonical_definition_ --lib`;
- `cargo test -p slug_core_v2 observed_generated_definition_ --lib`;
- `cargo test -p slug_core_v2 real_generated_selected_and_deferred_domains_are_structural --lib`;
- `cargo test -p slug_core_v2 lifecycle_identity_and_mapping_precedence_are_structural --lib`;
- full `cargo test -p slug_core_v2`;
- protected `cargo test -p slug_bzlmod_v2 --test canonical_selected_definition_observation_api`;
- direct dependent `cargo check -p slug_commands_v2`;
- `cargo fmt --all -- --check`; and
- exact accounting/physical/helper/test allowlist plus `git diff --check`.

Reuse Bazel 9.2 `BazelDepGraphFunction.computeCanonicalRepoNameLookup`,
`BazelDepGraphValue.getRepositoryMapping`, `SingleExtensionFunction`,
`SingleExtensionEvalFunction` and `ModuleExtensionResolutionTest`; add no
fixture or oracle. Buck2 DICE incrementality/cancellation and activation tests
remain concept/test evidence only.

Canonical selected/generated values, errors, order, Missing-only fallback,
equality/invalidation and lower events remain exact Bazel 9 compatibility. The
private observed key/carrier/typed outer and shared-Arc transaction-local epoch
are Slug-native. Carrier promotion, an observed upper caller, apparent/root
mapping, root definition/route/source/public/command/bootstrap activation and
exact Bazel configuration/output/ActionKey bytes remain unsupported/deferred.

## Terminal

ACCEPT returns only to a docs-only canonical-observation consumer frontier
audit. STOP a second file/key/owner/adapter, export/reexport/caller, public API,
upper compute change, child order or Missing polarity drift, semantic/error/
event/equality/retention drift, legacy invocation-print replay, epoch union
waiver, retained scratch/task/lock, fixture/oracle, cap/helper/test waiver,
milestone closure, M8/M7B or exact identity work. REPLAN before widening. M7
remains partial and M7A -> M8 -> M7B remains.

## Immediate predecessor

Accepted `66a669cc` exposes only the selected observation key/carrier/opaque
outer to core. The generated observed child is already private in this module.
Live source proves these are the complete lower prerequisites and canonical is
the first selected/generated composition owner.
