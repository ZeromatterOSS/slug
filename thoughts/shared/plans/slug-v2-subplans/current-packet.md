# Current Slug V2 Packet

Packet: `WP-2A-m1-host-glob-frontier-design`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Accepted predecessors: `b9fda97d`, `f5a9b249`, `bd4fb8db`
Result: freeze the smallest complete callerless observed frontier for the
existing private Host-glob graph, without implementing or activating it.

## Learned facts and research basis

The live loading chain remains adapter -> `HostGlobTraversalKey` ->
`HostGlobSegmentCandidatesKey` -> `HostRootPackageBoundaryKey`, followed
by dormant package attempts. Commit `bd4fb8db` now supplies the two natural-
owner lower inputs that the prior audit found missing:

- workspace `PathDirectoryListingObservationKey` retains the complete
  resolved-path epoch plus the exact final `DirectoryEntries` result; and
- Bzlmod `HostRootPackageBoundaryObservationKey` retains repository-ignore
  plus, when not ignored, package-lookup observations.

The source audit must compare Slug with Bazel commit
`8220c6198837d5c13d53fea211cf3282aa12408a`, especially
`UnixGlob.java`, `GlobFunction.java`, `GlobFunctionTest.java`, and
`PackageFunctionTest.java`. Reuse the accepted exact matcher evidence in
`9f42c3e5`, implementation evidence in `bd12c015`, and the existing
`glob-directory-invalidation` oracle. Add no oracle unless this bounded
audit demonstrates an unproved behavior.

Read `docs/developers/dice.md` before deciding key ownership. Reuse the
accepted Arc-backed `PathObservationEpoch`, `SmallMap`, `SmallSet`,
`Dupe` and `Allocative` patterns. Do not import V1 glob behavior or add an
interner, retained queue, vector, graph, store, cache or lock.

## Design decision

Design exactly one loading-private callerless observed Host-glob frontier that
consumes the new observed listing and boundary siblings. If the audit proves
that another uniquely smaller natural-owner observed predecessor is still
required for completeness, `REPLAN` to that one docs-only prerequisite
instead. This packet writes no Rust, tests, fixtures, oracle output or Cargo
metadata.

The design must freeze:

1. the producer key/value and the precise existing legacy driver shared with
   it, without one key computing its sibling;
2. every mutable predecessor that can decide a completed traversal, including
   selected and negative resolution/symlink probes, exact directory listings,
   package-boundary inputs and the decisive traversal prefix;
3. breadth-first ordinal/candidate order, recursive progress, boundary stops,
   grouped Need, first-ranked error and final sorted-path behavior;
4. the exact point at which a child Need prevents a parent carrier;
5. inner semantic errors versus typed outer epoch mismatch/conflict errors;
6. deterministic observation union order and exact first-Arc retention for
   equal duplicate observations;
7. ignored-directory and negative-marker short-circuits, proving no later
   listing, boundary or attempt activation;
8. completed-value equality/validity, warm reuse, A/B/A invalidation and
   simultaneous legacy/observed family isolation;
9. parent event absence and cancellation/drop safety without a production
   synchronization seam; and
10. the minimal implementation/test allowlist and exact cfg-aware production,
    test, total and physical-line caps.

## Ownership, request and memory model

The producer key owns the complete observed value. A completed value may retain
only one semantic `Result` Arc plus the existing Arc-backed observation epoch.
Traversal queues, candidate/visited sets, child carriers, evaluators,
transactions, union scratch and event batches remain compute-local.

No request overlay or historical/direct Host read may supply observations.
Existing serial admitted behavior remains the design target; do not invent
parallel traversal. Need, cancellation and completed outer error publish no
parent carrier or parent event data. Child cache state beyond the decisive
terminal remains dependency-owned. No shared lock may span a DICE compute.

## Compatibility boundary

Existing admitted Host-glob matching, traversal order, recursive progress,
boundary-stop, Need/error/event and sorted-path behavior remain exact. Existing
path, listing, repository-ignore, package-marker, package-boundary and Host
observation values remain exact. The carrier association, epoch aggregation
and exact-Arc identity are Slug-native.

BUILD retry, package loading, core/public publication, external or routed
repository globbing, repository/materializer work, native-Windows byte
ordering, V1 behavior and exact Bazel identity bytes remain
unsupported/deferred.

## Required evidence and proof design

The accepted design must cite:

- the decisive Slug source graph and matching pinned Bazel source/test anchors;
- one complete mutable-predecessor table covering success, empty, boundary,
  error and Need terminals;
- an exact deterministic epoch order for a discriminating recursive case;
- exact Arc retention and duplicate-first ownership;
- activation and event-absence proof, including ignored/negative shortcuts;
- cancellation/drop and successor-transaction recovery by source reasoning;
- warm and A/B/A proof without reconstructed or historical reads;
- retained-memory accounting and nine-category cleanup expectations; and
- exact implementation/test caps with an independent cohesion decision for
  any owner already over 1,500 physical lines.

Reuse accepted Bazel 9.2 oracle evidence when it discriminates the design.
Schedule new oracle work only after naming a demonstrated uncovered exact
surface.

## Exact authority and caps

Write only:

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`;
- this manifest; and
- `thoughts/shared/plans/slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`.

Read only:

- `app/slug_loading_v2/src/host_glob*.rs` and their directly referenced
  private tests/attempt owners;
- `app/slug_bzlmod_v2/src/host_package_boundary/{mod,tests}.rs`;
- `app/slug_workspace_v2/src/path_resolution.rs` and directly referenced
  observation/resolution owners;
- the three planning files above, `docs/developers/dice.md`, Stage 9 retained-
  utility evidence and the required utility-reuse skill sources;
- the pinned Bazel source/test files named above; and
- existing manifests or oracle files directly cited by those owners.

Ledger caps are 40 net lines in canonical, 320 in this manifest, 280 in Stage
2, and 640 aggregate. No cap-only correction is authorized.

## STOP / REPLAN

STOP on code, Cargo, fixture, oracle or generated writes; BUILD/package-load
activation; public/core/repository/materializer work; a generic certificate
framework; reverse dependencies; another retained container/cache/graph/store/
lock; reconstructed, direct or historical Host reads; event ownership;
parallelism; compatibility widening; or any cap excess.

`REPLAN` if the lower observed siblings are still insufficient; a complete
frontier crosses another natural semantic owner; legacy and observed behavior
cannot share one driver without drift; exact observations cannot be retained
without a second collection; inner/outer/Need polarity changes; a new oracle
is required before design acceptance; the proof requires a production test
seam; or cleanup finds a real split outside the bounded authority.

## Immediate successor

On acceptance schedule exactly one bounded callerless Host-glob frontier
implementation, or exactly one smaller docs-only natural-owner predecessor
design if the audit requires it. Do not combine adapter, BUILD evaluation,
package loading, core or public activation.
