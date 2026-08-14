# Current Slug V2 Packet

Packet: `WP-2A-m1-observed-path-frontier-key-design`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Result: design exactly one workspace observed-resolution sibling key and one
Bzlmod-private observed-Host-file sibling key as the lower complete frontier
primitive. This packet is documentation-only; it may select a four-file
callerless implementation successor but may not implement it.

## Fixed predecessors and narrow REPLAN

Commit `f0849151` accepts the private sole-root one-file final-validation
bridge. The next-consumer audit in `ea36fdcc` proves every remaining public
loading terminal has a multi-observation or dynamically expanding frontier.
The loading-frontier design activated in `c1d875ad`, with read-boundary
corrections `9d1c6b80` and `3a627ebb`, finds that the existing lower keys
discard the exact observation arcs needed for hierarchical composition.

`ResolvedPathKey` returns semantic route/state or `PathResolutionError`,
not the exact ordered Lstat/ReadLink demand/result pairs used by its resolution
machine. `HostFileBytesKey` then returns semantic bytes/error and discards
both that resolution frontier and the exact final FileBytes observation.
Reconstructing demands in Bzlmod would duplicate the workspace resolver and is
forbidden. Mutating the legacy value types would widen every lockfile,
repository-ignore, package, module, repo-file, and glob caller before a
consumer boundary is accepted.

The active loading-frontier packet prohibited selecting any new key, so it
records `REPLAN` rather than silently broadening. This prerequisite is
authorized to design only the two sibling keys below. It does not claim a
public/loading migration or M1 completion.

## Exact design candidates

Freeze or reject exactly this chain:

```text
PathObservationEpoch
        |
ResolvedPathObservationKey -> ObservedResolvedPath
        |
HostFileBytesObservationKey -> ObservedHostFileBytes
```

`ResolvedPathObservationKey` must have the same workspace namespace/path
structural identity as `ResolvedPathKey`, use the same resolution machine,
and capture each exact completed `Arc<PathObservationResult>` before the
machine transition. It must not compute `ResolvedPathKey`, perform a second
raw observation, or change the legacy key/value.

`ObservedResolvedPath` must contain one complete
`Result<ResolvedPath, PathResolutionError>` plus one exact immutable
`PathObservationEpoch` for both success and terminal error. Need and
cancellation return no carrier and drop accumulated scratch observations.

Bzlmod-private `HostFileBytesObservationKey` must consume only the observed
resolution key. Its `ObservedHostFileBytes` contains one complete
`Result<HostFileBytes, HostFileError>` plus the exact resolution epoch and,
only when resolution reaches a readable terminal path, the exact final
FileBytes demand/result. Missing, wrong-kind, resolution error, inconsistent
final read, operation error, present bytes, Need, and cancellation ordering
must remain identical to `HostFileBytesKey`.

The two keys are callerless except for the observed Host-file key consuming the
observed resolution key and focused tests computing the Host-file root.
Existing legacy consumers do not migrate in this packet or its immediate
implementation successor.

## Epoch and duplicate algebra

Reuse `PathObservationEpoch` and its retained
`Arc<SortedMap<PathObservationDemand, Arc<PathObservationResult>>>`; invent
no second retained map, interner, or cache. Design one shared-pairs
construction/union API that:

- sorts deterministically by exact demand identity;
- validates operation/result agreement and Host namespace where required;
- retains the exact result Arc without copying file bytes;
- coalesces duplicate demands only when the exact results are structurally
  equal, preferring an already shared Arc;
- rejects same-demand conflicting results with a typed error;
- accepts an empty epoch for a terminal reached before any Host demand; and
- preserves structural epoch equality independently of Arc pointer identity.

State whether path-resolution cardinality is already bounded by the resolution
machine; otherwise name one checked typed cap. All builders are request/DICE
compute scratch and disappear on Need, error before a complete carrier, or
cancellation.

## Visibility, equality, and memory

The observed resolution key/carrier may be doc-hidden public only because
Bzlmod already depends on workspace. Fields and constructors stay sealed;
expose borrowed result/epoch access only. The observed Host-file key/carrier
remain Bzlmod-private, so no Bzlmod `lib.rs` export is needed.

Completed carrier equality is structural result plus exact epoch. Validity is
complete-only; Need is invalid and never equal. Retention uses `Dupe`,
`Allocative`, shared Arc results, and the existing deterministic
`SortedMap`. No evaluator, transaction, updater, accepted snapshot, event,
repository result, materializer, worker, or lock is retained.

The dependency direction remains
`slug_workspace_v2 -> slug_bzlmod_v2 -> slug_loading_v2 -> slug_core_v2`.
No Cargo edge changes.

## Compatibility and proof contract

Legacy workspace resolution and Bzlmod Host-file keys, outputs, errors, Need
order, DICE equality, and all public behavior remain unchanged. Exact retained
observation values are the already admitted Host semantics. The sibling
carrier, aggregation, provenance, and future batch validation are Slug-native;
no new Bazel parity or public overlap is claimed.

A future implementation proof must cover shared-Arc epoch ordering, operation
mismatch, semantic duplicate coalescing, conflicting duplicate failure, and
empty/singleton/union; direct present/missing/wrong-kind/read-error;
relative/absolute symlink and retarget; resolution missing/error/cycle/expansion
terminal epochs; final FileBytes append exactly once; Need at resolution/final
read with no partial carrier; success/error A-B-A equality; absence of legacy
key activation; cancellation/drop; compact allocation/clone accounting; and
unchanged legacy/public results.

## Allowlist and caps

Edit exactly:

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`;
- `thoughts/shared/plans/slug-v2-subplans/current-packet.md`; and
- `thoughts/shared/plans/slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`.

Caps are 40 canonical, 260 current-packet, 220 Stage 2, and 520 total net
ledger lines. Read only `docs/developers/dice.md`; workspace
`src/{lib,path_observation,path_resolution}.rs`; Bzlmod
`src/{lib,host_file}.rs`; relevant Cargo manifests; the matching Stage 9
retained-utility row; the repo `slug-buck2-utility-reuse` skill; retained
`starlark_map/src/sorted_map.rs`, `gazebo/dupe/src/lib.rs`, and
`allocative/allocative/src/lib.rs`; and directly referenced focused tests.

No Rust, Cargo/BUILD, oracle, generated evidence, or other ledger write is
authorized. Independent review must confirm single-machine resolution,
complete success/error epochs, no legacy activation, equality/duplicate
algebra, one-way visibility, memory, compatibility, exact future allowlist and
caps, and no hidden third key.

STOP on code, a third key/store/graph, legacy key/value or caller migration,
loading/core/public caller, request revision/finalization, repository/module/
BUILD/`.bzl`/glob activation, generic public framework, reverse dependency,
direct or historical Host reads outside the existing observation owner,
watcher, oracle, JVM, or cap excess.

`REPLAN` if the resolution machine cannot expose every exact completed
observation without duplicating I/O, complete errors cannot retain their
prefix epoch, shared epochs require a new retained container, the doc-hidden
workspace carrier widens user API, the observed Host-file key must activate a
legacy caller, or the four-file implementation cannot remain bounded.

## Acceptance and immediate successor

Accept only after independent DICE/ownership/memory review freezes the two-key
identity, carrier, epoch algebra, exact four-Rust-file allowlist, caps, tests,
and STOP/`REPLAN` boundaries. Then activate only the callerless lower
vertical implementation. Its completion must schedule hierarchical package/
module/ignore frontier design; it must not claim public final validation.
