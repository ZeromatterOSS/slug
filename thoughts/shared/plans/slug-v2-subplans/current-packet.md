# Current Slug V2 Packet

Packet: `WP-2A-m1-observed-host-glob-segment-frontier-implementation`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Accepted design: `dc696b2d`
Result: implement exactly one private observed sibling at the natural
`HostGlobSegmentCandidatesKey` owner without activating traversal or callers.

## Frozen implementation contract

Add private `HostGlobSegmentCandidatesObservationKey` with the same
logical-directory/pattern structural identity as the legacy key and a distinct
Display. Add `ObservedHostGlobSegmentCandidates` retaining:

- `Arc<Result<HostGlobSegmentCandidates, HostGlobSegmentError>>`; and
- one `PathObservationEpoch`.

Its Value is
`SourcePreparationOutcome<Result<ObservedHostGlobSegmentCandidates,
ObservedPathFrontierError>>` with complete-only equality and validity.

Replace the legacy orchestration with one `Legacy | Observed` mode-aware
driver. Legacy computes only `ResolvedPathKey` and
`PathDirectoryListingKey`; observed computes only
`ResolvedPathObservationKey` and
`PathDirectoryListingObservationKey`. Neither segment key computes its
sibling.

## Literal behavior

Preserve the existing exact literal mapping:

1. Need returns Need with no carrier;
2. completed resolution error maps to the unchanged semantic segment error;
3. missing completes an empty candidate set;
4. present completes one candidate with the unchanged kind projection; and
5. observed success or semantic error retains the exact resolved-path epoch,
   while an outer frontier error completes without a carrier.

## Wildcard behavior

Preserve raw listing filtering, entry slots, pending-symlink batch concurrency,
slot projection, first semantic error, error-over-Need, all existing error
mapping and final candidate sort.

Observed order is:

1. compute the observed listing and begin with its exact epoch;
2. if no matched symlink is pending, complete without another base-resolution
   compute;
3. otherwise compute the observed base resolution to recover `real_path`,
   union listing then base, and retain listing Arcs for equal duplicates;
4. compute matched symlink resolutions with the existing ordered
   `compute_join`; and
5. process results in pending-slot order.

Outer handling is prefix-bounded. An outer error before the first semantic
error wins over prior Needs and returns completed outer error without a
carrier. At the first semantic error, stop parent aggregation: it wins over
prior Needs and retains only listing/base plus completed symlink epochs through
that slot. Later outcomes remain dependency-owned and cannot alter the parent.
If no semantic error exists, examine the full batch: first outer error wins
over Need; otherwise any Need returns Need without a carrier, and success
retains every completed epoch.

Listing/base outer errors publish no carrier. Listing or resolved-path semantic
errors retain the complete decisive epoch. Union uses stable
`PathObservationEpoch::from_shared` in listing, base, then pending-slot order.

## Ownership, memory and cohesion

The observed segment key solely owns the carrier. Listings, slots, pending
entries, join outcomes, needs, errors and union scratch remain compute-local.
Need, outer error and cancellation publish no carrier or parent event data.
There is no request overlay, direct/historical Host read, task, lock, cache,
interner, graph, store or retained work collection.

Keep the sibling in the 723-line natural segment owner. Because the existing
wildcard function crosses the 150-line complexity trigger, extract bounded
mode-aware lower-input and pending-symlink helpers; do not split ownership or
duplicate the full driver. Preserve immutable Arc slices, `Dupe`,
`Allocative` and the accepted compact utility boundary.

## Required proof

Add only colocated Unix tests proving:

- literal present, missing, semantic error, Need and outer error parity;
- wildcard listing success, missing and semantic/outer error parity;
- no-match/no-pending completion with zero additional segment base compute;
- matched symlink success, missing, semantic error and infinite expansion;
- exact listing/base/symlink Arc retention and listing-first duplicate Arc;
- earlier Need + semantic error + later outer outcome returns the semantic
  carrier with only the decisive prefix;
- no-semantic full-batch outer error wins over Need;
- complete-only equality/validity, warm reuse and A/B/A;
- cancellation/drop recovery without a production synchronization seam;
- zero parent event data; and
- observed graph activates only observed children and zero legacy segment,
  resolution or listing keys, while legacy and traversal activate zero observed
  segment children.

Reuse existing scripts, activation trackers and accepted Bazel 9.2 evidence.
Do not add a production test key, global state, fixture or oracle.

## Exact authority and caps

Write only:

- `app/slug_loading_v2/src/host_glob/mod.rs`; and
- `app/slug_loading_v2/src/host_glob/tests.rs`.

Completion-only scheduling writes may update canonical, this manifest and
Stage 2 under 180 aggregate net lines.

Against `bd4fb8db`:

- `mod.rs`: 280 production, zero test, 280 total net lines and 1,003 physical
  lines;
- `tests.rs`: zero production, 420 test, 420 total net lines and 1,309
  physical lines; and
- aggregate: 280 production, 420 test and 700 total net Rust lines.

No cap-only correction is authorized.

## Validation

Run serially with `CARGO_TARGET_DIR=.codex-cargo-target` and
`CARGO_BUILD_JOBS=1`:

- focused observed-segment tests;
- full `cargo test -p slug_loading_v2`;
- direct `cargo check -p slug_core_v2`;
- `cargo fmt --all -- --check`;
- strict Clippy and `scripts/v2_archive_status.sh`, recording inherited stops
  without presenting them as passes;
- exact cfg-aware line and physical accounting;
- artifact, scope, event, caller and family-activation scans; and
- `git diff --check`.

Require independent DICE/ownership, retained-memory and nine-category cleanup
acceptance before commit.

## Compatibility boundary

Existing admitted segment matching, selection, order, Need/error and candidate
behavior remain exact. Existing path-resolution and listing values remain
exact. Carrier association, epoch aggregation, decisive-prefix retention and
outer-error precedence are Slug-native.

Observed traversal, adapter activation, BUILD retry, package loading,
core/public publication, external/routed repository globbing,
repository/materializer work, native-Windows raw-byte ordering, V1 behavior
and exact Bazel identity bytes remain unsupported/deferred.

## STOP / REPLAN

STOP on any other Rust file; a second key/carrier/container/cache/graph/store/
lock; workspace/Bzlmod changes; observed traversal or adapter work; public/
core/BUILD/package-load/repository/materializer work; direct/reconstructed/
historical Host reads; parent events; changed batching/order/polarity; retained
work collections; generic certificates; fixture/oracle writes; or cap excess.

`REPLAN` if the shared driver changes legacy behavior; exact literal/listing/
matched-symlink observations cannot remain complete; prefix-bounded mixed
terminal behavior cannot be proved; family isolation needs duplicated drivers;
focused proof needs another file/key/seam/oracle; the cohesion split crosses
ownership; or any material correction is required.

## Immediate successor

On acceptance return only to docs-only
`WP-2A-m1-host-glob-frontier-design` using `dc696b2d` plus the implementation
commit. Do not combine traversal, adapter, BUILD or package-load work.
