# Current Slug V2 Packet

Packet: `WP-6-7A-host-instantiated-module-extension-repositories-observation-carrier-visibility-implementation`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Scheduling/design and Rust base: `c1c8e1d8`

## Goal and authority

Expose only the accepted observed instantiation key, carrier and opaque outer
to its validation sibling. Preserve the private driver and every semantic,
event, equality, retention and lifecycle behavior. Add one compile-only sibling
proof; do not compute the key or activate validation.

Write authority is exactly:

- `app/slug_loading_v2/src/module_extension_repository_instantiation.rs`,
  baseline 2,049 physical lines with the owning test module at 641; and
- `app/slug_loading_v2/src/module_extension_repository_validation.rs`, baseline
  1,156 physical lines with tests at 332, test-only.

Caps are <=60 production, <=50 proof and <=110 aggregate semantic; physical
caps are 2,110 and 1,210. Every changed helper/test stays below 100 lines.
Every other Rust file, test, fixture, oracle, Cargo/BUILD target, lib export,
API and caller is read-only. The large instantiation file remains cohesive
because it owns the private driver/representation; validation is its sole
future consumer and natural visibility witness.

## Exact crate-internal surface

Promote exactly these existing items to `pub(crate)`:

1. `HostInstantiatedModuleExtensionRepositoriesObservationKey` and its `new`;
2. `ObservedHostInstantiatedModuleExtensionRepositories`; and
3. its `result()` and `observations()` methods.

Keep key and carrier fields private. Preserve the key's
Debug/Clone/PartialEq/Eq/Hash/Allocative derives, legacy workspace identity,
`observed-{legacy Display}`, Complete-only equality and validity. Preserve the
carrier's Debug/Clone/PartialEq/Eq/Allocative/Dupe derives and exact retained
Result Arc plus transaction-local `PathObservationEpoch`.

Spell `result()` concretely as:

```rust
pub(crate) fn result(
    &self,
) -> &Arc<Result<
    HostInstantiatedModuleExtensionRepositories,
    HostInstantiatedModuleExtensionRepositoriesError,
>>
```

Keep `observations()` returning `&PathObservationEpoch`. Keep
`InstantiatedRepositoriesResult` private; expose no alias, field or constructor.

## Required opaque wrapper projection

Rename the current private terminal enum to
`InstantiatedModuleExtensionRepositoriesObservationError`. Preserve exactly:

```rust
Pure(HostPureModuleExtensionInvocationsObservationError)
```

The private driver and `InstantiatedRepositoriesDriverOutcome` continue to use
that private enum. Add exactly one crate-visible nominal wrapper:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub(crate) struct HostInstantiatedModuleExtensionRepositoriesObservationError(
    InstantiatedModuleExtensionRepositoriesObservationError,
);
```

The tuple field stays private. Add no accessor, inspector, conversion trait or
reexport. The observed key's associated Value is exactly:

```rust
SourcePreparationOutcome<Result<
    ObservedHostInstantiatedModuleExtensionRepositories,
    HostInstantiatedModuleExtensionRepositoriesObservationError,
>>
```

Wrap only the private driver's `Complete(Err(error))` in the observed key's
compute projection. Need and successful Complete projection remain unchanged.
No other production function wraps or unwraps the outer. Same-module proof may
name the private inner only as needed to preserve the accepted static producer
assertion; the validation sibling must treat the wrapper as opaque.

Do not change the legacy key/value/error, shared driver mode/helpers, Pure
contents, Result/epoch association, key Display/hash/equality/validity, event
ownership or any DICE dependency.

## Exact validation-sibling compile proof

Add exactly one test in the existing validation test module:
`instantiation_observation_surface_is_validation_sibling_usable`.

It may import only the crate-internal observed instantiation key, carrier and
opaque wrapper from `crate::module_extension_repository_instantiation`. It must:

- construct the observed key with `NormalizedAbsolutePath::new("/workspace")`
  and assert exact unchanged Display;
- define one nonexecuted local `inspect` taking
  `&<HostInstantiatedModuleExtensionRepositoriesObservationKey as Key>::Value`,
  `&ObservedHostInstantiatedModuleExtensionRepositories`, and
  `&HostInstantiatedModuleExtensionRepositoriesObservationError`;
- type-check `result()` against the concrete
  `Arc<Result<HostInstantiatedModuleExtensionRepositories,
  HostInstantiatedModuleExtensionRepositoriesError>>` and `observations()`
  against `PathObservationEpoch`; and
- cast `inspect` to the explicit
  `SourcePreparationOutcome<Result<carrier, opaque outer>>` function-pointer
  shape, following the accepted pure-carrier sibling smoke.

It must not construct a carrier or outer, inspect wrapper contents, compute a
key, add a DICE driver, observe dependencies/events or activate validation.
Preserve every accepted observed-instantiation test discriminator.

## Validation, compatibility and terminal

Reuse accepted Bazel 9.2, DICE, instantiation and validation evidence; add no
oracle. Run serially:

- the named
  `instantiation_observation_surface_is_validation_sibling_usable` test;
- focused `observed_instantiation_` tests;
- protected `real_key_` and `real_validation_` tests;
- full `cargo test -p slug_loading_v2`;
- direct dependent `cargo check -p slug_core_v2`;
- `cargo fmt --all -- --check`; and
- `git diff --check`.

Existing instantiation/validation values, errors, ordering, import/override
polarity, `RepoSpec` iteration, DICE equality and pure-owned events remain exact
Bazel 9 compatibility. The crate-internal key/carrier/opaque wrapper and
Result-Arc/epoch handoff are Slug-native. Validation observation,
generated/public/root-mapping/bootstrap activation and exact Bazel
configuration/output/ActionKey bytes remain unsupported/deferred.

Implementation ACCEPT returns only to a docs-only
`HostValidatedModuleExtensionRepositoriesKey` observation-owner design. STOP a
public/lib reexport, exposed alias/field/variant/inspector, second key/carrier/
adapter, caller or compute activation, validation semantic change, event/
equality/retention drift, third file, fixture/oracle work, cap waiver, upper or
parallel activation, milestone closure, M8/M7B or exact identity work. REPLAN
before widening. M7 remains partial and M7A -> M8 -> M7B remains.

## Immediate predecessor

Audit `c605d25f` proves the accepted observed instantiation carrier is the only
missing input to validation. Generated publication is later; selected
definition and root mapping are parallel.
