# Current Slug V2 Packet

Packet: `WP-6-7A-host-validated-module-extension-repositories-observation-implementation`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Scheduling and Rust base: `4b5e9d05`

## Goal and authority

Add one private observed sibling for
`HostValidatedModuleExtensionRepositoriesKey`. Preserve every public legacy
validation value, error, order, iterator and lower event. Retain only the exact
local semantic Result Arc plus the instantiation child transaction-local epoch.
Do not activate generated, canonical or public repository publication.

Write authority is exactly
`app/slug_loading_v2/src/module_extension_repository_validation.rs`, baseline
1,173 physical lines with the owning test module at 332. Caps are <=220
production, <=700 proof, <=920 aggregate semantic and <=2,100 physical. Add at
most six production and five test helpers plus three observed tests; the shared
driver stays below 120 and every changed helper/test below 200. Every other Rust
file, test, fixture, oracle, Cargo/BUILD target, public API/export and caller is
read-only. The owner remains cohesive because this file already owns the public
legacy key/value/error, validation reducer, certificate iterator, real fixture
and sole generated-publication boundary; splitting would expose retained
internals or duplicate proof plumbing.

## Exact owner and driver

Add exactly these private types:

1. `HostValidatedModuleExtensionRepositoriesObservationKey`, a newtype over the
   legacy key with the same workspace identity and `observed-{legacy Display}`;
2. `ObservedHostValidatedGeneratedRepositorySpecs`, holding the exact local
   `Arc<Result<HostValidatedGeneratedRepositorySpecs,
   HostValidatedGeneratedRepositorySpecsError>>` and one
   `PathObservationEpoch`; and
3. `HostValidatedModuleExtensionRepositoriesObservationError`, with exactly
   `Instantiation(HostInstantiatedModuleExtensionRepositoriesObservationError)`.

Keep every new field, Result alias and accessor private. Preserve the public
legacy key, value, error and outcome alias unchanged. Use matching
Debug/Clone/PartialEq/Eq/Hash/Allocative derives for the observed key and
Debug/Clone/PartialEq/Eq/Allocative/Dupe for carrier and outer as their fields
permit. Both keys retain Complete-only equality and validity.

Refactor one Legacy/Observed driver. Legacy computes only
`HostInstantiatedModuleExtensionRepositoriesKey`; Observed computes only
`HostInstantiatedModuleExtensionRepositoriesObservationKey`. After successful
instantiation semantics both call the existing `validate_repositories` exactly
once and project the same public semantic Result. Do not duplicate, reorder or
parallelize any validation or iterator behavior.

## Exact terminal, order, epoch and event law

- Instantiation DICE compute failure remains existing semantic
  `PrivateValidationError::InstantiationCompute` with an empty epoch.
- Instantiation Need remains immediate Need. An observed opaque instantiation
  outer maps only to the carrierless `Instantiation` outer; do not inspect or
  rewrap its private terminal.
- Accept a Complete instantiation carrier before semantics. Instantiation
  semantic failure remains existing `PrivateValidationError::Instantiation`
  inside `HostValidatedGeneratedRepositorySpecsError` and retains the complete
  child epoch. Success passes the cloned instantiated value and same epoch into
  local validation.
- This parent has one observed child: add no merge stage and do not rebuild,
  union or validate the epoch.

Preserve exact local order. Count mismatch is first. For each paired receipt and
instantiated request in extension order, full request mismatch precedes
generated-name membership. Check imports in declaration order before overrides;
a missing import passes only when generated or named by an override. Check
overrides in declaration order: `must_exist && !exists` is MissingOverride,
then `!must_exist && exists` is InjectCollision. First terminal wins.

Join terminals retain the full predecessor. Validation terminals retain the
predecessor, validated prefix count, current request, exact Import/Override
offender and exact error. Success retains that predecessor through the existing
public flattened exact-size certificate iterator. Every semantic result keeps
the complete child epoch.

Validation stores no evaluation event data. Fresh load/invocation batches stay
owned by the pure subtree; instantiation and validation rows are batchless on
success and every semantic terminal. Need, opaque outer and cancellation
publish no parent carrier or batch. Warm validation reuse is silent, and a
Reused instantiation child never replays a batch.

The only new DICE-retained state is the validation Result Arc plus accepted
instantiation epoch. The Result already owns the predecessor and all public
certificate projections. Generated-name set, paired iterators, driver mode and
child projection remain compute scratch. Retain no duplicate certificate,
evaluator heap, event batch, side cache, task, lock or command state. Ordinary
DICE owns equality cutoff, invalidation, eviction and cancellation; dropping
the compute cannot publish a partial carrier, and recovery recomputes normally.

## Proof and validation

Add exactly:

- `observed_validation_identity_finisher_and_terminal_algebra`;
- `observed_validation_real_order_events_and_parity`; and
- `observed_validation_lifecycle_cancellation_and_nonactivation`.

Together prove key/hash/Display/equality/validity; one-child/no-merge projection;
count/request Join and Import/Override offender prefixes; legacy/observed
semantic parity for success, instantiation error, MissingImport,
MissingOverride and InjectCollision; exact legacy/observed family and
single-child dependency rows; Need and first-terminal suppression; exact lower
print batches, batchless validation and warm silence; held Result/carrier/epoch
A -> B -> A for import/override and generated-repository changes; one
same-semantic/different-epoch observation-metadata axis; each carrier epoch as a
subset of its own transaction global epoch; poll-drop recovery; and zero
legacy-validation/generated/canonical/public/root-mapping activation. Compare
separate transactions semantically and require Arc identity only for an exact
cached value proven Reused.

The instantiation outer stays opaque. Reuse its accepted same-module Pure proof
and add only a static private-producer scan plus the real observed dependency
row; do not expose an accessor or construct invalid epochs. Reuse Bazel 9.2
`SingleExtensionFunction`, `SingleExtensionEvalFunction` and
`ModuleExtensionResolutionTest` evidence; add no oracle or fixture. Buck2 DICE
incrementality/cancellation/activation tests are concept/test evidence only.

Run serially:

- focused `observed_validation_` tests;
- protected `real_validation_`, `observed_instantiation_` and `real_key_` tests;
- full `cargo test -p slug_loading_v2`;
- direct dependent `cargo check -p slug_core_v2`;
- `cargo fmt --all -- --check`; and
- `git diff --check`.

## Compatibility and terminal

Existing validation values/errors/order, import/override polarity, public
certificate iteration, DICE equality and pure-owned events remain exact Bazel 9
compatibility. The private observation key/carrier/outer and Result-Arc/epoch
association are Slug-native. Generated/public/root-mapping/bootstrap observation
activation and exact Bazel configuration/output/ActionKey bytes remain
unsupported/deferred.

ACCEPT returns only to a docs-only generated repository publication frontier
audit. STOP a second file/key/owner/adapter, public API/lib export/caller change,
legacy/instantiation/validation semantic drift, parent event batch, epoch
merge/rebuild, retained scratch/evaluator state, lock/task across DICE,
generated/canonical/public/root-mapping/bootstrap activation, fixture/oracle
work, proof/cap waiver, milestone closure, M8/M7B or exact identity work. REPLAN
before widening. M7 remains partial and M7A -> M8 -> M7B remains.

## Immediate predecessor

`4b5e9d05` exposes exactly the accepted opaque instantiation observation key,
carrier, concrete Result accessor and epoch accessor to validation without a
caller. Live trace proves validation has no other semantic child.
