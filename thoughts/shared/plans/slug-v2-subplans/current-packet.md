# Current Slug V2 Packet

Packet: `WP-6-7A-host-root-apparent-repository-definition-observation-carrier-visibility-implementation`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Design and Rust base: pending docs commit / `04443253`

## Goal and authority

Implement only the designed same-crate handoff between the accepted callerless
root apparent-definition observation and its sole future sibling route
consumer. Expose exactly one `pub(super)` key/carrier/field-private opaque
outer and prove it with one test-only sibling compile smoke. Do not activate a
caller or change computation.

Authority is exactly:

- `app/slug_core_v2/src/runtime/root_apparent_repository_definition.rs`,
  baseline 1,714 physical/tests 529, SHA-256
  `9aba8dba56972fce08d23d9fb97a604a849e5aac4694b34c29b472e4e837dca5`;
- test-only `app/slug_core_v2/src/runtime/root_apparent_repository_route.rs`,
  baseline 1,088 physical/tests 374, SHA-256
  `131fb0fca448acb3786946500d91f66ece2b6ee54441cc65968a9ce4605131ee`.

No third file, module declaration, crate-root export, Cargo/BUILD, fixture,
oracle, adapter or semantic caller is authorized.

## Frozen production surface

Give exactly these existing nominal names `pub(super)` visibility:

- `HostRootApparentRepositoryDefinitionObservationKey`;
- `ObservedHostRootApparentRepositoryDefinition`; and
- `HostRootApparentRepositoryDefinitionObservationError`.

The key tuple field remains private. Promote only the exact constructor:

```rust
pub(super) fn new(
    workspace: NormalizedAbsolutePath,
    apparent_repo: ApparentRepoName,
) -> Option<Self>
```

Preserve root-name rejection and exact Display. For `/workspace` and `@first`
it is
`observed-host-root-apparent-repository-definition:"/workspace":@first`.

The carrier fields and `RootApparentRepositoryDefinitionResult` alias remain
private. Promote only these exact concrete borrowed accessors:

```rust
pub(super) fn result(
    &self,
) -> &Arc<
    Result<
        HostRootApparentRepositoryDefinition,
        HostRootApparentRepositoryDefinitionError,
    >,
>
pub(super) fn observations(&self) -> &PathObservationEpoch
```

Effective Key visibility requires one wrapper. Rename the current private enum
to `RootApparentRepositoryDefinitionObservationError`, retaining exactly:

- `Mapping(HostCanonicalRepositoryApparentMappingObservationError)`;
- `Definition { mapping: HostCanonicalRepositoryApparentMapping,
  error: HostCanonicalRepositoryDefinitionObservationError }`; and
- `Merge { mapping: HostCanonicalRepositoryApparentMapping,
  error: ObservedPathFrontierError }`.

Keep the exact `Debug`, `Clone`, `PartialEq`, `Eq`, `Allocative` derives and
manual `Dupe` implementation. The driver outcome and every Mapping/Definition/
Merge construction continue to use this private inner.

Add exactly:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Allocative)]
pub(super) struct HostRootApparentRepositoryDefinitionObservationError(
    RootApparentRepositoryDefinitionObservationError,
);

