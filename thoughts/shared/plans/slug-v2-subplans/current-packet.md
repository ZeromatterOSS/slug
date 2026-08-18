# Current Slug V2 Packet

Packet: `WP-2A-m1-host-glob-frontier-implementation`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Accepted design: `c271b07c`
Result: implement exactly the frozen callerless observed sibling at the natural
`HostGlobTraversalKey` owner.

## Design task

Audit the live loading adapter -> traversal -> segment-candidate ->
package-boundary graph. Account for every decisive traversal prefix,
breadth-first ordinal and candidate order, recursive progress, boundary stop,
grouped Need, first-ranked error and final sorted path.

The traversal is complete: its observed segment child supplies literal or
wildcard listing/base/matched-symlink observations and its observed boundary
child supplies ignore/package-lookup observations. The traversal alone owns
breadth-first state, recursive progress, stops, grouped Need, ranked errors and
final sorted paths; the adapter only projects its result.

Add private `HostGlobTraversalObservationKey` with the legacy structural
identity and distinct Display. Its `ObservedHostGlobTraversal` retains one
`Arc<Result<HostGlobTraversal, HostGlobTraversalError>>` plus one
`PathObservationEpoch`. Its Value wraps that carrier in
`SourcePreparationOutcome<Result<_, ObservedPathFrontierError>>` with
complete-only equality and validity.

One `Legacy | Observed` driver selects only matching segment and boundary key
families; neither traversal sibling computes the other. Adapter and callers
remain legacy.

Preserve the serial traversal exactly. Observed mode unions each completed
child epoch into one compute-local accumulated epoch in each state's segment
rank, then directory-boundary candidate-slot order, then later breadth-first
ordinal order. Union before inspecting the child's semantic Result. Reuse the
parent-module stable `PathObservationEpoch::from_shared` helper so conflicts
are detected at their exact rank and the first exact Arc wins duplicates.

Outer precedence is prefix-bounded by the first semantic terminal. An outer
child or union error before, or at the union of, that semantic rank wins over
prior Need and publishes no carrier. Otherwise the first semantic error wins
over prior Need and retains only completed epochs through its rank; later
outcomes remain dependency-owned. Without a semantic error, first outer wins
over Need; otherwise Need publishes no carrier and success retains every
completed epoch. Cancellation publishes nothing.

## Required audit and proof plan

- map literal, wildcard, recursive and package-boundary traversal branches to
  the accepted observed listing, segment and boundary siblings;
- freeze Legacy/Observed family isolation without one sibling computing the
  other;
- preserve existing breadth-first scheduling, grouped Need, ranked-error and
  final sort behavior;
- keep queues, visited sets, child carriers, join outcomes, union scratch and
  event batches compute-local;
- specify exact-Arc/order, mixed-terminal, no-event, cancellation, warm reuse,
  A/B/A and activation/nonactivation tests; and
- cite pinned Bazel 9.2 source or accepted evidence for every exact behavior.

The implementation proof must include literal/wildcard/recursive Files and
FilesAndDirs parity; segment/boundary terminal polarity; exact
segment-then-boundary/breadth-first/first-Arc order; earlier Need + semantic +
later outer decisive-prefix retention; no-semantic outer-over-Need; boundary
stops; complete-only equality/validity; warm/A-B-A; cancellation; zero events;
and legacy/observed/adapter family isolation.

Read only the bounded loading Host-glob adapter/traversal/segment owners,
Bzlmod package-boundary owner, workspace observation/resolution owners,
directly referenced tests/manifests and the utility-reuse sources named by
this packet. Do not run or add an oracle unless the audit demonstrates a
specific unproved parity gap.

## Authority and validation

Write only:

- `app/slug_loading_v2/src/host_glob/mod.rs` for a zero-net helper rename;
- `app/slug_loading_v2/src/host_glob/traversal.rs`; and
- `app/slug_loading_v2/src/host_glob/traversal_tests.rs`.

Against `dc6f6e02`, implementation caps are zero net/1,000 physical for
`mod.rs`, 350 production/880 physical for traversal, 470 tests/1,293 physical
for traversal tests, and 820 aggregate net Rust lines. No correction is
authorized. Completion-only scheduling may write canonical, this manifest and
Stage 2 under 180 aggregate net lines.

## Compatibility boundary

Existing admitted Host-glob matching, traversal, boundary, Need/error and path
ordering remain exact. Carrier association, epoch aggregation and exact-Arc
identity are Slug-native. BUILD/package-load, core/public publication,
repository/materializer work, native-Windows raw-byte ordering and exact Bazel
identity bytes remain unsupported/deferred.

## STOP / REPLAN

STOP on any other file; Cargo, fixture or oracle writes; BUILD/package-load or
adapter activation; public/core/repository/materializer work; direct,
reconstructed or historical Host reads; parent events; another carrier/
container/cache/graph/store/lock; retained work collections; changed traversal
order/ranking/stops/polarity; or cap excess.

`REPLAN` if a shared driver changes legacy behavior, a decisive child epoch
cannot remain complete, prefix terminals cannot be proved, family isolation
requires duplication, or proof needs another file/key/seam/oracle.

## Immediate successor

On acceptance return only to docs-only `WP-2A-m1-host-glob-frontier-design`
using `c271b07c` plus the implementation commit. Do not combine adapter,
BUILD/package-load or higher-caller activation.
