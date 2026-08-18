# Current Slug V2 Packet

Packet: `WP-2A-m1-observed-host-glob-segment-frontier-design`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Accepted predecessors: `9f42c3e5`, `bd12c015`, `f5a9b249`,
`bd4fb8db`
Result: freeze exactly one private observed sibling at the natural
`HostGlobSegmentCandidatesKey` owner, without implementing or activating it.

## Learned facts and research basis

The accepted Host-glob audit proved that traversal is not yet a complete
observed frontier. Its segment-candidate child currently erases:

- the `ResolvedPathKey` result selected by a literal fragment; and
- for wildcard fragments, the directory listing plus each matched symlink's
  `ResolvedPathKey` result.

Commit `bd4fb8db` now provides
`PathDirectoryListingObservationKey`, which owns wildcard base resolution
plus exact `DirectoryEntries`, and `ResolvedPathObservationKey` already
owns literal and matched-symlink resolution. The wildcard driver also needs
the base resolved-path semantic value only when at least one matched symlink
exists; computing the same observed resolution key then is a cache hit whose
duplicate epoch follows the listing epoch.

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
places the same facts in
`PatternWithoutWildcardProducer.java` (literal `FileValue`) and
`PatternWithWildcardProducer.java` (directory listing followed by matched
symlink `FileValue` batches). `DirectoryDirentProducer.java` owns the
separate ignore/package-boundary decision. Reuse matcher oracle `9f42c3e5`,
implementation evidence `bd12c015`, and the accepted
`glob-directory-invalidation` evidence; no new oracle is justified.

`dice/dice/src/api/computations.rs` implements `compute_join` with ordered
`join_all`, preserving pending-symlink input order. Stable
`PathObservationEpoch::from_shared` sorting retains the first exact Arc for
equal duplicates. The relevant Buck2/V1 decision remains compact utilities
only: keep the existing immutable Arc slices, `Dupe`, `Allocative` and
compute-local work vectors; reject V1 glob behavior and any retained queue,
set, cache, interner or graph.

## Decision and non-decisions

Freeze one private `HostGlobSegmentCandidatesObservationKey` beside the
legacy key, with identical structural inputs and a distinct Display identity.
Its carrier owns one shared semantic Result plus one
`PathObservationEpoch`. Both keys use one mode-aware segment driver and each
computes only its matching child-key family.

Do not change the workspace listing carrier, implement observed traversal,
activate the adapter, add events, alter public loading behavior, or select
BUILD/package-load policy. Traversal and boundary composition remain later
packets.

## Required frozen driver

The accepted design must specify:

1. private key, carrier, Value, equality and validity shapes;
2. literal legacy/observed branches over `ResolvedPathKey` and
   `ResolvedPathObservationKey`, preserving missing/present/error/Need;
3. wildcard legacy/observed branches over `PathDirectoryListingKey` and
   `PathDirectoryListingObservationKey`;
4. zero additional segment-driver base-resolution compute beyond the listing
   when no matched symlink is pending;
5. the additional matching-family base resolved-path compute/cache hit only in
   the existing nonempty-pending branch;
6. matched-symlink `compute_join` in listing/pending-slot order with unchanged
   slot projection, first semantic error and error-over-Need behavior;
7. deterministic observation order: listing first, cached base resolution
   second, then completed matched-symlink epochs in pending-slot order;
8. exact first-Arc duplicate retention, especially for the listing/base
   resolution overlap;
9. completed outer mismatch/conflict precedence, including mixed outer error,
   semantic error and Need outcomes, with no carrier;
10. Need and cancellation publication, parent event absence, warm reuse,
    A/B/A invalidation and legacy/observed family isolation; and
11. exact implementation/test allowlist, cfg-aware line caps, physical
    ceilings and cohesion decision.

