# Current Slug V2 Packet

Packet: `WP-6-7A-loaded-module-extension-definitions-observation-implementation`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Scheduling base: `99c23033`
Accepted predecessor: `99c23033`

## Goal and authority

Add one private observed sibling for
`HostLoadedModuleExtensionDefinitionsKey` in
`app/slug_loading_v2/src/bzl_module.rs`. Preserve the existing legacy owner
through one shared driver; activate no caller or upper extension owner.

Write exactly `app/slug_loading_v2/src/bzl_module.rs`. Every other file is
read-only, including Bzlmod, loading callers/modules, Cargo/BUILD metadata,
fixtures, oracles and planning documents until terminal rollover.

Baseline is 6,882 physical lines; the owned
`module_extension_definition_loading_tests` begins at 5,164. Caps are <=360
production, <=780 proof, <=1,140 aggregate semantic lines and <=8,025 physical
lines. Every helper/test remains below 200 lines and the shared driver below
150.

## Learned facts and evidence basis

Accepted `99c23033` exposes the observed definition-request key, carrier and
opaque outer. The crate-local `HostBzlModuleObservationKey` already owns each
root Bzl module's complete recursive epoch and local `EventBatch`. The legacy
loaded-definition key and its sole direct prepared-input consumer already live
in this file.

Pinned Bazel 9.2 `RegularRunnableExtension.load` establishes canonical Bzl
load then exported `ModuleExtension` selection;
`SingleExtensionEvalFunction` consumes that definition before extension
evaluation/generated repositories. Reuse this source evidence and accepted
lower proof; add no oracle.

The DICE contract is `docs/developers/dice.md`: dependencies explain warm
reuse, Complete-only values cut off by structural equality, and no lock spans a
DICE compute. Buck2 DICE is concept/test evidence only. There is no bridge,
fallback, side store, direct Host read or donor scheduler.

The 6,882-line file crosses the complexity trigger but remains the cohesive
owner for this one-key packet: it already contains the legacy key, both Host-Bzl
families, event tracker, real loading fixtures and direct consumer. Splitting
would add a private module-visibility seam and duplicate proof plumbing. The
single-file allowlist, caps, <150-line driver and one-owner stop bound growth.

## Types and natural owner

Keep the existing legacy key/value/error types exact. Add only:

- `HostLoadedModuleExtensionDefinitionsObservationKey`, a private sibling
  wrapping the legacy key;
- `ObservedHostLoadedModuleExtensionDefinitions`, retaining one local
  `Arc<Result<HostLoadedModuleExtensionDefinitions,
  HostLoadedModuleExtensionDefinitionsError>>` plus one cumulative
  `PathObservationEpoch`;
- one small stage enum distinguishing Host-Bzl child outer from epoch merge;
- one typed observation outer with:
  - request-child
    `HostSelectedExtensionDefinitionLoadRequestsObservationError`; and
  - per-request Bzl context containing the completed request aggregate,
    decisive request, stage and `ObservedPathFrontierError`; and
- one Legacy/Observed mode and bounded child adapters/shared driver.

The observed key owns the reusable semantic association. The request projection,
Bzl modules, frozen evaluator/module handles, label/export/downcast scratch and
event activations remain child-owned or compute-local.

## Shared-driver order and terminal algebra

Preserve exact order:

1. definition requests;
2. for each request in source order, root Bzl-target parse;
3. that request's Host Bzl module;
4. named-export lookup;
5. `FrozenModuleExtensionDefinition` downcast; and
6. manifest/projection append.

Request child behavior:

- legacy computes only
  `HostSelectedExtensionDefinitionLoadRequestsKey` with an empty epoch;
- observed computes only its accepted observation key;
- DICE compute failure in either family becomes the existing semantic
  `RequestsCompute` result with an empty prefix;
- Need and observed typed outer return carrierless;
- request semantic error becomes existing `Requests` with the request epoch;
  and
- the child adapter forwards its exact Result Arc/epoch to semantic finish,
  which clones a successful inner value once into the existing
  `Arc<HostSelectedExtensionDefinitionLoadRequests>` representation and drops
  the child Result Arc before parent publication.

For request i, let `P_i` be request epoch plus all successful prior Bzl
epochs. Label failure returns the existing request/Label semantic error with
`P_i`.

Host-Bzl child behavior:

- legacy computes only `HostBzlModuleEvalKey` with an empty child epoch;
- observed computes only `HostBzlModuleObservationKey`;
- wrap both DICE computes in `host_dice_invariant`; do not invent a semantic
  compute error;
- Need and observed typed outer return carrierless;
- merge each Complete child epoch into `P_i` strictly left-first before
  inspecting its semantic result;
