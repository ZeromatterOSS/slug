# Current Slug V2 Packet

Packet: `WP-2A-m1-external-singleton-observed-build-design`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Scheduling base: `a4dd40d6`
Accepted Rust base: `a4dd40d6`
Result: freeze the bounded observed publication contract for exactly one
nonroot exported-source build target.

## Exact docs authority and caps

Write only:

1. `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`: <=40 net;
2. this manifest: <=220 net; and
3. `thoughts/shared/plans/slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`:
   <=180 net.

Aggregate docs net <=420. Rust, Cargo manifests, BUILD, fixtures, oracles,
generated evidence, exports and public behavior are forbidden during design.

## Learned facts and owner decision

The existing private `BuildCommandRootObservationKey` is the uniquely
smallest complete owner. Public build already tries that structural root first.
Its syntax can admit exactly one nonroot `TargetPattern::Single` without
admitting multi-build. Its result and native publication boundary already own
the semantic build Result Arc, complete selected path epoch, accepted events,
repository sidecars and revision finalization.

The live legacy external branch sequences root anchor, repository route,
repository package, target-kind lookup and selected Host source. Accepted
observed siblings now cover every one of those mutable edges:
`RootModuleLoadingAnchorObservationKey`,
`RootRepositoryRouteObservationKey`,
`RepositoryPackageLoadObservationKey` and
`HostRepositorySourceFileObservationKey`. The accepted
`SourceCertificate` epoch can retain the source carrier's complete logical
resolution and FileBytes prefix, including Materialization namespace demands,
and the active materializer can reobserve it at final acceptance.

No lower producer, new DICE key or command-side side store is required.
`SingletonRootSingleBuildCommandKey` remains the neutral root-only rule and
filegroup owner. Multi-build remains later because request revision and source
certificate aggregation are singleton-only. One-shot adapters still create a
fresh runtime and reject external package paths.

Bazel 9.2 `BuildTool.buildTargets/processRequest`,
`TargetDefinitionContext.createInputFile` and `InputFile` remain the exact
public classification evidence. Existing accepted lifecycle evidence is
reused; no new oracle or fixture is needed. Buck2 DICE transaction,
cancellation and equality-cutoff evidence remains concept/test-only; no donor
code is imported.

## Frozen identity and driver contract

Keep the same `BuildCommandRootObservationKey(BuildCommandRootKey)`. Its
constructor continues to admit singleton root `PackageAll` and additionally
admits only one nonroot `Single`. Every nonroot `Single`, including an
external rule/filegroup, enters that observed identity and is accepted or
rejected only after observed package target-kind classification. Every root
`Single` (including root rule/filegroup), multi-target and other identity
continues through the exact current neutral/legacy path. The sole public
constructor keeps its existing observed -> neutral -> legacy order.

Use one observed-root dispatcher. The existing singleton PackageAll driver is
unchanged. Refactor the existing external branch into one private mode-aware
driver: the legacy generic root selects only legacy route/package/source
children and the observed root selects only their observed siblings. Neither
mode computes the other family, and target classification/result projection
remain one shared semantic path. For observed mode the exact order is:

1. observed root loading anchor, owned by the observed root;
2. observed root repository route in the shared branch;
3. observed repository package load in the shared branch;
4. target lookup and exact `ExportedFile` classification;
5. the existing `RequestRevisionKey` dependency for the admitted exported
   source; then
6. observed selected repository source.

Validate and union every completed child epoch left-first before semantic
inspection. Stable equal duplicates keep the first exact Result Arc.
Conflict or operation mismatch is a typed `ObservedPathFrontierError`.
Need or typed outer at any child returns immediately without a carrier and
activates no later child. The owner is sequential and performs no Need union.

Freeze semantic prefixes:

- anchor DICE compute failure is the existing infrastructure semantic with an
  empty prefix; anchor semantic error keeps the anchor prefix;
- route DICE failure keeps anchor; route semantic error keeps anchor+route;
- package DICE failure keeps anchor+route; package semantic error keeps
  anchor+route+package;
- missing target and non-exported target kind keep anchor+route+package and do
  not activate source;
- request-revision DICE failure keeps anchor+route+package and does not
  activate source;
- source DICE failure keeps anchor+route+package; source Absent, semantic
  error, accepted directory WrongKind, and Present keep the full
  anchor+route+package+source prefix; and
- successful exported-source completion keeps one target, no analysis, and an
  empty action closure exactly as today.

Preserve the current exact public quirk that a directory WrongKind source is
accepted as an exported source. Preserve all existing diagnostics and terminal
codes. Source Absent and semantic errors may gain private certificate storage
only so their exact diagnostic can participate in final retry; formatting and
public equality remain exact.

## Certificate, acceptance and events

For every terminal reached after a completed source child, build a
`SourceCertificate` from that child's entire observed epoch. It is an exact
Arc-identical subset of the full observed terminal epoch. Present, Absent,
accepted directory WrongKind and source semantic errors all retain that
certificate; earlier terminals retain none. The observed root's
`NativeCommandRoot` implementation exposes the full epoch and the semantic
Result's certificate, and initializes request revision only for its admitted
external `Single`, never for PackageAll.

