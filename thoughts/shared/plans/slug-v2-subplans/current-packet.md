# Current Slug V2 Packet

Packet: `WP-6-7A-host-canonical-selected-module-definition-observation-implementation`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Scheduling/design and Rust base: docs-only owner design / `7f9325e1`

## Goal and authority

Implement only the accepted private observation owner for
`HostCanonicalSelectedModuleDefinitionKey`. Preserve the legacy selected-
routes scan, values/errors/dispositions and lower event ownership. Do not expose
the carrier, edit core, or activate canonical/generated/root/publication.

Write authority is exactly
`app/slug_bzlmod_v2/src/selected_repo_spec.rs`, baseline 11,687 physical lines
with tests at 4,510. Every other Rust file, test, fixture, oracle, Cargo/BUILD
target, API, export, caller and plan is read-only. Production is <=230 lines,
proof <=680, aggregate semantic authority <=910 and physical size <=12,600.
Add at most six production and six test helpers, exactly three tests, one shared
driver below 120 lines, and keep every changed helper/test below 200. Retain the
one-file owner: this file already owns the private routes observation/result/
outer, selected reducer, public value/error/view and real fixture; a sibling
would expose private state only to evade the size trigger.

## Exact private surface

Add exactly these private nominal types:

1. `HostCanonicalSelectedModuleDefinitionObservationKey(
   HostCanonicalSelectedModuleDefinitionKey)`, with private `new`, identical
   workspace/canonical identity and Display `observed-{legacy Display}`.
   For `/workspace` and canonical `dep+`, assert
   `observed-host-canonical-selected-module-definition:"/workspace":@@dep+`.
2. `ObservedHostCanonicalSelectedModuleDefinition`, retaining exactly
   `Arc<Result<HostCanonicalSelectedModuleDefinition,
   HostCanonicalSelectedModuleDefinitionError>>` and one
   `PathObservationEpoch`, with private borrowed `result()` and
   `observations()` accessors.
3. `HostCanonicalSelectedModuleDefinitionObservationError::Routes(
   HostSelectedModuleRoutesObservationError)`.

The key derives Debug/Clone/PartialEq/Eq/Hash/Allocative. Carrier and outer
derive Debug/Clone/PartialEq/Eq/Allocative/Dupe. Narrow dead-code attributes are
allowed only where the callerless private surface requires them. Add no alias,
reexport, adapter or caller beyond private driver aliases.

The observed Key Value is
`SourcePreparationOutcome<Result<
ObservedHostCanonicalSelectedModuleDefinition,
HostCanonicalSelectedModuleDefinitionObservationError>>`. Both legacy and
observed keys use `complete_eq` equality and Complete-only validity.

## Shared driver and terminals

Add `CanonicalSelectedModuleDefinitionMode::{Legacy, Observed}`, one private
selected Result alias, one driver-outcome alias,
`complete_canonical_selected_definition_driver`, and
`compute_canonical_selected_module_definition`. Refactor the legacy key to
project this driver; the observed key projects its carrier. No other producer
or helper owns semantic selection.

Legacy requests only `HostSelectedModuleRoutesKey` and associates an empty
epoch. Observed requests only `HostSelectedModuleRoutesObservationKey`; a
successful carrier contributes its Result Arc and epoch unchanged. Exact
terminal law is:

- child Need returns Need immediately, with no parent carrier;
- DICE child compute failure produces existing
  `PrivateCanonicalSelectedModuleDefinitionError::RoutesCompute(message,
  canonical_repo)` with empty epoch;
- observed route outer produces carrierless
  `HostCanonicalSelectedModuleDefinitionObservationError::Routes(error)`;
- complete route semantic failure produces existing `Routes(predecessor,
  canonical_repo)` with the child epoch;
- successful routes run `find_canonical_route_ordinal` exactly once across
  every entry: Missing retains predecessor+canonical; Duplicate retains
  predecessor+canonical+first/conflicting ordinals; Unique then rejects
  BuiltinBazelTools as BuiltinDeferred retaining predecessor+ordinal+canonical,
  otherwise publishes the existing selected value with predecessor+ordinal.

Every complete semantic success/failure carries the unchanged child epoch.
This one-child owner performs no merge, rebuild, union or epoch validation.
Legacy projection debug-asserts the epoch is empty and treats an outer as
unreachable. Observed projection adds no unwrap/inspection path.

## Event, retention and lifecycle proof

