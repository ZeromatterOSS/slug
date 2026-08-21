# Current Slug V2 Packet

Packet: `WP-6-7A-host-pure-module-extension-invocations-observation-implementation`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Scheduling/design and Rust base: `f76bab3a`

## Goal and authority

Implement one private matching-family observation owner for
`HostPureModuleExtensionInvocationsKey`. Share its exact prepared/preflight/
invocation semantics between Legacy and Observed modes, associate the local
Result Arc with the cumulative transaction-local Host frontier, preserve its
own print-event batch, and keep instantiation and every upper owner inactive.

Write authority is exactly
`app/slug_loading_v2/src/module_extension.rs`, baseline 1,629 physical lines
with test support at 767 and the owning test module at 869. Caps are <=480
production, <=900 proof, <=1,380 aggregate semantic and <=3,010 physical. Add
at most eight production helpers, six test helpers and three observed-parent
tests. Keep shared driver/preflight/invocation helpers below 180 lines and every
changed helper/test below 200. Every other Rust file, test, fixture, oracle,
Cargo/BUILD target, API, export, caller and plan is read-only.

This module remains cohesive because it already owns the legacy DICE key,
extension evaluator ABI, print capture, repository-rule receipts, real fixtures
and the sole instantiation handoff. A split would expose private evaluator and
test seams without separating a semantic responsibility.

## Frozen owner and shared driver

Add only private `HostPureModuleExtensionInvocationsObservationKey`, private
`ObservedHostPureModuleExtensionInvocations` with local pure Result Arc and
`PathObservationEpoch`, one private typed outer, and one Legacy/Observed driver.
The key mirrors the legacy key's workspace identity and uses
`observed-{legacy Display}`. Both keys remain Complete-only equal/valid.
Give the key the legacy key's Debug/Clone/PartialEq/Eq/Hash/Allocative derives;
give carrier and outer Debug/Clone/PartialEq/Eq/Allocative/Dupe. Carrier fields
stay private with borrowed `result()` and `observations()` accessors used only
by same-module proof and the later visibility audit.

The outer has exactly three variants:

- `Prepared(HostPreparedModuleExtensionInputsObservationError)`, with no
  completed pure semantic context;
- `HostBzl { prepared, index, error }`, carrying the completed prepared Arc,
  current preflight index and lower `ObservedPathFrontierError`; and
- `Merge { prepared, index, error }`, carrying the same compact prefix and the
  exact parent merge error.

The prepared Arc plus index identifies every earlier successful preflight via
the ordered prepared inputs and definitions. Do not retain a child carrier,
Frozen Bzl module, duplicated request list or preflight projection in the
outer.

Preserve this exact shared-driver order:

1. compute and finish prepared inputs;
2. for each prepared input in order, check unsupported factors then parse the
   root-package Bzl target;
3. compute and finish that exact Host-Bzl module;
4. check manifest, export existence/kind and definition projection drift;
5. only after every preflight succeeds, invoke implementations in input order;
6. require `None` and retain ordered repository-rule call receipts.

Legacy computes only `HostPreparedModuleExtensionInputsKey` and
`HostBzlModuleEvalKey` and supplies empty epochs. Observed computes only
`HostPreparedModuleExtensionInputsObservationKey` and
`HostBzlModuleObservationKey`. Project Legacy by moving the exact local pure
Result Arc from the shared driver and asserting its epoch empty. Add no adapter,
second execution path or speculative/parallel task. Refactor the current
196-line `invoke_all` into bounded shared preflight and invocation helpers;
retain the existing prepared-injected test seam through those same helpers.

## Prefix and terminal algebra

Prepared DICE compute failure remains semantic `PreparedCompute` with an empty
epoch. Prepared Need or opaque typed outer is immediate and carrierless. Accept
a Complete prepared carrier and epoch before inspecting its semantics;
prepared semantic failure returns existing `Prepared` with that epoch and
suppresses all preflight/invocation work.

For input `index`, local `UnsupportedFactors` and `Label` errors occur before
its Host-Bzl compute and retain the prepared plus earlier-Bzl epoch. Host-Bzl
DICE compute failure remains existing `AfterPrepared` with `Invocation` error
and that prefix. Host-Bzl Need or lower typed outer is immediate and carrierless; map
the latter to `HostBzl { prepared, index, error }`.

On a Complete Host-Bzl carrier, merge its epoch into the accumulated prefix
left-first before inspecting its semantic Result. Equal duplicate demands keep
the accumulated left Arc. A lower operation mismatch remains `HostBzl`; a
valid-epoch same-demand value conflict becomes `Merge`. Host-Bzl semantic error
maps to existing `AfterPrepared` with `Bzl` error and the merged prefix.
Manifest mismatch, missing/wrong-kind export and projection mismatch remain existing Drift errors
with that prefix. First terminal suppresses every later preflight and all
invocation; do not union Needs or scan after failure.

