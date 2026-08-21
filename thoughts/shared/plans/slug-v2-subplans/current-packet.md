# Current Slug V2 Packet

Packet: `WP-6-7A-host-validated-module-extension-repositories-observation-carrier-promotion-design`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Scheduling and Rust base: `b8459b4e`

## Goal and design authority

Design only the smallest loading -> core visibility surface that lets
`HostGeneratedRepositoryDefinitionKey` name the accepted observed validation
Key Value and borrowed carrier. Freeze an opaque public outer projection,
exactly three doc-hidden reexports, one external-crate API smoke and bounded
implementation authority. Do not edit Rust or activate generated, canonical,
root-mapping, publication, command or bootstrap computation.

Design write authority is exactly:

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`, net <=40;
- this manifest, net <=180;
- `thoughts/shared/plans/slug-v2-subplans/06-analysis-toolchains-and-actions.md`,
  net <=220; and
- `.codex/skills/slug-agent-orchestration/references/routing-log.md`, net <=30.

Aggregate net growth is <=470. Every Rust file, test, fixture, oracle,
Cargo/BUILD target, API/export and caller is read-only. Schedule exactly one
visibility-only implementation successor on ACCEPT or one narrower
prerequisite/REPLAN.

## Live visibility and ownership facts

Accepted commit `b8459b4e` adds a callerless private validation observation at
`module_extension_repository_validation.rs:178-343`. The key and constructor
are private at 178-192, the carrier and borrowed accessors are private at
220-235, `result()` names private alias `ValidatedRepositoriesResult`, and the
private outer at 237 exposes the crate-internal instantiation outer directly.
`lib.rs:63-72` reexports only the legacy validation certificate, error, outcome
and key; it exports none of the three observed nominal types.

`HostGeneratedRepositoryDefinitionKey` at
`app/slug_core_v2/src/runtime/generated_repository_definition.rs:101-206` is
the first and only production consumer of the legacy validation key. At 168 it
computes that key, retains the validated certificate, scans its exact flattened
order for the requested canonical name, and returns the existing Loading,
LoadingCompute, Missing or Duplicate result with exact first/conflicting
ordinals. It owns no event batch. Core already depends one way on loading; no
reverse dependency or owner move is needed.

The generated-definition key has one production consumer: canonical
repository definition at line 467. Canonical definition first computes the
parallel selected-module definition at 428 and consults generated definition
only after an exact selected Missing. Root apparent mapping separately computes
the root repository mapping at 678; only non-root contexts compute canonical
definition at 704. Root apparent definition later resolves a root mapping
target and computes canonical definition at
`root_apparent_repository_definition.rs:310`. Root route, source-input and
source-path are the remaining production chain; the private source-observation
key is callerless, and command/bootstrap scans contain no validation or
generated-definition key. Those serial/parallel or inactive boundaries cannot
make the private validation carrier nameable in core and are false prerequisites.

## Selected prerequisite and design questions

The uniquely smaller successor is a carrier-promotion design, not generated
publication ownership. Freeze exactly the minimum nominal surface:

1. the existing validation observation key and public constructor;
2. the existing observed validation carrier with public borrowed accessors
   spelled using concrete
   `Arc<Result<HostValidatedGeneratedRepositorySpecs,
   HostValidatedGeneratedRepositorySpecsError>>` and
   `PathObservationEpoch`; and
3. one opaque doc-hidden public observation-error wrapper whose field and
   instantiation terminal remain private.

Determine the exact wrapper boundary required by Rust effective visibility.
The expected precedent renames the current private outer to an inner enum and
wraps only at the observed key's associated `Key::Value` boundary. Same-module
validation proof may inspect the inner algebra; an external consumer must only
carry the public opaque wrapper. Keep `ValidatedRepositoriesResult`, all fields,
the instantiation outer and its Pure terminal private. Add no inspector,
conversion trait, adapter key, copied carrier, reverse dependency or fourth
nominal type.

Freeze exactly three `#[doc(hidden)]` crate-root reexports and one new external
API smoke in the loading package. The smoke may construct only the observed key
to assert its existing Display and type-check the associated Value, borrowed
concrete Result/epoch accessors and opaque outer through a nonexecuted function
pointer. It must not construct a carrier/outer, compute any key, inspect a
terminal, add a semantic caller or activate core.

## Prospective implementation boundary

Prospective Rust authority is exactly:

- production and colocated proof
  `app/slug_loading_v2/src/module_extension_repository_validation.rs`, baseline
  1,810 physical lines with tests at 437;
- production reexports `app/slug_loading_v2/src/lib.rs`, baseline 86 physical
  lines; and
- one new external smoke
  `app/slug_loading_v2/tests/validated_repository_observation_api.rs`.

The design must hold <=70 production, <=40 colocated proof, <=60 external proof
and <=170 aggregate semantic lines; physical caps are 1,880/95/60. Every
changed helper/test stays below 100. The large validation file remains cohesive
because it owns the private driver, carrier and terminal representation; the
external smoke is the narrow public-visibility witness. No split or hot-path
measurement is warranted for a visibility-only change.

Preserve the accepted validation key identity and Display, Complete-only
equality/validity, exact Result Arc and transaction-local epoch association,
carrierless Need/outer behavior, lower-only print ownership, batchless
validation rows, warm silence, cancellation recovery and retained lifetime.
Add no DICE dependency, event owner, cache, lock, task or command state.

Reuse accepted validation identity/family/order/error/event/lifecycle/
cancellation/nonactivation proof and the prior Bzlmod carrier-promotion smoke
pattern. Add no oracle because visibility changes no Bazel-visible behavior.
The implementation design must require focused `observed_validation_`, the
external smoke, protected `real_validation_` and generated-definition tests,
full `cargo test -p slug_loading_v2`, direct dependent
`cargo check -p slug_core_v2`, formatting and `git diff --check`.

## Compatibility and terminal

Existing validation/generated values, errors, order, iterator projection,
DICE equality and lower event ownership remain exact Bazel 9 compatibility.
The doc-hidden cross-crate observation key/carrier/opaque outer and shared-Arc
transaction-local epoch association are Slug-native. Generated observation and
canonical/root-mapping/publication/command/bootstrap activation plus exact
Bazel configuration/output/ActionKey bytes remain unsupported/deferred.

Design ACCEPT schedules exactly
`WP-6-7A-host-validated-module-extension-repositories-observation-carrier-promotion-implementation`.
After implementation ACCEPT, return only to a docs-only
`HostGeneratedRepositoryDefinitionKey` observation-owner design. STOP any
semantic consumer or compute activation, public field/alias/error inspector,
fourth type/reexport, second key/carrier/adapter, validation/generated semantic
or event/equality/retention change, core source change, reverse dependency,
fixture/oracle or Cargo/BUILD work, third production file, cap/proof waiver,
canonical/root-mapping/publication/command/bootstrap activation, milestone
closure, M8/M7B or exact identity work. REPLAN before widening. M7 remains
partial and M7A -> M8 -> M7B remains.

## Immediate predecessor

`b8459b4e` accepts the one-child observed validation owner at +666/-29 total
and 1,810 physical lines. Its sole missing input to generated publication is
cross-crate effective visibility; canonical selection, root mapping and all
public/runtime boundaries are later or parallel.
