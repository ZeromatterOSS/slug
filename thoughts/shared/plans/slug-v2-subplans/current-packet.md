# Current Slug V2 Packet

Packet: `WP-2A-m1-native-demand-revision-publication-bridge-design`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Result: design only the private core bridge required for the accepted direct
root exported-source terminal to use request-revision final validation without
erasing the native command's full selected observation epoch.

## Fixed predecessors

The callerless private request family is accepted in `207fe438`; its focused
Bazel ordering evidence is `2ffad088` and its design is `94324880`. The
docs-only consumer audit was activated in `a10f8fd3` and is completed by this
selection. The public direct root and direct-local external exported-source
completion class is accepted in `42f4a64b`.

The private family owns one exact Host FileBytes demand, a complete-only root,
an exact `SourceCertificate`, current-version comparison, final host
reobservation, atomic observation/revision publication, bounded retry, and
provisional suppression. All five production commits share its async,
nonreentrant owner. It retains no transaction, evaluator, accepted snapshot,
worker, or semantic side cache.

## Audit result

The uniquely smallest public candidate is one explicit root
`TargetPattern::Single` whose loaded target is
`PackageTargetKind::ExportedFile`. The live path is:

1. one-shot CLI calls
   `evaluate_workspace_build_command_with_bzlmod_inputs`; the daemon calls
   `WorkspaceRuntime::build_command_with_bzlmod_inputs`;
2. both enter `WorkspaceRuntime::drive_command`, which creates the existing
   native-demand lease, repository session, attempt effects, and fixed DICE
   transaction;
3. `BuildCommandRootKey::compute` first completes
   `RootModuleLoadingAnchorKey`;
4. `compute_build_branch` completes `RootPackageLoadKey`, performs target
   lookup and kind selection, and only then constructs one contained Host
   `PathObservationKey(FileBytes)`;
5. Need is observed by the native command and retried; Complete currently
   records `BuildTargetCompletion::ObservedExportedSource` in
   `BuildRequestedTarget` and then `BuildCommandEvaluation`;
6. the attempt seals, `NativeDemandSealedAttempt::select` computes the exact
   activation closure and selects events plus demands;
7. `prepare_accept` builds and commits the selected
   `AcceptedNativeDemandSnapshot`, then the repository/materializer and native
   session accept; and
8. CLI/server projection emits the already accepted
   `dice_exported_source_file` terminal only after `AcceptedCommand`.

The source demand is exactly one file and already has public discriminating
evidence. Root anchor, package load, target lookup, and kind errors precede it.
A root filegroup remains `LoadedOnly` and does not read its member bytes in
this bounded command; it is not the selected source consumer.

The other candidates are not bounded first consumers. Root `MODULE.bazel`
expands `include()` and lockfile ownership over `WorkspaceSnapshotKey`.
Selected BUILD loading includes package/directory/build-file choice. A
`.bzl` load recursively expands children and cycles. Native query requires
the root-module anchor and arbitrary query-environment Needs. The legacy
`query_observations_*` adapter injects whole text/raw/directory snapshots.
The direct-local external exported source additionally requires repository
routing, package loading, materialization, and repository-source semantics.

## Why a bridge design is prerequisite

The exported-source certificate may remain private and same-crate, but the
existing `read_host_file` entry cannot be called unchanged:

- its mismatch path replaces `PathObservationEpochKey` with a one-entry epoch,
  while the native command's current and selected snapshots own the full
  root-anchor/package/source demand set;
- native attempt publication must retain that full epoch and its existing
  repository/input generations without making
  `AcceptedNativeDemandSnapshot` the certificate store;
- event/demand selection occurs outside the revision mutex and moves the effect
  owner to a sealed terminal phase, so a version/source retry needs an explicit
  suppression/reset path before another attempt can begin; and
- final reobservation and the selected-snapshot DICE commit must have one
  continuous owner linearization. Checking before selection or committing
  after releasing the owner leaves a host-mutation gap.

This is a private core publication bridge, not a cross-crate/public ABI and not
a loading-key rewrite. The existing native lease still serializes public
commands; public overlapping-request acceptance remains deferred.

## Design contract to freeze

The design must specify exact types and state transitions for:

