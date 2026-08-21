# Current Slug V2 Packet

Packet: `WP-6-7A-host-canonical-selected-module-definition-observation-carrier-promotion-implementation`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Scheduling/design and Rust base: `bdeab11d` / `a7d9ffcc`

## Goal and authority

Implement only the accepted doc-hidden Bzlmod -> core visibility surface for
the existing canonical selected-definition observation key, carrier and opaque
outer. Do not add a semantic consumer or change the private driver, selected
Result/epoch association, Need/terminal algebra, equality, validity, events,
retention or lifecycle.

Write authority is exactly:

- `app/slug_bzlmod_v2/src/selected_repo_spec.rs`, baseline 12,524 physical
  lines with tests at 4,668;
- `app/slug_bzlmod_v2/src/lib.rs`, baseline 415 physical lines; and
- new
  `app/slug_bzlmod_v2/tests/canonical_selected_definition_observation_api.rs`.

Every other Rust file, test, fixture, oracle, Cargo/BUILD target, API, caller and
plan is read-only. Production is <=80 lines, colocated proof <=40, lib semantic
change <=10, external proof <=70 and aggregate semantic authority <=200.
Physical caps are 12,645/425/70. Only a semantic-neutral wrapper adjustment to
the existing
`observed_canonical_selected_definition_identity_scan_and_terminal_algebra` is
allowed, and that test must remain under 200 lines; every new smoke/helper must
remain below 100. The large selected file remains cohesive because it owns the
private driver, carrier and projection. No split or hot-path measurement is
warranted for this visibility-only step.

## Frozen nominal surface

Promote exactly these three `#[doc(hidden)]` nominal types from
`selected_repo_spec.rs`:

1. existing `HostCanonicalSelectedModuleDefinitionObservationKey`;
2. existing `ObservedHostCanonicalSelectedModuleDefinition`; and
3. new `HostCanonicalSelectedModuleDefinitionObservationError`, an opaque
   public tuple wrapper with a private field.

Make the key and carrier `pub`, and make the key's existing
`new(NormalizedAbsolutePath, CanonicalRepoName) -> Self` public. Make only these
carrier accessors public:

- `result()` returning
  `&Arc<Result<HostCanonicalSelectedModuleDefinition,
  HostCanonicalSelectedModuleDefinitionError>>`; and
- `observations() -> &PathObservationEpoch`.

Keep all key/carrier/wrapper fields and `SelectedDefinitionResult` private.
Preserve the key's Debug/Clone/PartialEq/Eq/Hash/Allocative derives and the
carrier's Debug/Clone/PartialEq/Eq/Allocative/Dupe derives. Preserve exact
workspace/canonical identity, Complete-only equality/validity and Display. For
`/workspace` and canonical `dep+`, the smoke must assert
`observed-host-canonical-selected-module-definition:"/workspace":@@dep+`.

Add exactly these three `#[doc(hidden)]` crate-root reexports, and no fourth:

- `HostCanonicalSelectedModuleDefinitionObservationError`;
- `HostCanonicalSelectedModuleDefinitionObservationKey`; and
- `ObservedHostCanonicalSelectedModuleDefinition`.

Add no public Result/outcome alias, field, terminal inspector, outer
constructor, conversion trait, adapter key, copied carrier, caller or reverse
dependency.

## Exact private inner and projection

Rename the current private
`HostCanonicalSelectedModuleDefinitionObservationError` enum to
`CanonicalSelectedModuleDefinitionObservationError`. Preserve exactly its
`Routes(HostSelectedModuleRoutesObservationError)` variant and
Debug/Clone/PartialEq/Eq/Allocative/Dupe derives. The private driver outcome and
all private terminal construction continue to use this inner enum.

Define the public nominal wrapper as tuple struct
`HostCanonicalSelectedModuleDefinitionObservationError` with one private
`CanonicalSelectedModuleDefinitionObservationError` field, `#[doc(hidden)]`
and matching Debug/Clone/PartialEq/Eq/Allocative/Dupe derives. The omitted tuple-
field visibility makes construction and inspection private.

