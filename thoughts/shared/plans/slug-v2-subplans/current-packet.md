# Current Slug V2 Packet

Packet: `WP-6-7A-host-discovered-module-observation-implementation-retry`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Scheduling/proof correction: `b09d5e70`
Rust base: `223c8112`
Accepted semantic design: `b8e4cc03`

## Accepted correction and exact retry authority

The retained two-file candidate is production-sound. One private
`HostDiscoveredModuleObservationKey` and one shared Legacy/Observed driver
preserve effective selection followed by builtin, nonregistry closure or
registry preparation. Complete child epochs merge left-first before semantic
inspection; Need/typed outer is immediate and carrierless; builtin terminates
without discovery evaluation; nonregistry and registry alone own their matching
local MODULE evaluation batch. The carrier retains exactly one local Result Arc
plus a compact `PathObservationEpoch`; all evaluation, fragment, event and merge
scratch remains compute-local.

Independent terminal review found no production, ownership, order, event,
family, retention or cleanup defect. Against Rust base `223c8112`, the live
candidate measures +309 net and 16,811 physical in
`source_preparation.rs`, +809 proof and 8,040 physical in
`source_preparation_observation_tests.rs`, and +1,118 semantic/24,851 physical
aggregate. Six focused discovery tests and the complete 481-test bzlmod unit
suite plus integrations pass; formatting and diff hygiene pass.

The remaining 11 external proof lines cannot honestly discriminate all frozen
risks. Real branches still need explicit builtin override/compute/semantic,
nonregistry closure semantic/cycle, and registry missing-version/
`NonRegistryUnsupported` terminals with exact prefix and later suppression.
Proof must compare complete ordered child-to-discovery EventBatch sequences for
builtin, nonregistry and registry in both families, not only the final parent
batch. Preparation-epoch propagation and evaluated-MODULE changes also require
independent A-B-A cases with held Result/epoch handles; the effective held
carrier must be compared with the restored carrier. Compressing these into the
old ceiling would remove discriminating evidence. This is a proof-cap REPLAN,
not a semantic redesign or lower-owner prerequisite.

## Exact implementation authority

Write only the retained two-file Rust candidate. Every third Rust/test file,
fixture, oracle, plan document, caller and public export is read-only.

Freeze production and the exact two-file retry authority. Keep:

- `app/slug_bzlmod_v2/src/source_preparation.rs`, baseline 16,502 physical,
  <=360 production, <=40 colocated proof and <=16,950 physical; and
- `app/slug_bzlmod_v2/src/source_preparation_observation_tests.rs`, baseline
  7,231 physical, now <=1,200 proof and <=8,500 physical.

Aggregate cap is now <=1,600 semantic and <=25,450 physical. Helpers/tests stay
below 200 lines; `source_preparation.rs` remains the sole cohesive large-file
exception. No third file, export or caller is writable.

## Frozen retry contract

Preserve the frozen private key/carrier/typed outer, one Legacy/Observed driver,
effective -> builtin/nonregistry/registry order, left-first union before
semantics, first exact Arc on equal duplicates, typed conflict/operation
mismatch, carrierless Need/outer and exact legacy Result-Arc projection.
Effective compute has empty prefix; pure/effective/builtin terminals retain
effective; closure/preparation compute retains effective; and their semantic/
evaluation terminals retain the full effective+child prefix.

Builtin ends after validation plus neutral `BuiltinBazelToolsModuleKey`, with
no discovery evaluation or discovery-local batch. Only nonregistry/registry
evaluate MODULE bytes and publish exactly one matching local batch, including
empty/error under capture. Reached children retain sole ownership of their own
batches; child batches precede discovery and warm reuse is silent. Need/outer,
pre-evaluation terminals and cancellation publish no parent batch/state.

Retain only the local semantic Result Arc and compact epoch. Retain no child
carrier Arc, included-fragment collection, evaluator/event/merge scratch,
cache, interner, store, lock, task, direct Host read, revision or certificate.
Selected graph, repo-spec, extension and public callers remain inactive.

