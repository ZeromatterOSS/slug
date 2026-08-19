# Current Slug V2 Packet

Packet: `WP-6-7A-repository-materialization-observation-implementation`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Rust base: `cc847c98`
Accepted design: `b2fd01e7`

## Exact Rust authority and caps

Write only `app/slug_bzlmod_v2/src/source_preparation.rs` from the
14,379-line `cc847c98` baseline: <=180 production, <=400 tests, <=580
aggregate semantic and <=15,000 physical lines. The file is a cohesive
large-owner exception and every touched helper must remain below 200 lines.
Every other file is read-only.

## Frozen implementation authority

Add private structural
`RepositoryMaterializationObservationKey(RepositoryMaterializationKey)` and
private `ObservedRepositoryMaterialization`. Its value is
`SourcePreparationOutcome<Result<ObservedRepositoryMaterialization,
ObservedPathFrontierError>>`. Retain exactly one
`Arc<Result<RepositoryMaterialization, RepositoryMaterializationError>>` plus
one compact `PathObservationEpoch`; implement `Dupe` and `Allocative`.
Keep construction and borrowed result/epoch access module-local.

Use one Legacy/Observed materialization driver. Preserve request then result
order. Legacy selects only `RepositoryMaterializationRequestKey`; observed
selects only `RepositoryMaterializationRequestObservationKey`. Both compute
the same neutral `RepositoryMaterializationResultKey` after request success.
Neither key computes its sibling. Legacy moves the exact materialization Result
Arc produced by the shared driver.

Accept the observed request epoch before semantic inspection. The result key,
result epoch and generation inputs add no path observation, so forward the
request epoch unchanged; do not union an empty/synthetic epoch. Request DICE
compute failure projects the existing `RootModuleFiles` semantic error with an
empty prefix. Request semantic failure retains the request prefix. Result DICE
compute failure projects `ResultCompute` with the request prefix. Local and
immutable success, Spec, Transport, Materialization and MissingGeneration
semantic terminals retain the request prefix.

Request Need or typed outer returns immediately, carrierless, before result
activation. Result Need is also immediate and carrierless, including missing
result-epoch entry, request mismatch and stale transport/materialization
generation. There is no joined batch or Need union.

Need is invalid/self-unequal. Complete typed outer is valid/equal by outer
value. Complete carrier is valid/equal by semantic Result plus epoch. Parent
and result remain eventless for success, semantic error, Need, outer and
cancellation. Existing root MODULE/request children remain their sole event
owners; materialization result injection remains neutral.

Retain no request child Result Arc, result child carrier Arc, collection,
snapshot, generation map, cache, store, interner, lock, task, direct Host read,
revision, certificate or event state. Request/result extraction, result-key
construction and reducer scratch stay compute-local.

## Required proof

Discriminate:

- distinct key identity/hash/Display, accessors, `Dupe`/`Allocative` shape;
- exact legacy value/error and Result-Arc parity;
- exact request epoch membership and per-demand `Arc::ptr_eq`;
- request-compute empty, request-semantic full, result-compute prior and
  result-semantic/success full prefixes;
- local/immutable success and Spec/Transport/Materialization/MissingGeneration
  semantic errors;
- absent/mismatched result epochs and stale-generation Need;
- request Need/typed outer and result Need validity/equality, no carrier and
  later-child suppression;
- observed-request versus legacy-request family rows while the neutral result
  key remains shared;
- exact child event order, parent silence, warm suppression and no batch on
  Need/outer/cancel;
- real poll-drop/no-publication/same-DICE recovery;
- request and injected-result A-B-A with held Result/epoch Arcs; and
- zero repository-source, closure, registry/preparation, discovery,
  selected-graph, extension, generated-repository or public activation.

Run focused owner tests, full bzlmod, affected loading/query/core baselines,
formatting, diff-check, exact accounting, and AI-cleanup/Buck2 retention review.
Reuse accepted request/result tests, `docs/developers/dice.md` and pinned Bazel
9.2 module discovery sources. No fixture or oracle is authorized.

## Compatibility

Exact: existing materialization values/errors/order, request/result/generation
semantics, legacy Result Arc and child events. Slug-native: the private sibling,
carrier, typed outer and epoch association. Unsupported/deferred: repository
source and nonregistry closure observation, registry file/source preparation,
discovered modules, selected graph, extension evaluation/instantiation,
generated repository loading, external rules_rust analysis/actions, M8/M7B and
exact Bazel identity bytes.

## STOP and successor

STOP on any second file/key/caller/export, source/closure or registry/
preparation/discovery/selected-graph activation, direct Host read, semantic/
error/order/event/family drift, retained scratch/state, cap excess, proof
deletion or milestone closure. REPLAN rather than alter the path-neutral result
owner, weaken carrierless Need/outer behavior, or duplicate lower semantics.

After independent implementation ACCEPT, schedule only the docs-only
`WP-6-7A-selected-module-graph-observation-frontier-design`. Exactly one
immediate successor is authorized.
