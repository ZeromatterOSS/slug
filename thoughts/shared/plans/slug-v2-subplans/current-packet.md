# Current Slug V2 Packet

Packet: `WP-6-7A-loaded-module-extension-definitions-observation-frontier-audit`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Scheduling base: `e82057f2`
Accepted predecessor: `e82057f2`

## Audit authority

This packet is docs-only. Write authority is exactly the canonical plan, this
manifest, the Stage 6 owner plan and the orchestration routing log, at net caps
<=40/<=220/<=180/<=30 and <=470 aggregate. Rust, tests, fixtures, oracles,
Cargo/BUILD metadata, APIs, exports and callers are read-only.

The ordinary terminal rollover changes canonical/current/Stage only. Routing
remains unchanged unless the audit reaches formal `REPLAN` or records a
reusable routing lesson.

## Learned facts and research basis

Accepted `e82057f2` adds the private callerless evaluation-input request
observation owner. The private definition-request observation key/carrier is
still inside `slug_bzlmod_v2`; its legacy key/value are already hidden exports
consumed by `slug_loading_v2`.

`HostLoadedModuleExtensionDefinitionsKey` is in `slug_loading_v2`, which already
depends on Bzlmod. It currently preserves ordered definition requests, then for
each request parses the root Bzl target, computes `HostBzlModuleEvalKey`, selects
the named export and projects its heap-independent module-extension definition.
Its sole direct semantic consumer is `HostPreparedModuleExtensionInputsKey`,
which separately computes accepted evaluation-input requests first.

Accepted `HostBzlModuleObservationKey` already owns the complete source and
recursive-load epoch plus the one local Bzl evaluation `EventBatch`. Pinned
Bazel 9.2 `RegularRunnableExtension.load` establishes canonical Bzl load before
exported `ModuleExtension` selection, and `SingleExtensionEvalFunction`
consumes the loaded definition before evaluation/generated repositories. Reuse
that source basis and accepted lower proof; add no oracle.

The relevant DICE contract is `docs/developers/dice.md`: producer keys own
semantic discovery, equality includes every admitted input, warm reuse must be
explained by dependencies, and no lock spans a DICE compute. Buck2/Bazel donor
code is concept/test evidence only; no donor scheduler, semantic side store or
fallback is admitted.

## Frontier question

Determine the uniquely smallest complete next owner above the accepted request
and Bzl-module carriers. Audit whether it is an observed
`HostLoadedModuleExtensionDefinitionsKey`, or whether one narrower cross-crate
visibility/carrier prerequisite must be designed first. Do not presume that
exporting the private Bzlmod carrier and implementing the loading observation
belong in one packet; choose the smallest dependency-safe boundary.

Trace only far enough through `HostPreparedModuleExtensionInputsKey`, pure/
instantiated/validated extension owners, root mapping, generated repository
definitions and public/bootstrap consumers to reject false prerequisites. Do
not combine loaded definitions with evaluation inputs or any upper owner merely
because prepared inputs later joins them.

For each viable candidate establish:

- the natural DICE key/value producer, all direct consumers and the exact
  one-way crate visibility seam;
- exact definition-request -> per-request label -> Host Bzl module -> export ->
  wrong-kind/success order, including first/middle/last terminal suppression;
- matching Legacy/Observed families and the request/each-Bzl Complete epoch
  merge order, earliest exact duplicate Arc, conflict/operation mismatch,
  Need/typed-outer/DICE-compute/semantic precedence and whether any full scan or
  Need union exists;
- event ownership and exact ordered child batches, parent eventlessness, warm
  silence, cancellation/poll-drop recovery and direct-child DICE reuse;
- retained local Result/manifest/definition/epoch state versus compute-local
  request, module, export, merge and event scratch; and
- independent request, each loaded Bzl definition and pure export projection
  A -> B -> A behavior with held Result/epoch handles and unaffected Arcs.

## Decision and non-decisions

Reach exactly one terminal:

1. one independently reviewed smallest-owner design;
2. one uniquely smaller bounded cross-crate carrier/evidence prerequisite; or
3. formal `REPLAN` naming the contradictory ownership fact and one next packet.

Any design may name at most one implementation successor. No implementation,
export, caller or public activation is authorized by this audit.

Preserve admitted loaded-definition values, errors, order, manifests,
heap-independent projections and child events as exact. A private typed outer
and shared-Arc cumulative epoch association is Slug-native. Prepared/pure/
instantiated/validated evaluation, root mapping, generated repositories,
public/bootstrap activation, M8/M7B and exact identity bytes remain deferred.

## Request, lifetime, evidence and stops

The candidate is DICE-retained semantic state. It may retain only the owner-
local Result Arc plus a compact cumulative epoch and semantic manifest/
definition state already owned by that Result. Child carriers, frozen-module
handles beyond existing semantic ownership, traversal/export/event scratch,
maps, caches, interners, stores, locks, tasks, direct Host reads, revision or
certificate state remain compute-local or forbidden. Need/outer/cancellation
publishes no provisional parent value or batch.

Reuse accepted request and Host-Bzl identity, event, cancellation and lifecycle
proof. The audit must identify any genuinely missing discriminator before
authorizing new evidence. There is no fallback ledger because no bridge or
fallback is permitted.

STOP Rust/test/fixture/oracle/Cargo/BUILD changes; a second semantic owner;
reverse crate dependency; generic graph/route/mapping export; moved or
duplicated Bzl event ownership; retained Starlark callable/evaluator heap;
prepared/evaluation/root-mapping/generated/public activation; proof/cap waiver;
milestone closure; M8/M7B or exact identity-byte work. REPLAN before widening.
M7 remains partial and M7A -> M8 -> M7B remains.

## Immediate predecessor

Implementation `e82057f2`, from Rust base `094ba075` and accepted design
`1fdf641b`, completes the private evaluation-input request observation owner in
`selected_repo_spec.rs`. Request then root epochs merge left-first before
semantics; Need/outer is carrierless; root merge failures remain typed; the
parent is eventless and retains one local Result Arc plus its compact epoch.

Accepted accounting is +286 production/+681 proof/+967 aggregate at 11,657
physical lines. Four focused tests and the full 525-unit plus integration/doc
Bzlmod suite pass; formatting and diff hygiene pass. Independent correction
rereview returned `ACCEPT`. One unrelated mixed-horizon ordering assertion
failed once, then passed isolated and on a complete replay; it remains residual
inherited test-order flake risk rather than accepted semantic drift.
