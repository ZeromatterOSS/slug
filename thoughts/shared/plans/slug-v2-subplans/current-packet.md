# Current Slug V2 Packet

Packet: `WP-6-7A-registry-policy-observation-implementation`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Rust base: `a4623d6b`
Accepted design: `8d00d44a`

## Objective and exact authority

Implement the accepted private observed sibling of `RegistryPolicyKey`. Exact
Rust write authority is only `app/slug_bzlmod_v2/src/registry_dice.rs`, baseline
1,413 physical lines, with <=200 production, <=520 proof, <=720 aggregate
semantic and <=2,200 physical. Touched helpers remain below 200 lines; the one
owner/proof file is a cohesive exception.

## Frozen implementation contract

Add private crate-visible `RegistryPolicyObservationKey(RegistryPolicyKey)` and
`ObservedRegistryPolicy`. Its Value is
`SourcePreparationOutcome<Result<ObservedRegistryPolicy,
ObservedPathFrontierError>>`. Retain exactly one local
`Arc<Result<RegistryPolicy, RegistryFileError>>` plus the compact root-files
`PathObservationEpoch`; require `Dupe`/`Allocative`, borrowed accessors, no
export and no caller activation. Legacy projection moves the exact driver Arc.

Use one Legacy/Observed driver in exact injected registry URLs -> injected
lockfile mode -> matching root-files order. Legacy computes only
`RootModuleFilesKey`; Observed computes only accepted
`RootModuleFilesObservationKey`. URL/mode DICE failures remain exact semantic
`MissingRegistryUrls`/`MissingLockfileMode` with empty epoch. Legacy root DICE
failure is semantic `RootModuleFiles` with empty epoch. Observed root Need/typed
outer is immediate carrierless; observed root DICE failure is semantic empty-
prefix; Complete root semantic failure/success retains the exact child epoch
unchanged before semantic inspection.

There is one path-bearing child: perform no epoch union and invent no parent
conflict/mismatch. Complete carrier equality is semantic Result+epoch; Complete
outer equality is outer by value; Need is invalid/self-unequal. Parent is
eventless; root descendants retain sole batches. Need/outer/cancellation stores
no parent state and warm reuse emits nothing.

Retain no root carrier Arc or extra collection/cache/interner/store/lock/task,
direct Host read, request revision, certificate or event state. URL/mode/root
outcomes and event scratch remain compute-local or dependency-owned.

## Required proof and compatibility

Prove distinct identity/hash/Display/accessors/`Dupe`/`Allocative`; equality and
validity; exact legacy Result-Arc projection and observed parity; every URL,
mode and root compute/semantic/success terminal; real root Need/outer and later
suppression; exact epoch iteration and per-demand Arcs; exact observed/legacy
rows and reverse isolation; child-owned cold events with parent silence, warm
suppression and poll-drop recovery; URL/mode/MODULE/lockfile A-B-A with held
handles; and zero RegistryFile/preparation/discovery/selected/HostRegistry/
extension/public activation. Run focused proof, full bzlmod and affected
loading/query/core baselines, fmt, diff-check, accounting and cleanup/retention.

Exact: current URL/mode/root order, policy values/errors, legacy Result Arc and
child events. Slug-native: private sibling, Result-Arc+epoch and typed outer.
Deferred: registry I/O/generation, patches, preparation, discovery/selected
graph, extensions, M8/M7B and identity bytes.

STOP a second file/key/caller/export, registry-file/preparation/discovery/
selected activation, semantic/event/family/memory drift, cap excess or milestone
closure. If the accepted contract cannot fit, REPLAN. After independent ACCEPT,
return only to the docs-only selected-module-graph frontier audit.
