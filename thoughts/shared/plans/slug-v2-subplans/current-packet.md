# Current Slug V2 Packet

Packet: `WP-6-7A-repository-materialization-observation-design`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Scheduling/Rust base: `cc847c98`

## Exact docs authority

This packet is docs-only. Write only:

1. `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`: <=40 net lines.
2. `thoughts/shared/plans/slug-v2-subplans/current-packet.md`: <=180 net lines.
3. `thoughts/shared/plans/slug-v2-subplans/06-analysis-toolchains-and-actions.md`: <=160 net lines.
4. `.codex/skills/slug-agent-orchestration/references/routing-log.md`: <=30 net lines.

Aggregate docs delta is <=430 net lines. Rust, Cargo/BUILD metadata, fixtures,
oracles and every other plan are read-only.

## Accepted predecessor and owner decision

Implementation `cc847c98` accepts the private repository-materialization
request sibling from Rust base `3d174006` and design `e606e1b2`, with proof
correction `7592334b`. Final one-file accounting is +161 production/+471
tests/+632 aggregate at 14,379 physical lines. Focused 4/4, full bzlmod
433/433, loading 138/138 and full query pass. Core remains 245/246 only on the
recorded inherited stale visibility wording assertion. Formatting, diff,
cleanup/retention and independent review pass.

Do not freeze `HostSelectedModuleGraphKey` yet. Its nonregistry discovery path
uses `HostNonregistryModuleClosureKey`, which computes
`RepositoryMaterializationKey` before reading the root repository source.
`RepositorySourceFileKey` independently consumes the same materialization
owner. That key is the first complete reusable carrierless boundary after the
accepted request sibling: it sequences `RepositoryMaterializationRequestKey`
then the path-neutral/eventless `RepositoryMaterializationResultKey`.

Observing source, closure, discovery or selected graph first would duplicate or
bypass materialization semantics. Registry discovery separately crosses the
carrierless `ModuleSourcePreparationKey`/`RegistryFileKey` branch and
remains a later prerequisite. The uniquely smallest next owner is therefore one
private observed sibling of `RepositoryMaterializationKey`.

## Frozen design

Add private structural
`RepositoryMaterializationObservationKey(RepositoryMaterializationKey)` and
private `ObservedRepositoryMaterialization`. Its value is
`SourcePreparationOutcome<Result<ObservedRepositoryMaterialization,
ObservedPathFrontierError>>`. The carrier retains exactly one
`Arc<Result<RepositoryMaterialization, RepositoryMaterializationError>>` plus
one compact `PathObservationEpoch`, and implements `Dupe` and `Allocative`.
Expose only module-local construction and borrowed result/epoch accessors.

Use one Legacy/Observed materialization driver. Both modes preserve request
then result order. Legacy selects only
`RepositoryMaterializationRequestKey`; observed selects only
`RepositoryMaterializationRequestObservationKey`. Both then compute the same
neutral `RepositoryMaterializationResultKey` from the successful request.
Neither sibling computes the other. Legacy projects/moves the exact local
materialization Result Arc.

Accept and retain the observed request epoch before inspecting its semantic
Result. The result key and its generation/epoch inputs add no path observation,
so forward that request epoch unchanged and perform no epoch union. Request
DICE compute failure is semantic `RootModuleFiles` with an empty prefix.
Request semantic failure retains the request prefix. Result DICE compute
failure is semantic `ResultCompute` with the request prefix. Local/immutable
success and result semantic errors retain the request prefix.

Request Need or typed outer returns immediately with no carrier and does not
activate the result. Result Need likewise returns immediately with no carrier.
This includes missing result-epoch entry, request mismatch and stale
transport/materialization generation. There is no joined batch or Need union.

Need is invalid/self-unequal. Complete typed outer is valid/equal by outer
value. Complete carrier is valid/equal by semantic Result plus epoch. The
parent and result owner remain eventless on success, semantic error, Need,
outer and cancellation. Root MODULE/request children remain their existing
event owners; materialization result injection remains neutral.

Retain no request child Result Arc, result child carrier Arc, collection,
snapshot, generation map, cache, store, interner, lock, task, direct Host read,
revision, certificate or event state. Request extraction, result-key
construction and reducer scratch are compute-local. Keep all touched helpers
below 200 lines.

## Future implementation authority and caps

After independent design ACCEPT, authorize exactly:

- `app/slug_bzlmod_v2/src/source_preparation.rs`, from the 14,379-line
  `cc847c98` baseline: <=180 production, <=400 tests, <=580 aggregate
  semantic and <=15,000 physical lines.

The file is a cohesive large-owner exception. No second Rust file, export or
caller is authorized.

## Proof and compatibility

Discriminate distinct key identity/hash/Display and private access; exact
legacy value/error and Result-Arc parity; exact request epoch membership and
per-demand `Arc::ptr_eq`; request compute-empty, request-semantic full,
result-compute prior and result-semantic/full prefixes; local and immutable
success; spec, transport, materialization and missing-generation errors;
missing/mismatched/stale result Needs; request Need/outer and result Need
carrierlessness, validity/equality and later suppression; observed-request
versus legacy-request family rows with the neutral result shared; child-owned
events, eventless parent and warm silence; real poll-drop/no-publication/
same-DICE recovery; result injection and request A-B-A with held Result/epoch
Arcs; and zero source/closure/preparation/discovery/selected-graph activation.

Exact: existing materialization values/errors/order, request/result/generation
semantics, legacy Result Arc and child event behavior. Slug-native: the private
sibling/carrier, typed outer and epoch association. Unsupported/deferred:
repository source and nonregistry closure observation, registry file/source
preparation, discovered modules, selected graph, extension evaluation and
instantiation, generated repository loading, external rules_rust
analysis/actions, M8/M7B and exact Bazel identity bytes.

Reuse accepted request/result tests, `docs/developers/dice.md` and pinned Bazel
9.2 module discovery sources. No fixture or oracle is authorized.

## STOP and successor

STOP on Rust during this design, any second file/key/caller/export, source or
closure activation, registry/preparation/discovery/selected-graph activation,
direct Host read, event/family/error/order drift, retained scratch/state, cap
excess or milestone closure. REPLAN rather than merge a path-neutral result
input into the epoch or duplicate lower materialization semantics.

After independent design ACCEPT, schedule exactly one bounded
`WP-6-7A-repository-materialization-observation-implementation`. After
independent implementation ACCEPT, return only to the docs-only
`WP-6-7A-selected-module-graph-observation-frontier-design`. Exactly one
immediate successor is authorized.
