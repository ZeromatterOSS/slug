# Current Slug V2 Packet

Packet: `WP-6-7A-host-root-repository-mapping-observation-carrier-promotion-implementation`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Design and Rust base: pending docs commit / `7ee0522b`

## Goal and exact authority

Promote only the accepted private Bzlmod root-repository-mapping observation
carrier across the existing Bzlmod -> core dependency. Expose one doc-hidden
key, carrier and field-private opaque outer plus exactly three crate-root
reexports and one external API smoke. Add no semantic caller or activation.

Write exactly:

- `app/slug_bzlmod_v2/src/selected_repo_spec.rs`, baseline 13,362 physical
  lines, `#[cfg(test)]` at 4,862, SHA-256
  `f98ef97df33eadca597cf8e10714c00654864e316979bfce0eb1813005f99c67`;
- `app/slug_bzlmod_v2/src/lib.rs`, baseline 421, SHA-256
  `3fdd3d81d94ce7d3618f356114505d7c30515596a3adbe0f14fb7add30c5cea0`;
  and
- new
  `app/slug_bzlmod_v2/tests/root_repository_mapping_observation_api.rs`.

Every core/loading source, other Rust/test, fixture, oracle, Cargo/BUILD target,
command, public output and plan is read-only.

## Exact nominal surface and projection

Make exactly these three existing names doc-hidden public nominal types:

- `HostRootRepositoryMappingObservationKey(HostRootRepositoryMappingKey)`;
- `ObservedHostRootRepositoryMapping`; and
- `HostRootRepositoryMappingObservationError`.

The key retains Debug/Clone/PartialEq/Eq/Hash/Allocative and its private field.
Make only its existing `new(workspace: NormalizedAbsolutePath) -> Self` public.
Display remains `observed-{legacy}` and for `/workspace` is exactly
`observed-host-root-repository-mapping:"/workspace"`.

The carrier retains Debug/Clone/PartialEq/Eq/Allocative/`Dupe`, private
`result: RootRepositoryMappingResult` and private
`observations: PathObservationEpoch`. Keep `RootRepositoryMappingResult`
private. Make only these concrete borrowed accessors public:

- `result(&self) -> &Arc<Result<HostRootRepositoryMapping,
  HostRootRepositoryMappingError>>`; and
- `observations(&self) -> &PathObservationEpoch`.

Effective `Key::Value` visibility requires one projection. Rename the current
private enum to `RootRepositoryMappingObservationError`, retaining exactly
`Mappings(ExtensionMappingsObservationError)` and its current derives. Keep
`RootRepositoryMappingDriverOutcome`, child adapter, finisher and shared driver
on this private inner enum. Define doc-hidden public
`HostRootRepositoryMappingObservationError(
RootRepositoryMappingObservationError)` with a private field and matching
Debug/Clone/PartialEq/Eq/Allocative/`Dupe` derives.

The observed Key Value stays
`SourcePreparationOutcome<Result<ObservedHostRootRepositoryMapping,
HostRootRepositoryMappingObservationError>>`. Preserve its current Need and
success arms and wrap only `Complete(Err(inner))` at this Key projection.
Legacy projection, key equality/validity, Result/epoch identity, terminal
polarity, events, retention, cancellation and computation are unchanged.

Add no public Result/outcome alias, field, inner enum/variant, inspector,
outer constructor or conversion; no adapter, fourth nominal type, second key/
carrier, semantic caller or core compute.

## Exact exports and proof

In `lib.rs`, immediately after the existing complete legacy root-mapping
reexport block and before selected-extension reexports, add exactly these three
adjacent hidden pairs in this order, with no other root-mapping observation
reexport:

1. `#[doc(hidden)] pub use selected_repo_spec::HostRootRepositoryMappingObservationError;`
2. `#[doc(hidden)] pub use selected_repo_spec::HostRootRepositoryMappingObservationKey;`
3. `#[doc(hidden)] pub use selected_repo_spec::ObservedHostRootRepositoryMapping;`

