# Current Slug V2 Packet

Packet: `WP-6-7A-host-canonical-repository-apparent-mapping-observation-carrier-visibility-implementation`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Design and Rust base: pending docs commit / `bf093a17`

## Goal and authority

Implement only the designed same-crate handoff between the accepted callerless
canonical apparent-mapping observation and its sole future sibling consumer.
Expose exactly one `pub(super)` key/carrier/field-private opaque-outer surface
and prove it with one test-only sibling compile smoke. Do not activate a caller
or change computation.

Authority is exactly:

- `app/slug_core_v2/src/runtime/generated_repository_definition.rs`, baseline
  3,828 physical/tests 1,138, SHA-256
  `7b542ed4e0a661aa81a11651aba9bcd7ef62fc2a89bc947775a651c8a3d2f9db`;
- test-only
  `app/slug_core_v2/src/runtime/root_apparent_repository_definition.rs`,
  baseline 999 physical/tests 372, SHA-256
  `d13997171de5467fca54599d6c3aaf4e62eabfc95d224f91b72507401e63ae3e`.

No third file, module declaration, crate-root export, Cargo/BUILD, fixture,
oracle, adapter or semantic caller is authorized.

## Frozen production surface

Give exactly these existing names `pub(super)` visibility:

- `HostCanonicalRepositoryApparentMappingObservationKey`;
- `ObservedHostCanonicalRepositoryApparentMapping`; and
- `HostCanonicalRepositoryApparentMappingObservationError`.

The key tuple field remains private. Promote only the exact constructor
`pub(super) fn new(workspace: NormalizedAbsolutePath, context_repo:
CanonicalRepoName, apparent_repo: ApparentRepoName) -> Self`. Preserve exact
Display, including
`observed-host-canonical-repository-apparent-mapping:"/workspace":@@:@first`
for `/workspace`, root context and `@first`.

The carrier fields and `CanonicalRepositoryApparentMappingResult` alias remain
private. Promote only these exact concrete borrowed accessors:

```rust
pub(super) fn result(
    &self,
) -> &Arc<
    Result<
        HostCanonicalRepositoryApparentMapping,
        HostCanonicalRepositoryApparentMappingError,
    >,
>
pub(super) fn observations(&self) -> &PathObservationEpoch
```

Effective Key visibility requires one wrapper projection. Rename the current
private enum to `CanonicalRepositoryApparentMappingObservationError`, retaining
exactly `RootMapping(HostRootRepositoryMappingObservationError)` and
`Definition(HostCanonicalRepositoryDefinitionObservationError)` plus its
existing derives. The child outcome, driver, finisher and terminal construction
continue to use this private inner.

Add exactly:

```rust
pub(super) struct HostCanonicalRepositoryApparentMappingObservationError(
    CanonicalRepositoryApparentMappingObservationError,
);
```

Its field remains private. Both inner and wrapper derive exactly `Debug`,
`Clone`, `PartialEq`, `Eq`, `Allocative` and `Dupe`.
Only the observed Key projection wraps `Complete(Err(inner))`; Need, success,
legacy, equality/validity, branch order, Result Arc identity, epochs, events,
retention and cancellation remain byte-for-behavior unchanged. Do not add an
outer constructor, conversion, inspector, public field, alias or variant.

## Frozen proof

Allow only wrapper-spelling adjustments in existing
`observed_canonical_repository_apparent_mapping_identity_branch_and_terminal_algebra`:
finisher/child source assertions name the private inner; Key-Value construction
and matching use the field-private wrapper around the private `Definition`
inner. Preserve every existing terminal, dependency and epoch assertion and
source evidence for exactly one RootMapping inner, one Definition inner and one
Key projection. No lifecycle proof change or new generated-module test is
authorized.

Add exactly one test to the existing test-only sibling module in
`root_apparent_repository_definition.rs`:
`canonical_repository_apparent_mapping_observation_surface_is_sibling_usable`.
Use explicit test-only sibling imports for the three promoted names; do not
change production imports. The smoke:

