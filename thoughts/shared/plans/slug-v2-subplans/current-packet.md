# Current Slug V2 Packet

Packet: `WP-6-7A-host-generated-repository-definition-observation-implementation`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Scheduling/design and Rust base: docs-only owner design / `8990cf43`

## Goal and authority

Implement only the accepted private observation owner for
`HostGeneratedRepositoryDefinitionKey`. Reuse the accepted observed validation
child and preserve the legacy certificate scan, value/error algebra and lower
event ownership. Do not activate the later canonical-selected, canonical
definition, root-mapping, publication, command or bootstrap graph.

Write authority is exactly
`app/slug_core_v2/src/runtime/generated_repository_definition.rs`, baseline
2,426 physical lines with tests at 777. Every other Rust file, test, fixture,
oracle, Cargo/BUILD target, API, caller and plan is read-only. Production is
<=210 lines, proof is <=680, aggregate semantic authority is <=890 and the
file remains <=3,320 physical lines. Add at most six production and six test
helpers, one shared driver below 120 lines, exactly three tests, and keep every
helper/test below 200 lines. The file remains cohesive: before its test module
it already owns the generated reducer, sole canonical consumer, result/view,
errors, tracker plumbing and certificate fixtures; splitting would expose a
private result/view/error or duplicate that proof plumbing.

## Frozen owner and driver

Add exactly three private nominal types:

1. `HostGeneratedRepositoryDefinitionObservationKey`, a newtype over the
   legacy key with the same workspace/canonical-repository identity,
   `new(...)`, and Display `observed-{legacy Display}`. For `/workspace` and
   `CanonicalRepoName::new("generated")`, assert
   `observed-host-generated-repository-definition:"/workspace":@@generated`.
2. `ObservedHostGeneratedRepositoryDefinition`, retaining exactly an
   `Arc<Result<HostGeneratedRepositoryDefinition,
   HostGeneratedRepositoryDefinitionError>>` and one `PathObservationEpoch`.
3. `HostGeneratedRepositoryDefinitionObservationError::Validation(
   HostValidatedModuleExtensionRepositoriesObservationError)`.

Keep every type, field, constructor and accessor private. Add no alias,
reexport or caller. Factor the legacy compute into one private Legacy/Observed
driver. Legacy requests only `HostValidatedModuleExtensionRepositoriesKey` and
uses an empty epoch. Observed requests only
`HostValidatedModuleExtensionRepositoriesObservationKey` and carries its
epoch unchanged; this one-child owner performs no merge, rebuild, union or
epoch validation.

For either mode, child `Need` returns `Need` immediately. DICE compute failure
is the existing semantic `LoadingCompute(message)` with an empty epoch. A
complete semantic validation failure becomes the existing `Loading(error)` and
retains the observed child epoch. An observed opaque validation outer becomes
the carrierless parent `Validation(...)` outer. Child success clones the
certificate Arc once and runs the exact existing complete flattened scan: keep
the first matching ordinal, record the first conflicting ordinal, continue
through all remaining entries, then return `Duplicate`, success, or `Missing`
in that order. Success retains certificate plus ordinal; Missing/Duplicate
retain the certificate; Loading retains the validation error; LoadingCompute
retains only its message.

`HostCanonicalSelectedModuleDefinitionKey` is not a child. The later canonical
owner computes selected first and reaches generated only for selected
`Missing`; it and every upper key remain inactive here.

## Exact proof

Add exactly these tests:

1. `observed_generated_definition_identity_scan_and_terminal_algebra`: exact
   key identity/hash/Display/validity, single-child/no-merge, Need/outer and
   stage mappings; retain the protected full-scan Missing/Duplicate proof.
2. `observed_generated_definition_real_order_events_and_parity`: real
   legacy/observed success, Loading and Missing parity, defensive Duplicate and
   LoadingCompute source-stage proof, exact tracker edges to only the matching
   validation child, zero selected activation, lower HostBzl-load then pure-
   invocation print order, batchless instantiation/validation/generated rows,
   warm silence and first-terminal suppression.
3. `observed_generated_definition_lifecycle_cancellation_and_nonactivation`:
   held A-B-A over definition/order/mapping changes, same-semantic/different-
   epoch metadata, each recovery carrier epoch a subset of its own transaction
   global, Arc identity only on exact `Reused`, poll-drop recovery, and an upper
   denylist covering legacy generated, selected/canonical definition, apparent
   and root mappings, root apparent definition, route/source, public command
   and bootstrap keys.

Reuse the accepted validation wrapper proof; do not inspect or construct that
opaque outer. Prove its branch by source scan plus a real dependency row. Need,
outer and cancellation produce no carrier or print batch. Warm/Reused children
never replay lower prints. Retain only the generated Result Arc and child
epoch: no validation carrier, iterator/scan scratch, mode, evaluator, event,
duplicate certificate, cache, task or lock may escape compute.

Run serially:

- `cargo test -p slug_core_v2 observed_generated_definition_`;
- protected `cargo test -p slug_core_v2 generated_repository_definition::tests::`;
- full `cargo test -p slug_core_v2`;
- protected `cargo test -p slug_loading_v2 --test validated_repository_observation_api`;
- direct dependent `cargo check -p slug_commands_v2`;
- `cargo fmt --all -- --check`; and
- `git diff --check`.

## Compatibility and terminal

Existing generated values/errors, flattened certificate scan/order, ordinal,
equality, invalidation and lower Bzl/invocation events remain exact Bazel 9
compatibility. The private observation key/carrier/outer and shared-Arc
transaction-local epoch association are Slug-native. Canonical/root/publication
observation, command/bootstrap activation and exact Bazel configuration/output/
ActionKey bytes remain unsupported/deferred. Existing Bazel 9.2
`SingleExtensionFunction`, `SingleExtensionEvalFunction` and
`ModuleExtensionResolutionTest` source evidence plus Buck2 DICE incrementality/
cancellation concepts suffice; add no oracle.

ACCEPT returns only to a docs-only
`HostCanonicalRepositoryDefinitionKey` selected/generated frontier audit.
STOP a second file/key/owner/adapter, export/reexport/caller, selected child,
canonical/root/route/source/public/command/bootstrap activation, semantic/
scan/order/event/equality/retention drift, epoch merge, task/lock, fixture/oracle,
cap/helper/test waiver, milestone closure, M8/M7B or identity-byte work. REPLAN
before widening. M7 remains partial and M7A -> M8 -> M7B remains.

## Immediate predecessor

Commit `8990cf43` exposes exactly the observed validation child and opaque outer.
The live generated owner has one production consumer, the later canonical owner;
its selected-definition request is an upper missing-only branch, not a generated
dependency.
