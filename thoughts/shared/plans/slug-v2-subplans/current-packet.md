# Current Slug V2 Packet

Packet: `WP-6-7A-host-validated-module-extension-repositories-observation-carrier-promotion-implementation`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Scheduling/design and Rust base: `556de141` / `b8459b4e`

## Goal and authority

Implement only the accepted doc-hidden loading -> core visibility surface for
the existing validation observation key, carrier and opaque outer. Do not add a
semantic consumer or change the private driver, validation Result/epoch
association, Need/outer algebra, equality, validity, events, retention or
lifecycle.

Write authority is exactly:

- `app/slug_loading_v2/src/module_extension_repository_validation.rs`,
  baseline 1,810 physical lines with tests at 437;
- `app/slug_loading_v2/src/lib.rs`, baseline 86 physical lines; and
- new
  `app/slug_loading_v2/tests/validated_repository_observation_api.rs`.

Every other Rust file, test, fixture, oracle, Cargo/BUILD target, API, caller and
plan is read-only. Production is <=70 lines, colocated proof <=40, external
proof <=60 and aggregate semantic authority <=170. Physical caps are
1,880/95/60, and every changed helper/test remains below 100 lines. The large
validation file remains cohesive because it owns the private driver, carrier
and terminal representation; no split or hot-path measurement is warranted for
this visibility-only step.

## Frozen nominal surface

Promote exactly these three `#[doc(hidden)]` nominal types from
`module_extension_repository_validation.rs`:

1. existing `HostValidatedModuleExtensionRepositoriesObservationKey`;
2. existing `ObservedHostValidatedGeneratedRepositorySpecs`; and
3. new `HostValidatedModuleExtensionRepositoriesObservationError`, an opaque
   public tuple wrapper with a private field.

Make the key and carrier `pub`, the key's existing
`new(NormalizedAbsolutePath) -> Self` public, and only these carrier accessors
public:

- `result()` returning a borrowed `Arc` whose concrete payload is
  `Result<HostValidatedGeneratedRepositorySpecs,
  HostValidatedGeneratedRepositorySpecsError>`; and
- `observations() -> &PathObservationEpoch`.

Keep the key/carrier fields and `ValidatedRepositoriesResult` alias private.
Preserve exact key Debug/Clone/PartialEq/Eq/Hash/Allocative derives and carrier
Debug/Clone/PartialEq/Eq/Allocative/Dupe derives. Preserve workspace identity,
Complete-only equality/validity and exact Display. For `/workspace`, the smoke
must assert
`observed-host-validated-module-extension-repositories:"/workspace"`.

Add exactly these three `#[doc(hidden)]` crate-root reexports, and no fourth:

- `HostValidatedModuleExtensionRepositoriesObservationError`;
- `HostValidatedModuleExtensionRepositoriesObservationKey`; and
- `ObservedHostValidatedGeneratedRepositorySpecs`.

Add no public Result/outcome alias, field, error inspector, constructor for the
outer, conversion trait, adapter key, copied carrier or reverse dependency.

## Exact wrapper and projection

Rename the current private
`HostValidatedModuleExtensionRepositoriesObservationError` enum to
`ValidatedModuleExtensionRepositoriesObservationError`. Preserve exactly its
`Instantiation(HostInstantiatedModuleExtensionRepositoriesObservationError)`
variant and Debug/Clone/PartialEq/Eq/Allocative/Dupe derives. The private driver
outcome and all private terminal construction continue to use this inner enum.

Define the public nominal wrapper as tuple struct
`HostValidatedModuleExtensionRepositoriesObservationError` with one private
`ValidatedModuleExtensionRepositoriesObservationError` field, `#[doc(hidden)]`
and matching
Debug/Clone/PartialEq/Eq/Allocative/Dupe derives. The omitted tuple-field
visibility makes construction and inspection private.

Change only the observed key's associated `Key::Value` error to the public
wrapper. At the key projection, map only
`Complete(Err(inner))` to
`Complete(Err(HostValidatedModuleExtensionRepositoriesObservationError(inner)))`.
Need and successful Complete remain byte-for-byte in behavior. Add no unwrap
path: no current production consumer exists, and the later core owner must carry
the opaque outer without inspecting it. Update the existing private producer
scan only for the renamed inner terminal spelling; change no semantic proof.

## Exact external smoke and evidence

Add one test named
`validated_repository_observation_surface_is_cross_crate_usable`. It imports
the three hidden reexports plus existing public validation value/error,
`LoadingPreparationOutcome`, `dice::Key`, `NormalizedAbsolutePath`,
`PathObservationEpoch` and `Arc`. It:

1. constructs only the key for `/workspace` and asserts the exact Display
   above;
2. type-checks
   `<HostValidatedModuleExtensionRepositoriesObservationKey as Key>::Value`
   as `LoadingPreparationOutcome` over
   `Result<ObservedHostValidatedGeneratedRepositorySpecs,
   HostValidatedModuleExtensionRepositoriesObservationError>` through a
   nonexecuted function-pointer cast; and
3. type-checks the two borrowed carrier accessors against the exact concrete
   Result Arc and epoch types through a second nonexecuted function pointer.

The smoke must not construct a carrier/outer, inspect the wrapper, compute a
key, add a semantic caller or name the private alias/inner/instantiation outer.
Reuse all accepted validation proof and the earlier Bzlmod promotion pattern;
add no oracle because no Bazel-visible behavior changes.

Run serially:

- `cargo test -p slug_loading_v2 observed_validation_ --lib`;
- `cargo test -p slug_loading_v2 --test validated_repository_observation_api`;
- full `cargo test -p slug_loading_v2`;
- `cargo test -p slug_core_v2 generated_repository_definition::tests::`;
- direct dependent `cargo check -p slug_core_v2`;
- `cargo fmt --all -- --check`; and
- `git diff --check`.

## Compatibility and terminal

Existing validation/generated values, errors, order, certificate iteration,
DICE equality and lower event ownership remain exact Bazel 9 compatibility.
The doc-hidden cross-crate key/carrier/opaque outer and shared-Arc transaction-
local epoch association are Slug-native. Generated observation and canonical/
root-mapping/publication/command/bootstrap activation plus exact Bazel
configuration/output/ActionKey bytes remain unsupported/deferred.

Implementation ACCEPT returns only to a docs-only
`HostGeneratedRepositoryDefinitionKey` observation-owner design. STOP semantic
or compute activation, public field/alias/terminal inspection, a fourth type or
reexport, second key/carrier/adapter, validation/generated semantic or event/
equality/retention change, any core source change, reverse dependency, Cargo/
BUILD or fixture/oracle work, third production file, cap/proof waiver,
canonical/root-mapping/publication/command/bootstrap activation, milestone
closure, M8/M7B or exact identity work. REPLAN before widening. M7 remains
partial and M7A -> M8 -> M7B remains.

## Immediate predecessor

Design `556de141` confirms this three-type projection is the only missing
cross-crate prerequisite. `b8459b4e` remains the semantic Rust base and already
proves exact validation behavior, Result/epoch retention and upper
nonactivation.