Preserve every existing identity, equality, validity, exact Arc, prefix,
family, event, warm, cancellation, lifecycle and upper-exclusion discriminator.
The retry adds only real builtin success, explicit-override and invalid-version
cases; real nonregistry closure-semantic/unsupported-cycle cases; reachable
registry missing-version and preparation/evaluation terminals; and production-
called discovery compute/semantic/invariant projector tests plus accepted lower
typed builtin-error proof for unreachable classes. No hook or inconsistent
child injection is permitted. It also adds exact complete ordered neutral-child/
discovery EventBatch sequences, legacy/observed parity, direct rows and reverse-
family isolation for all three branches including empty/error batches; and
independent effective, closure, preparation and evaluated-MODULE A-B-A
restoration with original/restored Result+epoch handles.

Exact compatibility remains current branch selection, values/errors/order,
legacy Result Arc and child/discovery events. Slug-native remains the private
carrier, typed outer and epoch association. Selected graph/repo specs/
extensions, broader bootstrap work, M8/M7B and exact identity bytes remain
deferred.

STOP any production semantic/event/family/memory change, wider authority, upper
activation, proof hook/waiver, cap excess or milestone closure. REPLAN before
widening. After independent implementation ACCEPT return only to the docs-only
selected-module-graph frontier. M7 remains partial and M7A -> M8 -> M7B remains.

## Historical owner decision and frozen implementation contract

Two independent owner audits select `HostDiscoveredModuleKey` as the uniquely
smallest complete next owner. It always computes effective selection first,
then chooses immutable builtin content, accepted nonregistry closure, or
accepted registry source preparation. It alone evaluates the selected MODULE
bytes and owns the local evaluation `EventBatch`. `HostSelectedModuleGraphKey`
only compute-joins these leaves into BFS/fixed-point horizons; observing the
graph first would reconstruct child epochs and move or duplicate discovery's
event authority. Selected repo-spec and extension consumers are later.

No smaller prerequisite remains: every mutable/path-producing discovery child
has an accepted observed sibling, while builtin content is immutable and
neutral.

This design packet is read-only for Rust, tests, fixtures, oracles, public APIs
and callers. Write only canonical, current, this Stage and routing. Net docs
caps are canonical <=40, current <=220, Stage <=180 and routing <=30, with
<=470 aggregate.

## Future exact Rust authority and shape

After independent design ACCEPT, authorize exactly:

- `app/slug_bzlmod_v2/src/source_preparation.rs`, baseline 16,502 physical,
  <=360 production, <=40 colocated proof and <=16,950 physical; and
- `app/slug_bzlmod_v2/src/source_preparation_observation_tests.rs`, baseline
  7,231 physical, <=820 proof and <=8,100 physical.

Aggregate cap is <=1,220 semantic and <=25,050 physical. Helpers/tests stay
below 200 lines; `source_preparation.rs` is the sole cohesive large-file
exception. Every third file, export and caller remains read-only.

Add private crate-visible
`HostDiscoveredModuleObservationKey(HostDiscoveredModuleKey)` and
`ObservedHostDiscoveredModule`, with distinct hash/Display identity, borrowed
accessors, `Dupe`/`Allocative`, no export or caller. Retain exactly one local
`Arc<Result<HostDiscoveredModule, HostDiscoveredModuleError>>` plus one compact
`PathObservationEpoch`.

Use one Legacy/Observed driver. Its observed value is
`SourcePreparationOutcome<Result<ObservedHostDiscoveredModule,
HostDiscoveredModuleObservationError>>`, where the private outer error
preserves the exact typed effective, closure or preparation frontier class.
Legacy projection moves the exact local Result Arc.

## Frozen order and algebra

Preserve exact order:

1. matching effective selection for every module;
2. for `bazel_tools`, explicit-override/version validation then neutral
   `BuiltinBazelToolsModuleKey`, then terminate without discovery evaluation
   or a discovery-local batch;
