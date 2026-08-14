# Current Slug V2 Packet

Packet: `WP-2A-m1-native-demand-revision-publication-bridge`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Result: implement only the accepted private core bridge that gives the explicit
root exported-source terminal request-revision final validation while
preserving the native command's full observation epoch.

## Fixed predecessors and selected consumer

The callerless private request family is `207fe438`; its design is
`94324880` and focused ordering evidence is `2ffad088`. The public root and
direct-local external exported-source completion class is `42f4a64b`. Audit
activation `a10f8fd3` and bridge-design activation `f601465a` select one
explicit root `TargetPattern::Single` whose loaded kind is
`PackageTargetKind::ExportedFile`.

The live order remains one-shot/daemon build entry,
`WorkspaceRuntime::build_command_with_bzlmod_inputs`, `drive_command`,
`BuildCommandRootKey`, root-module anchor, `RootPackageLoadKey`, target
lookup, root exported-file kind selection, then one contained Host
`PathObservationKey(FileBytes)`. Need is observed by the native driver.
Complete success is `BuildTargetCompletion::ObservedExportedSource`; absence,
wrong-kind, and read error are `BuildCommandErrorKind::RootSource`. Events and
demands are selected before selected-snapshot publication, and CLI/server
output is projected only from `AcceptedCommand`.

Root filegroup, package-all, multi-target, rules, root module, BUILD discovery,
`.bzl`, query, legacy observation adapters, and direct-local external sources
are not consumers in this packet. Public commands remain serialized by
`NativeDemandSessionOwner`; this packet does not claim public overlap.

## Accepted private representations

`SourceCertificate` remains the exact demand plus exact
`Arc<PathObservationResult>`. Add only private sibling construction and
borrowing access.

For the sole admitted success, `BuildRequestedTarget` retains
`Some(SourceCertificate)`; every other successful target retains `None`.
For a completed root-source error,
`BuildCommandErrorKind::RootSource { observation, certificate }` retains the
same certificate while existing rendering uses only `observation`.
`BuildCommandEvaluation` and `BuildCommandError` expose a private borrowed
certificate selector. Thus present-to-absence, absence-to-present, read-error,
and content mutation all receive final validation without a public type/output
change. The certificate participates in complete terminal equality but never
enters `AcceptedNativeDemandSnapshot`.

Only an exactly one-target root build may attach the certificate. After anchor,
package, lookup, and kind selection, that branch structurally computes the
existing private `RequestRevisionKey` and then the existing path key.
Earlier Need/error precedence is unchanged. External exported source keeps its
current completion class with no certificate/revision dependency.

## Initialization and attempt publication

Add a private `NativeCommandRoot` hook that selects every syntactically
sole-root `BuildCommandRootKey` as initialization-capable. Multi-target,
package-all, query, cquery, synthetic, non-root, and external driving retain the
existing `commit` leaf. A sole-root rule or filegroup uses
`commit_native_attempt` but acquires no request-revision dependency or
certificate. Only the later exported-file classification consumes the key.

`RequestRevisionRuntime::commit_native_attempt(updater)` is called only for a
bridge-capable root and then acquires its async owner:

- if uninitialized, add only the initial `RequestRevisionKey` value to that
  same already-full updater, commit once, and then mark the owner initialized;
- if initialized, commit the updater unchanged.

No empty or duplicate `PathObservationEpochKey` is added. The initial
revision and native full epoch therefore become visible in one DICE version.
Injection failure leaves the owner uninitialized and the native abort guard
restores/fails closed. The other four production publishers remain routed
through the existing owner leaf; the callerless `base_transaction` retains
its current lazy two-key initialization.

## Selected-terminal state

In `runtime/events.rs`, `SealedCommandAttempt::select` returns the selected
sidecars with one armed private terminal token retaining the exact owner and
attempt ID. The only new transition is:

`Terminal(matching_id) -> Idle`

through a consuming `reset_to_idle`. It drops provisional selected
events/demands and permits `begin_attempt` to allocate a fresh ID. A consuming
accept disarms the token without resetting because the command closes. An
armed-token drop is cancellation cleanup and resets the matching terminal;
explicit retry/reset errors remain propagated rather than swallowed.

`NativeDemandTerminalSelection` and
`NativeDemandPreparedAcceptance` retain this token through selection,
snapshot construction, final validation, materializer acceptance, and native
session replacement. No selected event batch becomes `CommandOutputBuffer`
until the token is disarmed on acceptance.

## Preparation outside the revision owner

Refactor native preparation without changing its data:

1. seal the terminal and select its activation closure, event batches, and
   exact demands;
2. construct the selected `AcceptedNativeDemandSnapshot` and repository
   validations;
3. create a DICE updater and inject that complete selected native snapshot; and
4. retain the updater, selected sidecars, terminal token, snapshot, and
   validations as provisional command-owned preparation.

All four steps occur without the revision owner. Activation-closure reads,
event/demand selection, `RepositoryMaterializer::selected_epoch`, user-data
construction, and typed native snapshot injection never occur under the owner.

## Atomic finalization

Add one private `RequestRevisionRuntime::finalize_native` taking the
provisional terminal transaction, exact certificate, the already-injected
selected updater, and the command's full path epoch. It returns only:

- `Accepted { revision }`;
- `RetryVersionAdvanced`; or
- `RetrySourceChanged { merged_epoch }`.

Internally it acquires the async owner and performs no callback or DICE key
compute. It reads `selected_updater.existing_state()`, compares it with the
terminal transaction, and exact Host-reobserves only when still current.