- constructs only the key for `/workspace`, root context and `@first` and
  asserts the exact Display above;
- defines one nonexecuted `inspect` taking
  `&<HostCanonicalRepositoryApparentMappingObservationKey as Key>::Value`,
  `&ObservedHostCanonicalRepositoryApparentMapping` and
  `&HostCanonicalRepositoryApparentMappingObservationError`; inside it assigns
  `observed.result()` to
  `&Arc<Result<HostCanonicalRepositoryApparentMapping,
  HostCanonicalRepositoryApparentMappingError>>` and
  `observed.observations()` to `&PathObservationEpoch`;
- casts `inspect` to the exact function pointer whose first parameter is
  `&SourcePreparationOutcome<Result<ObservedHostCanonicalRepositoryApparentMapping,
  HostCanonicalRepositoryApparentMappingObservationError>>`, followed by the
  same carrier and opaque-error references; and
- does not construct or inspect the carrier/outer, compute the key, name the
  private alias/inner/variants, invoke root definition or activate semantics.

## Caps and validation

Caps are <=80 generated production, <=50 generated colocated proof and <=80
sibling proof; <=210 aggregate semantic lines and physical <=3,959/1,079. Add
no production helper or generated-module test and exactly one sibling smoke.
The adjusted identity test remains below 200 lines and the smoke below 100.
Add no new `rustfmt::skip`; preserve existing skips and require rustfmt-stable
bytes. The generated owner remains cohesive around its driver/carrier/Key
projection; the sibling changes only its colocated compile proof. No hot-path
or retained-representation change applies.

Run serially:

1. `cargo test -p slug_core_v2 observed_canonical_repository_apparent_mapping_ --lib`;
2. `cargo test -p slug_core_v2 canonical_repository_apparent_mapping_observation_surface_is_sibling_usable --lib`;
3. protected root apparent-definition and observed canonical-definition tests;
4. full `cargo test -p slug_core_v2`;
5. `cargo check -p slug_commands_v2`;
6. `cargo fmt --all -- --check`; and
7. exact two-file allowlist, baseline-SHA/accounting/physical/test-size/
   visibility/source-shape checks plus `git diff --check`.

Reuse the accepted owner proof and same-crate opaque-wrapper precedents. Add no
Bazel oracle because this handoff has no Bazel-visible behavior.

## Compatibility and stops

Apparent-mapping branches/order/targets/errors/predecessor, equality/
invalidation, epochs and lower events remain **exact** Bazel 9 compatibility.
The crate-internal opaque carrier and Result-Arc transaction-local epoch
handoff are **Slug-native**. Canonical-definition carrier visibility, root
apparent-definition ownership, route/source/public/command/bootstrap
observation and exact Bazel configuration/output/ActionKey bytes remain
**unsupported/deferred**.

STOP on a third file/type/key/carrier/adapter; crate-public visibility or
crate-root reexport; public field/alias/private-inner exposure/variant
inspector; canonical-definition visibility or root-definition activation;
semantic/branch/event/equality/epoch/retention drift; proof change beyond the
wrapper spelling and exact sibling smoke; new broad formatter skip; Cargo/
BUILD, fixture/oracle; cap/test/format waiver; upper activation, milestone
closure, M8/M7B or exact identity work. REPLAN before widening or on baseline
hash drift.

## Terminal

ACCEPT returns only to a docs-only root apparent-definition observation
prerequisite audit. That audit may separately select canonical-definition
carrier visibility; this packet must not preempt it. M7 remains partial and
M7A -> M8 -> M7B remains.

## Immediate predecessor

Committed audit `bf093a17` selected this two-file `pub(super)` handoff as the
unique prerequisite smaller than root-definition ownership. Accepted owner
`2022a7a2` is +599/-99 at 3,828 physical lines and remains callerless.
