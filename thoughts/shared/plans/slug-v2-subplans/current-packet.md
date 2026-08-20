# Current Slug V2 Packet

Packet: `WP-6-7A-host-selected-extension-definition-load-requests-observation-proof-cap-correction-design`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: 06-analysis-toolchains-and-actions.md
Scheduling base: 030f612a
Rust base: 2e0a19ae
Accepted design: d86c9a59
Implementation scheduling: 030f612a

## Formal proof-cap REPLAN

The implementation candidate preserves the accepted private one-child owner,
shared Legacy/Observed driver, mapping-first order, terminal polarity, event
ownership and compact Result-Arc+epoch retention. Independent review found no
production-semantic or ownership defect.

The frozen proof cannot honestly fit the original <=420 proof ceiling. The live
candidate measures +179 production/+398 proof/+577 aggregate semantic at 10,608
physical lines. Only 22 proof lines remain, while the required discriminators
need compact but real upper-family, equality/lifecycle and pure-terminal cases.

This correction is docs-only. Write authority is exactly canonical/current/
Stage 6/routing at net caps <=40/<=220/<=180/<=30 and <=470 aggregate. The dirty
Rust candidate is retained and non-writable during correction.

## Corrected retry authority

The retry remains limited to
app/slug_bzlmod_v2/src/selected_repo_spec.rs from Rust base 2e0a19ae and
accepted design d86c9a59. Keep production <=180 unchanged; raise proof only to
<=560, aggregate semantic to <=740 and physical to <=10,900 lines. Every other
file remains read-only and every helper/test remains below 200 lines.

Freeze production exactly as reviewed: one private callerless request
observation key/carrier; matching mapping child first; observed Complete epoch
accepted before semantics; carrierless Need/outer; empty mapping-compute prefix;
full mapping-semantic and pure-terminal prefix; exact legacy local Result Arc;
eventless parent; and retention of only the local Result Arc plus compact epoch.

## Missing proof only

Add compact discriminators that:

- scan the literal `host-bzl-module:`, `observed-host-bzl-module:`,
  `host-loaded-module-extension-definitions:`,
  `host-selected-extension-evaluation-inputs:`,
  `host-prepared-module-extension-inputs:`,
  `host-pure-module-extension-invocations:`,
  `host-instantiated-module-extension-repositories:`,
  `host-validated-module-extension-repositories:`,
  `host-root-repository-mapping:`,
  `host-canonical-selected-module-definition:`,
  `host-generated-repository-definition:` and `slug-command:` prefixes on
  success, error, Need, cancellation and recovery;
- prove equal request semantics with a changed mapping epoch compare unequal,
  separately from a pure request-result A -> B -> A under a held epoch, with
  held Result/epoch handles and unaffected exact per-demand Arcs;
- compose accepted reachable Unsupported/Invalid/InvalidContext pure
  projections through the production completion/driver boundary and prove the
  complete mapping epoch's exact demand order and shared Result Arcs; and
- preserve already passing family rows, complete child owner/batch parity,
  parent/warm silence, Need/outer and poll-drop recovery proof.

Do not add invalid-child hooks or oracle evidence.

## Correction terminal

Correction ACCEPT schedules only
`WP-6-7A-host-selected-extension-definition-load-requests-observation-implementation-retry`.
STOP production semantic/event/family/memory changes, a second file/key,
lib.rs/caller/export or upper activation, proof waiver/cap excess, milestone
closure or M8/M7B widening. REPLAN before widening. Only after retry ACCEPT
resume the docs-only definition/evaluation frontier. M7 remains partial and
M7A -> M8 -> M7B remains.



## Historical frozen implementation authority

Accepted design d86c9a59 selects
HostSelectedExtensionDefinitionLoadRequestsKey as the uniquely smallest
remaining reusable association. It is a semantic projection owner, not the
actual .bzl loader: it consumes selected extension mappings and produces the
ordered definition requests reused independently by
HostSelectedExtensionEvaluationInputRequestsKey and
HostLoadedModuleExtensionDefinitionsKey.

HostLoadedModuleExtensionDefinitionsKey also computes HostBzlModuleEvalKey for
each request. The accepted HostBzlModuleObservationKey already owns the complete
source/recursive-load epoch and local event batch, so no smaller Bzl observation
producer is missing. Observing the loader first would still lack the reusable
request association and would duplicate it for evaluation-input construction.

HostRootRepositoryMappingKey is a parallel public projection consumed by core
generated-repository mapping lookup. It does not load definitions or join
extension evaluation inputs. Defer its observed/public propagation rather than
combining that public boundary with this smaller prerequisite.

Keep the new request observation key/carrier private and callerless. Do not edit
lib.rs or export it now. A later independently designed loading-owner packet
must decide the single cross-crate handoff: promote/re-export this already
proved carrier and combine it with HostBzlModuleObservationKey. Premature export
would widen the Rust API before a consumer is authorized.

## Exact implementation authority

Future Rust write authority is exactly
app/slug_bzlmod_v2/src/selected_repo_spec.rs, baseline 10,031 physical lines
with first #[cfg(test)] at line 4,019. Caps are <=180 production, <=420 proof,
<=600 aggregate semantic and <=10,700 physical lines. Every helper/test remains
below 200 lines; the existing selected-repository pipeline remains one cohesive
large-file exception.

Every other Rust file, including app/slug_bzlmod_v2/src/lib.rs and all
slug_loading_v2/slug_core_v2 files, is read-only. Do not add a caller, export,
fixture or oracle.

## Frozen owner contract

Add private HostSelectedExtensionDefinitionLoadRequestsObservationKey wrapping
the legacy key and private ObservedHostSelectedExtensionDefinitionLoadRequests.
The carrier derives Dupe/Allocative, provides borrowed accessors and retains
exactly one local
Arc<Result<HostSelectedExtensionDefinitionLoadRequests,
HostSelectedExtensionDefinitionLoadRequestsError>> plus the exact compact
mapping epoch.

Use one Legacy/Observed semantic driver:

1. compute the matching selected-extension-mappings child;
2. for an observed Complete child, accept its exact epoch before inspecting its
   semantic Result; and
3. run the unchanged ordered definition-load-request projection.

Legacy selects only HostSelectedExtensionMappingsKey and contributes an empty
epoch. Observed selects only the accepted private mapping observation sibling.
Mapping Need/typed outer is immediate and carrierless. Mapping DICE-compute
failure is the existing semantic MappingsCompute terminal with an empty prefix.
Mapping semantic error and every Unsupported/Invalid/InvalidContext/success
projection retain the complete mapping prefix. There is no epoch union, Need
union, Bzl load or root-files/evaluation step. Legacy projection moves the
exact local Result Arc.

Need is invalid/self-unequal. Complete semantic values compare by local
Result+epoch; typed outers compare structurally. Equal semantics with changed
epoch remain unequal.

## Events, families and retention