1. retaining the exact demand/result `SourceCertificate` only on the direct
   root exported-source `BuildRequestedTarget` and exposing it privately from
   `BuildCommandEvaluation`;
2. consuming the injected request revision only after root anchor, package load,
   target lookup, and exported-source kind selection, before the existing path
   observation, so earlier Need/error order is unchanged;
3. initializing request revision/path epoch before that branch can compute,
   without a duplicate injected-key update or an observable half-initialized
   production transaction;
4. performing activation-closure/event/demand selection and preparing the
   selected-snapshot updater with no revision mutex held;
5. under the async owner, reading current state from that updater, comparing the
   terminal base, and exact final-reobserving the certificate;
6. on unchanged source, adding only the successor request revision to the
   already prepared full selected-snapshot updater, committing it once, then
   permitting materializer/native-session acceptance and terminal exposure;
7. on changed source, dropping the stale prepared updater/terminal/effects,
   merging the new exact result into the command's full path epoch, committing
   that full epoch plus successor revision from a fresh updater, resetting the
   sealed attempt to retry, and publishing no terminal;
8. on version advance, dropping the prepared updater and resetting the attempt
   without committing stale selected state;
9. on cancellation, observation/injection/publication/reset failure, or bounded
   nonprogress, dropping all certificate/updater/effect/terminal ownership and
   restoring or failing closed under the existing abort guard; and
10. proving no lock spans root/activation DICE computation, Starlark,
    event selection/formatting, repository/materializer calls, or a callback
    that can reacquire the owner.

The design must decide the smallest private sealed-terminal retry token or
state transition in `runtime/events.rs`. It must keep event lineage
service-owned but make selected provisional batches attempt-owned until final
acceptance. It must enumerate every commit and failure ordering affected by
the bridge, including selected-snapshot injection failure and restoration.

The future implementation proposal must name exact Rust files, formatted
production/test/total caps, focused proof, full validation, compatibility,
memory/lifecycle classes, STOPs, and `REPLAN` triggers. The expected maximum
Rust surface is `runtime/request_revision.rs`, `runtime/dice.rs`, and
`runtime/events.rs`; this design packet does not authorize them.

## Compatibility and evidence

Preserve the accepted serial public root exported-source present/absence/
read-error/output/event behavior and exact existing Need/error ordering.
Content-bearing certificate equality, request revisions, final reobservation,
stale-terminal suppression, retry/reset mechanics, and future overlap remain
Slug-native. Root-module/BUILD/`.bzl` snapshot migration, filegroup member
bytes, external repository sources, materialization, directory/glob
certificates, and public overlapping commands remain deferred.

Reuse `42f4a64b` for public terminal/lifecycle behavior and `2ffad088` only
for deterministic source-demand ordering and the Bazel client-lock boundary.
No Bazel evidence claims concurrent final validation. Add no oracle unless the
design identifies a specific uncovered exact behavior.

## Allowlist and caps

Edit exactly:

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`;
- `thoughts/shared/plans/slug-v2-subplans/current-packet.md`; and
- `thoughts/shared/plans/slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`.

Caps are 40 canonical, 260 current-packet, 240 Stage 2, and 540 total net
ledger lines. No Rust, Cargo/BUILD, CLI/server, oracle fixture, generated
evidence, or other ledger file is authorized. Read-only source and accepted
test/evidence inspection are permitted.

STOP on code writes, public API/output activation, changing the native command
or repository lease, repository/materializer semantics, snapshot/loading-key
replacement, a second DICE graph, global command lease, accepted-snapshot reuse
as a certificate store, one-entry overwrite of a full epoch, callback under the
revision owner, lock across compute/Starlark/event selection, new oracle
generation, JVM work, or cap excess.

`REPLAN` if full-epoch mismatch publication cannot be atomic with revision,
a sealed provisional terminal cannot return to retry without exposing effects,
initialization requires a half-published revision/epoch, the root source
certificate changes earlier loading error order, or the bridge requires
repository/public/cross-crate behavior.

## Acceptance and immediate successor

Accept only after independent DICE ownership, event-state, cleanup, and scope
review confirms one implementable core-only state machine and a bounded future
packet. Then activate that bridge implementation alone; do not combine root
MODULE/BUILD/`.bzl`, external repository, public overlap, or lease removal.
