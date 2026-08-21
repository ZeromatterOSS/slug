# Current Slug V2 Packet

Packet: `WP-6-7A-host-root-repository-mapping-observation-carrier-promotion-design`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Audit and Rust base: pending docs commit / `7ee0522b`

## Goal and decision authority

Design only the uniquely smaller cross-crate visibility prerequisite between
the accepted private Bzlmod root-repository-mapping observation and core's
future canonical apparent-mapping observation owner. Freeze one doc-hidden
key/carrier/opaque-outer handoff without changing computation or activating a
caller.

Write only the canonical plan, this manifest, Stage 6 and routing log at net
caps <=40/<=180/<=220/<=30 and <=470 aggregate. Rust, tests, fixtures,
oracles, Cargo/BUILD, APIs, exports and callers are read-only in this packet.

## Audited frontier

At accepted base `7ee0522b`, `selected_repo_spec.rs:4636-4860` owns private
`HostRootRepositoryMappingObservationKey`, private
`ObservedHostRootRepositoryMapping` and private
`HostRootRepositoryMappingObservationError::Mappings`. The carrier's private
`result()` names private `RootRepositoryMappingResult`; the outer exposes
private `ExtensionMappingsObservationError`. There is no crate-root reexport
and no production consumer of the observed key.

The legacy `HostRootRepositoryMappingKey` has exactly one production consumer:
the root-context branch of core
`HostCanonicalRepositoryApparentMappingKey` at
`generated_repository_definition.rs:879`. That core key's nonroot branch at
905 already shares a module with the private canonical-definition observation,
so canonical-carrier promotion is not a prerequisite. The apparent-mapping key
has exactly one production consumer, `HostRootApparentRepositoryDefinitionKey`
at `root_apparent_repository_definition.rs:266`; that consumer later computes
canonical definition at 310. Route/source/public/command/bootstrap branches
are still upper and contain no direct observed-root-mapping consumer.

Core already depends one way on Bzlmod and imports the legacy root-mapping
surface. Moving the owner, adding a Bzlmod-to-core edge, splitting the root and
nonroot apparent-mapping family or using an empty root epoch is forbidden.
Therefore the minimal next step is the Bzlmod carrier promotion, not apparent-
mapping ownership or canonical/root-definition visibility.

## Design deliverable

Freeze exactly three doc-hidden public nominal types and exactly three adjacent
crate-root reexports:

- existing `HostRootRepositoryMappingObservationKey`, with public one-argument
  `new(NormalizedAbsolutePath)` and unchanged Display;
- existing `ObservedHostRootRepositoryMapping`, with public borrowed concrete
  `&Arc<Result<HostRootRepositoryMapping, HostRootRepositoryMappingError>>`
  `result()` and `&PathObservationEpoch` `observations()` accessors; and
- one field-private opaque public
  `HostRootRepositoryMappingObservationError`.

Keep every field and `RootRepositoryMappingResult` private. Effective Key
visibility requires projection: rename the current private enum to
`RootRepositoryMappingObservationError`, preserving exactly
`Mappings(ExtensionMappingsObservationError)` and its derives, keep the shared
driver on that inner enum, and define the public outer as
`HostRootRepositoryMappingObservationError(inner)` with a private field and
matching derives. Wrap only observed `Complete(Err(inner))` at Key projection.
Need, success, legacy projection, equality/validity, events, retention and all
terminal semantics remain byte-for-byte equivalent.

Add no public Result/outcome alias, field, variant, inspector, outer
constructor, conversion, adapter, second key/carrier, semantic caller or core
compute. The later core owner may carry but cannot inspect the outer.

Freeze one external API smoke
`root_repository_mapping_observation_surface_is_cross_crate_usable` in new
`app/slug_bzlmod_v2/tests/root_repository_mapping_observation_api.rs`. It may
construct only the observed key for `/workspace`, assert exact Display
`observed-host-root-repository-mapping:"/workspace"`, and use nonexecuted
function pointers to prove the associated
`SourcePreparationOutcome<Result<ObservedHostRootRepositoryMapping,
HostRootRepositoryMappingObservationError>>` plus the two concrete borrowed
accessor signatures. It must not construct or inspect carrier/outer, compute a
key, import core or activate semantics.

## Authority, caps and validation

Prospective implementation authority is exactly:

- `app/slug_bzlmod_v2/src/selected_repo_spec.rs`, baseline 13,362 physical
  lines with tests at 4,862;
- `app/slug_bzlmod_v2/src/lib.rs`, baseline 421; and
- new `app/slug_bzlmod_v2/tests/root_repository_mapping_observation_api.rs`.

Freeze <=80 production, <=40 colocated proof, <=10 lib and <=70 external
proof; <=200 aggregate semantic lines and physical <=13,483/431/70. Allow only
a semantic-neutral opaque-wrapper spelling adjustment to existing
`observed_root_repository_mapping_identity_scan_and_terminal_algebra`, which
must remain below 200 lines; every new smoke/helper remains below 100. The
large source remains cohesive because it owns the inner driver, carrier and
projection. No hot-path measurement applies.

Prospective validation is serial:

- `cargo test -p slug_bzlmod_v2 observed_root_repository_mapping_ --lib`;
- `cargo test -p slug_bzlmod_v2 root_repository_mapping_observation_surface_is_cross_crate_usable --test root_repository_mapping_observation_api`;
- protected root-mapping/observed-extension-mappings and existing Bzlmod
  observation API smokes;
- full `cargo test -p slug_bzlmod_v2`;
- direct dependent `cargo check -p slug_core_v2`;
- `cargo fmt --all -- --check`; and
- exact allowlist/accounting/physical/reexport/test-size checks plus
  `git diff --check`.

Reuse the accepted owner proof and prior three-type promotion precedents. Add
no oracle: this visibility-only change has no Bazel-visible behavior.

Root-mapping values/errors/full-scan/order/views, equality/invalidation and
lower events remain **exact** Bazel 9 compatibility. The hidden cross-crate
carrier/opaque outer and Result-Arc transaction-local epoch handoff are
**Slug-native**. Apparent-mapping ownership/caller, canonical/root definition,
route/source/public/command/bootstrap observation and exact Bazel configuration/
output/ActionKey bytes remain **unsupported/deferred**.

## Terminal

ACCEPT schedules exactly
`WP-6-7A-host-root-repository-mapping-observation-carrier-promotion-implementation`,
then returns to a docs-only canonical apparent-mapping observation-owner
design. STOP Rust/test/API edits in this packet; implementation or caller/core
activation; public field/alias/terminal/inspector; fourth type/reexport;
nonadjacent or non-hidden reexport; second key/carrier/adapter; reverse
dependency; root-mapping semantic/order/event/equality/retention drift;
Cargo/BUILD, fixture/oracle; third production file; cap/proof/test-size waiver;
milestone closure, M8/M7B or exact identity work. REPLAN before widening. M7
remains partial and M7A -> M8 -> M7B remains.

## Immediate predecessor

Accepted implementation `7ee0522b` adds the private callerless root-mapping
Result-Arc+epoch owner at +968/-170 and 13,362 physical lines. Its packet
terminal requires this visibility audit before canonical apparent-mapping work.