The request parent remains eventless. The mapping child and its accepted lower
root/graph/discovery descendants retain exact batch ownership/order. Observed
direct rows contain only observed mappings; legacy rows contain only legacy
mappings. Warm reuse is silent and poll-drop publishes no request value/batch.

Retain only the local request Result Arc plus the compact mapping epoch. The
existing semantic Result may continue to own its predecessor mapping semantic
Arc, request slice, imports and overrides. Do not retain the child mapping
carrier Arc separately. Seen/namespace SmallMaps, request Vec, projection and
event scratch remain compute-local. Add no cache/interner/store/lock/task,
direct Host read, revision/certificate or retained event state.

## Required proof

Prove distinct key/hash/Display/accessors and Need/Complete equality/validity;
exact legacy local Result-Arc projection; production-used mapping child
finisher and compute/semantic projectors for Need, typed outer, empty/full
prefixes and later suppression; every reachable pure request projection error
and success with exact mapping epoch Arcs; exact observed/legacy direct rows,
reverse-family isolation, complete child owner/batch parity, parent silence and
warm silence; real poll-drop/no publication/same-DICE recovery; independent
mapping and pure-request A -> B -> A with held Result/epoch handles; and zero
Bzl loading, evaluation-input, root-mapping, prepared-input, extension
evaluation, generated-repository and public/bootstrap activation.

Reuse accepted lower proof. Add no invalid-child hook or oracle.

## Compatibility and terminal

Exact: current admitted definition-request values, errors, ordering, namespaces,
imports/overrides and child events. Slug-native: the private carrier, typed
outer and shared-Arc mapping epoch association. Deferred: carrier export,
definition Bzl loading, evaluation-input/root-files joins, root mapping,
prepared/evaluated extensions, generated repositories, public/bootstrap,
M8/M7B and exact identity bytes.

Implementation ACCEPT returns only to the docs-only extension
definition/evaluation frontier.
STOP a second Rust file/key, lib.rs or caller/export activation, Bzl/evaluation/
root-mapping/generated/public work, semantic/event/family/memory drift,
proof/cap waiver, milestone closure or M8/M7B widening. REPLAN before widening.
M7 remains partial and M7A -> M8 -> M7B remains.

## Historical accepted extension frontier audit

## Accepted selected-module-routes completion

Implementation `9d2f7a7d` from Rust base `ccf7421e` and accepted design
`a27eb1b0` completes the private selected-module-routes observation owner in
`selected_repo_spec.rs`. One Legacy/Observed driver preserves graph first,
repo specs second and unchanged canonical identity, repository mapping,
registry association and route projection.

Complete graph and repo-spec epochs merge immediately left-first before
semantics, so the direct graph Arc remains authoritative for equal duplicate
demands. Graph/repo child and merge outers remain typed and carrierless. Graph
compute/semantic and repo-spec compute/semantic terminals preserve their exact
empty/graph/full prefixes; every pure route terminal retains the full prefix.

The DICE value retains exactly one local route Result Arc plus one cumulative
compact epoch. Canonical/mapping SmallMaps, traversal, merge, terminal and event
scratch remain compute-local. The parent is eventless and accepted child batch
ownership/order remains exact.

Accepted accounting against `ccf7421e` is +256 production/+587 proof/+843
aggregate at 9,265 physical lines, within every frozen cap. Focused route proof
passes 6 tests; the full bzlmod suite passes 514 unit tests plus every
integration/doc target. Formatting, diff hygiene, cleanup/retention, security
and independent terminal review pass.

## Audit authority

This packet is docs-only. Write authority is exactly canonical/current/Stage 6/
routing at net caps <=40/<=220/<=180/<=30 and <=470 aggregate. Rust, tests,
fixtures, oracles, exports and callers are read-only. The ordinary ACCEPT
rollover changes canonical/current/Stage only; routing remains unchanged unless
the audit reaches a formal REPLAN or another reusable routing lesson.

## Frontier question

Trace the accepted route carrier through `HostSelectedExtensionMappingsKey`,
root extension usages and override/mapping projection, then through extension
definition/load/evaluation owners, generated-repository definitions and
public/bootstrap consumers only far enough to identify the uniquely smallest
complete remaining mutable frontier.

Do not presume extension mappings are the next owner. Check whether any
carrierless extension-definition load, module-extension evaluation, generated
repository or lockfile association is a strictly smaller reusable prerequisite.
Do not invent an umbrella owner or reopen accepted graph, Host-registry,
repo-spec or route owners for structural uniformity.

For each candidate, identify:

- the natural DICE semantic owner and all production consumers;
- exact Legacy/Observed child order and terminal precedence;
- which complete child epochs and Result Arcs exist and which edge remains
  carrierless;
- event ownership, exact batch order and whether the candidate parent is
  eventless;
- retained versus compute-local maps, mappings, loads, evaluation scratch and
  generated-repository state;
- Need/typed-outer/cancellation/warm behavior and A -> B -> A held-carrier
  lifecycles; and
- whether accepted lower proof composes or one uniquely smaller evidence/
  association prerequisite is necessary.

## Compatibility and terminal

Preserve admitted Bazel 9 extension mapping/load/evaluation values, errors,
order and events as exact. Private typed-outers/shared-Arc epoch association is
Slug-native. Generated/public/bootstrap breadth, M8/M7B and exact identity bytes
remain deferred unless live evidence proves one is the uniquely smaller M7A
prerequisite.

Reach exactly one terminal:

1. one independently reviewed smallest-owner design;
2. one uniquely smaller bounded evidence/association prerequisite; or
3. formal REPLAN.

A design may name at most one implementation successor. STOP Rust/test/oracle/
caller/export work, speculative extension or public activation, umbrella
ownership, milestone closure, M8/M7B work or bypassing accepted lower carriers.
M7 remains partial and M7A -> M8 -> M7B remains.

## Historical accepted route implementation packet

## Accepted selected registry repo-spec completion

Implementation `ccf7421e` adds the private selected registry repo-spec
observation owner in `selected_repo_spec.rs`. One Legacy/Observed driver
preserves selected graph first, graph-occurrence order, owner-local
root/nonregistry `None`, and registry Host-registry -> source JSON -> optional
registry metadata -> effective override -> augmentation order. The aggregate
retains the first typed outer while attempting every later Complete merge and
preserves outer -> semantic error -> incompatible Need -> compatible Need union
-> ordered success precedence.

The DICE value retains exactly one local semantic Result Arc plus one cumulative
compact epoch. Traversal, per-entry, merge, terminal and event state remains
compute-local; no child carrier, cache/interner/store/lock/task/direct Host read,
revision or certificate state is retained. The parent remains eventless and
accepted child batches retain exact ownership and order.

Accepted accounting against `e155d74f` is +492 production/+1,100 proof/+1,592
aggregate at 8,422 physical lines, within every frozen cap. Focused repo-spec
validation passes 42 unit and 3 integration tests; the full bzlmod suite passes
509 unit tests plus every integration/doc target. Formatting, diff hygiene,
cleanup, retention, security and independent terminal review pass.

