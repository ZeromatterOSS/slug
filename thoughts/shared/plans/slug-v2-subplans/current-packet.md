# Current Slug V2 Packet

Packet: `WP-6-7A-host-canonical-repository-apparent-mapping-observation-carrier-visibility-design`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Audit and Rust base: pending docs commit / `2022a7a2`

## Goal and decision authority

Design only the uniquely smaller same-crate visibility prerequisite between
the accepted private canonical apparent-mapping observation and its sole future
consumer in the sibling root apparent-definition module. Freeze one
`pub(super)` key/carrier/field-private opaque-outer surface and a sibling
compile smoke without changing computation or activating a caller.

Write only the canonical plan, this manifest, Stage 6 and routing log at net
caps <=40/<=180/<=220/<=30 and <=470 aggregate. Rust, tests, fixtures,
oracles, Cargo/BUILD, APIs, exports and callers are read-only in this packet.

## Audited frontier and decision

Accepted `2022a7a2` adds the private callerless
`HostCanonicalRepositoryApparentMappingObservationKey`,
`ObservedHostCanonicalRepositoryApparentMapping` and typed observation outer
at `generated_repository_definition.rs:836-1136`. The observed key has zero
production consumers. Its private Result accessor names private
`CanonicalRepositoryApparentMappingResult`; its current enum variants expose
the private canonical-definition outer as well as the opaque Bzlmod root-
mapping outer. A sibling cannot name the associated Value.

The legacy apparent-mapping key has exactly one production consumer,
`HostRootApparentRepositoryDefinitionKey` at
`root_apparent_repository_definition.rs:266`. That key always requests root-
context mapping first, defers main/builtin targets, then requests canonical
definition for the resolved target at line 310. The mapping observation handoff
is therefore necessary but must not activate this two-child upper owner.

Root apparent definition has exactly one production consumer, root apparent
route at its line 303. Route has exactly one production consumer, root apparent
source input at its line 186; source observation/path, repository route/source/
file, public command and bootstrap layers are later. None directly consumes
the apparent-mapping observation or replaces its epoch.

The canonical-definition observed surface is also private in the generated-
definition module and will require a separate sibling-visibility decision
before root-definition ownership. It is not a prerequisite to expose this
mapping carrier, and bundling two carrier families is wider than the current
edge. No `lib.rs` export, crate-public API, module move or adapter is needed.
Thus same-crate apparent-mapping visibility is uniquely smaller than root-
definition ownership for this frontier.

## Design deliverable

Freeze exactly these three existing names at `pub(super)` visibility:

- `HostCanonicalRepositoryApparentMappingObservationKey`, with only its
  existing three-argument `new` promoted to `pub(super)` and unchanged Display;
- `ObservedHostCanonicalRepositoryApparentMapping`, with private fields and
  concrete `pub(super)` borrowed Result-Arc and epoch accessors; and
- field-private opaque
  `HostCanonicalRepositoryApparentMappingObservationError`.

Keep `CanonicalRepositoryApparentMappingResult` private. Its concrete accessor
is exactly
`&Arc<Result<HostCanonicalRepositoryApparentMapping,
HostCanonicalRepositoryApparentMappingError>>`; observations remains
`&PathObservationEpoch`.

Effective Key visibility requires projection. Rename the current private enum
to `CanonicalRepositoryApparentMappingObservationError`, preserving exactly
its `RootMapping(HostRootRepositoryMappingObservationError)` and
`Definition(HostCanonicalRepositoryDefinitionObservationError)` variants and
derives. Keep the child outcome, driver and finisher on this private inner
enum. Add
`pub(super) struct HostCanonicalRepositoryApparentMappingObservationError(
CanonicalRepositoryApparentMappingObservationError)` with a private field and
matching derives. Wrap only observed `Complete(Err(inner))` at Key projection;
Need, success, legacy, equality/validity, branch order, epochs, events and
retention are unchanged.

Add no public field/alias/variant/inspector, outer constructor/conversion,
crate-root reexport, module declaration, adapter, semantic caller or second
carrier family.