impl Dupe for HostRootApparentRepositoryDefinitionObservationError {}
```

The field remains private. Only the observed Key projection wraps
`Complete(Err(inner))` as
`HostRootApparentRepositoryDefinitionObservationError(inner)`. Need, success,
legacy, equality/validity, child order, Result Arc identity, epochs, events,
retention and cancellation remain unchanged. Add no outer constructor,
conversion, inspector, public field, alias or variant.

## Frozen proof

Allow only wrapper-spelling/source-shape adjustments in existing
`observed_root_apparent_repository_definition_identity_staging_and_terminal_algebra`.
Driver terminal construction and source assertions name the private inner;
Key-Value construction uses the field-private wrapper around the private Merge
inner. Preserve every existing key/root/Display, Need, dependency, terminal,
merge, equality, validity, Arc and epoch assertion. Do not change real-order/
event or lifecycle/cancellation/nonactivation proof, and add no definition-
module test. Source evidence must prove exactly one Mapping, Definition and
Merge inner mapping plus exactly one wrapper at observed Key projection.

Add exactly one test to the existing test-only sibling module in
`root_apparent_repository_route.rs`:
`root_apparent_repository_definition_observation_surface_is_sibling_usable`.
Use explicit test-only sibling imports for the three promoted names; production
imports remain unchanged. The smoke:

- constructs only the key for `/workspace` and `@first` and asserts
  `observed-host-root-apparent-repository-definition:"/workspace":@first`;
- defines one nonexecuted `inspect` taking
  `&<HostRootApparentRepositoryDefinitionObservationKey as Key>::Value`,
  `&ObservedHostRootApparentRepositoryDefinition` and
  `&HostRootApparentRepositoryDefinitionObservationError`;
- assigns the carrier accessors to
  `&Arc<Result<HostRootApparentRepositoryDefinition,
  HostRootApparentRepositoryDefinitionError>>` and `&PathObservationEpoch`;
- casts `inspect` to the exact function pointer whose first parameter is
  `&SourcePreparationOutcome<Result<ObservedHostRootApparentRepositoryDefinition,
  HostRootApparentRepositoryDefinitionObservationError>>`, followed by the
  same carrier and opaque-error references; and
- does not construct or inspect carrier/outer, compute the key, name the
  private alias/inner/variants, invoke route or activate semantics.

## Caps and validation

Caps are <=80 definition production, <=50 definition colocated proof and <=80
route sibling proof; <=210 aggregate semantic additions and physical
<=1,845/1,168. Add no production helper or definition-module test and exactly
one sibling smoke. The adjusted identity test remains below 200 and the smoke
below 100. Add no new `rustfmt::skip`; preserve existing skips and require
rustfmt-stable bytes. The definition owner remains cohesive around its driver/
carrier/Key projection; the route changes only its colocated compile proof. No
hot-path or retained-representation change applies.

Run serially:

1. `cargo test -p slug_core_v2 observed_root_apparent_repository_definition_ --lib`;
2. `cargo test -p slug_core_v2 root_apparent_repository_definition_observation_surface_is_sibling_usable --lib`;
3. protected legacy root-definition and route tests;
4. full `cargo test -p slug_core_v2`;
5. `cargo check -p slug_commands_v2`;
6. `cargo fmt --all -- --check`; and
7. exact two-file allowlist, baseline-SHA/accounting/physical/test-size/
   visibility/wrapper/source-shape checks plus `git diff --check`.

Reuse accepted owner proof and same-crate opaque-wrapper precedents. Add no
Bazel oracle because this handoff has no Bazel-visible behavior.

## Compatibility and stops

Root-definition values/order/errors/views/equality/invalidation and lower
events remain **exact** Bazel 9 compatibility. The crate-internal opaque
carrier and Result-Arc transaction-local epoch handoff are **Slug-native**.
Route ownership, source/public/command/bootstrap observation and exact Bazel
configuration/output/ActionKey bytes remain **unsupported/deferred**.

STOP on a third file/type/key/carrier/adapter; crate-public visibility or
crate-root reexport; public field/alias/private-inner exposure/variant
inspector; route activation; lower-carrier redesign; semantic/order/event/
equality/epoch/retention drift; proof beyond wrapper spelling and exact sibling
smoke; new formatter skip; Cargo/BUILD, fixture/oracle; cap/test/format waiver;
upper source/public/command/bootstrap work, milestone closure, M8/M7B or exact
identity work. REPLAN before widening or on baseline hash drift.

## Terminal

ACCEPT returns only to a docs-only root apparent-route observation-owner
design. M7 remains partial and M7A -> M8 -> M7B remains.

## Immediate predecessor

Committed audit `04443253` proved this two-file `pub(super)` opaque-wrapper
handoff is the unique prerequisite smaller than route ownership. Accepted
owner `29795aeb` is +738/-103 at 1,714 physical lines and remains callerless.