The design must preserve current literal/wildcard mapping, listing raw-name
filter/order, matched-symlink batch concurrency, first-slot semantic error,
semantic error over aggregated Need, disappearance/inconsistent/cycle/infinite
expansion handling, and candidate sort.

## Ownership, request and memory model

`HostGlobSegmentCandidatesObservationKey` is the sole producer of the
observed segment value. A completed semantic success or error retains one
`Arc<Result<HostGlobSegmentCandidates, HostGlobSegmentError>>` plus the
existing Arc-backed epoch. Listing entries, candidate slots, pending symlinks,
join outcomes, needs, first error and union inputs stay compute-local.

Need, cancellation and completed outer error publish no carrier or event data.
Completed child cache state beyond the decisive terminal remains
dependency-owned. The request carries no overlay and performs no direct or
historical Host read. Existing serial request semantics and current
matched-symlink batching remain unchanged. No shared lock spans a DICE compute.

## Compatibility boundary

Existing admitted single-segment matching, candidate selection/order,
literal/wildcard/symlink error and Need behavior remain exact. Existing path
resolution and directory-listing values remain exact. The observed carrier
association, epoch aggregation and exact-Arc identity are Slug-native.

Observed traversal, adapter activation, BUILD retry, package loading,
core/public publication, external/routed repositories, materialization,
native-Windows raw-byte ordering, V1 behavior and exact Bazel identity bytes
remain unsupported/deferred.

## Evidence and validation design

Require proof for literal present/missing/error/Need; wildcard
present/missing/listing error; no-match and no-symlink short-circuits; matched
symlink success/missing/error/infinite expansion; exact listing/base/symlink
Arc retention; duplicate-first union; mixed semantic-error/Need and outer
error precedence; equality/validity; warm and A/B/A; cancellation recovery;
zero parent events; and zero cross-family or traversal activation.

Reuse existing test scripts and activation trackers. No production test key,
global state, controllable cancellation seam, fixture or oracle is authorized.
Validation must include focused segment tests, full `slug_loading_v2`, direct
`slug_core_v2` check, formatting, strict Clippy/archive dispositions, exact
accounting, artifact/scope scans and `git diff --check`.

## Exact authority and caps

Write only:

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`;
- this manifest; and
- `thoughts/shared/plans/slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`.

Candidate future Rust scope is exactly:

- `app/slug_loading_v2/src/host_glob/mod.rs`; and
- `app/slug_loading_v2/src/host_glob/tests.rs`.

Read only those Rust owners, directly referenced workspace observation and
resolution owners, the accepted lower-boundary owner, cited DICE/Bazel/tests/
oracle/manifests, the three planning files, Stage 9's matching row and required
utility-reuse sources.

This design packet must freeze future caps against the `bd4fb8db` baseline.
Current physical bases are 723 lines for `mod.rs` and 889 for `tests.rs`.
Docs ledger caps remain 40 canonical, 320 manifest, 280 Stage 2 and 640
aggregate net lines. No cap-only correction is authorized.

## STOP / REPLAN

STOP on Rust, Cargo, fixture, oracle or generated writes; another key/carrier/
container/cache/graph/store/lock; workspace/Bzlmod changes; observed traversal
or adapter work; public/core/BUILD/package-load/repository/materializer work;
direct/reconstructed/historical Host reads; events; changed batching/order/
polarity; a retained work collection; generic certificates; or cap excess.

`REPLAN` if the segment owner cannot retain every selected literal/wildcard/
matched-symlink observation; the shared driver changes legacy behavior; the
listing/base overlap cannot preserve its first exact Arc; outer, semantic and
Need precedence cannot remain distinct; family isolation requires duplicated
full drivers; focused proof needs another file/key/seam/oracle; or cleanup
finds a real split.

## Immediate successor

On design acceptance schedule only one bounded
`WP-2A-m1-observed-host-glob-segment-frontier-implementation`. Do not combine
traversal, adapter, BUILD evaluation, package loading, core or public work.