Change only the observed key's associated `Key::Value` error to the public
wrapper. At the key projection, map only `Complete(Err(inner))` to
`Complete(Err(HostCanonicalSelectedModuleDefinitionObservationError(inner)))`.
Need and successful Complete remain byte-for-byte in behavior. Add no unwrap
path: no current production consumer exists, and the later core owner must
carry the opaque outer without inspection.

In the existing identity/scan/terminal test, update only the outer projection
and match spelling to include the public wrapper around private `Routes`.
Change no input, control flow, semantic assertion or terminal expectation, and
keep the entire existing test below 200 lines. No other colocated test needs a
semantic change.

## Exact external smoke and evidence

Add one test named
`canonical_selected_definition_observation_surface_is_cross_crate_usable`. It
imports the three hidden reexports plus existing public selected value/error,
`SourcePreparationOutcome`, `dice::Key`, `CanonicalRepoName`,
`NormalizedAbsolutePath`, `PathObservationEpoch` and `Arc`. It:

1. constructs only the key for `/workspace` and canonical `dep+`, then asserts
   the exact Display above;
2. type-checks
   `<HostCanonicalSelectedModuleDefinitionObservationKey as Key>::Value` as
   `SourcePreparationOutcome<Result<
   ObservedHostCanonicalSelectedModuleDefinition,
   HostCanonicalSelectedModuleDefinitionObservationError>>` through a
   nonexecuted function-pointer cast; and
3. type-checks the two borrowed carrier accessors against the exact concrete
   selected Result Arc and epoch types through a second nonexecuted function
   pointer.

The smoke must not construct a carrier/outer, inspect the wrapper, compute a
key, add a semantic caller, import core or name the private alias/inner/routes
outer. Reuse the accepted selected-owner proof, the prior validation promotion
pattern and the existing definition/evaluation Bzlmod API smokes; add no oracle
because no Bazel-visible behavior changes.

Run serially:

- `cargo test -p slug_bzlmod_v2 observed_canonical_selected_definition_ --lib`;
- `cargo test -p slug_bzlmod_v2 --test canonical_selected_definition_observation_api`;
- protected `cargo test -p slug_bzlmod_v2 --test definition_request_observation_api`;
- protected `cargo test -p slug_bzlmod_v2 --test evaluation_input_request_observation_api`;
- full `cargo test -p slug_bzlmod_v2`;
- direct dependent `cargo check -p slug_core_v2`;
- `cargo fmt --all -- --check`; and
- `git diff --check`.

## Compatibility and terminal

Existing selected values/errors/dispositions/full scan/order/views, DICE
equality/invalidation and lower event ownership remain exact Bazel 9
compatibility. The doc-hidden cross-crate key/carrier/opaque outer and shared-
Arc transaction-local epoch association are Slug-native. Canonical/generated
observation composition, root/publication/command/bootstrap activation and
exact Bazel configuration/output/ActionKey bytes remain unsupported/deferred.

Implementation ACCEPT returns only to a docs-only canonical selected/generated
observation-owner design. STOP semantic or compute activation, public field/
alias/terminal inspection, a fourth type or reexport, second key/carrier/
adapter, selected semantic/event/equality/retention change, any core source
change, reverse dependency, Cargo/BUILD or fixture/oracle work, third production
file, cap/proof/test-size waiver, canonical/root/publication/command/bootstrap
activation, milestone closure, M8/M7B or exact identity work. REPLAN before
widening. M7 remains partial and M7A -> M8 -> M7B remains.

## Immediate predecessor

Design `bdeab11d` confirms this three-type projection is the only missing
cross-crate prerequisite. `a7d9ffcc` remains the semantic Rust base and already
proves exact selected behavior, Result/epoch retention and upper nonactivation.
