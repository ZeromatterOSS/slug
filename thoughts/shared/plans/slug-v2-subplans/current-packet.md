# Current Slug V2 Packet

Packet: `WP-6-7A-repository-source-file-observation-implementation-retry`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owner: `06-analysis-toolchains-and-actions.md`
Rust base: `ae8aa35e`
Accepted semantic design: `9040e168`
Accepted proof-cap correction: `edc533ff`

## Exact Rust authority and caps

Write only:

1. `app/slug_bzlmod_v2/src/source_preparation.rs`, from 14,940 physical lines:
   <=300 production, <=30 colocated proof and <=15,320 physical.
2. `app/slug_bzlmod_v2/src/source_preparation_observation_tests.rs`, from
   2,470 physical lines: <=700 test semantic lines and <=3,250 physical.

Aggregate semantic growth is <=1,030 and combined physical size is <=18,570.
`source_preparation.rs` is a cohesive large-owner exception. Every new or
touched helper stays below 200 lines. Every other file is read-only.

## Frozen production authority

Keep crate-private structural
`RepositorySourceFileObservationKey(RepositorySourceFileKey)` and
`ObservedRepositorySourceFileValue`. Preserve the preexisting
`ObservedRepositorySourceFile` enum as compute-local resolved projection.
The carrier contains exactly one local
`Arc<Result<RepositorySourceFileValue, RepositorySourceFileError>>` plus one
compact cumulative `PathObservationEpoch`; key/carrier are `Allocative` and
the carrier is `Dupe`. Expose only crate-private construction and borrowed
result/epoch accessors. Add no caller or `lib.rs` export.

Use one Legacy/Observed driver. Validate the relative path first. Legacy
selects only `RepositoryMaterializationKey` then `ResolvedPathKey`; observed
selects only `RepositoryMaterializationObservationKey` then
`ResolvedPathObservationKey`. Both select the same neutral
`PathObservationKey` for FileBytes. Neither sibling computes the other.
Project the exact legacy unwrapped value/error shape and preserve the nested
bytes Arc; only the observed sibling retains the local Result Arc.

## Order and terminal algebra

The exact order is relative-path validation -> materialization -> resolution ->
FileBytes when readable.

Invalid relative path and materialization DICE failure are semantic Complete
with empty epoch. Materialization Need/typed outer is immediate, carrierless
and suppresses resolution. Accept its Complete epoch before semantic
inspection; materialization semantic error retains that prefix.

After success, preserve the existing Host or Materialization namespace and
requested path. Invalid materialized path and resolution DICE failure retain
the materialization prefix. Resolution Need/typed outer is immediate and
carrierless. Merge the materialization prefix left-first with the complete
resolution epoch before semantics. Resolution semantic error, Absent and
WrongKind retain that merged prefix and suppress FileBytes.

FileBytes Need is carrierless. FileBytes DICE failure retains the prior prefix.
On Complete, append the exact shared demand/result before semantics. Present,
inconsistent Missing and observation Error retain the full epoch. Equal
duplicates preserve the earliest exact Arc; conflict or operation mismatch is
the existing typed `ObservedPathFrontierError`. This sequential owner has no
Need union.

Need is invalid/self-unequal. Complete typed outer is valid/equal by outer
value. Complete carrier is valid/equal by semantic Result plus epoch.

## Events, families and retention

Source parent, resolution and FileBytes owners remain eventless for every
terminal and cancellation. Accepted materialization/request/root descendants
remain sole owners of their matching local batches; warm reuse is silent.

Legacy direct dependencies are only legacy materialization/resolution plus
neutral FileBytes. Observed direct dependencies are only observed
materialization/resolution plus neutral FileBytes. Activate no preflight,
REPO-file, ignore, preparation, closure, discovery, selected graph, registry or
public caller.

Retain only the local semantic Result Arc, which may own the existing bytes
Arc, plus the epoch. Child carriers, requested/resolved paths, namespace,
source and union scratch remain dependency-owned or compute-local. Add no
other carrier Arc, collection, cache, store, interner, lock, task, direct Host
read, revision, certificate or event state.

## Corrected required proof

Retain the existing invalid-relative-path, source terminal, exact bytes/legacy
projection and upper-nonactivation proof. Add or restructure only proof, plus
line-neutral pure production-called projectors needed to discriminate existing
live compute-error branches.

Discriminate:

- distinct identity/hash/Display/accessors and Need/outer/carrier
  validity/equality;
- materialization/resolution Need, typed outer, compute and semantic terminals
  with exact empty/prior/merged prefixes and later suppression;
- invalid materialized path plus FileBytes compute/Need/Present/Missing/Error
  with exact prior/full prefixes and carrierlessness;
- exact epoch iteration order materialization -> resolution -> FileBytes,
  demand membership and every per-demand `Arc::ptr_eq`;
- production merge/append equal-first Arc, conflict and operation mismatch;
- exact observed and legacy direct dependency rows, including neutral
  `PathObservationKey` FileBytes and exclusion of the opposite family;
- phase-separated cold child-owned batches, parent/resolution/FileBytes
  silence, warm suppression and no batch on Need/outer/cancel;
- exact local and immutable namespace/value/error/bytes behavior;
- poll-drop followed by identical-request same-DICE recovery;
- local and immutable A -> B -> absent -> directory -> A with A==restored,
  held Result/bytes/epoch equality and restored per-demand Arc checks; and
- zero package-preflight/REPO/ignore/preparation/closure/discovery/
  selected-graph/registry/public activation.

Accepted lower-key proof supports but does not replace the new parent's branch,
prefix, dependency-row, event or lifecycle assertions. Run focused proof, full
bzlmod, affected loading/query/core baselines, fmt, diff-check, exact accounting
and AI-cleanup/Buck2 retention review. No fixture or Bazel oracle is authorized.

## Compatibility

Exact: current relative-path validation, materialization/source order, Host
versus Materialization namespace, symlink resolution, Absent/WrongKind/
FileBytes semantics, values/errors/nested bytes Arc and all legacy behavior.
Slug-native: the sibling, local Result Arc, compact epoch and typed outer.
Unsupported/deferred: observed preflight/REPO/ignore/preparation/closure,
registry source preparation/patches, discovery/selected graph, extensions,
generated repositories, rules_rust actions, M8/M7B and exact identity bytes.

## STOP and sole successor

STOP on a third file, caller/export, upper/registry activation, changed
production semantics/events/memory/families, direct Host read, retained state,
proof deletion, cap excess, M7A closure, M8/M7B/M9 or a second successor.
REPLAN rather than weaken exact Arc/order validation or hide proof through
overcompression.

After independent implementation ACCEPT schedule only
`WP-6-7A-selected-module-graph-observation-frontier-design`, docs-only.
