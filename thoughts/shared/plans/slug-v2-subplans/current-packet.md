# Current Slug V2 Packet

Packet: `WP-6-7A-host-prepared-module-extension-inputs-observation-carrier-visibility-design`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Audit and Rust base: `682c4a1e`

## Goal and authority

Design only the minimum crate-internal visibility surface by which the sibling
`module_extension.rs` owner can later consume the accepted callerless prepared-
input observation. Freeze exact nominal types, effective visibility, borrowed
accessors, opaque outer handling, compile proof, future implementation
authority/caps and nonactivation. Do not edit Rust or activate pure invocation.

Design write authority is exactly the canonical plan, this manifest, Stage 6
and the routing log. Net caps are <=40/<=180/<=220/<=30 respectively and <=470
aggregate. Rust, tests, fixtures, oracles, Cargo/BUILD, APIs, exports, callers
and other plans are read-only. The design may authorize at most one bounded
visibility-only implementation successor.

## Learned live frontier

`bzl_module.rs` is 9,108 physical lines with the owning test module at 5,750.
The accepted `HostPreparedModuleExtensionInputsObservationKey`,
`ObservedHostPreparedModuleExtensionInputs`, their private constructor and
borrowed Result/epoch accessors, and
`PreparedModuleExtensionInputsObservationError` occupy lines 3048-3409. They
are private to `bzl_module`, so sibling `module_extension` cannot name the key,
carrier or associated `Key::Value` outer.

`module_extension.rs` is 1,592 physical lines with test-only support at 767
and the owning test module at 869. `HostPureModuleExtensionInvocationsKey` and its legacy driver are at
98-211. Its compute at line 158 is the sole production consumer of
`HostPreparedModuleExtensionInputsKey`; there is no production compute of the
observed sibling. Pure's other mutable child needs no promotion:
`ObservedHostBzlModule`, `HostBzlModuleObservationKey`, constructor and borrowed
accessors are already `pub(crate)` at `bzl_module.rs:1303-1344`, with the key
implementation at 2318-2363.

The exact production chain above pure is serial: instantiation alone computes
pure at `module_extension_repository_instantiation.rs:191`; validation alone
computes instantiation at `module_extension_repository_validation.rs:208`;
generated repository definition alone computes validation in production at
`slug_core_v2/src/runtime/generated_repository_definition.rs:168`. The
validation crate-root export is a later publication boundary. Root repository
mapping independently computes selected extension mappings at
`selected_repo_spec.rs:4467`; canonical selected-module definition is the
parallel publication branch. These are not visibility or evidence
prerequisites for pure.

## Required design decision

Specify exactly one one-way `bzl_module` -> `module_extension` surface around
the existing observation key and carrier. Decide the narrow effective
visibility of the key, its constructor, the carrier and its borrowed
`result()`/`observations()` accessors. Inspect Rust `Key::Value` privacy and
choose exactly one opaque nominal outer technique: promote a safe existing
name only if that does not reveal Raw/Definitions/Merge internals, otherwise
wrap the existing private outer only at the observed key projection. The
future pure owner may carry that outer but must not inspect its private stages.

Preserve the exact key identity and Display, Complete-only equality/validity,
Need and outer carrierlessness, semantic Result Arc, transaction-local epoch,
child-owned events, warm silence, cancellation behavior and compact retention.
No new semantic fact or DICE dependency is introduced. Add no adapter key,
second carrier, result alias exposure, public field, outer inspector, crate-root
reexport, external API, reverse module edge, semantic caller, event batch,
cache, interner, task or lock.

Freeze a compile-discriminating sibling-module proof. It may type-check key
construction, Display, the associated outcome, borrowed carrier accessors and
opaque error handling from `module_extension` tests, but must not compute the
key, construct carrier/error internals, inspect stages or activate pure. State
why same-module proof alone does not discriminate the visibility seam.

## Future implementation bounds

Future Rust authority may include only
`app/slug_loading_v2/src/bzl_module.rs` and test-only additions in
`app/slug_loading_v2/src/module_extension.rs`. Starting physical baselines are
9,108 and 1,592. Hard ceilings are <=80 production, <=80 proof, <=160 aggregate
semantic and <=9,190/1,675 physical. Add at most two direct helpers and keep
every changed helper/test below 100 lines. A one-file production change plus
one sibling compile smoke is preferred; the design must justify anything
within these ceilings and REPLAN before adding a third file.

Reuse accepted prepared identity/finisher/order/event/lifecycle/cancellation/
nonactivation proof and Bazel 9.2 loading evidence; add no oracle. A future
implementation must run its sibling visibility smoke, protected
`observed_prepared_` tests, full `cargo test -p slug_loading_v2`, direct
`cargo check -p slug_core_v2`, `cargo fmt --all -- --check` and
`git diff --check` serially. The design itself runs only source/coherence and
diff checks.

## Compatibility, lifecycle and terminal

Existing prepared and pure values, errors, dependency order and child event
behavior remain exact Bazel 9 compatibility. The crate-internal key/carrier/
opaque-outer visibility and Result-Arc/epoch association are Slug-native. Pure,
instantiated, validated, generated/public/root-mapping/bootstrap activation and
exact Bazel configuration/output/ActionKey bytes are unsupported/deferred in
this packet.

Implementation ACCEPT after this design returns only to a docs-only pure-
invocation owner design. STOP Rust edits now; semantic/equality/event/retention
drift; public or crate-root export; private stage exposure; second key/carrier/
adapter/owner; pure or upper activation; fixture/oracle work; proof/cap waiver;
milestone closure; M8/M7B; or exact identity work. REPLAN if Rust effective
visibility requires an unbounded API, semantic owner or third file. M7 remains
partial and M7A -> M8 -> M7B remains.

## Immediate predecessor

Implementation `682c4a1e`, from design base `3738b2b4`, accepts the private
prepared Result-Arc plus compact observation epoch with raw-first children,
left-first merge, unchanged preparation semantics, child-only events and exact
upper nonactivation. This audit selects only the smaller sibling visibility
seam before pure ownership.
