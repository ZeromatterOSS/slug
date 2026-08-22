# Current Slug V2 Packet

Packet: `WP-6-7A-host-root-apparent-repository-source-path-input-observation-implementation`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Design and Rust base: pending docs commit / `c8d2d0b5`

## Goal and authority

Implement exactly one private Legacy/Observed owner for
`HostRootApparentRepositorySourcePathInputKey`. Preserve path normalization
before the sole source-input child, every legacy Result/error/view/identity
rule and lower event owner while adding only a local Result-Arc+epoch carrier
for the now-nameable observed child. Do not activate the callerless host source-
observation layer or the parallel public command branch.

Rust authority is exactly
`app/slug_core_v2/src/runtime/root_apparent_repository_source_path_input.rs`,
baseline 889 physical lines with first `#[cfg(test)]` at line 300 and SHA-256
`adb0cef588ca96622da614deb35bd6c3d43d9160b8b0892b3ce05e0a4ab937e2`.
Every second file, API/export/caller, fixture/oracle, Cargo/BUILD file and
orchestration document is read-only during implementation.

## Audited owner and upper boundary

Live source-path production first calls
`host_repository_relative_path(requested_path)` at lines 222-231. An invalid
path returns the semantic `Path` terminal without requesting any DICE child.
For a valid relative path it requests exactly legacy
`HostRootApparentRepositorySourceInputKey` at lines 233-253: Need returns
immediately, DICE failure becomes semantic `Compute`, completed source error
becomes `Source`, successful source without a consistent view becomes
`InvalidSource`, and a consistent source yields the existing certificate. No
second child, join, merge or direct Host filesystem read exists.

Accepted `c8d2d0b5` makes the observed source-input key, carrier, concrete
Result/epoch accessors and opaque outer sibling-nameable. It is the complete
observed child; no visibility or lower-owner prerequisite remains.

The legacy source-path key has exactly one production consumer:
`HostRootApparentRepositorySourceObservationKey` imports it at
`root_apparent_repository_source_observation.rs:28` and computes it at line
234. That source-observation key has zero production callers. Public command
analysis instead uses the parallel Bzlmod route/observation path at
`runtime/dice.rs:4476-4494`, and root bootstrap remains imperative and dormant.
These upper/parallel branches stay inactive and are not prerequisites.

## Frozen production contract

Add private nominal
`HostRootApparentRepositorySourcePathInputObservationKey(
HostRootApparentRepositorySourcePathInputKey)` with the exact existing
three-argument `Option<Self>` construction, including root-name rejection and
the unchanged `requested_path: PathBuf` identity. Display is
`observed-{legacy Display}`; `/workspace`, `@first`, `pkg/file.bzl` renders:

```text
observed-HostRootApparentRepositorySourcePathInputKey { workspace: NormalizedAbsolutePath { path: "/workspace" }, apparent_repo: ApparentRepoName("first"), requested_path: "pkg/file.bzl" }
```

Add private `ObservedHostRootApparentRepositorySourcePathInput` retaining only
`Arc<HostRootApparentRepositorySourcePathInputResult>` plus
`PathObservationEpoch`, with private borrowed `result` and `observations`
accessors. Use Debug/Clone/PartialEq/Eq/Allocative/Dupe.

Add private typed outer
`HostRootApparentRepositorySourcePathInputObservationError::Source(
HostRootApparentRepositorySourceInputObservationError)` with Debug/Clone/
PartialEq/Eq/Allocative and manual Dupe. It has no carrier/epoch, Merge or
OperationMismatch variant and does not expose the child's private inner.

Factor only the existing computation into private
`RootApparentRepositorySourcePathInputMode::{Legacy, Observed}`, one
`compute_root_apparent_repository_source_path_input` driver and exactly one
pure `finish_root_apparent_repository_source_path_input`. Driver Value is
`SourcePreparationOutcome<Result<(Arc<HostRootApparentRepositorySourcePathInputResult>,
PathObservationEpoch),
HostRootApparentRepositorySourcePathInputObservationError>>`. The legacy Key
projects the Result Arc and asserts an empty epoch; the observed Key publishes
the carrier. Both use complete_eq equality and complete-only validity.

Both modes normalize the requested path exactly once before any DICE request.
Path failure completes the same semantic `Path` Result with an empty epoch.
Legacy requests only the legacy source-input key and supplies an empty epoch.
Observed requests only `HostRootApparentRepositorySourceInputObservationKey`:
Need is immediate; its opaque outer maps directly to carrierless parent
`Source`; success supplies the original source-input Result Arc and cloned
epoch. A DICE compute failure remains the same semantic `Compute` Result with
the normalized path and an empty epoch.

After child completion run the existing disposition/view/certificate algebra
once. Source semantic error maps to `Source`; a successful source without a
valid view or a certificate association failure maps to `InvalidSource`;
consistent Main/Builtin/selected inputs succeed exactly as today, while
generated and other completed source errors remain `Source`.
Every child-complete semantic terminal or success retains the exact child
Result Arc where legacy does and forwards the child epoch unchanged. With one
child there is no prefix union, epoch rebuild, fallback, join or parent
mismatch algebra.

