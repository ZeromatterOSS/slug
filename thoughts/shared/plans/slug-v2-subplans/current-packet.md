# Current Slug V2 Packet

Packet: `WP-6-7A-host-root-apparent-repository-definition-observation-carrier-visibility-design`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Audit and Rust base: pending docs commit / `29795aeb`

## Goal and decision authority

Design only the uniquely smaller same-crate visibility prerequisite between
the accepted private root apparent-definition observation and its sole future
consumer in the sibling root apparent-route module. Freeze one minimal
`pub(super)` key/carrier/field-private opaque-outer surface plus sibling compile
proof without changing computation or activating a caller.

Write only the canonical plan, this manifest, Stage 6 and routing log at net
caps <=40/<=180/<=220/<=30 and <=470 aggregate. Rust, tests, fixtures,
oracles, Cargo/BUILD, exports and callers are read-only in this packet.

## Audited frontier and decision

Accepted `29795aeb` adds private
`HostRootApparentRepositoryDefinitionObservationKey`,
`ObservedHostRootApparentRepositoryDefinition` and
`HostRootApparentRepositoryDefinitionObservationError` at
`root_apparent_repository_definition.rs:259-528`. The observation surface has
zero production consumers. Its key/new, carrier, private-alias Result accessor
and typed outer are private to that module, so the sibling route cannot name
the Key associated Value.

The outer exposes Mapping, Definition and Merge variants containing the two
lower opaque sibling-visible outers, the successful mapping value and frontier
error. Promoting the enum directly would reveal terminal structure that the
route does not need. Effective Key visibility therefore requires a field-
private nominal wrapper around a renamed private inner at Key projection.

The legacy `HostRootApparentRepositoryDefinitionKey` has exactly one
production consumer: `HostRootApparentRepositoryRouteKey` imports it at
`root_apparent_repository_route.rs:28` and computes it at line 303. Route owns
only validation and source-capability projection from this one predecessor, so
the accepted observation is the exact future child and no second semantic
prerequisite exists. Reusing the legacy child would discard its epoch.

The legacy route has exactly one production consumer, root apparent source
input at `root_apparent_repository_source_input.rs:186`; source input has one
production consumer, source-path input at its line 234; source-path input has
one production consumer, source observation at its line 234; that source
observation remains callerless. The public command still uses Bzlmod
`RootRepositoryRouteKey`/its observation in `dice.rs:4476-4494`, not this core
Host route, and `root_bootstrap.rs` remains explicitly dormant. None consumes
the root-definition observation directly, supplies sibling visibility or
replaces its epoch.

No crate-public API, `runtime/mod.rs` or crate-root reexport, module move,
adapter, lower-carrier promotion, route owner, source/public/command/bootstrap
activation is required. Thus same-crate root-definition carrier visibility is
uniquely smaller than root-route observation ownership.

## Design deliverable

Freeze exactly one minimal same-crate surface:

- the existing root apparent-definition observation key and only its existing
  two-argument Option constructor at `pub(super)`, preserving root rejection
  and exact `observed-` Display;
- the existing carrier with private fields and concrete `pub(super)` borrowed
  `Arc<Result<HostRootApparentRepositoryDefinition,
  HostRootApparentRepositoryDefinitionError>>` and `PathObservationEpoch`
  accessors; and
- private inner `enum RootApparentRepositoryDefinitionObservationError`,
  retaining
  Mapping/Definition/Merge variants and the existing derives/Dupe; plus
  field-private opaque
  `pub(super) struct HostRootApparentRepositoryDefinitionObservationError(
  RootApparentRepositoryDefinitionObservationError)` with matching existing
  derives/Dupe, wrapping only the observed Key error projection.

Keep `RootApparentRepositoryDefinitionResult`, inner enum, fields and variants
private. Add no public field/alias/variant/inspector, outer constructor/
conversion, crate-root export, adapter or semantic caller.

