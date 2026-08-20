# Current Slug V2 Packet

Packet: `WP-6-7A-loaded-module-extension-definitions-observation-design`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Scheduling base: `99c23033`
Accepted predecessor: `99c23033`

## Design authority

This packet is docs-only. Write authority is exactly the canonical plan, this
manifest, the Stage 6 owner plan and the orchestration routing log, at net caps
<=40/<=220/<=180/<=30 and <=470 aggregate. Rust, tests, fixtures, oracles,
Cargo/BUILD metadata, APIs, exports and callers are read-only.

The ordinary terminal rollover changes canonical/current/Stage only. Routing
remains unchanged unless the design reaches formal `REPLAN` or records a
reusable routing lesson.

## Accepted prerequisites and natural owner

Accepted `99c23033` exposes the observed definition-request key, carrier and
opaque outer through Bzlmod's doc-hidden API. Loading already depends one way on
Bzlmod. The loading-crate `HostBzlModuleObservationKey` already owns each root
Bzl module's complete source/recursive-load epoch and local evaluation
`EventBatch`.

The existing `HostLoadedModuleExtensionDefinitionsKey` is the natural owner.
It consumes ordered definition requests, then for each request parses the root
Bzl target, computes the Host Bzl module, selects the named export, rejects a
wrong kind and projects the heap-independent module-extension definition.
`HostPreparedModuleExtensionInputsKey` is its sole direct semantic consumer
and independently computes evaluation-input requests first.

No carrierless prerequisite remains. Prepared/pure/instantiated/validated
extension evaluation, root mapping, generated repository definitions and
public/bootstrap consumers are later or parallel work.

## Design objective

Freeze one private matching Legacy/Observed loaded-definition owner in
`slug_loading_v2`. Determine the exact shared driver, carrier, typed outer,
proof matrix and caps without implementing or activating a caller.

Preserve the legacy value, error, request order, `BzlLoadManifest`,
heap-independent `ModuleExtensionDefinitionProjection` and request association.
The observed carrier may add only one local Result Arc and cumulative
`PathObservationEpoch`.

## Order and terminal algebra

The shared driver must preserve:

1. definition requests;
2. for each request in source order:
   root Bzl-target parse;
3. that request's Host Bzl module;
4. named-export lookup;
5. `FrozenModuleExtensionDefinition` downcast; and
6. manifest/projection append.

Observed mode computes the accepted observed request child first. For each
request whose label parses, it computes exactly one
`HostBzlModuleObservationKey`. Merge every Complete epoch strictly left-first
into the cumulative prefix before inspecting that child's Bzl semantic result
or performing export/wrong-kind projection. Equal duplicate observations retain
the earliest/request-side Arc. Conflict or operation mismatch becomes one typed
outer identifying the Bzl request/stage.

Stop at the first request compute/semantic, label, Bzl compute/Need/outer/
semantic, merge, export or wrong-kind terminal. A first terminal suppresses all
later request work; a middle terminal follows only preceding successes; a last
terminal follows every prior success. There is no full scan, speculative
parallelism or Need union.

Freeze exact empty/request/prior-Bzl/current-Bzl prefix behavior for DICE compute
and semantic terminals. Need and child typed outer publish no provisional parent
carrier or event batch. Preserve request and Bzl child DICE-compute failures as
`host_dice_invariant` panics in both families; do not convert them into a new
loaded-definition semantic error or prefix.

## Families, events and lifetime

Legacy computes only legacy request/Bzl keys with empty epochs. Observed computes
only observed request/Bzl keys. One shared driver must exclude mixed families.

The parent emits no events. Each observed Host Bzl child remains the sole owner
of its exact local batch; successful ordered children replay in request order.
Warm parent reuse is silent. If parent recomputation reaches an unchanged Host
Bzl child, DICE reports its existing Reused activation with no batch; it must
not reevaluate the child or re-emit the cached local batch.
Cancellation/poll-drop before parent publication leaves no provisional parent
value or parent batch, and a same-DICE retry recovers.

Retain only the owner-local loaded-definition Result Arc, cumulative epoch and
semantic request/manifest/projection state already reachable from that Result.
Observed request/Bzl carriers, frozen module handles beyond existing semantic
ownership, label/export/downcast/merge/event vectors, maps, caches, interners,
stores, locks, tasks, Host reads, revisions and certificates remain compute-local
or forbidden. No lock may span a DICE compute.

## Proof and compatibility

Reuse the accepted request and Host-Bzl identity, event, cancellation and
lifecycle proof. Add discriminators only for the new composition:

- distinct key/hash/Display and Complete-only equality/validity;
- Legacy/Observed family exclusion and exact child order;
- first/middle/last Need/outer/compute/semantic/export/wrong-kind suppression;
- left-first request/each-Bzl epoch merging, earliest duplicate Arc and typed
  conflict/operation mismatch;
- exact ordered child batches, parent/warm/cancel silence and direct-child reuse;
- held Result/epoch handles across independent request, each Bzl definition and
  pure export-projection A -> B -> A, with unaffected Arcs retained; and
- zero prepared/pure/instantiated/validated/root-mapping/generated/public
  activation.

Use existing Bazel 9.2 `RegularRunnableExtension.load` and
`SingleExtensionEvalFunction` source evidence; add no oracle unless the design
finds a genuinely missing Bazel-visible discriminator.

Exact compatibility is existing loaded-definition values, errors, order,
manifests, projections and child Bzl events. The private observed key/carrier,
typed outer and cumulative epoch association are Slug-native. Upper evaluation,
root mapping, generated/public/bootstrap work, M8/M7B and exact identity bytes
remain deferred.

## Terminal and stops

Reach exactly one independently reviewed terminal:

1. one bounded loaded-definition observation implementation successor; or
2. formal `REPLAN` naming a contradictory ownership/error/event fact and one
   next packet.

STOP Rust/test/fixture/oracle/Cargo/BUILD changes; Bzlmod changes or exports; a
second owner/prerequisite; prepared/evaluation/root-mapping/generated/public
activation; moved or duplicated Bzl events; retained Starlark evaluator/callable
heap; full scan/Need union/speculative tasks; direct Host reads or locks across
DICE; proof/cap waiver; milestone closure; M8/M7B or exact identity-byte work.
M7 remains partial and M7A -> M8 -> M7B remains.

## Immediate predecessor

Implementation `99c23033`, from design `83b5ac7a` and Rust base `e82057f2`,
promotes only the accepted observed request seam. Accounting is +21 production,
+4 colocated proof, +29 external proof and +54 aggregate at 11,676/409/29
physical lines. Focused/full Bzlmod, external API, dependent loading, formatting
and diff gates pass; independent final review returned `ACCEPT`.
