# Current Slug V2 Packet

Packet: `WP-6-7A-host-canonical-selected-module-definition-observation-owner-design`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Scheduling and Rust base: canonical selected/generated frontier audit / `7f9325e1`

## Goal and docs authority

Design exactly one private observation owner for
`HostCanonicalSelectedModuleDefinitionKey`. Freeze its reuse of the existing
private selected-routes observation, exact route scan and terminal algebra,
Result/epoch retention, lifecycle/cancellation proof and upper nonactivation.
Do not implement Rust or expose the new carrier to `slug_core_v2`.

Docs write authority is exactly the canonical plan, this packet, Stage 6 and
the orchestration routing log at net caps <=40/<=180/<=220/<=30 and <=470
aggregate. Every Rust file, test, fixture, oracle, Cargo/BUILD target, API,
export and caller is read-only.

## Learned frontier

The accepted `7f9325e1` generated observation key, Result-Arc/epoch carrier and
opaque validation outer are private in
`generated_repository_definition.rs`, the same module as the future canonical
owner. They need no visibility prerequisite.

The canonical legacy owner at lines 518-605 computes public
`HostCanonicalSelectedModuleDefinitionKey` first. Selected success, Need,
non-Missing terminal and compute failure return without requesting generated.
Only selected `Missing` is retained and followed by the generated request;
generated success publishes Generated, generated Missing combines both
missing certificates, and other generated failures preserve the selected
Missing. This order and suppression are exact and must remain inactive during
the present design.

No selected-definition observation exists. The legacy selected key at
`selected_repo_spec.rs:2324-2449` has exactly one child,
`HostSelectedModuleRoutesKey`; its private observed sibling, carrier and outer
already exist in the same file at 1582-2083. Therefore a private selected-
definition owner is uniquely smaller than canonical ownership or cross-crate
promotion. After implementation, a separate visibility audit must decide the
minimal bzlmod -> core surface.

Canonical definition has exactly two production upper consumers: non-root
apparent mapping at generated-definition line 799 and root apparent definition
at its line 310. Root apparent route then source-input/source-path/public
repository and command/bootstrap form later branches. None is a prerequisite.

## Design decisions to freeze

Choose or REPLAN exactly one private Legacy/Observed owner in
`selected_repo_spec.rs`:

- an observed key with the same workspace/canonical identity and
  `observed-{legacy Display}`;
- a carrier holding only the exact
  `Arc<Result<HostCanonicalSelectedModuleDefinition,
  HostCanonicalSelectedModuleDefinitionError>>` plus one
  `PathObservationEpoch`; and
- a typed outer over `HostSelectedModuleRoutesObservationError`.

Freeze one shared driver. Legacy must request only legacy routes and use an
empty epoch; Observed must request only observed routes and carry its epoch
unchanged. Specify immediate Need, carrierless observed outer, DICE compute
failure, semantic Routes failure, full Missing/Unique/Duplicate scan, builtin-
deferred rejection and exact first/conflicting ordinal behavior. The scan must
consume every route even after a duplicate is known. One child requires no
epoch merge, rebuild, union or validation.

Freeze exact event ownership: the new owner is batchless, preserves every
accepted lower batch/order, never replays a warm child and publishes no carrier
or batch for Need, outer or cancellation. Retain only the selected Result Arc
and child epoch; route carrier, iterator/scan scratch, evaluator, event, mode,
cache, task and lock die before publication.

Freeze three tests covering:

1. identity/Display, Need/outer algebra and the complete scan/terminal matrix;
2. real legacy/observed semantic parity, exact single child rows, lower event
   parity, batchless/warm behavior and first-terminal suppression; and
3. held Result/carrier/epoch A-B-A, same-semantic/different-epoch metadata,
   per-transaction epoch subsets, conditional Reused Arc identity, poll-drop
   recovery and zero canonical/generated/root/route/source/public/command
   activation.

Reuse the accepted selected-routes observation proof and existing
`pure_canonical_selected_definition_exhausts_and_retains_identity` plus real
selected-definition lifecycle coverage. Name the applicable Bazel 9.2
selection/resolution source tests and Buck2 DICE incrementality/cancellation
concept evidence. Add no oracle absent a demonstrated observable gap.

## Prospective implementation boundary

Prospective Rust authority is exactly
`app/slug_bzlmod_v2/src/selected_repo_spec.rs`, baseline 11,687 physical
lines with tests at 4,510. Cap production at <=230, proof at <=680, aggregate
semantic authority at <=910 and physical size at <=12,600. Permit at most six
production and six test helpers, exactly three tests, a shared driver below 120
lines and every helper/test below 200. The design must either retain this
one-file authority with a concrete cohesion finding or REPLAN; a sibling file
cannot expose private routes state merely to avoid the size trigger.

Prospective serial validation is focused `observed_canonical_selected_`,
protected selected-definition and observed-routes tests, full
`cargo test -p slug_bzlmod_v2`, direct dependent
`cargo check -p slug_core_v2`, formatting and `git diff --check`.

## Compatibility and terminal

Existing selected definition values/errors, route order/scan, public views,
dispositions, equality/invalidation and lower events are exact Bazel 9
compatibility. The private observation carrier and transaction-local epoch are
Slug-native. Cross-crate promotion, canonical/generated observation
composition, root/route/source/public/command/bootstrap activation and exact
Bazel configuration/output/ActionKey bytes remain unsupported/deferred.

Design ACCEPT may schedule exactly one private implementation, then return only
to a selected-carrier visibility audit. STOP implementation in this packet,
second Rust file/key/owner/adapter, public API/export/caller, canonical or
generated compute, changed selected semantics/order/disposition/event/equality/
retention, epoch merge, task/lock, fixture/oracle, cap waiver, upper activation,
milestone closure, M8/M7B or identity-byte work. REPLAN before widening. M7
remains partial and M7A -> M8 -> M7B remains.

## Immediate predecessor

Commit `7f9325e1` accepted the private generated-definition observation owner
at +658/-182 in one file. It deliberately proved zero selected/canonical/root/
route/source/public activation and returned to this frontier audit.