## Owner decision

`HostSelectedModuleRoutesKey` is the uniquely smallest complete remaining owner.
It alone owns exact selected graph -> selected registry repo specs -> canonical
identity/repository mapping/route projection. The repo-spec Result deliberately
does not retain the graph, so routes must compute both accepted children.

Routes are independently consumed by `HostCanonicalSelectedModuleDefinitionKey`
and `HostSelectedExtensionMappingsKey`. The latter adds root extension usages,
overrides and mapping semantics. Observing extension mappings first would
duplicate route ownership. Canonical lookup and mapping helpers are pure
owner-local code with no independent mutable edge or semantic consumer.

## Exact implementation authority

Write only `app/slug_bzlmod_v2/src/selected_repo_spec.rs`, baseline 8,422
physical with first `#[cfg(test)]` at line 3,466. Caps are <=300 production,
<=600 proof, <=900
aggregate semantic and <=9,400 physical lines. Every helper/test remains below
200 lines. The existing large file remains cohesive because it already owns the
selected repository graph -> repo-spec -> route -> extension projection; do not
split or add a second file/key merely for size.

## Frozen implementation contract

Add private `HostSelectedModuleRoutesObservationKey(HostSelectedModuleRoutesKey)`
and `ObservedHostSelectedModuleRoutes`. The carrier derives `Dupe`/`Allocative`,
provides borrowed accessors and retains exactly one existing
`Arc<Result<HostSelectedModuleRoutes, HostSelectedModuleRoutesError>>` plus one
cumulative compact `PathObservationEpoch`. Add no caller or export.

Use one Legacy/Observed semantic driver. Legacy selects only legacy graph and
repo-spec children with empty epochs. Observed selects only
`HostSelectedModuleGraphObservationKey` followed by
`HostSelectedRegistryRepoSpecsObservationKey`. Both modes share the unchanged
canonical lookup, repository mapping, registry association and route projection
and legacy moves the exact local Result Arc.

Merge every Complete child epoch immediately left-first and before semantic
inspection. Merge the direct graph prefix before the repo-spec cumulative epoch,
so equal graph demands already contained in repo specs retain the direct graph's
exact Arc. Conflict or operation mismatch is typed outer. Use a private outer
that distinguishes Graph child, RepoSpecs child and Graph/RepoSpecs merge stage.

Graph Need/typed outer is immediate and carrierless and suppresses repo specs.
Graph DICE-compute failure is semantic with an empty epoch. Graph semantic error
retains the graph epoch and suppresses repo specs. Repo-spec Need/typed outer is
immediate and carrierless. Repo-spec DICE-compute failure retains only the
already-complete graph prefix because no repo-spec carrier exists. Repo-spec
semantic error retains the merged graph+repo-spec prefix. Canonical collision,
mapping invalidity, registry mismatch and success retain that same full merged
prefix. There is no Need union or batch fold at this sequential owner.

Complete equality is local semantic Result+epoch; Need is invalid/self-unequal
and typed outer compares by value. The parent remains eventless. Graph/discovery
descendants retain exact event ownership/order and repo specs remain eventless.
Warm reuse is silent; poll-drop publishes no accepted parent state and recovery
uses the same DICE.

Retain only the route Result Arc+compact epoch. Canonical/mapping `SmallMap`s,
route traversal, child carrier, merge, terminal and event scratch remain
compute-local. Add no retained collection/cache/interner/store/lock/task/direct
Host read/revision/certificate/event state.

## Required proof

Prove distinct key/hash/Display, accessors, `Dupe`/`Allocative`, Complete/Need/
outer equality and exact legacy Result-Arc projection. Drive production child
finishers and compute projectors through every graph/repo-spec Need, typed outer,
DICE-compute and semantic position with exact empty/graph/full prefixes and
later suppression.

Prove graph-first duplicate exact-Arc preservation, conflict and operation
mismatch. Exercise exact root/nonregistry/registry route values and order,
canonical collision, mapping invalidity and every registry mismatch. Compare
exact observed/legacy direct dependency vectors, reverse family isolation and
the complete ordered child EventBatch sequence with zero parent batch, warm
silence and real poll-drop/no-publication/same-DICE recovery.

Independently vary graph, repo-spec and pure route/mapping inputs through
A -> B -> A while holding Result/epoch handles and asserting unaffected child
Arcs remain exact. Prove zero canonical-definition, extension mapping/load/
evaluation, generated-repository, public and bootstrap activation. Unreachable
DICE/outer classes use production projectors plus accepted lower proof; do not
add hooks or inject inconsistent child values.

## Compatibility and terminal

Existing admitted route values/errors/order/events remain exact. The private
typed outer/shared-Arc epoch association is Slug-native. Extension/generated/
public/bootstrap breadth, M8/M7B and exact identity bytes remain deferred.

STOP a second key/file, caller/export, changed route/event semantics, retained
mapping/traversal state, upper activation, proof waiver, cap excess or milestone
closure. REPLAN before widening. After independent implementation ACCEPT resume
only the docs extension frontier. M7 remains partial and M7A -> M8 -> M7B
remains.

## Historical accepted design: owner decision

`HostSelectedRegistryRepoSpecsKey` is the uniquely smallest complete remaining
owner. It computes the accepted selected graph first, scans `graph.resolved` in
order, and alone owns the registry-only Host-registry -> source JSON registry
file -> parse -> optional registry JSON file -> module projection -> effective
override -> augmented repo-spec sequence. Root and nonregistry entries are its
owner-local `None` terminals.

The entry computation has no other consumer, so inventing a per-entry DICE key
would retain an artificial intermediate owner. The sole production consumer is
`HostSelectedModuleRoutesKey`, which adds separate route semantics; routes,
extensions and public/bootstrap consumers are later and cannot absorb this
missing aggregate epoch.

## Exact design authority

Write only canonical, current, this Stage and the orchestration routing log, at
net caps <=40/<=220/<=180/<=30 and <=470 aggregate. Rust, tests, fixtures,
oracles, exports and callers remain read-only until independent design ACCEPT.

The sole future Rust authority is
`app/slug_bzlmod_v2/src/selected_repo_spec.rs`, baseline 6,830 physical with
first `#[cfg(test)]` at line 2,974. Permit <=520 production, <=1,100 proof,
<=1,620 aggregate semantic and <=8,500 physical lines. Helpers/tests remain
below 200 lines; no second file, key, export or caller is writable.

Add private crate-visible `HostSelectedRegistryRepoSpecsObservationKey` and
`ObservedHostSelectedRegistryRepoSpecs`. The carrier derives `Dupe` and
`Allocative`, has borrowed accessors, and retains exactly one existing
`Arc<Result<HostSelectedRegistryRepoSpecs,
HostSelectedRegistryRepoSpecsError>>` plus one cumulative compact
`PathObservationEpoch`.