3. for a nonregistry override, empty-version validation then matching
   nonregistry closure;
4. otherwise missing-version validation then matching module-source
   preparation; and, only for nonregistry and registry branches,
5. owner-local MODULE evaluation and matching discovery batch publication.

Legacy selects only legacy effective/closure/preparation keys. Observed selects
only their accepted observed siblings. Builtin remains neutral. Merge every
observed Complete child epoch into the cumulative prefix left-first before
semantic inspection. Equal duplicates retain the earliest exact Arc; conflict
or operation mismatch is typed outer.

Effective Need/outer, closure Need/outer and preparation Need/outer are
immediate and carrierless, activate no later child/evaluation and store no
parent batch. This sequential owner performs no Need union. Effective DICE
compute error has empty prefix. Effective semantics and every pure validation
or builtin compute/semantic terminal retain the effective prefix.
Closure/preparation DICE compute errors retain effective only; their semantic
terminals, unsupported/cycle cases, evaluation errors and successes retain the
full effective+child prefix.

Need is invalid/self-unequal. Complete outer equality is outer-by-value and
Complete carrier equality is local semantic Result+epoch.

## Events, lifetime and proof

For nonregistry and registry, each legacy/observed discovery sibling remains
sole owner of its matching local MODULE evaluation batch. Reached effective/
closure/preparation descendants keep their existing child batches. Evaluation
success/error stores exactly one local batch, including empty when event capture
is enabled. Builtin terminates after its neutral child and stores no
discovery-local batch; other pre-evaluation terminals, Need/outer/cancel also
store none. Cold child batches precede discovery; warm reuse is silent.

Retain no effective/closure/preparation carrier Arc, included-fragment Vec,
logical-id/evaluator/event scratch, extra collection, cache, interner, store,
lock, task, direct Host read, revision or certificate. Existing provenance
inside the semantic Result remains exact; all other branch/merge scratch is
compute-local.

Prove key/hash/Display/accessors/`Dupe`/`Allocative`, Need/outer/carrier
equality and exact legacy Result-Arc projection. Table effective, builtin,
nonregistry and registry validation/Need/outer/DICE-compute/semantic/evaluation
positions with exact prefixes and later suppression. Prove duplicate first Arc,
conflict and operation mismatch.

Drive real builtin success/error; nonregistry version/closure/cycle/evaluation
terminals; registry missing-version/preparation/unsupported/evaluation
terminals. Prove builtin has zero discovery-local batch while preserving neutral
builtin child behavior. Prove exact cumulative epoch order and per-demand shared
Arcs, exact direct dependency rows, both family directions, exact nonregistry
and registry child/discovery batches including empty/error, parent ownership,
warm silence, poll-drop/no publication/same-DICE recovery, and independent
effective/closure/preparation and evaluated-MODULE A-B-A with held Result/epoch
handles. Assert zero selected-graph/repo-spec/extension/public activation.

Exact compatibility is current branch selection, values/errors/order, legacy
Result Arc and discovery/child events. Slug-native is the private carrier,
typed outer and epoch association. Selected graph/repo specs/extensions,
broader bootstrap work, M8/M7B and exact identity bytes remain deferred.

STOP a third file/export/caller, selected-graph activation, moved/duplicated
event ownership, semantic/family/order drift, weakened Arc/epoch association,
extra retained state, direct Host read, cap excess or milestone closure. If the
design cannot fit, REPLAN before widening.

After independent design ACCEPT schedule exactly
`WP-6-7A-host-discovered-module-observation-implementation`. Only after that
implementation ACCEPT return to the docs-only selected-module-graph frontier.
M7 remains partial and M7A -> M8 -> M7B remains.

## Historical second proof-cap REPLAN

The retained two-file implementation candidate is production-sound: one shared
Legacy/Observed semantic driver preserves effective -> source or policy ->
ordered registry attempts -> resolve-all patches -> per-resolution FileBytes
and immediate apply; all Complete epochs merge left-first before semantics;
Need/outer is carrierless; the parent is eventless; and retention is exactly
one local Result Arc plus the compact epoch. Independent review found no owner,
order, event, family, memory or cleanup defect.