For unchanged source, allocate a checked successor revision, add only
`RequestRevisionKey` to the already-injected selected updater, commit once,
record the published revision, and return Accepted. Materializer/session
acceptance and terminal/event exposure happen only afterward.

For version advance, drop the selected updater and return Retry without a
commit. For changed source, drop the stale selected updater, require the
certificate demand in the command's full epoch, replace exactly that entry,
create a fresh updater, inject the merged full `PathObservationEpochKey` plus
one successor revision, commit once, and return the merged epoch. All other
current native inputs/repository results remain in the current DICE version.
The stale selected snapshot is never accepted or used as certificate storage.

The driver assigns a changed merged epoch back to command state, consumes the
terminal token through `reset_to_idle`, drops the stale terminal/effects, and
starts a new attempt without `NativeDemandCommand::progress`. Version retry
performs the same reset without changing command epoch. Bound these
bridge-only terminal resets at eight; exhaustion is typed nonprogress and uses
normal abort/restoration.

## Failure, cancellation, and memory

Observation, initialization, selected-snapshot injection, revision injection,
epoch merge, commit-test fault, reset, materializer acceptance, native-session
replacement, restoration, cancellation, and nonprogress publish no stale
terminal or provisional event. Before irreversible acceptance, the armed token
and abort guard reset/suppress effects and restore the prior accepted snapshot;
cleanup failure fails closed. A changed-epoch commit followed by reset failure
still exposes no terminal and restoration reasserts the prior path snapshot.
No lock spans restoration.

Memory classes:

- service: DICE, revision owner/allocator, native lease owner, repository
  materializer, event lineage;
- DICE semantic: injected revision/full path epoch, root terminal and exact
  certificate, dependency/equality state;
- command: native lease/session, full current epoch, provisional terminal,
  selected token/events/demands, snapshot/validation, updater;
- scratch: exact reobservation and merged epoch.

No transaction, updater, certificate, event selection, evaluator, accepted
semantic snapshot, or repository validation is newly retained after command
completion. `AcceptedNativeDemandSnapshot` remains only native
inputs/repository/path selection, never certificate authority.

## Compatibility and evidence

Preserve the byte-for-byte public root-source surface as an existing
regression/non-widening invariant; do not claim new root-specific Bazel parity.
Reuse `42f4a64b` only for the shared completion/lifecycle boundary and its
accepted external-source slice. Reuse `2ffad088` only for deterministic source
ordering and the serialized Bazel client boundary. No new oracle is required.

Exact certificate representation, content-sensitive internal equality, revision
numbers, current-version retry, final reobservation, stale-terminal/event
suppression, sealed reset, and any future overlap are Slug-native. Filegroup
member bytes, root module/BUILD/`.bzl` migration, external repository/
materialized sources, directory/glob unions, public overlapping commands, and
historical host reads remain unsupported/deferred.

## Implementation allowlist, caps, and proof

Edit exactly:

- `app/slug_core_v2/src/runtime/request_revision.rs`;
- `app/slug_core_v2/src/runtime/dice.rs`;
- `app/slug_core_v2/src/runtime/events.rs`; and
- canonical/current/Stage 2 ledgers only at completion.

Caps are three Rust paths, 600 net production lines, 750 in-module test lines,
and 1,350 total added Rust lines. The separate ledger cap is 260 net lines. One
correction may adjust caps only; it may not add behavior or files.

Focused proof must cover initial revision plus full native epoch in one commit;
single-root branch-late revision dependency; exact-Arc certificate retention
for present and source error; unchanged acceptance; V1-to-V2,
V1-to-absence/error, and absence-to-present stale suppression; full-epoch
one-entry replacement with unrelated demands preserved; version advance;
selected event/demand suppression and fresh attempt ID; exact commit/reobserve/
retry counters; warm reuse and restoration; forced observation, initialization,
selected-injection, revision-injection, publication, reset, materializer,
session, and restoration failures; cancellation during selection/finalization;
eight-attempt nonprogress; no leaked token/updater/certificate/event; no owner
at root/activation/event/materializer barriers; and no certificate in the
accepted snapshot.

Run focused tests, full `cargo test -p slug_core_v2`,
`cargo clippy -p slug_core_v2 --all-targets -- -D warnings`, targeted Bazel
Rust tests if available, formatting, archive/status/artifact checks,
`git diff --check`, formatted line accounting, and independent
DICE/event/cleanup review. Record inherited repository/tooling baselines
explicitly rather than widening this packet.

STOP on any other file, CLI/server/public output or API, public overlap or lease
removal, repository/materializer behavior, another root/target kind, loading/
snapshot migration, new DICE key/store/graph, accepted-snapshot certificate
storage, one-entry overwrite of a full epoch, unbounded retry, callback or
compute/Starlark/event/repository work under the owner, oracle growth, watcher,
historical host state, JVM work, or cap excess.

`REPLAN` if initial revision/full epoch cannot share one native attempt
commit, error terminals cannot retain the exact certificate privately, selected
terminal cannot reset without event exposure, full-epoch mismatch publication
cannot be atomic with revision, cleanup needs a retained updater/transaction,
or the bridge requires repository/public/cross-crate behavior.

## Immediate successor

Accept only after complete proof and independent ownership/event/cleanup
review. Then audit the next smallest source-certificate consumer; do not
combine root module/BUILD/`.bzl`, external repository, lease removal, or
public overlap with this bridge.
