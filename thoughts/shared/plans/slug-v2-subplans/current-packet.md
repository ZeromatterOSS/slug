# Current Slug V2 Packet

Packet: `WP-6-7A-host-canonical-repository-definition-observation-carrier-visibility-implementation`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Design and Rust base: pending docs commit / `05ddd7fc`

## Goal and authority

Implement only the designed same-crate handoff between the accepted canonical-
definition observation and the future root apparent-definition observation
owner. Expose exactly one `pub(super)` key/carrier/field-private opaque outer,
adjust its existing same-file consumer at the wrapper boundary and prove the
surface with one test-only sibling smoke. Do not activate a caller or change
computation.

Authority is exactly:

- `app/slug_core_v2/src/runtime/generated_repository_definition.rs`, baseline
  3,843 physical/tests 1,152, SHA-256
  `ea48d5e52dbad37bfc79e745ae0d6e24cc3e2b133b45fb4e861b5373810722ba`;
- test-only
  `app/slug_core_v2/src/runtime/root_apparent_repository_definition.rs`,
  baseline 1,042 physical/tests 372, SHA-256
  `c06fa8c8a2ebed243e32168a411c4f36bc1ff0d48803e077c431ae4c37aef19e`.

No third file, module declaration, crate-root export, Cargo/BUILD, fixture,
oracle, adapter or root-definition caller is authorized.

## Frozen production surface

Give exactly these existing nominal names `pub(super)` visibility:

- `HostCanonicalRepositoryDefinitionObservationKey`;
- `ObservedHostCanonicalRepositoryDefinition`; and
- `HostCanonicalRepositoryDefinitionObservationError`.

The key tuple field remains private. Promote only the exact constructor:

```rust
pub(super) fn new(
    workspace: NormalizedAbsolutePath,
    canonical_repo: CanonicalRepoName,
) -> Self
```

Preserve exact Display. For `/workspace` and
`CanonicalRepoName::new("requested")`, it is
`observed-host-canonical-repository-definition:"/workspace":@@requested`.

The carrier fields and `CanonicalRepositoryDefinitionResult` alias remain
private. Promote only these exact concrete borrowed accessors:

```rust
pub(super) fn result(
    &self,
) -> &Arc<
    Result<HostCanonicalRepositoryDefinition, HostCanonicalRepositoryDefinitionError>,
>
pub(super) fn observations(&self) -> &PathObservationEpoch
```

Effective Key visibility requires one wrapper. Rename the current private enum
to `CanonicalRepositoryDefinitionObservationError`, retaining exactly:

- `Selected(HostCanonicalSelectedModuleDefinitionObservationError)`;
- `Generated { selected_missing: HostCanonicalSelectedModuleDefinitionError,
  error: HostGeneratedRepositoryDefinitionObservationError }`; and
- `Merge { selected_missing: HostCanonicalSelectedModuleDefinitionError,
  error: ObservedPathFrontierError }`.

Keep its exact `Debug`, `Clone`, `PartialEq`, `Eq`, `Allocative` derives and
manual `Dupe` implementation. The canonical driver outcome and every Selected/
Generated/Merge construction continue to use this private inner.

Add exactly:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(super) struct HostCanonicalRepositoryDefinitionObservationError(
    CanonicalRepositoryDefinitionObservationError,
);

