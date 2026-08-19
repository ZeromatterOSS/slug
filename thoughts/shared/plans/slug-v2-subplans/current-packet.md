# Current Slug V2 Packet

Packet: `WP-6-7A-repository-materialization-request-observation-implementation-retry`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Rust base: `3d174006`
Accepted semantic design: `e606e1b2`
Accepted proof-cap correction: `7592334b`

## Exact Rust authority and corrected caps

Write only `app/slug_bzlmod_v2/src/source_preparation.rs` from the
13,747-line `3d174006` baseline: <=180 production, <=480 tests, <=660
aggregate semantic and <=14,480 physical lines. The file is a cohesive
large-owner exception and every touched helper remains below 200 lines. Every
other file is read-only.

The retained candidate is +161
production/+319 tests/+480 aggregate at 14,227 physical lines. It fits the
production cap; corrected proof limits leave 161 test-net and 253 physical
lines for the missing observed terminal and lifecycle matrix.

## Frozen owner and implementation contract

Add the private structural
`RepositoryMaterializationRequestObservationKey(RepositoryMaterializationRequestKey)`
and private observed carrier containing exactly one local
`Arc<Result<RepositoryMaterializationRequest, RepositoryMaterializationError>>`
plus one compact `PathObservationEpoch`, with `Dupe` and `Allocative`.
Keep only the module-local constructor and borrowed result/epoch accessors
needed by the later materialization sibling.

Use one Legacy/Observed driver. Normalize the workspace first, select only the
matching `HostEffectiveModuleOverrideKey` or
`HostEffectiveModuleOverrideObservationKey`, then use one pure shared
projection for nonregistry override selection, canonical repository identity,
local/command path policy and local versus immutable request kind. Legacy moves
the exact local Result Arc; neither sibling computes the other.

Invalid workspace and effective DICE compute failure are semantic Complete with
an empty epoch. Effective Need or typed outer is immediate and carrierless. A
completed effective semantic error and every missing/unsupported override,
invalid canonical repository, request-kind/spec error and local/immutable
success terminal retain the complete effective prefix. Accept the observed
child epoch before semantic inspection and forward it unchanged; there is no
joined batch or Need union.

Need is invalid/self-unequal. Complete typed outer is valid/equal by outer
value. Complete carrier is valid/equal by semantic Result plus epoch. Preserve
the child's exact shared Result Arcs. The parent is eventless for every terminal
and cancellation; root MODULE children remain sole event owners.

Retain only the local request Result Arc plus compact epoch. Effective child
carrier, normalized workspace, canonical formatting, request-kind and
projection scratch remain compute-local. Add no collection, cache, store,
interner, lock, task, direct Host read, revision, certificate or event owner.
Do not activate materialization result injection, repository source,
preparation/closure, discovery, selected graph, extension, analysis or any
caller.

The retry may only restructure or add proof. Production semantics,
identity, driver order, Result-Arc projection, event ownership, retained state
and the <=180 production cap are frozen. Required additions must drive the live
observed owner, or a pure reducer used directly by it, through empty
invalid-workspace/effective-compute prefixes and full missing, unsupported,
canonical, request-kind/spec and success prefixes. They must discriminate
root-local, command-absolute and HTTP/Git immutable projections; malformed
request kinds; command/request-kind A-B-A with held Result and epoch Arcs; and
later-child suppression. Existing proof remains required.

## Required proof and compatibility

Discriminate distinct key identity/Display and private access; exact legacy
request/error/value and Result-Arc projection; exact effective epoch membership
and per-demand `Arc::ptr_eq`; empty invalid-workspace/effective-compute
prefixes; full effective-semantic/missing/unsupported/canonical/spec/success
prefixes; root-local, command-absolute, http/git immutable and malformed
request-kind behavior; Need/outer validity, equality, no carrier and later
suppression; both family directions and exact parent dependency rows; child
cold events, eventless parent, warm silence, real poll-drop/no-publication/
same-DICE recovery; root/command/request-kind A-B-A with held Result and epoch
Arcs; and zero later-owner/public activation.

Reuse Bazel 9.2 `BazelModuleResolutionFunction.discoverAndSelect`,
`Discovery.run/advanceHorizon` and `DiscoveryTest`, the accepted
materialization-request/effective-override tests and
`docs/developers/dice.md`. No new fixture or oracle is authorized.

Exact: existing request values/errors/order, normalized path/canonical
repository semantics, legacy Results and child events. Slug-native: the private
sibling/carrier, typed outer and epoch association. Unsupported/deferred:
materialization/source observation, registry patches, nonregistry closure,
discovered modules, selected graph, generated repositories, rules_rust
analysis/actions, M8/M7B and exact Bazel identity bytes.

Run focused owner tests, full bzlmod, affected loading/query/core baselines,
formatting, diff-check, exact accounting and AI-cleanup/Buck2 retention review.

## STOP and successor

STOP on any second Rust file/key/caller/export,
downstream activation, semantic or error drift, event/family change, retained
child carrier or scratch collection, direct Host read, new state, cap excess
or milestone closure. REPLAN rather than weaken exact request behavior,
discard an existing discriminator or fabricate proof.

After independent implementation ACCEPT, schedule only the docs-only
`WP-6-7A-selected-module-graph-observation-frontier-design`. Exactly one
immediate successor is authorized.