The corrected retry still cannot be accepted under its proof ceiling. Measured
against Rust base `0f9a0559`, the live candidate is +367 net in
`source_preparation.rs` at 16,502 physical and +1,780 proof lines in
`source_preparation_observation_tests.rs` at 7,042 physical: +2,147 semantic and
23,544 physical aggregate. Focused new proof and full `slug_bzlmod_v2`
validation pass, but independent terminal review still finds four frozen
discriminators absent: distinct observed-key equality/hash, reconstruction of
the exact multi-patch cumulative epoch from the reached child carriers,
lockfile mode/content A-B-A, and patch symlink-retarget A-B-A. Only 20 external
proof lines remain, so forcing the old cap would replace lifecycle evidence
with nondiscriminating compression.

Formally REPLAN only to this docs-only proof correction. During correction the
two dirty Rust files are retained and non-writable. Write only canonical,
current, this Stage and the orchestration routing log; every Rust/test file,
fixture, oracle, caller and public export is read-only.

Freeze production and the two-file retry authority unchanged. Keep
`source_preparation.rs` at <=700 production, <=60 colocated proof and <=16,900
physical. Raise only `source_preparation_observation_tests.rs` to <=2,100 proof
and <=7,500 physical, making <=2,860 semantic and <=24,400 physical aggregate.
Helpers/tests remain below 200 lines.

The retry must add a distinct-key equality/hash discriminator; reconstruct the
effective -> policy -> decisive registry-file -> all reached resolutions ->
reached FileBytes cumulative epoch from the exact child carriers and compare
iteration order and shared Arcs; vary lockfile mode/content A-B-A; and vary a
patch symlink target A-B-A, retaining the original and restored Result/epoch
handles. Preserve every accepted compute/semantic projector, mismatch,
first/middle/last terminal, family, event, warm, cancellation, lifecycle and
upper-exclusion discriminator.

STOP any production semantic/event/family/memory change, third file, export,
caller, upper activation, proof waiver, cap excess or milestone closure. After
independent correction ACCEPT schedule exactly
`WP-6-7A-module-source-preparation-observation-implementation-retry-2`; only
after retry ACCEPT return to the docs-only selected-module-graph frontier.

## Owner decision and exact implementation authority

The selected-graph frontier audit selects `ModuleSourcePreparationKey` as the
uniquely smallest complete next owner. It owns effective selection and the
nonregistry/registry split; ordered registry search; every completed registry
file; and the owner-local two-phase root-patch pipeline. Accepted observed
siblings now cover effective selection, repository source, registry policy,
registry file and resolved path, while neutral `PathObservationKey` supplies
the exact FileBytes Arc. Patch resolution/application has no other semantic
consumer. `HostDiscoveredModuleKey` only consumes completed preparation and
separately owns MODULE evaluation/events; selected graph only joins discovery
horizons. No smaller prerequisite or upper owner is warranted.

Write authority is exactly:

- `app/slug_bzlmod_v2/src/source_preparation.rs`, baseline 16,135 physical:
  <=700 production, <=60 colocated proof and <=16,900 physical; and
- `app/slug_bzlmod_v2/src/source_preparation_observation_tests.rs`, baseline
  5,262 physical: <=2,100 proof and <=7,500 physical.

Aggregate semantic cap is <=2,860 and aggregate physical cap <=24,400. Helpers
and tests remain below 200 lines; the shared owner file is the sole cohesive
large-file exception. Every third file, export and caller is read-only.

## Frozen owner, order and algebra