impl Dupe for HostCanonicalRepositoryDefinitionObservationError {}
```

The field remains private. Only the observed canonical Key projection wraps
`Complete(Err(inner))` as
`HostCanonicalRepositoryDefinitionObservationError(inner)`. Need, success,
legacy, equality/validity, selection order, Result Arc identity, epochs,
events, retention and cancellation remain unchanged. Add no outer constructor,
conversion, inspector, public field, alias or variant.

## Frozen same-file consumer boundary

`canonical_definition_apparent_mapping_child` remains the sole production
consumer. In its Observed branch only, destructure
`HostCanonicalRepositoryDefinitionObservationError(error)` from the child
terminal and pass the private `error` into
`CanonicalRepositoryApparentMappingObservationError::Definition(error)`.
Change that Definition variant's payload from the old nominal enum to the
renamed private `CanonicalRepositoryDefinitionObservationError`.

This is the only authorized consumer adjustment. It must not add an adapter,
clone, new error translation, epoch merge, branch, caller or compute. The
apparent owner still preserves exactly one selected child epoch and its own
opaque wrapper remains unchanged.

## Frozen proof

Allow only wrapper-spelling/source-shape adjustments in two existing tests:

- `observed_canonical_repository_definition_identity_staging_and_terminal_algebra`
  must name the private inner for all three driver terminals and prove exactly
  one public wrapper at canonical Key projection;
- `observed_canonical_repository_apparent_mapping_identity_branch_and_terminal_algebra`
  must construct its synthetic Definition terminal with the private canonical
  inner and prove exactly one wrapper destructure at the child boundary.

Preserve every existing semantic, Need, terminal, dependency, equality,
validity, Arc and epoch assertion. Do not change real-order/event or lifecycle/
cancellation/nonactivation proof, and add no generated-module test.

Add exactly one test to the existing test-only sibling module in
`root_apparent_repository_definition.rs`:
`canonical_repository_definition_observation_surface_is_sibling_usable`.
Use explicit test-only sibling imports for the three promoted names; production
imports remain unchanged. The smoke:

- constructs only the key for `/workspace` and `@@requested`, then asserts the
  exact Display above;
- defines one nonexecuted `inspect` taking
  `&<HostCanonicalRepositoryDefinitionObservationKey as Key>::Value`,
  `&ObservedHostCanonicalRepositoryDefinition` and
  `&HostCanonicalRepositoryDefinitionObservationError`;
- assigns the carrier accessors to
  `&Arc<Result<HostCanonicalRepositoryDefinition,
  HostCanonicalRepositoryDefinitionError>>` and `&PathObservationEpoch`;
- casts `inspect` to the exact function pointer whose first parameter is
  `&SourcePreparationOutcome<Result<ObservedHostCanonicalRepositoryDefinition,
  HostCanonicalRepositoryDefinitionObservationError>>`, followed by the same
  carrier and opaque-error references; and
- does not construct or inspect the carrier/outer, compute, name the private
  alias/inner/variants, invoke root definition or activate semantics.

## Caps and validation

Caps are <=80 generated production, <=50 generated colocated proof and <=80
sibling proof; <=210 aggregate semantic lines and physical <=3,974/1,122. Add
no production helper or generated-module test and exactly one sibling smoke.
Both adjusted identity tests remain below 200 and the smoke below 100. Add no
new `rustfmt::skip`; preserve existing skips and require rustfmt-stable bytes.
The generated module remains cohesive around its canonical and apparent
drivers/carriers/projections; the sibling changes only its compile proof. No
hot-path or retained-representation change applies.

Run serially:

1. `cargo test -p slug_core_v2 observed_canonical_repository_definition_ --lib`;
2. `cargo test -p slug_core_v2 observed_canonical_repository_apparent_mapping_identity_branch_and_terminal_algebra --lib`;
3. `cargo test -p slug_core_v2 canonical_repository_definition_observation_surface_is_sibling_usable --lib`;
4. protected `request_shape_and_target_precedence_are_total`,
   `real_generated_selected_and_deferred_domains_are_structural`,
   `lifecycle_identity_and_mapping_precedence_are_structural` and existing
   apparent-mapping sibling-surface tests;
5. full `cargo test -p slug_core_v2`;
6. `cargo check -p slug_commands_v2`;
7. `cargo fmt --all -- --check`; and
8. exact two-file allowlist, baseline-SHA/accounting/physical/test-size/
   visibility/wrapper/source-shape checks plus `git diff --check`.

Reuse the accepted owner proof and same-crate opaque-wrapper precedents. Add no
Bazel oracle because this visibility-only handoff has no Bazel-visible change.

## Compatibility and stops

Canonical-definition selection/generation order, targets, failures, equality/
invalidation, epochs and lower events remain **exact** Bazel 9 compatibility.
The crate-internal opaque carrier and Result-Arc transaction-local epoch
handoff are **Slug-native**. Root apparent-definition ownership and its later
carrier visibility, route/source/public/command/bootstrap observation and exact
Bazel configuration/output/ActionKey bytes remain **unsupported/deferred**.

STOP on a third file/type/key/carrier/adapter; crate-public visibility or
crate-root reexport; public field/alias/private-inner exposure/variant
inspector; root-definition activation; apparent-mapping redesign beyond the
exact unwrap boundary; semantic/order/event/equality/epoch/retention drift;
proof beyond two wrapper-only identity adjustments and the exact sibling smoke;
new formatter skip; Cargo/BUILD, fixture/oracle; cap/test/format waiver; upper
route/source/public/bootstrap work, milestone closure, M8/M7B or exact identity
work. REPLAN before widening or on baseline hash drift.

## Terminal

ACCEPT returns only to a docs-only root apparent-definition observation-owner
design. M7 remains partial and M7A -> M8 -> M7B remains.

## Immediate predecessor

Committed audit `05ddd7fc` proved this two-file `pub(super)` opaque-wrapper
handoff is the unique prerequisite smaller than root-definition ownership.
