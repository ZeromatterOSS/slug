# Current Slug V2 Packet

Packet: `WP-6-7A-host-root-repository-mapping-observation-implementation`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Design and Rust base: pending docs commit / `c96ae09d`

## Goal and exact authority

Add the private callerless observation sibling of
`HostRootRepositoryMappingKey` in its existing Bzlmod owner. Share its exact
one-child extension-mappings projection and exhaustive root-ordinal reducer
between Legacy and Observed modes. Publish one root-mapping Result Arc plus the
unchanged child observation epoch; activate no core consumer.

Write only `app/slug_bzlmod_v2/src/selected_repo_spec.rs`, baseline 12,564
physical lines with `#[cfg(test)]` at line 4,678. Every other Rust file, test,
fixture, oracle, Cargo/BUILD target, API, export, caller and plan is read-only.

## Exact nominal surface and driver

Add private `HostRootRepositoryMappingObservationKey` as a newtype over the
legacy key, with private one-argument `new` and Display `observed-{legacy}`.
For `/workspace`, exact Display is
`observed-host-root-repository-mapping:"/workspace"`.

Add private `ObservedHostRootRepositoryMapping` containing exactly
`RootRepositoryMappingResult` and `PathObservationEpoch`, with private borrowed
`result()` and `observations()` accessors. Define the private result alias as
`Arc<Result<HostRootRepositoryMapping, HostRootRepositoryMappingError>>` and
leave the public legacy outcome's concrete spelling unchanged. Add private typed outer
`HostRootRepositoryMappingObservationError::Mappings(
ExtensionMappingsObservationError)`. Key Value is
`SourcePreparationOutcome<Result<carrier, outer>>`. Use matching Debug/Clone/
PartialEq/Eq/Hash/Allocative derives on the key and Debug/Clone/PartialEq/Eq/
Allocative/`Dupe` on carrier and outer; add no export, public field, alias,
adapter or caller.

Reuse the existing private `RoutesMode::{Legacy, Observed}` and generic
`RepoSpecChild`; do not add another mode or child enum. Factor the current key
body only into `root_mapping_complete`,
`root_repository_mapping_mappings_child`, `finish_root_repository_mapping`,
`drive_root_repository_mapping` and
`project_legacy_root_repository_mapping`. Both Key implementations call the
one driver; the legacy projection moves the exact Result Arc, and the observed
projection constructs the carrier or forwards the typed outer.

Legacy computes only `HostSelectedExtensionMappingsKey` with empty epoch.
Observed computes only `HostSelectedExtensionMappingsObservationKey` and
borrows its exact Result Arc and epoch. DICE child compute failure remains the
existing semantic `PrivateRootRepositoryMappingError::Compute(message)` with
empty epoch. Need returns immediately and publishes no carrier. Observed child
outer becomes carrierless `Mappings(error)`. Complete child semantic error
remains `Predecessor(predecessor)` with the exact child epoch.

On semantic success, move the same mappings Result Arc through the reducer.
Refactor the route loop through one production-used private iterator helper
that records the first root ordinal, records only the first conflicting root,
and consumes the entire iterator. `root_mapping_ordinal` then preserves the
context check. Exact terminals are `Missing`, `Duplicate { first,
conflicting }`, and `Context { ordinal }`; each `Invalid` retains the same
predecessor Arc and child epoch. Success retains that predecessor plus the root
ordinal and child epoch. Mapping entry/order storage and borrowed views are
unchanged. There is one child and no epoch merge, rebuild, union or validation.

The observed key uses `complete_eq` and Complete-only validity: Need is invalid
and self-unequal; Complete carrier/outer compares structurally. Result equality
alone cannot cut off an epoch-only change.

## Events, retention and lifecycle

Root mapping owns no event batch. Its fresh observed dependency row is exactly
`[observed-host-selected-extension-mappings]`; legacy is exactly
`[host-selected-extension-mappings]`. Accepted lower event payloads remain
equal, with observed owner
`bzlmod-observed-host-root-module-file:"/selected-repo-spec-test"` and legacy
owner `root-module-evaluation:/selected-repo-spec-test`. Parent and every warm/
Reused row are batchless. Need, child outer and cancellation publish neither
carrier nor parent batch; semantic terminals retain lower child events without
replay.

