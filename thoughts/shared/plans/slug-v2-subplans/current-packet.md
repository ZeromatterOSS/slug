# Current Slug V2 Packet

Packet: `WP-6-7A-repository-materialization-request-observation-design`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Scheduling and Rust base: `3d174006`
Result: formal REPLAN from the selected-module-graph frontier to the uniquely
smaller repository-materialization-request observation owner.

## Accepted predecessor and learned facts

Implementation `3d174006` accepts the private effective-module-override
sibling. Its matching-family driver forwards the exact root MODULE-files epoch,
preserves command-policy precedence and legacy Result Arc behavior, remains
eventless, and retains one local Result Arc plus one compact epoch. Final
accounting against `a3efa1b7` is +175 production/+420 tests/+595 aggregate at
6,647 physical lines. Focused proof, full bzlmod/loading/query validation and
the established single inherited core visibility-wording baseline pass the
accepted gate; formatting, diff hygiene, retention cleanup and independent
review are clean.

The resumed live trace shows that `HostSelectedModuleGraphKey` still reaches
`HostDiscoveredModuleKey`, whose registry and nonregistry paths cross
`ModuleSourcePreparationKey` and `HostNonregistryModuleClosureKey`.
The nonregistry closure/source path computes `RepositoryMaterializationKey`,
which first computes the carrierless `RepositoryMaterializationRequestKey`.
Registry discovery uses `ModuleSourcePreparationKey`; a complete observed
sibling for that shared preparation owner must also preserve its existing
nonregistry branch, which would otherwise duplicate or bypass the request
projection. The request key is therefore the smallest reusable owner of
workspace, effective override, canonical repository identity and
local/immutable request kind. The builtin discovery branch adds no Host path
observation.

Bazel 9.2 `BazelModuleResolutionFunction.discoverAndSelect`,
`Discovery.run/advanceHorizon` and `DiscoveryTest` remain the exact
discovery/order/override evidence. Slug's existing materialization-request and
effective-override tests are the discriminating local evidence; no new oracle
or fixture is justified. DICE ownership, dependency recording, cancellation
and equality follow `docs/developers/dice.md`.

## Docs authority and future Rust envelope

This design packet may write only:

1. `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`;
2. `thoughts/shared/plans/slug-v2-subplans/current-packet.md`;
3. `thoughts/shared/plans/slug-v2-subplans/06-analysis-toolchains-and-actions.md`;
4. `.codex/skills/slug-agent-orchestration/references/routing-log.md`.

Docs caps are <=40 canonical, <=180 current, <=160 Stage 6 and <=30 routing
net lines, <=410 aggregate. Rust, tests, fixtures, oracles, Cargo/BUILD metadata
and Stage 10 are read-only during design.

After independent design ACCEPT, future implementation authority is exactly
`app/slug_bzlmod_v2/src/source_preparation.rs` from the 13,747-line
`3d174006` baseline: <=180 production, <=320 tests, <=500 aggregate semantic
and <=14,300 physical lines. The file is a cohesive large-owner exception;
every touched helper remains below 200 lines. No second Rust file is authorized.

## Frozen owner and terminal algebra

Add one private structural
`RepositoryMaterializationRequestObservationKey(RepositoryMaterializationRequestKey)`
and one private observed carrier containing exactly one local
`Arc<Result<RepositoryMaterializationRequest, RepositoryMaterializationError>>`
plus one compact `PathObservationEpoch`, with `Dupe` and `Allocative`.
Expose only crate-local constructor and borrowed result/epoch accessors needed
by the later materialization sibling.

Use one Legacy/Observed driver. It first normalizes the workspace, then selects
only matching `HostEffectiveModuleOverrideKey` or
`HostEffectiveModuleOverrideObservationKey`, and finally runs one pure shared
projection for nonregistry override selection, canonical repository identity,
local/command path policy and local versus immutable request kind. Legacy moves
the exact local Result Arc. Neither sibling computes the other.

Invalid workspace and effective-override DICE compute failure are semantic
Complete with an empty epoch. Observed effective Need or typed outer returns
immediately with no carrier. A completed effective semantic error retains the
full effective prefix. Missing/unsupported override, invalid canonical
repository, request-kind/spec errors and local/immutable success also retain
the full effective prefix. Merge/accept the completed child epoch before
semantic inspection; no joined batch or Need union exists at this sequential
owner.

Need is invalid and self-unequal. Complete typed outer is valid/equal by outer
value. Complete carrier is valid/equal by semantic Result plus complete epoch.
Stable shared-epoch construction must preserve the child's exact Result Arcs.
The parent is eventless on success, semantic failure, Need, outer and
cancellation; root MODULE children remain sole event owners.

## Retention, request behavior and proof

Retain only the local request Result Arc plus compact epoch. The effective child
carrier, normalized workspace temporary, canonical-name formatting, request
kind and projection scratch remain compute-local. Add no collection, cache,
store, interner, lock, task, direct Host read, revision, certificate or event
owner. Downstream materialization result injection, repository Needs and
request/session publication remain unchanged and unactivated.

Proof must discriminate:

- distinct identity/Display, crate-private construction/access, `Dupe` and
  `Allocative`;
- exact legacy request/error/value and Result-Arc projection;
- exact effective epoch membership and per-demand `Arc::ptr_eq`;
- empty invalid-workspace/effective-compute prefixes and full
  effective-semantic/missing/unsupported/canonical/spec/success prefixes;
- root-local, command-absolute, http/git immutable and malformed request-kind
  behavior;
- Need and typed outer validity/equality/no carrier and later nonactivation;
- both family directions, including exact parent dependency rows;
- child-owned cold event order, eventless parent, warm suppression, real
  poll-drop/no-publication/same-DICE recovery;
- root/command override and request-kind A-B-A restoration with held Result and
  epoch Arcs; and
- zero materialization-result/source, closure, preparation, discovery,
  selected-graph, extension, analysis or public activation.

## Compatibility, STOP and successor

Exact: existing request values/errors/order, normalized path and canonical
repository semantics, legacy Results and child events. Slug-native: the private
sibling/carrier, typed outer and epoch association. Unsupported/deferred:
materialization/source observation, registry patches, nonregistry closure,
discovered modules, selected graph, extension/generated repositories,
rules_rust analysis/actions, M8/M7B and exact Bazel identity bytes.

STOP on Rust during design; any second file, caller/export, materialization or
source activation, semantic/error/event/family drift, retained child carrier or
scratch collection, direct Host read, new state, cap excess or milestone
closure. REPLAN rather than weaken exact request semantics or fabricate proof.

After independent design ACCEPT, schedule exactly one bounded
`WP-6-7A-repository-materialization-request-observation-implementation`.
After implementation ACCEPT, return only to the docs-only
`WP-6-7A-selected-module-graph-observation-frontier-design`.
