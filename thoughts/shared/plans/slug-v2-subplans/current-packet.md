# Current Slug V2 Packet

Packet: `WP-6-7A-host-prepared-module-extension-inputs-observation-carrier-visibility-implementation`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Scheduling/design base: `1e20b072`
Rust base: `682c4a1e`

## Goal and authority

Expose only the accepted prepared-input observation surface to its future
sibling pure-invocation owner. Add one opaque crate-internal outer wrapper and
one compile-discriminating sibling test. Do not compute the observed key from
pure, change semantics or activate any caller.

Rust authority is exactly production
`app/slug_loading_v2/src/bzl_module.rs`, baseline 9,108 physical lines with its
owning test module at 5,750, and test-only
`app/slug_loading_v2/src/module_extension.rs`, baseline 1,592 with test support
at 767 and its owning test module at 869. Caps are <=40 production, <=45 proof,
<=85 aggregate semantic and <=9,150/1,640 physical. Add only one nominal
wrapper, one sibling test and one nested compile helper; keep every changed
helper/test below 70 lines. Every other Rust file, test, fixture, oracle,
Cargo/BUILD target, API, export, caller and plan is read-only.

The large producer remains cohesive because this packet changes only the
visibility and outer projection of an existing DICE owner. The sibling consumer
module is proof-only. Splitting or reexporting the surface would widen rather
than simplify the ownership boundary.

## Frozen crate-internal surface

In `bzl_module.rs`, change exactly these existing items to `pub(crate)`:

- `HostPreparedModuleExtensionInputsObservationKey` and only its `new`
  constructor;
- `ObservedHostPreparedModuleExtensionInputs`; and
- only its borrowed `result()` and `observations()` accessors.

Keep the key tuple field and both carrier fields private. Spell `result()` as:

```rust
pub(crate) fn result(
    &self,
) -> &Arc<
    Result<HostPreparedModuleExtensionInputs, HostPreparedModuleExtensionInputsError>,
>
```

Keep `observations()` exactly
`pub(crate) fn observations(&self) -> &PathObservationEpoch`. Do not expose or
rename `PreparedModuleExtensionInputsResult`.

Add exactly this nominal shape beside the existing private outer enum:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub(crate) struct HostPreparedModuleExtensionInputsObservationError(
    PreparedModuleExtensionInputsObservationError,
);
```

The tuple field remains private and there is no constructor, accessor,
conversion, Display/Error implementation or unwrapper. A wrapper is required:
the private enum cannot remain in the associated Value of a crate-visible type,
and promoting the enum would reveal its Raw/Definitions/Merge variants.

Change only the observation key's associated `Key::Value` error to
`HostPreparedModuleExtensionInputsObservationError`. At its existing
`SourcePreparationOutcome::Complete(Err(error))` arm, construct the wrapper and
return `Complete(Err(HostPreparedModuleExtensionInputsObservationError(error)))`.
The private shared driver and finishers continue to use and inspect only
`PreparedModuleExtensionInputsObservationError`. Need and success projection,
legacy projection, key equality/validity and existing same-module proof remain
unchanged.

Add no crate-root/lib reexport, external visibility, adapter key, result alias,
public field, stage inspector, reverse dependency, semantic caller or second
carrier/error type.

## Exact sibling compile proof

In the existing `module_extension.rs` `tests` module, import exactly the key,
carrier and opaque error from `crate::bzl_module`. Add one test named
`prepared_observation_surface_is_sibling_module_usable`.

Construct only the key with `NormalizedAbsolutePath::new("/workspace")` and
assert exact Display:

```text
observed-host-prepared-module-extension-inputs:"/workspace"
```

Define one nested `inspect` function taking, in order:

1. `&<HostPreparedModuleExtensionInputsObservationKey as Key>::Value`;
2. `&ObservedHostPreparedModuleExtensionInputs`; and
3. `&HostPreparedModuleExtensionInputsObservationError`.

Inside it, type-check `observed.result()` as exactly
`&Arc<Result<HostPreparedModuleExtensionInputs,
HostPreparedModuleExtensionInputsError>>` and `observed.observations()` as
exactly `&PathObservationEpoch`. Cast the function item to a function pointer
whose first parameter spells the concrete
`&SourcePreparationOutcome<Result<ObservedHostPreparedModuleExtensionInputs,
HostPreparedModuleExtensionInputsObservationError>>`; retain the two carrier/
error parameters. This proves the associated Value and all names are usable
from the sibling module.

Do not construct an outcome, carrier or error; compute either prepared key;
inspect or unwrap the error; add a synthetic hook; or activate pure. Existing
same-module tests cannot replace this proof because they can already reach
private items.

## Invariants and validation

Preserve exact key hash/identity/Display, Complete-only equality/validity,
carrierless Need/typed outer behavior, semantic Result Arc, transaction-local
epoch, raw-first child order, left-first merge, child-owned event batches, warm
silence, cancellation recovery and compact retention. The wrapper changes only
the observed key's outer projection; it owns no semantic fact, dependency,
request input, event, memory, cache, task or lock. Add no fallback.

Reuse accepted prepared identity/finisher/order/event/lifecycle/cancellation/
nonactivation proof and Bazel 9.2 loading evidence; add no oracle. Run serially:

- `cargo test -p slug_loading_v2 prepared_observation_surface_is_sibling_module_usable`;
- protected `cargo test -p slug_loading_v2 observed_prepared_`;
- full `cargo test -p slug_loading_v2`;
- direct dependent `cargo check -p slug_core_v2`;
- `cargo fmt --all -- --check`; and
- `git diff --check`.

Existing prepared and pure values, errors, dependency order and child event
behavior remain exact Bazel 9 compatibility. The crate-internal key/carrier/
opaque-outer visibility and Result-Arc/epoch association are Slug-native. Pure,
instantiated, validated, generated/public/root-mapping/bootstrap activation and
exact Bazel configuration/output/ActionKey bytes remain unsupported/deferred.

Implementation ACCEPT returns only to a docs-only pure-invocation owner design.
STOP semantic/equality/event/retention drift, public/lib export, private alias/
field/variant exposure, outer unwrapping, second type/key/carrier/adapter,
production `module_extension.rs` change, caller/compute activation, third file,
fixture/oracle work, proof/cap waiver, milestone closure, M8/M7B or exact
identity work. REPLAN before widening. M7 remains partial and
M7A -> M8 -> M7B remains.

## Immediate predecessor

Audit/design `1e20b072` proves pure is the sole production consumer of prepared
inputs, the observed prepared surface is its only visibility blocker, the
Host-Bzl observed child is already crate-visible, and every instantiated,
validated, generated/public or root-mapping owner is later or parallel.