Invocation begins only with the full preflight epoch. Preserve `module_ctx`
allocation, frozen implementation lifetime, invocation order, print capture,
repository-rule state, completed receipt/current-call prefixes, evaluator error
and non-None Result behavior exactly. Every invocation terminal and success
retains the full epoch.

## Events, retention and lifecycle

The pure parent continues to own exactly the extension implementation-print
batch. Prepared and Host-Bzl children remain sole owners of their batches. A
shared predicate publishes one pure batch only when the driver returns a
semantic Complete after prepared semantics succeeded: success or
`AfterPrepared`, including an empty batch for preflight errors. Prepared/
PreparedCompute, Need, typed outer and cancellation publish no pure batch.
Fresh child events precede pure invocation prints; Reused children do not
replay; warm pure reuse is silent. Both Legacy and Observed store through the
same predicate before projection.

Retain only the pure semantic Result Arc, cumulative epoch and prepared/
receipt/call projections already reachable from the Result. Frozen modules and
callables, tag-class/preflight Vecs, invocation Module/Heap/context, print
capture, repository-rule state, event assembly and child carriers/results are
compute-local. Add no side store, global cache, interner, manual task or lock
across a DICE computation.

Use at most three tests:

1. `observed_pure_identity_finisher_and_prefix_algebra` proves key hash/Display,
   carrier accessors, Complete/Need/outer equality/validity, every finisher,
   left-first duplicate Arc, valid-epoch Merge conflict and exact
   HostBzl-vs-Merge stage/prefix. Reuse accepted opaque prepared-outer and lower
   operation-mismatch proof; construct no malformed epoch or visibility hook.
2. `observed_pure_real_order_terminals_events_and_parity` proves real
   Legacy/Observed Result parity; prepared -> ordered Host-Bzl -> all-preflight
   -> ordered invocation; reachable or prepared-injected prepared, factor/label, Bzl, drift,
   invocation, non-None and success terminals; first-terminal suppression;
   child-only load batches; exact parent prints including empty Complete,
   failure prefix, warm silence and isolated Reused/None.
3. `observed_pure_lifecycle_cancellation_and_nonactivation` preserves held
   parent/prepared/Host-Bzl Result+epoch handles across root-tag and
   implementation-source A -> B -> A plus an observation-metadata axis with the
   same semantic Result and a different epoch; validates each carrier epoch
   against its own transaction's global epoch; proves poll-drop no-parent-publication and recovery; and proves
   exact family/upper nonactivation.

Across transactions compare semantic Results and frozen projections, not whole
epoch maps. Require pointer identity only for an exact cached value proven
Reused. Preserve every held historical handle. Observed activation/dependency
rows must exclude legacy prepared/Host-Bzl/pure keys and exact upper
`HostInstantiatedModuleExtensionRepositoriesKey`,
`HostValidatedModuleExtensionRepositoriesKey`,
`HostRootRepositoryMappingKey`,
`HostCanonicalSelectedModuleDefinitionKey`,
`HostGeneratedRepositoryDefinitionKey` and `slug-command:`. Reverse-family
legacy proof excludes all observed siblings. The sole production consumer at
`module_extension_repository_instantiation.rs:191` remains unchanged and
inactive.

## Validation, compatibility and terminal

Reuse pinned Bazel 9.2 `RegularRunnableExtension.load` and
`SingleExtensionEvalFunction` evidence plus accepted prepared/Host-Bzl/pure
tests; add no oracle. Run serially:

- focused `observed_pure_` tests;
- protected `real_repository_rule_`, `observed_prepared_` and `observed_bzl_`;
- full `cargo test -p slug_loading_v2`;
- direct dependent `cargo check -p slug_core_v2`;
- `cargo fmt --all -- --check`; and
- `git diff --check`.

Existing pure values/errors/order/evaluator ABI/repository-rule receipts and
event behavior remain exact Bazel 9 compatibility. The private key, carrier,
typed outer and Result-Arc/epoch association are Slug-native. Instantiation,
validation, generated/public/root-mapping/bootstrap activation and exact Bazel
configuration/output/ActionKey bytes remain unsupported/deferred.

Implementation ACCEPT returns only to a docs-only instantiation frontier
audit. STOP semantic/equality/event/retention drift, second file/key/adapter/
owner, visibility/export/caller change, retained Starlark heap/callable, lock or
manual task across DICE, upper activation, fixture/oracle work, proof/cap
waiver, milestone closure, M8/M7B or exact identity work. REPLAN before
widening. M7 remains partial and M7A -> M8 -> M7B remains.

## Immediate predecessor

Visibility implementation `f76bab3a` exposes only the accepted prepared
observation key/carrier/opaque outer inside loading and proves sibling use
without computing it. Pure's observed prepared and Host-Bzl children are now
both available; no smaller carrier or evidence prerequisite remains.