Use one Legacy/Observed semantic driver. Legacy selects only the legacy graph,
Host-registry, registry-file and effective siblings and contributes empty
epochs. Observed selects only the accepted matching siblings and their exact
epochs. Both modes share all parse, projection, entry and aggregate semantics
and legacy projection moves the exact local Result Arc.

## Exact order and aggregate algebra

Compute the selected graph first. Then visit every graph occurrence in order.
Root and nonregistry occurrences complete locally with `None`; each registry
occurrence computes, in order, matching Host registry, source JSON registry
file, its parse/projection, conditional registry JSON registry file, module
hash/pure projection, matching effective override and final augmentation.
Within one entry, any terminal suppresses its later children exactly as today.

Merge every observed Complete child epoch immediately, left-first and before
semantic inspection: graph first, then each entry and each child in the order
above. Equal duplicate demands preserve the earliest exact Arc. Conflicting
values or operation mismatch are typed outer. Continue the existing full
cross-entry scan after an entry semantic error or Need; its valid cumulative
prefix remains compute-local so a later higher-precedence terminal or the final
Complete result contains every Complete sibling epoch actually reached.

Use a private stage-aware outer that distinguishes graph, Host registry by
module, source/registry file by module and URL, effective override by module,
and merge by module and stage. After the first child or merge outer, continue
attempting every later Complete merge but retain the first occurrence outer.
Final precedence is:

1. first typed child/merge outer by graph-entry and child order;
2. first semantic or DICE-compute error by graph-entry order;
3. first incompatible Need union;
4. the compatible Need union; and
5. ordered success.

Final Need or outer is carrierless and discards all provisional epoch scratch.
Complete semantic error and success retain the full valid cumulative prefix.
Graph Need/outer is immediate and suppresses entries; graph semantic failure
retains the graph epoch and suppresses entries.

The aggregate remains eventless. Selected-graph/discovery descendants retain
their exact existing batches; Host-registry, registry-file and effective
parents remain eventless and lower event ownership is unchanged. Observed and
legacy complete batch sequences stay exact, warm reuse is silent, and poll-drop
publishes no aggregate row, value or batch.

Retain no graph or child carrier, per-entry epoch/list/map, join frontier,
override cache, parser/event scratch, cache/interner/store/lock/task/direct Host
read, revision or certificate state. Existing semantic registry observations,
attempts, effective override and repo specs remain only inside the one Result;
all traversal, merge and terminal scratch is compute-local.

## Required proof

- distinct key equality/hash/Display, borrowed accessors, `Dupe`/`Allocative`,
  Complete Result-Arc projection and Complete/Need/outer equality/validity;
- production-used graph and per-entry adapters/finishers for every DICE-compute,
  semantic, Need and typed-outer position, with exact prefixes and later-child
  suppression;
- exact first-Arc duplicate handling, conflict and operation mismatch, and a
  full-scan table for first/middle/last semantic error, compatible/incompatible
  Need, child outer and merge outer across multiple entries;
- root/nonregistry `None`, source JSON parse/projection, optional registry JSON
  suppression and every existing registry-file/effective/augmentation terminal;
- exact observed/legacy direct dependency vectors, reverse family isolation,
  complete ordered child EventBatch parity, zero parent batch and warm silence;
- real poll-drop with no aggregate publication and same-DICE recovery;
- independent selected-graph, Host-registry, source JSON, registry JSON and
  effective-override A -> B -> A restoration with held Result/epoch handles and
  unaffected per-demand Arc preservation; and
- zero route, selected extension, generated repository, public command or
  bootstrap activation on success, error, Need, outer and cancellation paths.

Preserve admitted Bazel 9 values/errors/order/events as exact. Private typed
outers and shared-Arc epoch association are Slug-native. Routes, extensions,
generated/public/bootstrap breadth, M8/M7B and exact identity bytes remain
deferred.

## Terminal discipline

STOP a second key/file, caller/export, route/extension/public activation,
changed legacy precedence or event ownership, retained traversal state, proof
waiver, cap excess, milestone closure, M8 or M7B work. REPLAN before widening.
After independent design ACCEPT schedule only
`WP-6-7A-selected-registry-repo-specs-observation-implementation`; after its
ACCEPT resume only the route/extension frontier. M7 remains partial and
M7A -> M8 -> M7B remains.

## Historical Host-registry-function observation implementation

Packet: `WP-6-7A-host-registry-function-observation-implementation`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Rust base: `2a4041bb`
Accepted design: `38f40427`

## Exact implementation authority

Write only `app/slug_bzlmod_v2/src/host_registry.rs`, baseline 1,536 physical
with first `#[cfg(test)]` at line 529. Caps are <=220 production, <=520 proof,
<=740 aggregate semantic and <=2,300 physical; every helper/test remains below
200 lines. Every other Rust file, caller, export, fixture and oracle is
read-only.

## Frozen implementation contract

Add the accepted private `HostRegistryFunctionObservationKey` and
`ObservedHostRegistryFunction`, retaining exactly one local semantic Result
Arc plus the exact visible-lockfile epoch. Use one Legacy/Observed driver and
preserve mode -> vendor projection -> conditional refresh -> matching visible
lockfile -> resolved spelling -> mirrors -> primary URI/hash policy -> ordered
mirror validation -> Result.

Legacy uses `HostVisibleLockfileKey` with an empty epoch; Observed uses only
`HostVisibleLockfileObservationKey` and forwards its exact epoch/shared Arcs.
Preserve the duplicated neutral mode dependency inside that child and legacy
DICE-invariant behavior. Do not reorder the parent.

Mode/vendor/refresh failures complete with an empty epoch. Visible Need/typed
outer is immediate and carrierless. Visible semantic error and every later
mirrors/URI/success terminal retain the exact visible epoch. No union or
reconstruction is permitted. Legacy projection moves the exact Result Arc.

Complete equality is semantic Result+epoch; Need is invalid/self-unequal and
typed outer compares by value. Parent/children remain eventless; warm reuse is
silent and cancellation publishes nothing. Retain no child carrier, parser/URI
or mirror-selection scratch, second collection, map/cache/interner/store/lock/
task/direct Host read/revision/certificate/event state.

## Proof and terminal

Complete the accepted production-used identity/terminal/prefix proof, exact
Refresh/non-Refresh rows and reverse family isolation, Need/outer and shared-Arc
association, exact legacy Result Arc, every current error/scheme/hash-policy/
primary-and-mirror-URI terminal, zero batches/warm/cancel recovery, independent
same-key mode/vendor/refresh/visible/mirrors A -> B -> A held-carrier lifecycles,
workspace/original-registry key-identity reuse and zero repo-spec/route/
extension/public/bootstrap activation.

Exact compatibility is current Host registry values/errors/order/URI/hash/
mirror behavior/events. Private typed outer/shared-Arc association is
Slug-native. Repo specs/routes/extensions/public/bootstrap, M8/M7B and identity
bytes remain deferred.

