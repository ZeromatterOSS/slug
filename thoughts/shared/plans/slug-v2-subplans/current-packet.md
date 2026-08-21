# Current Slug V2 Packet

Packet: `WP-6-7A-host-canonical-repository-definition-observation-carrier-visibility-design`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Audit and Rust base: pending docs commit / `4fe0bf1c`

## Goal and decision authority

Design only the uniquely smaller same-crate visibility prerequisite between
the accepted private canonical-definition observation and the future root
apparent-definition observation owner. Freeze one minimal `pub(super)` opaque
carrier surface plus sibling compile proof without changing computation or
activating any caller.

Write only the canonical plan, this manifest, Stage 6 and routing log at net
caps <=40/<=180/<=220/<=30 and <=470 aggregate. Rust, tests, fixtures,
oracles, Cargo/BUILD, exports and callers are read-only in this packet.

## Audited frontier and decision

Accepted `4fe0bf1c` promotes the apparent-mapping observation key, carrier,
concrete borrowed Result/epoch accessors and opaque outer to `pub(super)`.
The root-definition sibling smoke compiles, while the observation remains
callerless. Apparent-mapping visibility is no longer a blocker.

`HostRootApparentRepositoryDefinitionKey` currently computes legacy apparent
mapping first at `root_apparent_repository_definition.rs:266`. A mapping
failure terminates immediately. Successful main and `bazel_tools` targets
defer before any definition lookup; only another resolved target reaches the
legacy canonical-definition child at line 310. Mapping therefore owns the
prefix and target disposition, and definition is a conditional second child.

The accepted canonical-definition observation at
`generated_repository_definition.rs:496-727` has one current production
consumer: the nonroot branch of the observed apparent-mapping owner at line
958. Its key/new, carrier, private-alias accessors and typed outer remain
private to that sibling module. Root apparent definition cannot name the
associated Value or preserve the definition epoch. Reusing the legacy child
would silently discard observation ownership and is not an admissible bridge.

The private canonical outer has Selected, Generated and Merge variants. Its
Generated variant names the private generated-definition outer and Merge names
the local frontier error, so direct enum promotion would reveal lower terminal
structure. A field-private nominal wrapper at Key projection is the bounded
effective-visibility shape; exact names and projection are reserved for this
design packet.

Root apparent definition has exactly one production consumer,
`HostRootApparentRepositoryRouteKey` at
`root_apparent_repository_route.rs:303`. Route, source input/source
observation/path input, repository publication/materialization, command and
bootstrap layers do not consume either lower observed carrier directly and
cannot replace its epoch. They are not prerequisites.

Thus canonical-definition carrier visibility is uniquely smaller than root
apparent-definition ownership. It needs no crate-public API, `lib.rs` reexport,
module move, adapter, alias or semantic caller because both modules share the
core runtime parent.

## Design deliverable

Freeze exactly one minimal same-crate surface:

- the existing canonical-definition observation key and only its existing
  two-argument constructor at `pub(super)`;
- the existing carrier with private fields and concrete borrowed canonical
  Result-Arc and `PathObservationEpoch` accessors at `pub(super)`; and
- one `pub(super)` field-private opaque outer, retaining all Selected/
  Generated/Merge details in a renamed private inner used by the driver and
  wrapping only the observed Key error projection.

The design must determine the exact nominal names, Display and accessor
signatures from live types. It must add no public field, lower variant,
private-alias exposure, constructor/conversion/inspector for the outer,
crate-root export, adapter or caller.

Freeze exactly one test-only sibling proof in
`root_apparent_repository_definition.rs`. It may construct only the canonical-
definition observation key and assert exact Display, then use nonexecuted
function pointers to prove the associated Value and concrete borrowed carrier
accessors. It must not construct or inspect carrier/outer, compute, activate
root definition, or name private inner/variants/alias.

Audit the existing
`observed_canonical_repository_definition_identity_staging_and_terminal_algebra`
for wrapper-only spelling changes. Preserve all accepted selected-first,
generated-on-semantic-Missing, merge, Need, outer, equality, validity,
dependency and epoch assertions. Do not change real-order/event or lifecycle/
cancellation/nonactivation proof.

## Prospective authority, caps and validation

Prospective implementation authority is exactly:

- `app/slug_core_v2/src/runtime/generated_repository_definition.rs`, baseline
  3,843 physical/tests 1,152, SHA-256
  `ea48d5e52dbad37bfc79e745ae0d6e24cc3e2b133b45fb4e861b5373810722ba`;
- test-only
  `app/slug_core_v2/src/runtime/root_apparent_repository_definition.rs`,
  baseline 1,042 physical/tests 372, SHA-256
  `c06fa8c8a2ebed243e32168a411c4f36bc1ff0d48803e077c431ae4c37aef19e`.

Prospective caps are <=80 generated production, <=50 generated proof and <=80
sibling proof; <=210 aggregate semantic lines and physical <=3,974/1,122.
Add no production helper or new generated-module test and exactly one sibling
smoke. The adjusted identity test stays below 200 and the smoke below 100.
The large generated module remains cohesive because it owns the canonical
driver/carrier/projection; the sibling changes only its colocated compile
proof. No hot-path or retained-representation change applies.

Prospective validation is serial: focused observed canonical-definition tests,
the exact sibling smoke, protected apparent-mapping and root-definition tests,
full `slug_core_v2`, direct `slug_commands_v2` check, formatting, then exact
two-file allowlist/SHA/accounting/physical/test-size/visibility/source-shape
checks and `git diff --check`. Reuse accepted owner and same-crate opaque-
wrapper proof; add no Bazel oracle for a visibility-only change.

Canonical-definition selection/generation order, targets, failures, equality/
invalidation, epochs and lower events remain **exact** Bazel 9 compatibility.
The crate-internal opaque Result-Arc+epoch handoff is **Slug-native**. Root
apparent-definition ownership, its later carrier visibility, route/source/
public/command/bootstrap observation and exact Bazel configuration/output/
ActionKey bytes remain **unsupported/deferred**.

## Terminal

ACCEPT schedules exactly
`WP-6-7A-host-canonical-repository-definition-observation-carrier-visibility-implementation`,
then returns only to a docs-only root apparent-definition observation-owner
design. STOP Rust/test/API edits in this packet; root-definition activation;
crate-public/export or public field/alias/inner/variant/inspector; apparent-
mapping redesign; third file/type/key/carrier/adapter; canonical semantics/
order/event/equality/epoch/retention drift; proof beyond wrapper spelling and
one sibling smoke; Cargo/BUILD, fixture/oracle; cap/proof/test/format waiver;
upper route/source/public/bootstrap work, milestone closure, M8/M7B or exact
identity work. REPLAN before widening or hash drift. M7 remains partial and
M7A -> M8 -> M7B remains.

## Immediate predecessor

Accepted visibility implementation `4fe0bf1c` is +73/-15 across exactly the
generated-definition owner and root-definition test module. Its terminal
requires this prerequisite audit before root-definition ownership.
