# Current Slug V2 Packet

Packet: `WP-2A-m1-observed-path-frontier-key-implementation`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Result: implement only the accepted callerless workspace observed-resolution
and Bzlmod-private observed-Host-file sibling-key vertical. Preserve every
legacy key and caller; do not activate loading or core.

## Fixed predecessor and compatibility

Design packet `8a87ce8a` resolves the prerequisite selected after
`c1d875ad`, `9d1c6b80`, and `3a627ebb`. The existing
`ResolvedPathKey` and `HostFileBytesKey` remain byte-for-byte behavior and
value-contract invariants. They still own all current production callers.

The new chain is callerless except that
`HostFileBytesObservationKey` consumes
`ResolvedPathObservationKey` and focused tests compute the observed Host-file
root. It proves the lower complete frontier representation only; it does not
satisfy package/module/loading aggregation, core request revision, public
overlap, or M1 completion.

Exact existing Host observation semantics remain exact where already admitted.
The sibling-key identity, retained frontier, union/conflict algebra, and future
batch validation are Slug-native. No new Bazel parity or oracle is required.

## Shared-Arc epoch construction

In `slug_workspace_v2::PathObservationEpoch`, add one bounded construction or
union API over
`(PathObservationDemand, Arc<PathObservationResult>)` pairs. Preserve the
existing owned-result constructor and its behavior.

The shared API must:

- deterministically sort by exact demand identity;
- validate demand/result operation agreement;
- retain exact result Arcs without copying file bytes or link payloads;
- coalesce same-demand structurally equal results, preserving one shared Arc;
- reject same-demand structurally different results with a typed epoch error;
- accept empty and singleton epochs;
- retain the existing
  `Arc<SortedMap<PathObservationDemand, Arc<PathObservationResult>>>`;
- avoid a second retained map, interner, cache, hash identity, or deep clone;
  and
- remain `Dupe` and `Allocative`.

Pointer identity is a memory/clone property, not DICE semantic equality.
Structural demand/result equality remains authoritative.

Add doc-hidden public `ObservedPathFrontierError` wrapping the typed epoch
construction error. Shared duplicate conflict and operation mismatch are
completed infrastructure failures, not panics and not legacy semantic
resolution/Host-file errors. They retain no partial carrier.

Both sibling key values use an outer complete
`Result<ObservedCarrier, ObservedPathFrontierError>` inside `PathOutcome`.
Need remains the only incomplete state.

## Observed resolution sibling

Add doc-hidden public `ResolvedPathObservationKey` with exactly the namespace
and normalized logical-path structural identity of `ResolvedPathKey`.
Bzlmod may construct it through a sealed public constructor; fields remain
private.

Add doc-hidden public `ObservedResolvedPath` containing:

- one complete `Result<ResolvedPath, PathResolutionError>`; and
- one exact `PathObservationEpoch` containing every completed Lstat/ReadLink
  observation consumed before that success or terminal error.

Expose borrowed `result()` and `observations()` only. Derive or implement
structural equality, `Dupe`, and `Allocative`.

The legacy `Result<ResolvedPath, PathResolutionError>` stays inside the
successful outer observed carrier, including when that legacy result is Err.

Factor the existing resolution state-machine driver so both keys use the same
transition logic and Need/error order. The legacy key does not compute or
retain the new carrier. The observed key captures each exact completed
`Arc<PathObservationResult>` before passing it to the machine and creates the
epoch only at terminal success/error. A lower Need is forwarded unchanged;
scratch pairs drop. Cancellation retains nothing. The observed key never
computes `ResolvedPathKey` and never performs a second observation for one
machine transition.

Key equality is complete carrier equality; validity is complete-only. Need is
invalid and never equal. An empty epoch is allowed only for a terminal reached
before any Host observation.

Export only the observed key/carrier from workspace `lib.rs` under
`#[doc(hidden)]`. Do not export a builder or mutable collection.

## Observed Host-file sibling

In Bzlmod `host_file.rs`, add crate-private
`HostFileBytesObservationKey` with the same normalized logical-path identity
as `HostFileBytesKey`, and crate-private `ObservedHostFileBytes` containing:

- one complete `Result<HostFileBytes, HostFileError>`; and
- the complete exact `PathObservationEpoch`.

Its key value uses the same outer completed
`ObservedPathFrontierError` when final-FileBytes union fails; it does not
launder that failure into `HostFileError`.