STOP a second file/key, caller/export, ordering/error/event/memory drift, upper
activation, proof waiver, cap excess or milestone closure. REPLAN before
widening. After independent implementation ACCEPT resume only the docs frontier
for selected registry repo specs. M7 remains partial and M7A -> M8 -> M7B
remains.

## Historical Host-registry-function observation design

Packet: `WP-6-7A-host-registry-function-observation-design`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Scheduling base: `4ebdf9ea`
Rust base: `2a4041bb`

## Owner decision

`HostRegistryFunctionKey` is the uniquely smallest complete remaining owner.
It alone owns exact lockfile mode -> vendor projection -> conditional refresh
token -> visible lockfile -> resolved registry spelling -> module mirrors ->
primary URI/hash policy -> mirror validation -> Result order. Its only mutable
path child is the accepted `HostVisibleLockfileObservationKey`; every other
child is injected, neutral or a path-free projection.

Its sole production consumer is the per-entry registry projection inside
`HostSelectedRegistryRepoSpecsKey`. That larger key separately consumes the
selected graph, registry files, effective overrides and full-batch entry
algebra. Observing repo specs first would combine and duplicate this reusable
policy boundary. Routes and extension/public consumers are later still.

## Exact design authority

Write only canonical, current, this Stage and the orchestration routing log, at
net caps <=40/<=220/<=180/<=30 and <=470 aggregate. Rust, tests, fixtures,
oracles, exports and callers remain read-only until independent design ACCEPT.

The sole future Rust authority is
`app/slug_bzlmod_v2/src/host_registry.rs`, baseline 1,536 physical with first
`#[cfg(test)]` at line 529. Permit <=220 production, <=520 proof, <=740
aggregate semantic and <=2,300 physical lines. Every helper/test remains below
200 lines; no second file, export or caller is writable.

Add private crate-visible `HostRegistryFunctionObservationKey` and
`ObservedHostRegistryFunction`. The carrier derives `Dupe`/`Allocative`,
has borrowed accessors, and retains exactly one existing
`Arc<Result<HostRegistryFunctionValue, HostRegistryFunctionError>>` plus the
exact compact `PathObservationEpoch` from the visible-lockfile child.

## Shared driver and exact order

Use one Legacy/Observed semantic driver:

1. compute the neutral lockfile mode;
2. compute the neutral vendor-directory projection;
3. only in Refresh mode compute the neutral refresh token;
4. Legacy computes `HostVisibleLockfileKey` with an empty epoch, while
   Observed computes `HostVisibleLockfileObservationKey` and forwards its exact
   epoch/shared Arcs;
5. form the resolved registry spelling;
6. compute the neutral module-mirrors input; and
7. parse the primary URI, select the exact scheme/hash policy, validate mirrors
   in order and construct the existing Result.

Preserve the existing duplicated neutral mode relationship inside the visible
child and the current legacy DICE-invariant behavior. Do not reorder the parent
to match the child's internal file-first order.

Mode, vendor and conditional-refresh semantic failures complete with an empty
epoch. Visible-lockfile Need or observed typed outer is immediate, carrierless
and suppresses spelling/mirrors/URI work. Visible-lockfile semantic failure and
every later mirrors/primary-URI/mirror-URI/success terminal retain the exact
visible-lockfile epoch. No union or reconstruction exists because no other
child owns path observations. Legacy projection moves the exact local Result
Arc.

Complete equality is local semantic Result plus epoch. Need remains invalid and
self-unequal; typed outer compares by its outer value. The parent and all
semantic children remain eventless; lower path observations stay lower-owned.
Warm reuse is silent and cancellation publishes no parent value or batch.

Retain no visible child carrier, URI/parser scratch, second mirror collection,
map/cache/interner/store/lock/task/direct Host read, revision/certificate or
event state. Existing semantic lockfile/vendor/mirror/refresh Arcs remain only
inside the one local Result; all driver scratch is compute-local.

## Required proof

- distinct key equality/hash/Display, accessors, `Dupe`/`Allocative`, and
  Complete/Need/outer equality and validity;
- production-used Legacy/Observed child adapters and finishers for every mode,
  vendor, refresh, visible-lockfile, mirror and URI compute/semantic position,
  with exact empty/full prefixes and later suppression;
- exact observed/legacy direct dependency rows in Refresh and non-Refresh modes,
  including only the matching visible-lockfile family and reverse isolation;
- carrierless Need/typed outer, exact visible epoch demand order/shared Arcs and
  exact legacy semantic Result-Arc projection;
- every current value/error/message, scheme/hash-policy combination, valid and
  invalid primary URI and ordered mirror validation terminal;
- zero complete ordered EventBatch sequences, parent/child silence, warm reuse,
  real poll-drop/no-publication and same-DICE recovery;
- independent same-key mode, vendor, refresh-token, visible-lockfile and mirrors
  A -> B -> A restoration with held Result/epoch handles and unaffected Arc
  preservation; and
- workspace/original-registry key-identity A -> B -> A reuse plus zero
  repo-spec/route/extension/public/bootstrap activation.

Workspace and original-registry changes are key-identity reuse, not same-key
invalidation. Reuse accepted visible-lockfile and injected-input proof; add no
oracle because this packet associates current exact semantics.

## Compatibility and terminal

Exact: current Host registry values/errors, order, URI/hash/mirror semantics and
eventlessness. Slug-native: private typed outer and shared-Arc epoch
association. Deferred: selected repo specs/routes, extensions, public/bootstrap
activation, M8/M7B and exact identity bytes.

After independent design ACCEPT schedule only
`WP-6-7A-host-registry-function-observation-implementation`; after
implementation ACCEPT resume only the docs frontier for selected registry repo
specs. STOP a second Rust file/key, caller/export, order/error/event drift,
retained child state, upper activation, proof waiver, cap excess, milestone
closure, M8/M7B work or bypassing M7A -> M8 -> M7B. REPLAN before widening.

## Historical Host-registry-function frontier audit

Packet: `WP-6-7A-host-registry-function-observation-frontier-audit`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Accepted implementation: `2a4041bb`
Semantic design: `ba21c0e8`
Rust base: `d5e8f461`

## Accepted visible-lockfile completion

Implementation `2a4041bb` adds the private Host-visible observation owner in
`host_lockfile.rs`. One Legacy/Observed driver preserves exact
FileBytes -> lockfile mode -> file semantic/Host-parser order, including present
bytes under Off. Missing mode retains the completed file epoch and still wins
over a stored file error; Need/typed outer remains carrierless.

The observed value retains exactly one local semantic Result Arc plus the exact
compact Host FileBytes epoch. No union, child carrier, parsing scratch,
collection/cache/interner/store/lock/task/direct Host read, revision,
certificate or event state is retained. The owner and children remain
eventless.

