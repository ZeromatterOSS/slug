# Current Slug V2 Packet

Packet: `WP-6-7A-host-instantiated-module-extension-repositories-observation-implementation`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Scheduling and Rust base: `0dcf2eea`

## Goal and authority

Add one private observed sibling for
`HostInstantiatedModuleExtensionRepositoriesKey`. Preserve all legacy
instantiation semantics, order, errors and pure-child event ownership. Retain
only the exact local semantic Result Arc plus the pure child transaction-local
epoch. Do not activate its sole upper consumer, validation.

Write authority is exactly
`app/slug_loading_v2/src/module_extension_repository_instantiation.rs`,
baseline 1,380 physical lines with the owning test module at 532. Caps are
<=220 production, <=700 proof, <=920 aggregate semantic and <=2,300 physical.
Add at most six production and five test helpers plus three observed tests; the
shared driver stays below 120 lines and every changed helper/test below 200.
Every other Rust file, test, fixture, oracle, Cargo/BUILD target, API, export and
caller is read-only. The owner remains cohesive because this file already owns
the legacy key, instantiation/schema/label functions, real fixture and sole
validation boundary; splitting would expose private retained values or
duplicate proof plumbing.

## Exact owner and driver

Add exactly these private types:

1. `HostInstantiatedModuleExtensionRepositoriesObservationKey`, a newtype over
   `HostInstantiatedModuleExtensionRepositoriesKey` with the same workspace
   identity and `observed-{legacy Display}`;
2. `ObservedHostInstantiatedModuleExtensionRepositories`, holding the exact
   local `Arc<Result<HostInstantiatedModuleExtensionRepositories,
   HostInstantiatedModuleExtensionRepositoriesError>>` and one
   `PathObservationEpoch`; and
3. `HostInstantiatedModuleExtensionRepositoriesObservationError`, with exactly
   `Pure(HostPureModuleExtensionInvocationsObservationError)`.

Keep all fields, result aliases and accessors private. Preserve the legacy key.
Use matching Debug/Clone/PartialEq/Eq/Hash/Allocative derives for the key and
Debug/Clone/PartialEq/Eq/Allocative/Dupe for carrier and outer as their fields
permit. Both keys use Complete-only equality and validity.

Refactor one Legacy/Observed driver. Legacy computes only
`HostPureModuleExtensionInvocationsKey`; Observed computes only
`HostPureModuleExtensionInvocationsObservationKey`. After successful pure
semantics, both invoke the existing `instantiate_repositories` exactly once
and project the same semantic Result.
Do not duplicate or reorder the prepared/request receipt join, generated
canonical namespace, base/call/override mapping precedence, repository-rule
schema lookup, supplied attribute order, legacy/None omission,
mandatory/default validation, label resolution/visibility or ordered
`RepoSpec` construction.

## Terminal, epoch and event law

- Pure DICE compute failure remains semantic `InvocationsCompute` with an empty
  epoch. Need remains immediate Need.
- Observed pure opaque error maps only to the carrierless `Pure` outer. Do not
  inspect or rewrap its private terminal.
- Accept a Complete pure carrier before semantics. Pure semantic failure remains
  existing `Invocations` and retains the complete child epoch. Success passes
  the cloned pure value and the same epoch into local instantiation.
- This parent has one observed child: add no merge stage and do not rebuild,
  union or validate the epoch.
- Local count/request Join, Namespace and Attribute terminals retain the full
  pure epoch and exact `AfterInvocations` predecessor/completed/request/current/
  call prefixes. Success retains the same epoch. First terminal wins in exact
  request and call order.

Instantiation stores no evaluation event data. Fresh pure load and
invocation-print batches stay on the pure subtree; every instantiation parent
row is batchless. Need, opaque outer and cancellation publish no parent carrier
or batch. Warm parent reuse is silent, and a Reused pure child never replays a
batch.

The carrier's Result already owns the predecessor, completed/current repository
projections and `RepoSpec`s. Driver mode, child projection and construction
vectors are compute scratch. Retain no evaluator heap, duplicate request state,
event batch, side cache, task, lock or command state. Ordinary DICE owns
equality cutoff, invalidation, eviction and cancellation; dropped work cannot
publish a partial carrier, and recovery recomputes normally.

## Proof and validation

Add exactly:

- `observed_instantiation_identity_finisher_and_terminal_algebra`;
- `observed_instantiation_real_order_events_and_parity`; and
- `observed_instantiation_lifecycle_cancellation_and_nonactivation`.

Together prove key/hash/Display/equality/validity; one-child/no-merge projection;
Join/Namespace/Attribute prefixes; legacy/observed semantic parity for success,
pure failure and local failures; exact legacy/observed family and single-child
dependency rows; Need and first-terminal suppression; child-only exact print
batches, batchless parents and warm silence; held Result/carrier/epoch A -> B ->
A for call/schema/mapping changes; same-semantic/different-epoch observation
metadata; each carrier epoch as a subset of its own transaction global epoch;
poll-drop recovery; and zero
`HostValidatedModuleExtensionRepositoriesKey` activation. Compare separate
transactions semantically and require Arc identity only for an exact cached
value proven Reused.

The pure outer stays opaque. Reuse its accepted same-module HostBzl/Merge proof
and add only a static private-producer scan plus the real observed dependency
row; do not expose an accessor, construct invalid epochs or edit
`module_extension.rs`.

Reuse Bazel 9.2 `RegularRunnableExtension.run`,
`SingleExtensionEvalFunction.compute` and `ModuleExtensionResolutionTest`
evidence; add no oracle. Buck2 DICE incrementality/cancellation/activation tests
are concept/test evidence only. Run serially:

- focused `observed_instantiation_` tests;
- protected `pure_instantiation_`, `real_key_`, `observed_pure_` and validation
  `real_validation_` tests;
- full `cargo test -p slug_loading_v2`;
- direct dependent `cargo check -p slug_core_v2`;
- `cargo fmt --all -- --check`; and
- `git diff --check`.

## Compatibility and terminal

Existing values/errors/order, namespaces, attribute/label semantics, `RepoSpec`
projections, DICE equality and pure-owned events remain exact Bazel 9
compatibility. The private key/carrier/outer and Result-Arc/epoch association
are Slug-native. Validation observation, generated/public/root-mapping/bootstrap
activation and exact Bazel configuration/output/ActionKey bytes remain
unsupported/deferred.

ACCEPT returns only to a docs-only
`HostValidatedModuleExtensionRepositoriesKey` observation-frontier audit. STOP
a second file/key/owner/adapter, visibility/lib export/caller, legacy or pure
semantic drift, parent event batch, epoch merge/rebuild, retained evaluator
state, lock/task across DICE, validation/generated/public/root-mapping/bootstrap
activation, fixture/oracle work, proof/cap waiver, milestone closure, M8/M7B or
exact identity work. REPLAN before widening. M7 remains partial and M7A -> M8
-> M7B remains.

## Immediate predecessor

`0dcf2eea` exposes exactly the accepted opaque pure observation key, carrier,
concrete Result accessor and epoch accessor to this sibling without a caller.
Live trace proves instantiation has no other semantic child and validation is
its sole production consumer.