The observed key consumes only `ResolvedPathObservationKey`. Resolution
success/error/missing/wrong-kind ordering remains identical to the legacy key.
When resolution reaches a regular or special file, compute the same final Host
FileBytes demand once, retain its exact result Arc, union it with the resolution
epoch, and then project the same present/inconsistent/operation-error semantic
result. Missing, wrong-kind, resolution error, cycle, and expansion terminals
carry only their exact completed resolution epoch. Need at either stage returns
no carrier.

Expose private borrowed result/epoch access for focused tests and the next
hierarchical owner only. Do not edit Bzlmod `lib.rs`, activate a current
caller, or route a legacy key through the sibling.

## Failure, cancellation, and memory

Construction conflict or operation mismatch returns the frozen typed outer
infrastructure error and never a partial carrier. It must preserve every
underlying semantic Host-file or resolution result and cannot alter
legacy/public Display text.

All retained memory is DICE-value lifetime: semantic result plus one shared
immutable epoch. Scratch pair buffers drop on Need, error before carrier
construction, or cancellation. No evaluator, transaction, updater, event,
accepted snapshot, repository result, materializer, worker, observer lease, or
lock is retained. No lock or callback is introduced.

Use the existing resolution-machine hop/expansion bounds. STOP rather than add
an unrelated cardinality policy.

## Implementation allowlist, caps, and proof

Edit exactly these Rust files:

- `app/slug_workspace_v2/src/path_observation.rs`;
- `app/slug_workspace_v2/src/path_resolution.rs`;
- `app/slug_workspace_v2/src/lib.rs`; and
- `app/slug_bzlmod_v2/src/host_file.rs`.

At completion edit only canonical, current-packet, and Stage 2 ledgers. No
Cargo/BUILD file or dependency change is authorized.

Corrected caps are 380 net production lines, 650 in-module test lines, and
1,030 total added Rust lines. Physical file ceilings after formatting are
1,750 lines for `path_observation.rs`, 4,400 for `path_resolution.rs`, 580
for workspace `lib.rs`, and 1,100 for `host_file.rs`. Completion-ledger
growth is capped at 200 net lines. The single cap-only correction is consumed
by the required discriminating proof; it adds no files, callers, or behavior.

Focused proof must cover:

- shared-Arc epoch empty/singleton/union ordering, operation mismatch,
  structurally equal duplicate coalescing, conflicting duplicate failure, and
  pointer-preserving no-deep-clone behavior;
- outer frontier-error complete equality/validity, no partial retained epoch,
  and forced duplicate-conflict/operation-mismatch coverage at both sibling
  boundaries;
- direct present, missing, wrong-kind, and final FileBytes operation error;
- relative and absolute symlink chains, retarget, resolution observation error,
  inconsistent state, cycle, and expansion terminal epochs;
- exact prefix plus final FileBytes retention, with the final demand once;
- Need at resolution and final-read stages with no partial carrier;
- complete-only equality/validity and success/error A-B-A restoration;
- legacy key results/equality/Need order unchanged and no legacy-key activation
  by the sibling chain;
- cancellation/drop with no retained scratch/evaluation data/transaction; and
- `Dupe`/Allocative compact ownership.

Run focused tests, full `cargo test -p slug_workspace_v2`, full
`cargo test -p slug_bzlmod_v2`, downstream
`cargo check -p slug_core_v2`, strict relevant Clippy, formatting,
`scripts/v2_archive_status.sh`, status/artifact checks,
`git diff --check`, exact cfg-aware line accounting, and independent
DICE/ownership/memory review. Record inherited baselines explicitly.

STOP on any other file, third key/store/graph, legacy key/value/caller
migration, loading/core/public caller, request revision/finalization,
repository/module/BUILD/`.bzl`/glob activation, public API/wire/output,
generic certificate framework, panic/unreachable or legacy-error laundering
for frontier construction, reverse dependency, direct or historical Host
read outside the existing observation owner, watcher, oracle, JVM, or cap
excess.

`REPLAN` if the shared state-machine driver changes legacy ordering, complete
errors cannot retain the exact prefix, the observed Host-file path needs a
legacy caller or extra key, epoch sharing deep-clones payloads, doc-hidden
workspace visibility is insufficient, existing expansion bounds do not bound
the carrier, or the four-file/cap boundary cannot hold.

## Immediate successor

Accept only after complete proof and independent DICE/ownership/memory review.
Then run a docs-only hierarchical Host-loading frontier composition audit,
starting with repository-ignore and root-module predecessors before package
markers. Do not activate package/module/loading/core/public consumers with this
lower vertical.