Accepted accounting is +114 production/+280 proof/+394 aggregate at 1,359
physical lines, within every frozen cap. Focused Host-lockfile validation passes
10/10; the full bzlmod suite passed 501 unit tests plus all integration/doc
targets before the bounded proof-only tracker correction, and focused validation
passes afterward. Formatting and diff hygiene pass. Independent terminal review
ACCEPTs exact order/errors/Off behavior, family/Arc association, cancellation,
upper isolation, compact retention and cleanup.

## Exact docs-only frontier authority

This audit may write only canonical, current, this Stage and the orchestration
routing log, at net caps <=40/<=220/<=180/<=30 and <=470 aggregate. Rust, tests,
fixtures, oracles, exports and callers are read-only.

Trace the accepted `HostSelectedModuleGraphObservationKey` and
`HostVisibleLockfileObservationKey` through `HostRegistryFunctionKey`, then
through selected registry repo specs, routes, extension-generated repositories
and public/bootstrap consumers only far enough to identify the uniquely smallest
complete remaining mutable frontier. Do not presume that Host registry function
is complete merely because its visible-lockfile prerequisite is now observed,
and do not reopen accepted lower owners for structural uniformity.

The audit must establish:

- the first reusable semantic producer that can retain one exact Result Arc and
  a complete shared-Arc epoch without reconstructing graph or lockfile state;
- exact selected-graph -> visible lockfile -> registry function -> repo-spec/
  route/extension order, Need/typed-outer/error precedence and later suppression;
- matching Legacy/Observed families, event ownership/order, warm silence and
  poll-drop recovery;
- independent graph, visible-lockfile, registry-policy/spec/route/generated-
  repository A -> B -> A invalidation with held Result/epoch handles; and
- compact Buck2-shaped retention with parsing/join/event scratch compute-local
  and no cache/interner/store/lock/task/direct Host read.

Preserve admitted Bazel 9 values/errors/order/events as exact. Private typed
outers and shared-Arc epoch association remain Slug-native. Repo-spec/route/
extension breadth, bootstrap execution, M8/M7B and exact identity bytes remain
deferred unless live evidence proves one is the uniquely smaller prerequisite.

## Terminal discipline

Reach exactly one terminal: one independently reviewed smallest-owner design,
one uniquely smaller evidence/association prerequisite, or formal REPLAN. A
design may name at most one implementation successor. STOP Rust/tests/oracles/
caller/export changes, speculative public activation, umbrella ownership,
milestone closure, M8/M7B work, or bypassing the accepted graph/visible-lockfile
carriers. M7 remains partial and M7A -> M8 -> M7B remains.

## Historical visible-lockfile observation implementation

Packet: `WP-6-7A-host-visible-lockfile-observation-implementation`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Rust base: `d5e8f461`
Accepted design: `ba21c0e8`

## Exact implementation authority

Write only `app/slug_bzlmod_v2/src/host_lockfile.rs`, baseline 965 physical
with first `#[cfg(test)]` at line 142. Caps are <=140 production, <=280 proof,
<=420 aggregate semantic and <=1,400 physical; every helper/test remains below
200 lines. Every other Rust file, caller, export, fixture and oracle is read-only.

## Frozen implementation contract

Implement the accepted private `HostVisibleLockfileObservationKey` and
`ObservedHostVisibleLockfile` below. One Legacy/Observed driver must preserve
exact file -> mode -> file semantic/Host-parser order. Legacy uses only
`HostFileBytesKey` with an empty epoch; Observed uses only
`HostFileBytesObservationKey` and forwards its exact epoch/shared Arcs; both
then use the neutral `RootModuleLockfileModeKey`.

File Need/observed typed outer is immediate and carrierless. Once file Complete
exists, missing mode wins over stored file semantic errors and retains the file
epoch. Every File/Missing/Present/BadLockfile/UncaughtParse/unsupported-version/
success terminal retains the same exact epoch. Explicitly preserve Off+present
Host parsing; do not substitute the mode-first root-files Ignored/empty carrier.
Legacy projection moves the exact existing Result Arc.

Retain only one local semantic Result Arc plus the compact epoch; derive
`Dupe`/`Allocative` and add borrowed crate-private accessors. Add no epoch
union, child carrier, bytes/parser scratch, collection/cache/interner/store/
lock/task/direct Host read, revision/certificate or event state. Parent and
children remain eventless; warm reuse is silent and cancellation publishes
nothing.

## Proof and terminal

Complete the accepted production-used identity/family/terminal proof, exact
file-first rows and reverse isolation, missing-mode precedence, every exact
error/success variant, pointer-identical epoch forwarding, Off discriminator,
zero batches/warm replay, poll-drop recovery, independent mode and
bytes/symlink A -> B -> A held-carrier lifecycles and zero root-files/Host-
registry/repo-spec/route/extension/public/bootstrap activation.

Exact compatibility is current Host-visible values/errors/order/parser/events.
The private outer/shared-Arc association is Slug-native; every upper owner and
M8/M7B remain deferred. STOP a second file/key, caller/export, root-carrier
substitution, semantic/order/event/memory drift, proof waiver, cap excess or
milestone closure. REPLAN before widening. After independent implementation
ACCEPT resume only the docs frontier for Host registry-function observation,
then repo specs. M7 remains partial and M7A -> M8 -> M7B remains.

## Historical visible-lockfile observation design

Packet: `WP-6-7A-host-visible-lockfile-observation-design`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Scheduling/Rust base: `98aaf23c` / `d5e8f461`

## Owner decision

`HostVisibleLockfileKey` is the uniquely smaller prerequisite before
`HostRegistryFunctionKey` and selected registry repo-spec ownership. It alone
preserves the current Host lockfile order: observe `MODULE.bazel.lock` bytes,
read the neutral lockfile mode, then inspect file semantics and parse the
visible lockfile. Its semantic Result is reusable by the later Host registry
function without involving root MODULE state.

The accepted `RootModuleFilesObservationKey` is not a substitute. It reads
mode first, short-circuits `LockfileMode::Off` to
`VisibleLockfileRead::Ignored` with an empty lockfile epoch, carries unrelated
root MODULE observations and projects different errors. In contrast,
`HostVisibleLockfileKey` still observes and parses present bytes in Off mode
and returns an `Arc<BazelLockfile>`. Reusing the root carrier would therefore
change exact order, values, errors and epoch membership.

## Exact authority and shape

This design may write only canonical, current, this Stage and the orchestration
routing log, at net caps <=40/<=220/<=180/<=30 and <=470 aggregate. Rust, tests,
fixtures, oracles, exports and callers remain read-only until independent design
ACCEPT.

The sole future Rust authority is
`app/slug_bzlmod_v2/src/host_lockfile.rs`, baseline 965 physical with the first
`#[cfg(test)]` at line 142. Allow <=140 production, <=280 proof, <=420
aggregate semantic and <=1,400 physical lines. Every helper/test stays below
200 lines; this cohesive owner file is not a split trigger.