External repository materialization contributes repository requests and
validations to the exact activation closure. Therefore this same root opts into
`ClosureRepositories` selection only for the admitted external identity.
PackageAll and every other observed root retain strict-empty repository
selection. `selected_snapshot` remains the sole constructor/validator of
repository sidecars, full path demand/value/Arc validation remains
unconditional, and no collection is added to the terminal carrier.

The accepted epoch certificate finalizer reobserves the complete source epoch
through the active materializer session. Equal demands preserve every original
Arc; changed demands alone publish a revision and retry. Need, typed outer,
cancellation, association/selection/materializer/finalization failure and
restorable abort preserve the prior accepted path/repository/event state and
publish nothing provisional.

The observed root owns no event batch. Anchor, route/module and repository
package/source children remain the sole matching-family batch owners. Semantic
acceptance preserves their exact closure order; Need/outer/cancel publishes
none and warm reuse suppresses all unchanged batches. Cold external build and a
subsequent root observed build share the observed anchor family, so the
existing exact no-replay public expectation must pass without an event-side
special case.

## Retention and compatibility

The root DICE value retains exactly one local semantic
`Arc<Result<BuildCommandEvaluation, BuildCommandError>>` plus one compact
`PathObservationEpoch`. The semantic Result may retain one compact source
certificate epoch sharing the source carrier's exact Arcs. Child carrier Arcs,
route/package/source outcomes, selected path, union entries and event scratch
remain compute-local or dependency-owned. Accepted selected repository/path/
event epochs retain their existing compact shapes.

Add no retained map, Vec, cache, store, interner, lock or task; no direct Host
read, revision/certificate duplicate, new event owner or DICE key. The large
`dice.rs` and build proof file remain cohesive owner exceptions; every new or
materially touched helper stays below 200 lines.

Exact: public external exported-source values/errors/target classification,
BUILD and module event text/order, lifecycle behavior, root PackageAll,
neutral root Single, multi-target and every legacy/direct API.

Slug-native: external admission into the existing observed sibling, complete
epoch/carrier association, typed outer propagation, closure-selected
repository-sidecar policy and private certificate attachment.

Unsupported/deferred: multi-build certificate/branch aggregation, one-shot
cutover, broader build identities/action analysis, external-glob support and
exact Bazel identity bytes.

## Future implementation authority and proof

After independent design ACCEPT, authorize exactly:

1. `app/slug_core_v2/src/runtime/dice.rs`: <=260 production net and <=11,220
   physical; and
2. `app/slug_core_v2/src/runtime/tests/build_command_tests.rs`: <=360 test net
   and <=3,350 physical.

Aggregate semantic <=620 and combined physical <=14,570 against
`a4dd40d6`. No relocation or third Rust file.

Require:

- distinct admitted identity/Display/equality/validity and unchanged
  PackageAll/neutral/legacy routing plus exact legacy shared-driver result
  parity;
- exact anchor->route->package->source epoch membership/order and per-demand
  `Arc::ptr_eq`, stable duplicate first Arc, conflict and operation mismatch;
- every prefix above, including Need/typed outer at every child and later-child
  nonactivation;
- Present, Absent, directory WrongKind, source semantic, missing target and
  wrong-target-kind parity, with the revision dependency activated only after
  exported-source classification and source certificate present only after
  source;
- exact source-certificate/full-epoch association and Host+Materialization
  reobservation through symlink retarget, create/edit/delete/directory/recreate
  and A/B/A;
- nonempty selected repository requests and validations for a local-override
  external target, exact selected path Arcs, and strict-empty PackageAll;
- exact cold child batch order/text, semantic-error batches, warm suppression,
  no root event replay after external build, and no parent batch;
- observed->zero legacy and legacy/neutral->zero observed activation, including
  concurrent roots and zero multi-build/one-shot/query activation;
- pointer-distinct association abort, forced selection/materializer/revision
  failure, real poll-drop cancellation, no publication, same-DICE recovery and
  retained-result/manifest/bytes lifetimes; and
- exact caps, formatting, focused build/revision/server proof, full core
  baseline, archive status, Buck2 retention scan, AI cleanup and independent
  implementation review.

STOP on any other file, new key/state/event owner, lower-carrier change, public
API/export, partial epoch/certificate validation, strict-root repository-policy
relaxation, family/order/diagnostic drift, retained scratch, direct Host read,
cap excess, multi-build/one-shot activation or M1 closure. REPLAN if external
repository sidecars cannot be selected without terminal retention, if a source
terminal cannot carry the exact child epoch certificate, or if the bounded
two-file owner cannot preserve exact public behavior. After design ACCEPT,
schedule exactly one implementation; after implementation ACCEPT, return to
one docs-only remaining M1 owner audit.