Each observed carrier retains only the root-mapping Result Arc and compact
child epoch. Existing success/error values retain the predecessor Arc and
ordinal/reason exactly. Child carrier, duplicate maps/order, iterator/scan
scratch, mode, evaluator/event state, cache, task and lock die before
publication. DICE owns serialization; no manual lock or task crosses compute.

Add exactly:

- `observed_root_repository_mapping_identity_scan_and_terminal_algebra`;
- `observed_root_repository_mapping_real_order_events_and_parity`; and
- `observed_root_repository_mapping_lifecycle_cancellation_and_nonactivation`.

The first proves key equality/hash/Display/accessors, Complete/Need/outer
equality/validity, compute/Predecessor/Missing/Duplicate/Context/success
terminals, exact epoch/Arc forwarding and the production-used iterator's full
consumption plus first/conflicting ordinals. It also proves the driver names
only the observed mappings child, has no merge, and the legacy projection moves
the Result Arc.

The real proof requires exact observed/legacy one-child rows, successful and
predecessor-error parity, retained predecessor Arc, borrowed mapping order,
exact lower event owner/payload parity, batchless parent/warm behavior,
matching-family exclusion and no parent replay. Reuse the existing root-mapping
publication/corruption fixtures and accepted observed-mappings proof.

The lifecycle proof holds Result and epoch handles across A-B-A changes to root
repo name, import order and override/inject order; proves identical semantic
source with changed metadata has equal Result but unequal epoch/observed Value;
checks each epoch against its own transaction global, Arc identity only on a
proven Reused value, real poll-drop with no parent publication, and same-DICE
recovery. Deny legacy root mapping, definition requests, canonical-selected,
generated/canonical definition, core apparent mapping/root definition/route/
source, materialization and public command/bootstrap activation. Use tracker/
source-direction evidence only; add no malformed runtime injection or hook.

## Caps, validation and compatibility

Caps are <=230 production, <=680 proof, <=910 aggregate semantic and <=13,480
physical. Add at most six production/six test helpers, exactly three tests,
keep the shared driver below 120 and every helper/test below 200. The large file
remains cohesive because it already owns the selected-mappings carrier, root
mapping reducer/value/error/view, trackers and real fixtures; splitting would
expose private state. This is not a demonstrated hot path.

Run serially:

- `cargo test -p slug_bzlmod_v2 observed_root_repository_mapping_ --lib`;
- `cargo test -p slug_bzlmod_v2 root_mapping_ --lib`;
- `cargo test -p slug_bzlmod_v2 observed_extension_mappings_ --lib`;
- full `cargo test -p slug_bzlmod_v2`;
- direct dependent `cargo check -p slug_core_v2`;
- `cargo fmt --all -- --check`; and
- exact allowlist/accounting/physical/helper/test checks plus `git diff --check`.

Reuse Bazel 9.2 `BazelDepGraphFunction.computeCanonicalRepoNameLookup`,
`BazelDepGraphValue.getRepositoryMapping` and `BazelDepGraphFunctionTest`.
Buck2 DICE incrementality, cancellation and activation tests remain concept/
test evidence. Add no fixture or oracle.

Root-mapping values/errors/full-scan/order/views, DICE equality/invalidation
and lower events remain exact Bazel 9 compatibility. The private observation
key/carrier/typed outer and Result-Arc transaction-local epoch association are
Slug-native. Promotion/caller, canonical apparent mapping, root definition/
route/source, public command/bootstrap activation and exact Bazel configuration/
output/ActionKey bytes remain unsupported/deferred.

## Terminal

ACCEPT returns only to a docs-only root-mapping carrier-visibility audit. STOP
a second file/key/owner/adapter, export/reexport/caller, public API, core edit,
partial family, root ordinal/order/semantic/event/equality/retention drift,
epoch merge, copied mapping/order, retained scratch/task/lock, fixture/oracle,
cap/helper/test waiver, upper activation, milestone closure, M8/M7B or exact
identity work. REPLAN before widening. M7 remains partial and M7A -> M8 -> M7B
remains.

## Immediate predecessor

Design base `cdcdfe24` selects this one-child owner after accepted canonical
implementation `c96ae09d`; the observed selected-mappings child is already
private in the same source file.