Add private crate-visible
`ModuleSourcePreparationObservationKey(ModuleSourcePreparationKey)` and
`ObservedModuleSourcePreparation`, with distinct Display/hash identity,
borrowed accessors, `Dupe`/`Allocative`, no export/caller. Its value is
`SourcePreparationOutcome<Result<ObservedModuleSourcePreparation,
ObservedPathFrontierError>>`. Retain exactly one local
`Arc<Result<ModuleSourcePreparation, ModuleSourcePreparationError>>` plus one
compact cumulative `PathObservationEpoch`. Use one Legacy/Observed driver and
move the exact local Result Arc through legacy projection.

Preserve exact order: normalize workspace -> matching effective selection; then
either matching nonregistry repository source, missing-version terminal, or
matching registry policy -> override registry or policy registries in order ->
registry files in occurrence order. On Found, resolve every admitted main-repo
patch in declared-label order and retain reached resolutions in that order.
Then process each retained resolution in order as FileBytes -> immediate
cumulative patch application before continuing to the next patch.

Observed mode selects only observed effective/source/policy/registry-file/
resolved-path siblings; legacy selects only legacy siblings. FileBytes remains
neutral. Merge every Complete child epoch into the cumulative prefix left-first
before semantic inspection. Append each exact shared FileBytes Result Arc before
inspection. Equal duplicates retain the earliest Arc; conflict or operation
mismatch is typed outer. Any Need or child typed outer is immediate,
carrierless, activates no later child and performs no Need union.

Prefixes are exact: invalid workspace and effective compute are empty; effective
semantic and missing version retain effective; nonregistry source compute keeps
effective and its semantic result keeps effective+source. Policy compute keeps
effective; policy semantic keeps effective+policy. Registry-file compute keeps
the prior cumulative prefix; its semantic result includes the decisive file
epoch; all-miss includes every completed attempt. Invalid patch path keeps the
registry prefix. Resolution compute keeps prior; resolution semantic includes
the merged resolution. FileBytes compute keeps prior; Missing/Error/Present
includes the appended result. Patch parse/application errors and success retain
the full reached prefix.

## Events, lifetime and proof

Preparation siblings are eventless. Reached ROOT/MODULE/lockfile descendants
remain sole existing batch owners; discovery remains the later evaluation/event
owner. Need/outer/cancel stores no parent state and warm reuse publishes no new
batch.

Retain no child carrier Arc, resolved path, patch bytes/list, policy URL/search
scratch, extra collection, cache, interner, store, lock, task, direct Host read,
revision, certificate or event state. Existing semantic
`module_file_attempts` remains only inside the local Result; all other attempt
and merge scratch is compute-local.

Prove key/hash/Display/accessors/equality/validity and exact legacy Result-Arc
parity. Table every stage's Need/outer/compute/semantic prefix plus duplicate
first-Arc/conflict/mismatch. Drive real nonregistry Present/Absent/error,
override-registry and multi-registry NotFound->Found/all-miss, every registry
file terminal, and patch skip/invalid/missing/wrong-kind/resolution,
first/middle/last resolution and FileBytes Need/outer/compute/Missing/Error,
patch parse/apply error and later suppression. An early patch-application
failure must retain the prefix through that patch's FileBytes and suppress every
later FileBytes activation.

Prove exact direct dependency vectors and both family directions; exact child
batch order with parent silence/warm; poll-drop same-DICE recovery; independent
registry bytes, URL/mode/MODULE/lockfile and patch symlink/path/bytes A-B-A with
held Result/epoch handles; and zero discovery/selected/selected-repo/
HostRegistry/extension/public activation.

Exact compatibility is current values/errors, registry search/attempt order,
patch filtering/two-phase order/application and child events. Slug-native is the
private carrier, typed outer and epoch association. Discovery evaluation/events,
selected graph/repo specs, extensions, M8/M7B and exact identity bytes remain
deferred.

STOP a third file/export/caller, upper activation, reordered registry IO or
patches, semantic/event/family/memory drift, direct Host read, cap excess or
milestone closure. If this cannot fit, REPLAN before widening. After independent
implementation ACCEPT return only to the docs-only selected-module-graph
frontier for the discovery owner. M7 remains partial and M7A -> M8 -> M7B
remains.
