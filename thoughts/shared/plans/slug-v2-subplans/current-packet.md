# Current Slug V2 Packet

Packet: `WP-6-7A-host-nonregistry-package-preflight-observation-implementation`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Scheduling base and accepted design: `0c5a1366`
Rust base: `754e7619`

## Objective and exact authority

Implement the independently accepted private package-preflight observation
owner without activating its horizon consumer.

Write exactly `app/slug_bzlmod_v2/src/source_preparation.rs` from the 15,267-line
`754e7619` baseline at <=320 production and <=15,650 physical, plus
`source_preparation_observation_tests.rs` from 3,170 lines at <=720 proof and
<=3,950 physical. Aggregate is <=1,040 semantic and <=19,600 physical.
Every other Rust/Cargo/BUILD/fixture/oracle/caller/public file is read-only.
Touched helpers remain below 200 lines.

## Frozen production contract

Add private `HostNonregistryPackagePreflightObservationKey`,
`ObservedHostNonregistryPackagePreflight`, and stage-aware
`HostNonregistryPackagePreflightObservationError`. The Key Value is exactly
`SourcePreparationOutcome<Result<ObservedHostNonregistryPackagePreflight,
HostNonregistryPackagePreflightObservationError>>`. Its carrier retains one
local `Arc<Result<HostNonregistryPackagePreflight,
HostNonregistryPackagePreflightError>>` plus cumulative
`PathObservationEpoch`. Require `Dupe`/`Allocative`, borrowed accessors, and no
export or caller activation.

Use one Legacy/Observed driver in exact order: effective override,
invalid-package-name short circuit, neutral deleted-package policy, repository
ignore, `BUILD.bazel`, then `BUILD`. Legacy selects only legacy effective,
ignore and marker-source families; observed selects only their accepted
observed siblings. Both share the neutral deleted-policy projection. Neither
computes the other family.

Accept the effective epoch before semantics. Effective failure, missing
nonregistry override, invalid package name, policy error and nonempty deleted
policy retain only that prefix. Merge effective left-first with a Complete
ignore epoch before ignore semantics. Ignored stops both markers. For each
reached marker, merge the accumulated prefix left-first with its Complete
source epoch before semantics. `BUILD.bazel` Present stops `BUILD`; the second
marker and NoBuild retain both marker epochs. Equal duplicates keep the
earliest exact Arc; conflict/operation mismatch is typed outer.

The outer distinguishes effective/ignore/marker child frontier failures and
stage-specific effective/policy/ignore/marker DICE compute failures. All are
carrierless and suppress later work. Invalid-name is pure; a semantic deleted-
policy projection error remains a Complete carrier with the effective prefix.
Preserve the legacy compute invariant and invent no semantic error. Need applies
only at effective, ignore and markers. There is no Need union. Need is invalid/
self-unequal; Complete outer is valid/equal by outer value; Complete carrier is
valid/equal by semantic Result plus epoch.

The preflight parent stays eventless. Root MODULE and matching REPO descendants
remain sole batch owners; effective, policy, ignore parent and marker sources
remain eventless. Legacy moves the exact local Result Arc. Retain no child
carrier, policy value, matcher, marker bytes, extra collection/state, cache,
interner, store, lock, task, direct Host read, revision, certificate or event
state.

## Required proof

Discriminate key/hash/Display, accessors, Dupe/Allocative and exact legacy
Result-Arc/value/error parity. Cover Need/child outer at effective, ignore and
first/second marker positions; DICE-compute outer at every computed dependency;
and every semantic terminal with exact prior/merged prefixes and later-child
suppression. Distinguish semantic deleted-policy projection error from policy
DICE failure. Assert invalid name precedes policy, deleted policy precedes ignore,
ignored precedes markers, and `BUILD.bazel` Present precedes `BUILD`.

Prove exact epoch iteration and per-demand `Arc::ptr_eq`, first equal Arc,
conflict and operation mismatch. Assert exact legacy/observed direct dependency
rows and reverse isolation, with the neutral deleted-policy child shared.
Assert exact child-owned ROOT/REPO batch order and text, parent/effective/
policy/ignore/source silence, warm suppression, and poll-drop recovery.

Exercise local and immutable marker A -> B -> absent -> directory -> A,
`BUILD.bazel` <-> `BUILD` preference, held Result/epoch readability and restored
child-parent Arc identity. Prove zero horizon/closure/discovery/selected-graph/
registry/extension/public activation. Reuse accepted semantic evidence; add no
Bazel oracle because ordering, values, errors and child events remain exact.

Run focused proof, full bzlmod and affected loading/query/core baselines, fmt,
diff-check, exact accounting and AI-cleanup/Buck2 retention review.

## Compatibility and STOP

Exact: current effective/invalid-name/deleted-policy/ignore/marker ordering,
`BUILD.bazel` preference, values/errors, legacy Result Arc and child events.
Slug-native: private sibling, Result-Arc+epoch carrier and typed outer.
Unsupported/deferred: horizon/closure/discovery/selected graph, registry
preparation/patches, extension repositories, M8/M7B and identity bytes.

STOP a caller/export/third file, legacy/order/event/family drift, semantic
compute-error invention, direct Host read, extra retained state, upper/registry
activation, cap excess or milestone closure. REPLAN if the exact owner cannot
fit.

After independent implementation ACCEPT schedule only the docs-only
`WP-6-7A-host-nonregistry-module-closure-observation-design`.
