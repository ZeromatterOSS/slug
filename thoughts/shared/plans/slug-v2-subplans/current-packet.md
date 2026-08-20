# Current Slug V2 Packet

Packet: `WP-6-7A-registry-policy-observation-design`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Scheduling and Rust base: `a4623d6b`

## Objective and exact authority

Freeze the uniquely smallest complete registry-prefix owner: a private observed
sibling of `RegistryPolicyKey`. Do not implement it or activate registry files,
module preparation, discovery or selected graph in this packet.

Write authority is exactly:

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`, <=40 net lines;
- this manifest, <=220 net lines;
- `thoughts/shared/plans/slug-v2-subplans/06-analysis-toolchains-and-actions.md`,
  <=180 net lines;
- `.codex/skills/slug-agent-orchestration/references/routing-log.md`, <=30 net
  lines.

Aggregate docs growth is <=470 net lines. Rust, Cargo, BUILD, fixtures, oracles,
callers and public files are read-only.

## Owner and carrier

`RegistryPolicyKey` owns the exact injected registry-URL -> injected lockfile-
mode -> root-MODULE-files policy projection. `RegistryFileKey` consumes it in
both local and remote branches, while `ModuleSourcePreparationKey` consumes it
before the ordered registry attempts. Observing either larger owner first would
duplicate this shared prefix.

Add private crate-visible `RegistryPolicyObservationKey(RegistryPolicyKey)` and
`ObservedRegistryPolicy`. Its Value is
`SourcePreparationOutcome<Result<ObservedRegistryPolicy,
ObservedPathFrontierError>>`. The carrier retains exactly one local
`Arc<Result<RegistryPolicy, RegistryFileError>>` plus one compact
`PathObservationEpoch`. Require `Dupe`/`Allocative`, borrowed result/observation
accessors, no export and no caller activation. Legacy projection moves the exact
local Result Arc.

Use one Legacy/Observed driver. Preserve exact order: neutral injected
`RootModuleRegistryUrlsKey`, neutral injected `RootModuleLockfileModeKey`, then
matching root-files family. Legacy selects only `RootModuleFilesKey`; Observed
selects only accepted `RootModuleFilesObservationKey`. Neither sibling computes
the other family.

## Terminal, event and retention contract

Registry-URL and lockfile-mode DICE failures remain the exact semantic
`MissingRegistryUrls` and `MissingLockfileMode` results with an empty epoch and
suppress root files. Legacy root-files DICE failure remains semantic
`RootModuleFiles` with empty epoch.

Observed root Need and typed outer return immediately carrierless. Observed
root-files DICE compute failure remains semantic `RootModuleFiles` with empty
epoch. For every Complete observed root terminal, accept its exact epoch before
semantic inspection. Root semantic failure and successful policy projection
retain that epoch unchanged. There is only one path-bearing child, so this owner
performs no epoch union and creates no parent conflict/operation-mismatch class.

Complete carrier equality is semantic Result plus epoch; Complete outer equality
is outer by value; Need is invalid and self-unequal. The policy parent is
eventless. Reached root MODULE/lockfile descendants remain sole owners of their
existing batches; Need, typed outer and cancellation publish no parent state,
and warm reuse emits nothing new.

Retain only the existing local policy semantic Result Arc—which naturally owns
its URL list, lockfile mode and visible-lockfile projection—plus the compact
root epoch. The root carrier Arc, temporary URL/mode/root outcomes and event
scratch stay compute-local or dependency-owned. Add no collection, cache,
interner, store, lock, task, direct Host read, request revision, certificate or
event state.

## Future implementation and proof

After independent design ACCEPT, exact future Rust authority is only
`app/slug_bzlmod_v2/src/registry_dice.rs`, baseline 1,413 physical lines, with
<=200 production, <=520 proof, <=720 aggregate semantic and <=2,200 physical.
Touched helpers remain below 200 lines; the owner/proof file is one cohesive
exception.

Proof must discriminate:

- distinct key identity/hash/Display, accessors, `Dupe`/`Allocative`, Complete
  equality/validity, Need invalid/self-unequal and outer equality;
- exact legacy semantic Result-Arc projection and observed result parity;
- registry-URL, lockfile-mode and root-files DICE-compute failures, root semantic
  failure and success with exact empty/full prefixes and later suppression;
- real root Need/typed outer carrier polarity and zero retained carrier;
- exact observed and legacy direct-dependency rows, neutral URL/mode children,
  matching root-files family isolation and unchanged epoch iteration/per-demand
  `Arc::ptr_eq`;
- cold child-owned MODULE/lockfile batches with parent silence, exact legacy
  event parity, warm suppression and poll-drop cancellation/same-DICE recovery;
- URL, lockfile-mode, MODULE and visible-lockfile A -> B -> A lifecycles with
  held Result/epoch handles and restored semantic equality;
- zero `RegistryFileKey`, `ModuleSourcePreparationKey`, `HostDiscoveredModuleKey`,
  `HostSelectedModuleGraphKey`, `HostRegistryFunctionKey`, extension or public
  activation, exact cap accounting, fmt/diff and cleanup/retention review.

## Compatibility, terminal and STOP

Exact: current registry URL/mode/root-files order, `RegistryPolicy` values and
errors, legacy Result Arc and child events. Slug-native: private observed sibling,
Result-Arc+epoch carrier and typed outer. Unsupported/deferred: registry file
I/O and generation, root patches, source preparation, discovery/selected graph,
extension-generated repositories, M8/M7B and exact identity bytes.

Return exactly one terminal: independent design ACCEPT scheduling only
`WP-6-7A-registry-policy-observation-implementation`, or formal REPLAN. STOP
Rust/tests/oracles, a second file/key/caller/export, registry-file/preparation/
discovery/selected activation, semantic/event/family/memory drift, cap excess,
compatibility widening or milestone closure. After implementation ACCEPT,
return only to the docs-only selected-module-graph frontier audit.
