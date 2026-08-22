# Current Slug V2 Packet

Packet: `WP-6-7A-host-root-apparent-repository-source-input-observation-proof-correction-implementation`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
REPLAN and candidate base: pending docs commit / `5d0f11ca`

## Goal and authority

Retain the current one-file source-input observation candidate and correct or
validate only its proof after the latest unvalidated refactor. Preserve the
accepted Legacy/Observed owner, exact projection/terminal/epoch/event/lifecycle
contract and sibling smoke while freezing production and API byte-for-byte.

Rust authority is only lines 440+ of
`app/slug_core_v2/src/runtime/root_apparent_repository_source_input.rs`.
Production lines 1..=439, every second file, API/helper/test count and names,
fixtures/oracles/Cargo/BUILD/exports/callers and orchestration docs are
read-only during proof correction.

The retained candidate exceeds only the former proof/aggregate addition caps:
proof is +697/-11 against <=620 and aggregate is +936/-81 against <=860.
Production is +239/-70 within its <=240 cap, all spans are below 200, and the
file remains below its physical ceiling. This formal REPLAN raises proof and
aggregate caps narrowly; it changes no semantic, proof or activation contract.

## Audited frontier

Accepted `b61f8f1a` exposes exactly the crate-internal observed route key/new,
carrier borrowed Result-Arc/epoch accessors and opaque outer. Source input can
name its complete associated Value without seeing the route Definition
terminal. No visibility or lower-owner prerequisite remains.

Live source input has one DICE child. It imports legacy
`HostRootApparentRepositoryRouteKey` at line 24 and requests it at line 186.
Need returns immediately; child DICE failure becomes semantic `Compute`; a
completed route error becomes `Route`; a successful route without a source or
with an invalid certificate becomes `InvalidRoute`; `host_repository_source_input`
projection failure becomes `Projection`; Main or accepted Builtin/spec-backed
input completes successfully. Every completed semantic terminal retains the
exact route Result Arc when one exists. There is no second child or merge.

The legacy source-input key has exactly one production consumer: source-path
input imports it at its line 27 and computes it at line 234. Source path has
one production consumer, host source observation at its line 234; that key has
zero production callers. Public command analysis instead computes the
parallel Bzlmod route/observed-route branch at `runtime/dice.rs:4476-4494`.
Root bootstrap remains imperative and dormant. These upper/parallel layers are
not prerequisites and stay inactive.

Committed design `400b819e` selects exactly this one-file private owner, not a
visibility prerequisite.

## Frozen accepted production contract

Add private nominal
`HostRootApparentRepositorySourceInputObservationKey(HostRootApparentRepositorySourceInputKey)`
with the exact existing two-argument `Option<Self>` construction, root-name
rejection and Display `observed-{legacy Display}`. `/workspace`, `@first`
renders exactly:

```text
observed-HostRootApparentRepositorySourceInputKey { workspace: NormalizedAbsolutePath { path: "/workspace" }, apparent_repo: ApparentRepoName("first") }
```

Add private `ObservedHostRootApparentRepositorySourceInput` retaining only
`Arc<HostRootApparentRepositorySourceInputResult>` plus
`PathObservationEpoch`, with private borrowed `result` and `observations`
accessors. Derive Debug/Clone/PartialEq/Eq/Allocative/Dupe.

Add private typed outer
`HostRootApparentRepositorySourceInputObservationError::Route(
HostRootApparentRepositoryRouteObservationError)` with matching
Debug/Clone/PartialEq/Eq/Allocative and manual Dupe. It has no carrier or epoch
and no Merge/OperationMismatch variant.

Factor only existing compute into
`RootApparentRepositorySourceInputMode::{Legacy, Observed}`, one private
`compute_root_apparent_repository_source_input` driver and exactly one pure
`finish_root_apparent_repository_source_input`. Driver Value is
`SourcePreparationOutcome<Result<(Arc<HostRootApparentRepositorySourceInputResult>,
PathObservationEpoch), HostRootApparentRepositorySourceInputObservationError>>`.
The legacy Key projects the Result Arc and asserts an empty epoch; the observed
Key publishes the carrier. Both use complete_eq equality and complete-only
validity.

Legacy requests exactly the legacy route and supplies an empty epoch. Observed
requests exactly `HostRootApparentRepositoryRouteObservationKey`: Need is
immediate; its opaque child outer maps directly to carrierless parent Route
outer; child success supplies the original Result Arc and cloned epoch. A DICE
compute error remains the same semantic source-input Compute terminal with an
empty epoch. Do not inspect or construct the opaque child outer/private inner.

Run the existing projection once after child completion. Route semantic error
maps to `Route`; absent source or failed association maps to `InvalidRoute`;
`host_repository_source_input` failure maps to `Projection`; Main and valid
Builtin/spec-backed dispositions succeed. Preserve exact workspace/apparent/
canonical/capability/policy association and one projection call. Every success
or semantic terminal forwards the child epoch unchanged and retains the exact
route Result Arc where legacy does. With one child there is no prefix union,
epoch rebuild, join, fallback or parent mismatch algebra.

Source input is eventless. The observed route child owns every lower event
batch and its ordering. Parent execution adds no event on Need, outer, compute,
finisher terminal or success; direct-parent event rows equal direct-child rows,
and every warm activation is batchless. Dependency vectors are exactly legacy
source input -> legacy route and observed source input -> observed route.

