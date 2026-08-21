# Current Slug V2 Packet

Packet: `WP-6-7A-host-prepared-module-extension-inputs-observation-implementation`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Scheduling/design base: `3738b2b4`
Rust base: `50881fc0`

## Goal and authority

Implement only one private matching-family observed sibling for
`HostPreparedModuleExtensionInputsKey`. Share the existing prepared semantics
between Legacy and Observed modes, retain one local Result Arc plus compact
transaction-local epoch, and keep pure invocation and every upper owner
inactive.

Write authority is exactly `app/slug_loading_v2/src/bzl_module.rs`, baseline
8,288 physical lines with the owning test module at line 5,452. Caps are <=380
production, <=1,050 proof and <=1,430 aggregate semantic at <=9,725 physical.
Add at most six direct helpers and three observed-parent tests; keep the shared
driver below 150 lines and every changed helper/test below 200. Every other
Rust file, test, fixture, oracle, Cargo/BUILD target, API, export, caller and
plan is read-only.

The large file remains cohesive for this packet because it already owns the
legacy prepared key and pure preparation function, the private loaded child
observation, the imported Bzlmod child surface, activation/dependency tracker,
real extension fixtures and existing prepared tests. Splitting the one private
sibling would create a new visibility seam and duplicate proof plumbing.

## Frozen owner and driver

Add only private `HostPreparedModuleExtensionInputsObservationKey`,
`ObservedHostPreparedModuleExtensionInputs`, one private typed outer, and one
Legacy/Observed driver. Preserve this order:

1. compute selected evaluation-input requests;
2. finish their child boundary and semantics;
3. compute loaded module-extension definitions;
4. finish their child boundary, merge observations and semantics; and
5. run unchanged `prepare_module_extension_inputs` join/schema/class/coercion.

Legacy computes only the two legacy child keys with empty epochs. Observed
computes only `HostSelectedExtensionEvaluationInputRequestsObservationKey`
then `HostLoadedModuleExtensionDefinitionsObservationKey`. Project Legacy by
moving the exact local prepared Result Arc from the shared driver. Add no
adapter key, public/exported type, caller or second semantic owner.

## Prefix and terminal algebra

The private outer has exactly three variants/stages:

- `Raw`, carrying the opaque Bzlmod evaluation-input observation outer and no
  completed semantic context;
- `Definitions`, carrying the completed raw semantic aggregate plus the loaded-
  definition child outer; and
- `Merge`, carrying the completed raw semantic aggregate plus the exact
  `ObservedPathFrontierError` from combining the definition epoch.

Raw DICE compute failure remains semantic `RawCompute` with an empty epoch.
Raw Need or typed outer is immediate and carrierless. A Complete raw carrier is
accepted before raw semantics; raw semantic failure returns `Raw` with the raw
epoch and suppresses definitions.

Definitions DICE compute failure remains semantic
`Definitions { error: Err }` with the raw prefix. Definitions Need or typed
outer is immediate and carrierless; the typed outer retains raw semantic context but publishes no
parent carrier. On Complete, merge the definition epoch into the raw prefix
left-first before inspecting definition semantics. Equal duplicate demands
retain the raw-side Arc. A valid-epoch same-demand value conflict returns the
carrierless `Merge` outer. An operation mismatch rejected by a lower child
remains in that child's carrierless `Raw` or `Definitions` outer. Definition
semantic failure returns existing
`Definitions { error: Ok }` with the merged prefix.

Only after both child semantics succeed may existing request-aggregate/count/
order joins, tag-schema validation, tag-class lookup and repository-aware
attribute coercion run. Every `AfterInputs` error and success retains the full
merged prefix. First terminal suppresses every later child and local step; do
not union Needs or scan/speculate past a terminal.

## Events and retention

The prepared parent owns no event batch. Accepted request/root/Host-Bzl children
remain sole owners of their exact batches. Fresh evaluation follows raw then
definitions dependency order; shared lower children may be Reused without
batch replay. Warm parent reuse is silent, and changed-parent/unchanged-child
recomputation reports only the child family's accepted Reused/None behavior.
Need, typed outer and cancellation publish no parent carrier or batch.

Retain only the local prepared semantic Result Arc, compact cumulative epoch and
semantic projections already reachable from that Result. Child carriers,
child Result Arcs, full frozen Bzl modules/heaps, join/coercion Vec/SmallMap
scratch, event data, locks and tasks remain compute-local. Add no side store,
cache, interner or lock across a DICE compute.

## Required proof

Use at most three observed-parent tests covering:

1. key hash/Display, carrier accessors, Complete/Need/outer equality/validity,
   left-first duplicate Arc, valid-epoch conflict at `Merge`, lower child
   conflict/operation mismatch at `Raw` or `Definitions`, and every finisher
   prefix/terminal stage; reuse the accepted opaque raw-child outer proof rather
   than constructing or inspecting it in loading, and add no malformed epoch or
   synthetic hook;
2. exact legacy/observed prepared Result parity, dependency order, raw-first
   suppression, reachable raw/definition/local errors and success, child-only
   fresh batches, warm silence and isolated Reused/None; and
3. held parent/raw/definition Result+epoch A -> B -> A across independent raw
   and definition changes, poll-drop/no-publication/recovery, per-transaction
   carrier-to-global-epoch association and exact nonactivation.

Across transactions compare semantic Results and frozen projections; treat
epochs as transaction-local frontiers and require pointer identity only for an
exact cached value proven Reused. Preserve all held historical handles.

Production-slice and all-key rows must exclude legacy/observed family mixing and
the exact upper families `HostPureModuleExtensionInvocationsKey`,
`HostInstantiatedModuleExtensionRepositoriesKey`,
`HostValidatedModuleExtensionRepositoriesKey`,
`HostRootRepositoryMappingKey`,
`HostCanonicalSelectedModuleDefinitionKey`,
`HostGeneratedRepositoryDefinitionKey` and `slug-command:`. Do not infer the
new parent contract only from accepted child tests.

## Validation, compatibility and terminal

Reuse accepted Bazel 9.2 loading/Bzlmod evidence; add no oracle. Run serially:

- focused `observed_prepared_` and protected `real_prepared_inputs_` tests;
- protected `observed_loaded_` loading tests;
- full `cargo test -p slug_loading_v2`;
- direct dependent `cargo check -p slug_core_v2`;
- `cargo fmt --all -- --check`; and
- `git diff --check`.

Existing prepared values/errors/order, schema/class validation, attribute
coercion and child events remain exact Bazel 9 compatibility. The private key,
carrier, typed outer and shared-Arc epoch association are Slug-native.

Implementation ACCEPT returns only to a docs-only pure-invocation frontier
audit. STOP semantic/event/equality/retention drift, second file/key/adapter/
owner, API/export/caller change, retained Starlark heap, lock across DICE,
pure/upper activation, fixture/oracle work, proof/cap waiver, milestone closure,
M8/M7B or exact identity work. REPLAN before widening. M7 remains partial and
M7A -> M8 -> M7B remains.

## Immediate predecessor

Design `3738b2b4` confirms the prepared key at `bzl_module.rs:3051-3115` owns
the raw-first join and has one production consumer at `module_extension.rs:158`.
Accepted children are the public opaque evaluation-input observation API at
`selected_repo_spec.rs:3937-4000,4275-4303` and the same-module loaded-definition
observation at `bzl_module.rs:2475-2509,2833-2870`. Existing preparation
semantics are at `bzl_module.rs:2920-3015`; existing real prepared tests begin
at `bzl_module.rs:6244`.
