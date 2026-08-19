# Current Slug V2 Packet

Packet: `WP-2A-m1-source-certificate-epoch-acceptance-implementation`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Scheduling base: `5cd5e72c`
Accepted Rust base: `2e1c1334`
Accepted design: `5cd5e72c`
Result: implement the epoch-shaped final source-certificate prerequisite before
external exported-source build publication.

## Exact Rust authority and caps

Write exactly these files against Rust base `2e1c1334`:

1. `app/slug_core_v2/src/runtime/request_revision.rs`: <=120 production and
   <=180 colocated test net, <=1,750 physical;
2. `app/slug_core_v2/src/runtime/dice.rs`: <=80 production and <=40 colocated
   test net, <=11,050 physical; and
3. `app/slug_core_v2/src/runtime/tests/build_command_tests.rs`: <=300 test net,
   <=3,000 physical.

Aggregate semantic cap <=720 and combined physical <=15,800. The large core
owner/test files remain cohesive exceptions; every new or materially touched
helper stays below 200 lines. Cargo manifests, BUILD, fixtures, oracles,
generated evidence, `repository_io.rs`, events, bzlmod/loading, exports,
callers and public activation are forbidden.

## Learned facts and decision

The completed post-query audit selects a uniquely smaller prerequisite. The
eventual upper owner is the existing structurally distinct
`BuildCommandRootObservationKey`: public build already tries it first, and
syntax can admit exactly one nonroot `TargetPattern::Single` without widening
multi-build. Its external branch can later sequence accepted observed anchor,
route, repository-package and Host-source carriers.

The current one-demand `SourceCertificate` is incomplete for that owner.
`HostRepositorySourceFileObservationKey` retains a complete resolution plus
source epoch. Absent and non-file terminals stop before FileBytes, symlink
retargeting requires the logical resolution prefix as well as real-path bytes,
and Materialization-namespace demands cannot be refreshed by
`RequestRevisionOwner::observe_exact`, which supplies no materialization
roots. Extracting one FileBytes entry is therefore neither total nor
race-complete. Do not alter or duplicate the accepted lower carriers.

Bazel 9.2 `BuildTool.buildTargets/processRequest` remains the exact public
build boundary; `TargetDefinitionContext.createInputFile` and `InputFile`
remain the accepted exported-source classification evidence. This packet
changes only Slug-native acceptance/retry association. Retained Buck2-derived
DICE transaction/cancellation behavior is grounded in
`dice/dice/src/transaction.rs` and
`dice/dice/src/impls/tests/general.rs`; no donor code is imported.

## Frozen certificate and finalization contract

Replace the private certificate payload with a nonempty compact
`PathObservationEpoch`. Keep a one-demand constructor for the existing root
source path and add a checked epoch constructor/accessor. The epoch retains the
exact shared Result Arcs from the semantic producer, derives `Allocative` and
`Dupe`, rejects empty/duplicate/conflicting or operation-mismatched input, and
adds no second map or interner. A terminal certificate must be an exact
demand/value/`Arc::ptr_eq` subset of the full terminal/selected path epoch.

After terminal selection, selected-snapshot preparation and complete observed
terminal validation, `finalize_native` must reobserve every certificate
demand through one synchronous callback backed by the active
`RepositoryMaterializer::observe_native` session. That existing owner supplies
both Host and retained Materialization namespace roots. The callback performs no
DICE compute and no await.

Hold the request-revision publication owner continuously across exact
reobservation and revision publication. The command owns the workspace lease
before opening the materializer session; finalization may synchronously enter
the materializer while holding the revision owner, but no path may hold the
materializer mutex while acquiring the revision owner. Freeze and prove this
lock order and keep every mutex guard outside DICE computation.

If every demand/value is equal, preserve the original certificate/full-epoch
Result Arcs and commit the already prepared selected updater. If any value
changed, drop that updater, replace only changed certificate demands in the
full epoch with newly observed Arcs, preserve every equal certificate and
unrelated Arc, publish the next request revision and retry. Missing certificate
demands, value/Arc association mismatch, observation failure, namespace/root
failure, injection or publication failure are typed fail-closed session errors;
they never publish a terminal.

Need, typed outer, cancellation, selection/validation/materializer failure and
restorable abort preserve the prior accepted path/repository/event snapshot.
Revision retry emits no provisional events. Accepted event reconciliation and
repository selection remain unchanged and occur at their existing atomic
boundary.

## Memory, request and compatibility

The semantic build Result may retain one certificate epoch in addition to the
existing complete selected epoch; both share immutable Result Arcs. Reobserved
epoch, changed-demand scan and replacement entries are command scratch. Add no
retained map, Vec, cache, store, interner, task or lock and no direct Host read
outside the existing materializer callback. Overlapping commands remain
serialized by the native workspace lease; no historical filesystem snapshot is
invented.

Exact: current root-source success/error bytes, diagnostics, revision retries,
public build/query/cquery results, event order and every legacy/direct API.

Slug-native: epoch-shaped certificate, exact Arc association, materialization-
aware final reobservation and private retry mechanics.

Unsupported/deferred: admission of external observed build, multi-build
certificate aggregation, one-shot cutover, broader build/query surfaces and
exact Bazel identity bytes.

No fixture or new oracle is needed: accepted source/build lifecycle evidence is
reused. No fallback or Stage 9 ledger row is created because the implementation
reuses the existing V2 `PathObservationEpoch`, `Dupe` and `Allocative`
representation without importing donor code.

## Required proof

Prove:

- existing singleton root one-demand semantic/result/event parity and exact
  certificate/full-epoch Arc identity;
- nonempty construction plus empty/duplicate/conflict/operation-mismatch
  rejection;
- multi-demand equality and one/multiple changed demands, preserving exact
  equal and unrelated Arcs;
- Host and Materialization namespace refresh through the active repository
  session;
- missing/directory and symlink-retarget A/B/A certificate epochs, including
  equal FileBytes behind a changed resolution prefix;
- observation, injection and publication failure plus poll-drop cancellation
  leave prior accepted path/repository/event state and recover;
- retry publishes no provisional batch and accepted changed source batches
  retain existing order/suppression;
- legacy/public output parity and zero external observed-root activation; and
- exact accounting, formatting, focused revision/build tests, direct server
  dependent, archive status, Buck2 retention scan, AI cleanup and independent
  final review.

STOP on any fourth Rust file, lower-carrier/public/caller change, new key/store/
lock/task, direct Host observation, partial certificate comparison, changed
legacy output/event semantics, cap excess or external-build activation. REPLAN
if the materializer callback cannot refresh every namespace while preserving
the lock/atomicity contract. After implementation ACCEPT, return directly to
one docs-only external singleton observed-build design; do not activate
multi-build, one-shot or close M1.