The new owner is batchless. Its observed dependency row is exactly the observed
routes key and its legacy row exactly the legacy routes key. For the protected
real fixture, preserve the observed lower batch-owner order
`bzlmod-observed-host-root-module-file:"/selected-repo-spec-test"`, then
`host-discovered-module:"/selected-repo-spec-test":dep@1`; preserve legacy
`root-module-evaluation:/selected-repo-spec-test`, then the same discovered
module, with identical batch payload vectors. Every parent and warm/Reused row
is batchless. Need, outer, first terminal and cancellation publish no parent
carrier/batch, and no lower batch is replayed.

DICE retains only the selected Result Arc and route epoch. The Result already
retains exactly routes+ordinal or the existing error fields. Route carrier,
iterator/scan counters, evaluator, event, mode, cache, task and lock are
compute scratch. DICE owns serialization; no lock/task crosses compute.

Add exactly:

1. `observed_canonical_selected_definition_identity_scan_and_terminal_algebra`
   for key/hash/Display/accessors/equality/validity, single-child/no-merge,
   Need/outer/RoutesCompute/Routes/Missing/Duplicate/BuiltinDeferred stages and
   the protected exhaustive first/conflicting scan;
2. `observed_canonical_selected_definition_real_order_events_and_parity` for
   real root/registry/nonregistry/Missing/BuiltinDeferred/Routes parity, exact
   family child rows, lower batch owner/payload vectors, parent/warm
   eventlessness and first-terminal suppression; and
3. `observed_canonical_selected_definition_lifecycle_cancellation_and_nonactivation`
   for held Result/carrier/epoch A-B-A across registry source, selected
   version, route/mapping order and local-path changes; metadata-only revision
   with equal Result/legacy value, different epoch/observed value and therefore
   no observed equality cutoff; every carrier epoch a subset of its own
   transaction `PathObservationEpochKey`; Arc identity only for a proven
   Reused value; poll-drop absence and recovery; and zero legacy-selected,
   root-mapping, selected-extension, canonical/generated/root/route/source/
   public/command/bootstrap activation.

Use real tracker rows where types are available and a bounded private-producer/
dependency-direction source scan for inaccessible core/command denylist types.
Do not add malformed production hooks. Reuse
`pure_canonical_selected_definition_exhausts_and_retains_identity`, real
selected lifecycle and accepted observed-routes tests.

## Evidence, validation and terminal

Bazel 9.2 `BazelDepGraphFunction.computeCanonicalRepoNameLookup`,
`BazelDepGraphValue.getRepositoryMapping`, `ModuleKey` canonical-name rules
and `BazelDepGraphFunctionTest` are accepted exact source/test evidence.
`dice/dice/docs/incrementality.md`, `cancellations.md`,
`dice_tests/src/linear_recompute.rs` and
`dice/src/impls/tests/activation_tracker.rs` are concept/test evidence only.
No oracle or fixture is needed.

Run serially:

- `cargo test -p slug_bzlmod_v2 observed_canonical_selected_definition_`;
- protected `cargo test -p slug_bzlmod_v2 pure_canonical_selected_definition_`;
- protected `cargo test -p slug_bzlmod_v2 real_canonical_selected_definition_`;
- protected `cargo test -p slug_bzlmod_v2 observed_routes_`;
- full `cargo test -p slug_bzlmod_v2`;
- direct dependent `cargo check -p slug_core_v2`;
- `cargo fmt --all -- --check`; and
- `git diff --check`.

Existing selected values/errors, dispositions, full scan/order, views,
equality/invalidation and lower batches remain exact Bazel 9 compatibility.
The private Result-Arc/transaction-local epoch carrier is Slug-native.
Cross-crate promotion, canonical/generated observation composition, root/
publication/command/bootstrap activation and exact Bazel configuration/output/
ActionKey bytes remain unsupported/deferred.

ACCEPT returns only to a docs-only selected-carrier visibility audit. STOP a
second Rust file/key/owner/adapter, export/reexport/caller, core edit, canonical/
generated compute, selected semantic/order/disposition/event/equality/retention
drift, epoch merge, retained scratch, task/lock, fixture/oracle, cap/helper/test
waiver, upper activation, milestone closure, M8/M7B or identity-byte work.
REPLAN before widening. M7 remains partial and M7A -> M8 -> M7B remains.

## Immediate predecessor

Audit `1f93f448` proves the private selected owner is the uniquely smallest
prerequisite. Rust base `7f9325e1` already supplies the later same-module
generated observation branch and proves it remains inactive here.