Retain only the source-input Result Arc plus compact child epoch. The existing
Result success/error already retains its required route Arc and projected
input/request Arc. Child carrier, mode, source disposition scratch and closure
die before publication; add no cache/store/interner/task/lock or command
borrow. DICE owns serialization. Poll-drop publishes no parent activation,
dependency, carrier or event; recovery recomputes lawfully.

## Exact proof

Retain exactly three tests:

- `observed_root_apparent_repository_source_input_identity_finisher_and_terminal_algebra`;
- `observed_root_apparent_repository_source_input_real_families_events_and_parity`;
- `observed_root_apparent_repository_source_input_lifecycle_cancellation_and_nonactivation`.

The first proves key/root/Display/hash/accessors/equality/validity, Need,
exact child edge, finisher disposition/error/Arc/epoch algebra and the opaque-
outer source mapping without private injection. `InvalidRoute` remains bounded
pure defensive algebra because lawful accepted routes supply a source.

The real proof covers Generated projection error, SelectedNonregistry Need then
WorkspaceRelative and CommandAbsolute success, mapping/missing Route terminal,
Main and Builtin success: exact legacy Result semantic parity, exact route Arc
retention, unchanged child epoch, direct-child lower event vectors and warm
batchlessness. Reuse accepted route selected-registry static/pure evidence and
accepted `host_repository_source_input` policy tests; do not fabricate a real
private-mirror registry row.

Lifecycle holds parent and observed-route child carriers through mapping,
definition and local-policy A-B-A, verifies semantic restoration, neutral same-
Result/different-epoch invalidation, parent epoch exactly child and a subset of
the same transaction-global epoch, Arc identity only on Reused, cancellation/
recovery and zero source-path/source-observation/public-command/bootstrap
activation. No cross-transaction child pairing or stale epoch claim.

Preserve the accepted route-surface sibling smoke byte-for-byte. Extend only
shared tracker/helpers and update existing
`production_edge_is_only_route_then_pure_projection` to require exactly one
legacy route request, one observed route request, one pure projection and no
other predecessor. Preserve all existing legacy assertions.

## Retained candidate, caps and validation

Entry candidate is 1,711 physical with full SHA-256
`f2188f3cb08b7f64cf87e09c2bcbb67c84a3dcceacc5bda03abbb41f0554c632`.
Production lines 1..=439 are byte-frozen at SHA-256
`6bf7709327d6b0070ca17449f655d747f816c050e8ea3c023921ebb49c5bb9fc`.
Accounting against `5d0f11ca` is +239/-70 production, +697/-11 proof and
+936/-81 aggregate. Proof begins exactly at `#[cfg(test)]` line 440.

Retain exactly eight test helpers and the three named tests, add no
`rustfmt::skip`, and keep every helper/test span below 200. Production/API,
production-helper count, proof-helper count and test count/names may not
change. Correct only assertions/source scans/imports inside the existing proof
when a fresh gate demonstrates a concrete miss.

Corrected caps are <=240 production additions, <=720 proof additions, <=960
aggregate additions and <=1,750 physical. Relative to the entry candidate,
headroom is exactly 23 proof additions, 24 aggregate additions and 39 physical
lines. Deletions do not authorize replacement breadth or cap transfer.

Run a completely fresh serial validation; no result predating the latest
refactor is admissible. Validate the three exact tests; protected legacy
source-input and accepted route-observation tests/smoke; full
`cargo test -p slug_core_v2`; direct `cargo check -p slug_commands_v2`;
`cargo fmt --all -- --check`; exact one-file allowlist, entry/prefix SHA,
production/proof/aggregate accounting, physical/helper/test/span/source-shape
and `git diff --check`. Reuse accepted Bazel 9.2
`BazelDepGraphFunction.computeCanonicalRepoNameLookup`,
`BazelDepGraphValue.getRepositoryMapping`, repository source-capability tests
and original owner `e4292de7`; Buck2 DICE lifecycle is concept/test evidence.
Add no fixture or oracle.

## Compatibility and stops

Source-input Main/Builtin/spec projection, Need and terminal order, retained
route identity, policies, errors, equality/invalidation and lower events remain
**exact** Bazel 9 compatibility. The private Result-Arc+transaction-local epoch
carrier/outer is **Slug-native**. Source-path/source observation, public
command/bootstrap activation and exact identity bytes remain
**unsupported/deferred**.

STOP any production-prefix/hash/API/helper/test name or count change; second
file/key/child/owner/adapter, visibility/export/caller, source-path
or upper activation, semantic/projection/policy/order/error/event/equality/
retention drift, epoch merge/rebuild, parent OperationMismatch, retained child/
scratch/task/lock, private/malformed injection, fixture/oracle, cap/helper/test/
format waiver or new `rustfmt::skip`, stale validation, Cargo/BUILD, milestone
closure, M8/M7B or exact identity work. REPLAN before cap widening, production
change or any proof correction that cannot fit the frozen existing helpers and
tests.

## Terminal

ACCEPT requires the complete fresh serial gates above and returns only to docs-
only source-input carrier visibility/source-path consumer audit. M7 remains
partial and M7A -> M8 -> M7B remains.

## Immediate predecessor

Accepted design `400b819e` and scheduling correction `5d0f11ca` authorize the
one-file owner. The current retained candidate implements production within
cap but exceeds the former proof/aggregate caps and has not received fresh
post-refactor serial validation; this REPLAN corrects only those constraints.
