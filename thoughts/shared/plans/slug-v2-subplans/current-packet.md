# Current Slug V2 Packet

Packet: `WP-6-7A-host-pure-module-extension-invocations-observation-carrier-visibility-implementation`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Scheduling/design and Rust base: `9bab80b3`

## Goal and authority

Expose only the accepted observed pure-invocation key/carrier/opaque outer to
its instantiation sibling. Preserve the private driver and every semantic,
event, equality, retention and lifecycle behavior. Add one compile-only sibling
proof; do not compute the key or activate instantiation.

Write authority is exactly:

- `app/slug_loading_v2/src/module_extension.rs`, baseline 2,232 physical lines
  with first `#[cfg(test)]` at 895; and
- `app/slug_loading_v2/src/module_extension_repository_instantiation.rs`,
  baseline 1,363 physical lines with tests at 532, test-only.

Caps are <=60 production, <=50 proof and <=110 aggregate semantic; physical
caps are 2,290 and 1,415. Every changed helper/test remains below 100 lines.
Every other Rust file, test, fixture, oracle, Cargo/BUILD target, lib export,
API and caller is read-only. This pair is cohesive because
`module_extension.rs` owns the private pure driver and representation while
the instantiation sibling is its sole future consumer and natural visibility
witness.

## Exact crate-internal surface

Promote exactly these existing items to `pub(crate)`:

1. `HostPureModuleExtensionInvocationsObservationKey` and its existing `new`;
2. `ObservedHostPureModuleExtensionInvocations`; and
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
) -> &Arc<Result<HostPureModuleExtensionInvocations,
                 HostPureModuleExtensionInvocationsError>>
```

Keep `observations()` returning `&PathObservationEpoch`. Keep
`PureInvocationsResult` private; expose no alias, field or constructor.

## Required opaque wrapper projection

Rename the current private terminal enum to
`PureModuleExtensionInvocationsObservationError`. Preserve exactly its three
variants and all fields:

- `Prepared(HostPreparedModuleExtensionInputsObservationError)`;
- `HostBzl { prepared, index, error }`; and
- `Merge { prepared, index, error }`.

The private driver, helpers and `PureInvocationsDriverOutcome` continue to use
that private enum. Add exactly one crate-visible nominal wrapper:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Allocative, Dupe)]
pub(crate) struct HostPureModuleExtensionInvocationsObservationError(
    PureModuleExtensionInvocationsObservationError,
);
```

The tuple field stays private. Add no accessor, inspector, conversion trait or
reexport. The associated `Key::Value` is exactly:

```rust
SourcePreparationOutcome<Result<
    ObservedHostPureModuleExtensionInvocations,
    HostPureModuleExtensionInvocationsObservationError,
>>
```

Wrap only the private driver's `Complete(Err(error))` in the observed key's
compute projection. Need and successful Complete projection remain unchanged.
No other production function wraps or unwraps the outer. Same-module proof may
match the wrapper's private inner to preserve HostBzl-vs-Merge stage/prefix
coverage; sibling code may only treat it as opaque.

Do not change the legacy key/value/error, private driver modes/helpers,
Prepared/HostBzl/Merge contents, Result/epoch association, event predicate,
key Display/hash/equality/validity, or any DICE dependency.

## Exact sibling compile proof

Add exactly one test in the existing instantiation test module:
`pure_observation_surface_is_instantiation_sibling_usable`.

It may import only the crate-internal observed pure key, carrier and opaque
wrapper needed from `crate::module_extension`. It must:

- construct the key with `NormalizedAbsolutePath::new("/workspace")` and
  assert unchanged Display;
- define one nonexecuted local `inspect` function taking
  `&<HostPureModuleExtensionInvocationsObservationKey as Key>::Value`,
  `&ObservedHostPureModuleExtensionInvocations`, and
  `&HostPureModuleExtensionInvocationsObservationError`;
- inside `inspect`, type-check `result()` against the concrete
  `Arc<Result<HostPureModuleExtensionInvocations,
  HostPureModuleExtensionInvocationsError>>` and `observations()` against
  `PathObservationEpoch`; and
- cast `inspect` to the explicit
  `SourcePreparationOutcome<Result<carrier, opaque outer>>` function-pointer
  shape, following the accepted prepared-carrier sibling smoke.

It must not construct a carrier or outer, inspect wrapper contents, compute a
key, add a DICE driver, observe dependencies/events, or activate the existing
instantiation key. Adjust same-module pure outer assertions only as required by
the wrapper; preserve every accepted pure test discriminator.

## Validation, compatibility and terminal

Reuse accepted Bazel 9.2 and pure/instantiation evidence; add no oracle. Run
serially:

- the named `pure_observation_surface_is_instantiation_sibling_usable` test;
- focused `observed_pure_` tests;
- protected `pure_instantiation_` and `real_key_` tests;
- full `cargo test -p slug_loading_v2`;
- direct dependent `cargo check -p slug_core_v2`;
- `cargo fmt --all -- --check`; and
- `git diff --check`.

Existing pure and instantiation values, errors, order, namespace/attribute/
label semantics, DICE equality and event behavior remain exact Bazel 9
compatibility. The crate-internal key/carrier/opaque wrapper and Result-Arc/
epoch handoff are Slug-native. Instantiation observation, validation,
generated/public/root-mapping/bootstrap activation and exact Bazel
configuration/output/ActionKey bytes remain unsupported/deferred.

Implementation ACCEPT returns only to a docs-only
`HostInstantiatedModuleExtensionRepositoriesKey` observation-owner design.
STOP a public/lib reexport, exposed alias/field/variant/inspector, second key/
carrier/adapter, caller or compute activation, instantiation semantic change,
event/equality/retention drift, third file, oracle/fixture work, cap waiver,
validation/generated/root-mapping/public/bootstrap activation, milestone
closure, M8/M7B or exact identity work. REPLAN before widening. M7 remains
partial and M7A -> M8 -> M7B remains.

## Immediate predecessor

Audit `a8482660` proves the accepted pure carrier is the only missing input to
the instantiation owner: validation/generated publication is later and root
mapping/canonical selection is parallel. No public surface or semantic
prerequisite remains.
