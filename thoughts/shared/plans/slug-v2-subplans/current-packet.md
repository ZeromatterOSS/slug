# Current Slug V2 Packet

Packet: `WP-2A-m1-observed-host-glob-input-frontiers-implementation`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Accepted design: `f5a9b249`
Result: implement the two callerless natural-owner observed input frontiers
required by a later complete Host-glob traversal, without activating traversal,
BUILD evaluation, package loading, core, or public overlap.

## Frozen decision

The Host-glob audit proved that direct traversal remains partial while two
lower semantic owners erase exact observations:

- workspace `PathDirectoryListingKey` returns only directory-listing
  semantics, not its resolved-path epoch plus final `DirectoryEntries`
  observation; and
- Bzlmod `HostRootPackageBoundaryKey` returns only ignored/deleted/package/
  no-package semantics, not its repository-ignore plus package-lookup epochs.

Implement exactly one observed sibling beside each legacy owner. Reuse the
existing `PathObservationEpoch`, `ObservedPathFrontierError`, `Dupe` and
`Allocative` patterns. Do not create another retained container, cache,
interner, graph, store, lock or task.

## Workspace implementation

In `slug_workspace_v2::path_resolution` add:

- doc-hidden public `ObservedPathDirectoryListing { result:
  Result<PathDirectoryListing, PathDirectoryListingError>, observations:
  PathObservationEpoch }` with borrowed `result()` and `observations()`;
- doc-hidden public `PathDirectoryListingObservationKey { namespace,
  logical_path }`, the legacy structural identity, distinct Display and public
  constructor; and
- Value `PathOutcome<Result<ObservedPathDirectoryListing,
  ObservedPathFrontierError>>`, `complete_eq` equality and `is_complete`
  validity.

Replace the legacy orchestration with one private mode-aware directory-listing
driver. Legacy mode computes only `ResolvedPathKey`, returns its unchanged
Value/error/API and discards the guaranteed-empty transient epoch. Observed
mode computes only `ResolvedPathObservationKey`.

Preserve this exact order and polarity:

1. resolution Need returns Need with no carrier;
2. completed resolution error, missing or wrong-kind retains only the complete
   resolution epoch in observed mode;
3. only a present directory computes the existing exact
   `DirectoryEntries` demand;
4. final Need returns Need with no carrier;
5. observed mode appends the exact returned result Arc before interpreting
   Present/Missing/Error; and
6. operation mismatch or epoch conflict is a completed outer
   `ObservedPathFrontierError`, never a semantic listing error or sibling-path
   panic.

The legacy fixed-operation mismatch remains its existing unreachable invariant.
Neither key computes its sibling. Reexport only the new carrier and key under
`#[doc(hidden)]` from the workspace crate root.

## Bzlmod implementation

In `slug_bzlmod_v2::host_package_boundary` add:

- doc-hidden public `ObservedHostRootPackageBoundary { result:
  Arc<Result<HostRootPackageBoundary, HostRootPackageBoundaryError>>,
  observations: PathObservationEpoch }` with borrowed `result() -> &Result<...>`
  and `observations()`;
- doc-hidden public `HostRootPackageBoundaryObservationKey { workspace,
  package }`, the legacy structural identity, distinct Display and public
  constructor; and
- Value `PathOutcome<Result<ObservedHostRootPackageBoundary,
  ObservedPathFrontierError>>`, complete-only equality and validity.

Replace the legacy orchestration with one private mode-aware boundary driver.
Legacy mode computes only `HostRepositoryIgnoreKey` followed, when not
ignored, by `HostRootPackageLookupKey`. Observed mode computes only the
matching observed siblings.

Preserve this exact order and polarity:

1. repository-ignore Need or outer error returns without a parent carrier;
2. completed repository-ignore semantic error retains its epoch inside the
   unchanged boundary error;
3. a matching ignore entry completes `IgnoredDirectory` with only the ignore
   epoch and never activates lookup;
4. otherwise compute lookup, union ignore then lookup epochs before semantic
   interpretation, and map Package/Deleted/NoBuildFile/Invalid/error exactly as
   legacy; and
5. lookup Need/outer error or epoch conflict publishes no parent carrier.

Use `PathObservationEpoch::from_shared` with the accumulated left epoch first,
retaining the earlier exact Arc for equal duplicate observations. Neither key
computes its sibling or stores event data. Reexport only the new carrier and key
under `#[doc(hidden)]` from the Bzlmod crate root.

## Ownership, equality and cancellation

A completed listing value retains one inline semantic result plus the existing
Arc-backed epoch. A completed boundary value retains one semantic Result Arc
plus that epoch. Child carriers, resolved paths, ignore matcher, package
projection, union vectors, mode scratch, evaluator, transaction and event data
remain compute-local.