Freeze exactly one test-only sibling smoke in
`root_apparent_repository_definition.rs`:
`canonical_repository_apparent_mapping_observation_surface_is_sibling_usable`.
It constructs only the observed key for `/workspace`, root context and
`@first`, and asserts exact Display
`observed-host-canonical-repository-apparent-mapping:"/workspace":@@:@first`.
Nonexecuted function pointers prove the exact associated
`SourcePreparationOutcome<Result<ObservedHostCanonicalRepositoryApparentMapping,
HostCanonicalRepositoryApparentMappingObservationError>>` and concrete borrowed
accessors. It cannot construct or inspect carrier/outer, compute the key, name
the private alias/inner/variants, invoke root definition or activate semantics.

Allow only the private-inner/public-wrapper spelling adjustment in existing
`observed_canonical_repository_apparent_mapping_identity_branch_and_terminal_algebra`.
All terminal/dependency/epoch assertions remain. Source evidence continues to
prove exactly one RootMapping and one Definition inner mapping, one wrapper at
Key projection, and no upper consumer in the producer span.

## Authority, caps and validation

Prospective implementation authority is exactly:

- `app/slug_core_v2/src/runtime/generated_repository_definition.rs`, baseline
  3,828 physical/tests 1,138, SHA-256
  `7b542ed4e0a661aa81a11651aba9bcd7ef62fc2a89bc947775a651c8a3d2f9db`;
- test-only
  `app/slug_core_v2/src/runtime/root_apparent_repository_definition.rs`,
  baseline 999 physical/tests 372, SHA-256
  `d13997171de5467fca54599d6c3aaf4e62eabfc95d224f91b72507401e63ae3e`.

Freeze <=80 generated production, <=50 generated proof and <=80 sibling proof;
<=210 aggregate semantic lines and physical <=3,959/1,079. Add no production
helper or new generated-module test; add exactly one sibling smoke. The
adjusted identity test remains below 200 and the new smoke below 100. The large
generated owner remains cohesive around the private driver/carrier/projection;
the sibling file changes only its colocated compile proof. No hot-path or
retained-representation change applies.

Prospective validation is serial:

- `cargo test -p slug_core_v2 observed_canonical_repository_apparent_mapping_ --lib`;
- `cargo test -p slug_core_v2 canonical_repository_apparent_mapping_observation_surface_is_sibling_usable --lib`;
- protected root apparent-definition and observed canonical-definition tests;
- full `cargo test -p slug_core_v2`;
- direct dependent `cargo check -p slug_commands_v2`;
- `cargo fmt --all -- --check`; and
- exact two-file allowlist/SHA/accounting/physical/test-size/visibility checks
  plus `git diff --check`.

Reuse the accepted owner proof and prior same-crate opaque-wrapper precedents.
Add no oracle: this visibility-only change has no Bazel-visible behavior.

Apparent-mapping branch/order/targets/errors/predecessor retention, equality/
invalidation, epochs and lower events remain **exact** Bazel 9 compatibility.
The crate-internal carrier/opaque outer and Result-Arc transaction-local epoch
handoff are **Slug-native**. Canonical-definition carrier visibility, root
apparent-definition ownership, route/source/public/command/bootstrap
observation and exact Bazel configuration/output/ActionKey bytes remain
**unsupported/deferred**.

## Terminal

ACCEPT schedules exactly
`WP-6-7A-host-canonical-repository-apparent-mapping-observation-carrier-visibility-implementation`,
then returns to a docs-only root apparent-definition observation prerequisite
audit. STOP Rust/test/API edits in this packet; implementation/caller/root-
definition activation; public field/alias/inner/variant/inspector; crate-public
or crate-root export; canonical-definition visibility; third file/type/key/
carrier/adapter; semantic/branch/event/equality/epoch/retention drift; Cargo/
BUILD, fixture/oracle; cap/proof/test-size/format waiver; upper activation,
milestone closure, M8/M7B or exact identity work. REPLAN before widening. M7
remains partial and M7A -> M8 -> M7B remains.

## Immediate predecessor

Accepted owner `2022a7a2` is +599/-99 at 3,828 physical lines and preserves the
exact root/nonroot child epochs without merge. Its packet terminal requires
this sibling-visibility audit before any root-definition work.