Add one private `HostVisibleLockfileObservationKey(HostVisibleLockfileKey)`
and one private `ObservedHostVisibleLockfile`. The carrier retains exactly one
existing semantic
`Arc<Result<HostVisibleLockfileValue, HostVisibleLockfileError>>` and the exact
`PathObservationEpoch` from its Host FileBytes child. Derive `Dupe` and
`Allocative`, provide borrowed crate-private accessors, and add no export or
caller.

## Shared driver and terminal algebra

Use one Legacy/Observed semantic driver:

1. form the visible-lockfile logical path;
2. Legacy computes only `HostFileBytesKey` and contributes an empty epoch;
   Observed computes only `HostFileBytesObservationKey` and forwards its exact
   epoch and shared demand Arcs;
3. after a Complete file child, compute the shared neutral
   `RootModuleLockfileModeKey`; and
4. only then inspect Missing/Present/file error and call the existing exact Host
   parser.

File Need or an observed typed outer returns immediately carrierless and
suppresses mode and parse. Preserve the legacy DICE invariant behavior. Once
the file child is Complete, a missing mode wins over any stored file semantic
error and retains the exact file epoch. File error, Missing, Present,
`BadLockfile`, `UncaughtParse`, unsupported-version and success terminals all
retain that same epoch. No root-files family, epoch union, reconstruction or
direct Host read is permitted.

Complete equality is the local semantic Result plus exact epoch; Need remains
invalid and self-unequal, while a typed outer compares by its outer value.
Legacy projection moves the exact existing Result Arc. The parent and both
semantic children remain eventless; lower path observations remain lower-owned.
Warm reuse emits nothing and poll-drop publishes no parent value or batch.

Retain no child carrier, file bytes/parser scratch, second collection,
map/cache/interner/store/lock/task, revision/certificate or event state. The
DICE value is only the local Result Arc plus compact shared-Arc epoch.

## Required proof

- distinct key equality/hash/Display, accessors, `Dupe`/`Allocative`, and
  Complete/Need/outer equality and validity;
- production-used family adapters and finishers proving exact file-first rows,
  reverse Legacy/Observed isolation, carrierless file Need/outer, missing-mode
  precedence after a completed file and later suppression;
- exact `File`, `BadLockfile`, `UncaughtParse`, unsupported-version and
  success variants/messages for Missing, Present, WrongKind, read and resolution
  terminals, with every child epoch demand and Result Arc pointer-identical;
- explicit Off mode with present bytes proving the Host parser and nonempty exact
  file epoch, discriminating the root-files `Ignored`/empty-epoch behavior;
- exact zero parent/child semantic batches, warm silence, real poll-drop with no
  publication and same-DICE recovery;
- independent mode and lockfile bytes/symlink A -> B -> A restoration with held
  semantic Result and epoch handles; and
- zero `RootModuleFiles`, `HostRegistryFunction`, repo-spec, route, extension,
  public-command or bootstrap activation.

Reuse accepted lower Host FileBytes/path evidence; add no oracle because this
packet associates the current exact Host-visible semantics rather than changing
them.

## Compatibility and terminal discipline

Exact: current `HostVisibleLockfileValue`/`HostVisibleLockfileError`, file ->
mode -> parse order, parser behavior and eventlessness. Slug-native: the private
typed outer and shared-Arc epoch association. Deferred: Host registry function,
repo specs/routes/extensions, public/bootstrap activation, M8/M7B and exact
identity bytes.

After independent design ACCEPT schedule only
`WP-6-7A-host-visible-lockfile-observation-implementation`; after implementation
ACCEPT resume the docs-only selected frontier for Host registry-function
observation, then repo specs. STOP a second Rust file/key, caller/export,
root-carrier substitution, parser/order/error/event drift, extra retained state,
upper activation, cap excess, proof waiver, milestone closure, M8/M7B work or
bypassing M7A -> M8 -> M7B. REPLAN before widening.

## Historical selected-module-graph observation design

Packet: `WP-6-7A-host-selected-module-graph-observation-design`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Scheduling/Rust base: `e399cd10` / `c6b1e108`

## Accepted owner decision

`HostSelectedModuleGraphKey` is the uniquely smallest complete aggregate. It
alone sequences root MODULE files, neutral command policy, candidate effective
overrides, root/discovered request transformation, repeated BFS horizons and
fixed-point rounds, then graph selection/rewrite. `raw_root`, `raw_discovered`,
`transform_request` and the override cache are owner-local mechanisms with no
other consumer. Every mutable child now has an accepted observed sibling.

`HostSelectedRegistryRepoSpecsKey` and `HostSelectedModuleRoutesKey` are later
direct consumers that add repo-spec/route semantics. They must not absorb or
duplicate graph ownership. No smaller carrierless prerequisite remains.

## Exact docs-only design authority

Write only canonical, current, `06-analysis-toolchains-and-actions.md` and the
routing log. Rust, tests, fixtures, oracles, callers and public exports are
read-only. Net docs caps are canonical <=40, current <=220, Stage <=180 and
routing <=30, with <=470 aggregate.

After independent design ACCEPT, future Rust authority is exactly:

- `app/slug_bzlmod_v2/src/selected_graph.rs`, baseline 1,592 physical and first
  test boundary 907, <=520 production, <=320 colocated proof and <=2,450
  physical; and
- `app/slug_bzlmod_v2/src/source_preparation_observation_tests.rs`, baseline
  8,406, <=1,500 proof and <=10,000 physical.

Aggregate cap is <=2,340 semantic and <=12,450 physical. Helpers/tests remain
below 200 lines. `selected_graph.rs` is one cohesive large-file owner; every
third file, export and caller remains read-only.

## Frozen owner and order

Add private `HostSelectedModuleGraphObservationKey(HostSelectedModuleGraphKey)`
and `ObservedHostSelectedModuleGraph`. The carrier is `Dupe`/`Allocative`, has
crate-private constructor/borrowed accessors and no caller, and retains exactly
one `Arc<Result<HostSelectedModuleGraph, HostSelectedModuleGraphError>>` plus one
cumulative compact `PathObservationEpoch`.

Use one Legacy/Observed driver. Legacy selects legacy root files, effective
overrides and discovered leaves with empty epochs; Observed selects exactly
their accepted observed siblings. Both share parsing, transformation,
BFS/fixed-point and graph-selection logic and project the identical legacy
Result Arc.

Preserve exact order: root files; neutral command policy; candidate overrides
in root-override order followed by new command-override order; root dependency
transformations including implicit `bazel_tools`; BFS horizons in first-seen
`next_horizon` order; per-success raw-discovered transformations in horizon
order; repeated fixed-point rounds; final select/rewrite. Cache each effective
semantic value once and append its epoch only on first computation. Repeated
fixed-point/discovery epochs merge as equal duplicates, preserving the earliest
exact Arc.

## Prefix and full-horizon algebra