Need, completed outer error and cancellation publish no parent carrier or
parent event data. Completed semantic and outer errors remain valid structural
DICE values; Need remains invalid and self-unequal. Existing child observations
remain ordinary dependency-owned cache state after a parent Need or
cancellation. No shared lock spans a DICE computation.

## Required proof

Add only colocated tests in the authorized owner test modules:

- directory listing parity for present, missing, wrong-kind, resolution error
  and `DirectoryEntries` Present/Missing/Error;
- exact resolution/listing Arc retention, final demand exactly once,
  complete-only equality/validity, warm and A/B/A;
- listing Need plus forced operation mismatch/conflict as completed outer
  errors with no carrier;
- boundary ignored short-circuit and repository-ignore semantic error;
- Package/Deleted/NoBuildFile/Invalid/package-lookup error parity, deterministic
  ignore-then-lookup epoch union and exact first-Arc retention;
- boundary Need and forced union conflict/mismatch polarity;
- activation trackers proving each observed graph activates its observed
  children but zero legacy listing/boundary/ignore/lookup keys;
- zero parent event data and source-proven cancellation/drop safety; and
- direct loading compile through only the doc-hidden exports.

Preserve all existing legacy tests. Do not add a test-only production key,
global state or controllable cancellation seam.

## Exact authority and caps

Write only:

- `app/slug_workspace_v2/src/path_resolution.rs`;
- `app/slug_workspace_v2/src/lib.rs`;
- `app/slug_bzlmod_v2/src/host_package_boundary/mod.rs`;
- `app/slug_bzlmod_v2/src/host_package_boundary/tests.rs`; and
- `app/slug_bzlmod_v2/src/lib.rs`.

Completion-only writes may update canonical, this manifest and Stage 2 under
180 total net ledger lines. No Cargo, oracle, fixture or generated write is
authorized.

Caps against the accepted baselines are:

- workspace `path_resolution.rs`: 115 production, 180 in-module tests, 295
  total net lines and 4,641 physical lines;
- workspace `lib.rs`: four production, zero tests, four total and 576
  physical;
- Bzlmod boundary `mod.rs`: 120 production, zero tests, 120 total and 398
  physical;
- Bzlmod boundary `tests.rs`: zero production, 240 tests, 240 total and 1,090
  physical;
- Bzlmod `lib.rs`: four production, zero tests, four total and 391 physical;
  and
- aggregate: 243 production, 420 tests and 663 total net Rust lines.

No cap-only correction is authorized. The 4,346-line workspace owner requires
independent pre/post cohesion review; keep the sibling adjacent to its natural
resolution/listing owner unless review finds a concrete split that fits this
allowlist.

## Validation

Run serially with `CARGO_TARGET_DIR=.codex-cargo-target` and
`CARGO_BUILD_JOBS=1`:

- focused observed-listing and observed-boundary tests;
- full `cargo test -p slug_workspace_v2`;
- full `cargo test -p slug_bzlmod_v2`;
- direct `cargo check -p slug_loading_v2`;
- `cargo fmt --all -- --check`;
- strict Clippy and archive-status checks, recording inherited stops without
  presenting them as passes; and
- exact cfg-aware line accounting, physical ceilings, artifact scan and
  `git diff --check`.

Require independent DICE/ownership, compact-memory and nine-category cleanup
acceptance before commit.

## Compatibility boundary

Existing admitted serial directory-listing, path-resolution,
repository-ignore, package-marker, package-boundary and Host-glob
order/Need/error/event behavior remain exact. Existing admitted Host
observation values remain exact. The two callerless carrier associations,
epoch union and exact-Arc identity are Slug-native.

Observed segment candidates, traversal and adapter, BUILD retry, package
loading, core publication, public overlap, external/routed repository globbing,
repository/materializer work, native-Windows byte ordering and exact Bazel
identity bytes remain unsupported/deferred.

## STOP / REPLAN

STOP on any other file; a third semantic key/carrier; legacy Value/error/caller/
output change; one sibling key computing the other; duplicated full driver; a
generic certificate framework; reconstructed/direct/historical Host read; a
second retained container; event ownership; reverse dependency; loading/glob/
BUILD/package-load/core/public/repository/materializer/watcher/JVM work; or any
cap excess.

REPLAN if either shared driver changes legacy behavior; inner/outer/Need
polarity cannot remain distinct; exact observations cannot be retained at
their natural owner; ignored short-circuit or duplicate-epoch order changes;
doc-hidden one-way visibility fails; focused proof needs another key/file/seam;
cleanup finds a real split; or any material correction is required.

## Immediate successor

On implementation acceptance schedule only docs-only
`WP-2A-m1-host-glob-frontier-design` using accepted predecessor
`f5a9b249` plus the new implementation commit. Do not combine traversal,
adapter, BUILD or package-load work.