Add exactly one external test
`root_repository_mapping_observation_surface_is_cross_crate_usable`. It
constructs only `HostRootRepositoryMappingObservationKey::new` for
`/workspace` and asserts the exact Display above. A nonexecuted value function
pointer proves
`<HostRootRepositoryMappingObservationKey as dice::Key>::Value` equals
`SourcePreparationOutcome<Result<ObservedHostRootRepositoryMapping,
HostRootRepositoryMappingObservationError>>`. A nonexecuted carrier function
pointer proves the exact concrete borrowed Result-Arc and epoch accessor types.
The test cannot construct or inspect carrier/outer, compute the key, name the
private alias/inner/variant, import core or activate semantics.

In existing
`observed_root_repository_mapping_identity_scan_and_terminal_algebra`, change
only the typed-inner and public-wrapper spelling required by projection; keep
all assertions and cases unchanged. In existing
`observed_root_repository_mapping_lifecycle_cancellation_and_nonactivation`,
replace only the now-obsolete `lib.rs` absence clause with proof that each exact
hidden reexport pair occurs once and that the extracted root-mapping
observation reexport list is exactly Error, Key, Observed in the order above.
Continue proving all three names absent from loading `bzl_module.rs`, core
`generated_repository_definition.rs` and core
`root_apparent_repository_definition.rs`. Do not weaken tracker, lifecycle,
cancellation, recovery or upper-nonactivation assertions.

## Caps, validation and compatibility

Caps are <=80 production, <=40 colocated proof, <=10 lib, <=70 external proof
and <=200 aggregate semantic lines; physical maxima are <=13,483/431/70.
Both adjusted existing tests remain below 200 lines and every new smoke/helper
below 100. Add no new colocated test or helper. The large selected owner remains
cohesive because it owns the private inner driver, carrier and Key projection;
a visibility-only split would expose or duplicate private state. This is not a
demonstrated hot path and changes no retained representation.

Run serially:

- `cargo test -p slug_bzlmod_v2 observed_root_repository_mapping_ --lib`;
- `cargo test -p slug_bzlmod_v2 root_repository_mapping_observation_surface_is_cross_crate_usable --test root_repository_mapping_observation_api`;
- `cargo test -p slug_bzlmod_v2 --test canonical_selected_definition_observation_api --test definition_request_observation_api --test evaluation_input_request_observation_api`;
- full `cargo test -p slug_bzlmod_v2`;
- direct dependent `cargo check -p slug_core_v2`;
- `cargo fmt --all -- --check`; and
- exact three-file allowlist, SHA baseline/accounting/physical/test-size,
  exactly-three hidden reexport and `git diff --check` checks.

Reuse the accepted root-mapping owner proof and prior opaque promotion
precedents. Add no Bazel oracle: visibility is behavior-neutral. Existing Bazel
9.2 `BazelDepGraphFunction.computeCanonicalRepoNameLookup`,
`BazelDepGraphValue.getRepositoryMapping` and `BazelDepGraphFunctionTest`
remain the semantic evidence; Buck2 DICE lifecycle proof remains concept/test
evidence.

Root-mapping values/errors/full-scan/order/views, equality/invalidation and
lower events remain **exact** Bazel 9 compatibility. The hidden cross-crate
carrier/opaque outer and Result-Arc transaction-local epoch handoff are
**Slug-native**. Canonical apparent-mapping observation/caller, root definition/
route/source/public/command/bootstrap activation and exact Bazel configuration/
output/ActionKey bytes remain **unsupported/deferred**.

## Terminal

ACCEPT returns only to a docs-only canonical apparent-mapping observation-owner
design. STOP a fourth file/type/reexport, nonadjacent or non-hidden export,
public field/alias/inner/variant/inspector, adapter/caller/core edit, semantic/
event/equality/retention drift, altered test assertion beyond wrapper/reexport
projection, Cargo/BUILD, fixture/oracle, cap/test-size/format waiver, upper
activation, milestone closure, M8/M7B or exact identity work. REPLAN before
widening or if any baseline hash differs. M7 remains partial and
M7A -> M8 -> M7B remains.

## Immediate predecessor

Committed audit `681732db` proves this three-type promotion is the uniquely
smaller prerequisite after accepted private owner `7ee0522b`; core's apparent-
mapping root branch remains the sole future semantic consumer.