Freeze exactly one test-only sibling proof in
`root_apparent_repository_route.rs`. It may construct only the observed key for
`/workspace` and `@first`, assert exact Display
`observed-host-root-apparent-repository-definition:"/workspace":@first`, and
use nonexecuted function pointers to prove the associated
`SourcePreparationOutcome<Result<ObservedHostRootApparentRepositoryDefinition,
HostRootApparentRepositoryDefinitionObservationError>>` plus concrete borrowed
accessors. It must not construct or inspect carrier/outer, compute the key,
name the private alias/inner/variants, invoke route or activate semantics.

Audit only wrapper spelling in existing
`observed_root_apparent_repository_definition_identity_staging_and_terminal_algebra`.
Preserve every key/root/Display, two-child order, terminal, merge, equality,
validity, event, epoch, lifecycle, cancellation and nonactivation assertion.
Source proof must continue to establish exactly one Mapping, Definition and
Merge inner mapping and exactly one wrapper at observed Key projection.

## Prospective authority, caps and validation

Prospective implementation authority is exactly:

- `app/slug_core_v2/src/runtime/root_apparent_repository_definition.rs`,
  baseline 1,714 physical/tests 529, SHA-256
  `9aba8dba56972fce08d23d9fb97a604a849e5aac4694b34c29b472e4e837dca5`;
- test-only `app/slug_core_v2/src/runtime/root_apparent_repository_route.rs`,
  baseline 1,088 physical/tests 374, SHA-256
  `131fb0fca448acb3786946500d91f66ece2b6ee54441cc65968a9ce4605131ee`.

Prospective caps are <=80 definition production, <=50 definition proof and
<=80 route sibling proof; <=210 aggregate semantic additions and physical
<=1,845/1,168. Add no production helper or new definition-module test and
exactly one sibling smoke. The adjusted identity test remains below 200 and
the smoke below 100. The definition file remains cohesive around its accepted
driver/carrier/projection; the route file changes only its colocated compile
proof. No hot-path or retained-representation change applies.

Prospective validation is serial: focused observed root-definition tests; the
exact sibling smoke; protected legacy root-definition and route tests; full
`cargo test -p slug_core_v2`; direct dependent
`cargo check -p slug_commands_v2`; `cargo fmt --all -- --check`; exact two-file
allowlist/SHA/accounting/physical/test-size/effective-visibility/source-shape
checks; and `git diff --check`. Reuse accepted owner and opaque same-crate
wrapper evidence. Add no Bazel oracle for a visibility-only change.

Root-definition values/order/errors/views/equality/invalidation and lower
events remain **exact** Bazel 9 compatibility. The crate-internal opaque
Result-Arc+transaction-local epoch handoff is **Slug-native**. Route ownership,
source/public/command/bootstrap observation and exact Bazel configuration/
output/ActionKey bytes remain **unsupported/deferred**.

## Terminal

ACCEPT schedules exactly
`WP-6-7A-host-root-apparent-repository-definition-observation-carrier-visibility-implementation`,
then returns only to a docs-only root apparent-route observation-owner design.
STOP Rust/test/API edits in this packet; route/caller activation; crate-public
or crate-root export; public field/alias/inner/variant/inspector; lower-carrier
redesign; third file/type/key/carrier/adapter; root-definition semantic/order/
event/equality/epoch/retention drift; proof beyond wrapper spelling and one
sibling smoke; Cargo/BUILD, fixture/oracle; cap/proof/test/format waiver; source/
public/command/bootstrap work, milestone closure, M8/M7B or exact identity
work. REPLAN before widening or hash drift. M7 remains partial and
M7A -> M8 -> M7B remains.

## Immediate predecessor

Accepted owner `29795aeb` is +738/-103 at 1,714 physical lines and preserves
the exact mapping-first/conditional-definition epoch merge, real-family parity,
held-child lifecycle, cancellation and upper nonactivation. Its terminal
requires this carrier-visibility/consumer audit before route ownership.