Sequential Complete child epochs merge into the cumulative prefix left-first
before semantic inspection. Root-files DICE compute error has empty prefix;
command-policy compute retains root prefix; effective compute retains the prior
prefix; effective semantic retains its merged epoch; pure transform/select
errors retain the exact reached prefix.

For each compute-join horizon, scan the full input `next` order and merge every
`Complete(Ok(carrier))` epoch before terminal selection, even when another leaf
is Need/error. First horizon-ordered merge conflict/operation mismatch or child
typed outer is a carrierless fail-closed outer. Otherwise preserve current
`finish_horizon` precedence exactly: first DICE-compute or semantic leaf error
by horizon order > incompatible Need > compatible Need union > ordered success.
Need/outer retains no provisional epoch. A Complete semantic error retains the
full valid merged sibling epoch. Any terminal suppresses raw conversion, later
horizons/fixed-point rounds and select/rewrite.

Need is invalid/self-unequal. Complete outer equality is outer-by-value;
Complete carrier equality is local semantic Result plus epoch.

## Events, retention and proof

The graph siblings remain eventless. Full-batch discovery children retain sole
ownership and exact order of their existing batches even on graph Need/error;
observed/legacy sequences match. Warm reuse is silent. Poll-drop/cancellation
publishes no graph row/value/batch; ordinary child DICE reuse remains allowed.

Retain no `RawModule`, dependency/frontier/prior-name/seen set, override cache,
horizon outcome, event scratch, child carrier Arc, map, cache, interner, store,
lock, task or direct Host read. Existing graph Arc slices remain semantic Result
state; every traversal/join/merge structure stays compute-local.

Prove key/hash/Display/accessors/equality/validity and exact legacy Result-Arc
projection; duplicate first Arc/conflict/operation mismatch; root/policy/
candidate-effective first/middle/last compute/semantic/Need/outer prefixes; and
full horizon first/middle/last compute/semantic/outer, compatible/incompatible
Needs, full epoch reconstruction and later suppression.

Drive exact observed/legacy dependency vectors and reverse family isolation;
implicit bazel_tools, duplicate candidate, diamond/cycle, nodep second round and
mixed nonregistry+registry horizons; exact graph parity and child batches with
zero parent batch/warm silence; poll-drop recovery; independent root-files,
command-policy, effective, nonregistry, registry, recursive and mixed-horizon
A-B-A with held Result/epoch and unaffected-Arc preservation; and zero repo-
spec/routes/extension/public activation.

Exact compatibility is existing graph values/errors/order/events. Slug-native
is the private carrier, typed outer and shared-Arc epoch association. Repo-spec,
routes, extensions, public activation, broader M7A, M8/M7B and exact identity
bytes remain deferred.

STOP a third Rust file, caller/export activation, changed legacy precedence or
child event ownership, retained traversal state, weakened Arc association, cap
excess or milestone closure. REPLAN before widening. After independent design
ACCEPT schedule only `WP-6-7A-host-selected-module-graph-observation-implementation`;
after implementation ACCEPT return only to the docs-only selected frontier.
M7 remains partial and M7A -> M8 -> M7B remains.

## Historical selected-module-graph frontier audit resume 4

Packet: `WP-6-7A-selected-module-graph-observation-frontier-audit-resume-4`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Accepted implementation: `c6b1e108`

## Accepted discovered-module completion

Implementation `c6b1e108` completes the private observed discovery owner from
Rust base `223c8112`, semantic design `b8e4cc03` and proof correction
`b09d5e70`. The shared driver preserves effective selection followed by the
builtin, nonregistry-closure or registry-preparation branch. Complete epochs
merge left-first before semantic inspection; Need/typed outer is carrierless;
builtin publishes no discovery-local batch; nonregistry and registry retain
sole ownership of their matching evaluation batch.

The accepted carrier retains one exact local Result Arc plus a compact
`PathObservationEpoch`. No child carrier, evaluator/event/merge scratch, cache,
interner, store, lock, task or direct Host read is retained. Exact scope and
accounting against `223c8112` are +309/16,811 physical in
`source_preparation.rs`, +1,175/8,406 in
`source_preparation_observation_tests.rs`, and +1,484/25,217 aggregate, within
every corrected cap. Focused discovery proof passes 10/10 and the complete
`slug_bzlmod_v2` suite passes 485 unit tests plus every integration/doc test;
formatting and diff hygiene pass. Independent final review ACCEPTs production,
proof, family/event ownership, retention and cleanup with no REPLAN.

## Exact docs-only audit authority

This packet is read-only for Rust, tests, fixtures, oracles, public APIs and
callers. Write only canonical, current, `06-analysis-toolchains-and-actions.md`
and the orchestration routing log. Net docs caps are canonical <=40, current
<=220, Stage <=180 and routing <=30, with <=470 aggregate.

Trace accepted `HostDiscoveredModuleObservationKey` leaves through the complete
`HostSelectedModuleGraphKey` BFS/fixed-point join and every direct consumer.
Inspect the selected-graph driver, discovery request/horizon ordering and every
remaining mutable/path-producing or carrierless edge. Inspect selected
repo-spec, extension and public consumers only far enough to prove that they
are later owners or a necessary part of the smallest candidate. Do not reopen
accepted effective, closure, repository, registry-file, preparation, patch or
discovery ownership for structural uniformity.

The audit must determine the uniquely smallest complete owner and establish:

- exact key/Result/Arc/epoch association and whether the aggregate can retain a
  compact local Result Arc plus complete cumulative epoch without rebuilding
  child state;
- exact input/BFS/fixed-point order, complete-batch Need/typed-outer/semantic
  precedence, first-Arc conflict behavior and later-work suppression;
- matching Legacy/Observed families, child evaluation-event ownership/order,
  warm silence, cancellation recovery and zero provisional parent state;
- lifecycle restoration and held-Arc behavior for nonregistry, registry,
  recursive dependency and mixed horizons; and
- compact Buck2-shaped retention with join/frontier/event scratch compute-local
  and no new cache/interner/store/lock/task/direct Host read.

Preserve current Bazel 9 branch/result/error/order/events as exact. Private
typed outer and shared-Arc epoch association are Slug-native. Selected repo
specs/extensions, broader bootstrap work, M8/M7B and exact identity bytes remain
deferred unless live owner evidence proves one is the uniquely smaller
prerequisite.

Terminate with exactly one independently reviewable docs-only design for the
smallest complete owner, one uniquely smaller prerequisite design, or formal
REPLAN with contradictory evidence and one smallest next audit/design. At most
one successor may be scheduled; no Rust authority exists before independent
design ACCEPT.

STOP direct implementation, speculative selected-graph/public activation,
moving or duplicating child event ownership, weakened Arc/epoch equality,
retained-state growth, proof waiver, cap excess, M7 acceptance, M8/M7B work or
exact identity-byte work. M7 remains partial and M7A -> M8 -> M7B remains.

## Historical discovered-module implementation authority

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