- equal duplicate observations retain the earliest/request-side Arc;
- conflict or operation mismatch returns the typed Bzl/Merge outer with request
  context and no parent carrier;
- Bzl semantic error returns the existing request/Bzl semantic error with the
  merged prefix; and
- export, wrong-kind and success projection follow only a successful Bzl result
  and retain that same merged prefix.

Stop at the first terminal. First/middle/last terminals suppress every later
label/Bzl/export operation. Do not full-scan, join, speculate or union Need.
Legacy projection moves the exact local Result Arc and rejects any impossible
observed outer; observed projection wraps the same Result Arc+epoch.

## Events, revision behavior and lifetime

The parent never captures or stores evaluation data. Fresh observed Host-Bzl
children remain sole owners of their exact batches and activate in request
order. Warm parent reuse is silent. If a changed request invalidates the parent
but reaches an unchanged Bzl child, DICE reports that child Reused with
`batch: None`; it neither reevaluates nor re-emits the cached batch. Semantic
Bzl failures may still own the child batch already captured before failure.
Need and typed outer own no batch.

Independent transactions associate each Result with its exact request and Bzl
epochs. Prove request, each Bzl source/recursive load and pure export projection
A -> B -> A separately while holding prior Result/epoch handles and unaffected
Arcs. Poll-drop before parent publication stores no parent value/batch; a
same-DICE successor recomputes or reuses valid children and recovers.

Retain only the local loaded-definition Result Arc, cumulative epoch and
semantic requests/manifests/projections reachable from that Result. Child
carriers, request/Bzl Result Arcs beyond semantic ownership, frozen module/eval
heap, merge/label/export/event vectors, maps, caches, interners, stores, locks,
tasks, revisions and certificates are compute-local or forbidden. No lock spans
a DICE compute.

A completed typed outer retains only the opaque request-child error, or the
completed request aggregate plus decisive request/stage/frontier error. It
retains no epoch, loaded-definition Result, Bzl carrier or frozen module.

## Discriminating proof

Extend only the existing colocated loading tests. Require:

- observation-key distinct hash/Display and Complete-only equality/validity;
- synthetic empty/request/prior/current prefix terminals, first duplicate Arc,
  conflict and operation mismatch;
- real Legacy/Observed family exclusion and exact request/Bzl order;
- first/middle/last label, Need, outer, Bzl semantic, export and wrong-kind
  suppression;
- exact semantic Result/manifest/projection and request context;
- fresh ordered child batches, warm-parent silence, reused-child no-batch,
  semantic-failure batch ownership and parent eventlessness;
- independent held-handle request/Bzl/export A -> B -> A;
- poll-drop publication silence and same-DICE recovery; and
- an isolated production-slice/activation assertion excluding
  prepared/pure/instantiated/validated/root-mapping/generated/public keys.

Reuse accepted lower identity/event/cancellation proof rather than duplicating
recursive Bzl matrices. Add no test hook or fixture.

## Compatibility and validation

Exact: existing loaded-definition values, semantic errors, request order,
manifests, projections and child Bzl events.

Slug-native: private observed key/carrier, typed outer, cumulative epoch and
internal Display token.

Deferred: prepared/pure/instantiated/validated evaluation, root mapping,
generated repositories, public/bootstrap activation, M8/M7B and exact Bazel
identity bytes.

Run serially:

1. `CARGO_BUILD_JOBS=1 cargo test -p slug_loading_v2
   module_extension_definition_loading_tests::observed_loaded --quiet`;
2. the exact focused test names added if the prefix differs;
3. `CARGO_BUILD_JOBS=1 cargo test -p slug_loading_v2 --quiet`;
4. `CARGO_BUILD_JOBS=1 cargo check -p slug_core_v2 --quiet`;
5. `cargo fmt --all -- --check`; and
6. `git diff --check`.

## Terminal and stops

Implementation ACCEPT records accounting/validation and activates only the
docs-only prepared/evaluation observation frontier. REPLAN before wider or
semantically different work.

STOP a second file/key/owner; Bzlmod/API/export/caller changes; prepared/pure/
instantiated/validated/root-mapping/generated/public activation; moved or
duplicated child events; retained Starlark evaluator/callable heap; full scan,
Need union or speculative tasks; direct Host reads; locks across DICE; new
fixture/oracle/hook; proof/cap waiver; milestone closure; M8/M7B or exact
identity-byte work. M7 remains partial and M7A -> M8 -> M7B remains.

## Immediate predecessor

Carrier promotion `99c23033`, from design `83b5ac7a`, exposes only the
accepted observed request seam. Scheduling commits `900c7b54` and
`99b2bf01` select this owner and clarify invariant Host-Bzl DICE errors plus
Reused/no-batch child behavior. The checkout is otherwise clean.