Source path is eventless. Path normalization owns no event and the observed
source-input child owns every lower event batch/order. Parent Need, outer,
compute, semantic and success rows add no event; direct-parent event vectors
equal direct-child vectors and every warm activation is batchless. Dependency
vectors are exactly legacy source path -> legacy source input and observed
source path -> observed source input; invalid path has no child edge.

Retain only the local source-path Result Arc plus compact child epoch. The
existing Result already retains its exact source-input Arc and normalized
relative path. Child carrier, mode, requested-path clone, views/disposition and
event scratch die before publication; add no map/cache/interner/store/task/lock
or command borrow. DICE owns serialization. Poll-drop publishes no parent
activation/dependency/carrier/event and same-DICE recovery recomputes lawfully.

## Exact proof

Add exactly three tests:

- `observed_root_apparent_repository_source_path_input_identity_finisher_and_terminal_algebra`;
- `observed_root_apparent_repository_source_path_input_real_families_events_and_parity`;
- `observed_root_apparent_repository_source_path_input_lifecycle_cancellation_and_nonactivation`.

The first proves root rejection, exact Display/hash/requested-path identity,
accessors/equality/validity, invalid-path-before-child with empty epoch, exact
legacy/observed child edges, Need, opaque Source outer, compute/Source/
InvalidSource/success finisher algebra and exact retained child Arc/epoch.
Use only lawful child values; fabricate no opaque outer or malformed epoch.

The real proof covers accepted Main, Builtin, generated Source terminal,
selected-nonregistry WorkspaceRelative and CommandAbsolute families plus
mapping/missing Source terminals: exact legacy Result semantic parity,
requested/relative-path identity, source-input/request Arc retention, unchanged
child epoch, complete lower event vectors and warm batchlessness. Reuse the
accepted observed-source-input family/event proof rather than injecting private
mirror or route state.

Lifecycle holds parent and observed-source-input child carriers through
mapping, definition and local-policy A-B-A changes/restorations. Prove semantic
change/restoration and immutability, neutral same-Result/different-lawful-epoch
invalidation, each recovered child epoch equal to its same-transaction parent
epoch and a subset of that transaction-global epoch, no cross-transaction
association, and Arc identity only on a proven Reused row. Prove poll-drop/
same-DICE recovery, warm silence, exact dependency vectors and zero source-
observation/public-command/bootstrap activation.

Preserve the accepted source-input visibility smoke and all legacy tests.
Extend only shared tracker/helpers and update
`production_edge_is_path_then_source_input_only` to require exactly one legacy
source-input request, one observed source-input request, one pure path
normalization and no other predecessor.

## Caps and validation

Caps are <=240 production additions, <=620 proof additions, <=860 aggregate
semantic additions and <=1,750 physical lines. Add at most six private
production helpers and eight proof helpers, exactly the three tests above,
keep the driver below 150 and every new/changed helper or test below 200. Add no
`rustfmt::skip`; formatting has no waiver. The file stays cohesive below the
2,000-line trigger as sole path-input value/error/view/driver/proof owner and is
not a demonstrated hot path.

Run serially: the exact three observation tests; protected source-input
visibility smoke and legacy source-path plus observed-source-input suites;
full `cargo test -p slug_core_v2`; direct `cargo check -p slug_commands_v2`;
`cargo fmt --all -- --check`; exact one-file allowlist/entry-SHA/accounting/
physical/helper/test/driver/source-shape checks; and `git diff --check`. Reuse
accepted Bazel 9.2 `BazelDepGraphFunction.computeCanonicalRepoNameLookup`,
`BazelDepGraphValue.getRepositoryMapping`, repository source/path capability
tests and original owner `e4292de7`; Buck2 DICE lifecycle is concept/test
evidence. Add no fixture or oracle.

## Compatibility and stops

Path normalization, requested/relative-path identity, source-input projection,
Main/Builtin/selected values, generated/source errors and terminal order,
equality/invalidation and lower events remain **exact** Bazel 9 compatibility.
The private Result-Arc+
transaction-local epoch carrier/outer is **Slug-native**. Carrier visibility,
source observation, public command/bootstrap activation and exact Bazel
configuration/output/ActionKey bytes remain **unsupported/deferred**.

STOP a second file/key/child/owner/adapter, visibility/export/caller/source-
observation work, semantic/path/view/order/error/event/equality/retention drift,
epoch merge/rebuild or parent mismatch, retained child/scratch/task/lock,
private/malformed injection, fixture/oracle, cap/helper/test/format waiver or
new `rustfmt::skip`, Cargo/BUILD, milestone closure, M8/M7B or exact identity
work. REPLAN before widening or entry-hash drift.

## Terminal

ACCEPT returns only to a docs-only source-path carrier-visibility/source-
observation consumer audit. M7 remains partial and M7A -> M8 -> M7B remains.

## Immediate predecessor

Accepted visibility commit `c8d2d0b5` changed source input +28/-12 and source
path test-only +44, leaving the source-path production owner unchanged and its
observed child sibling-nameable.
