# Current Slug V2 Packet

Packet: `WP-2A-m1-routed-repository-policy-observation-implementation`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Scheduling base: `7f60a5c4`
Rust base: `e4ee0a8e`
Design authority: `7f60a5c4`
Result: implement only the accepted crate-private observed routed REPO/ignore
producer siblings; do not activate external package lookup, loading, or query.

## Authority and caps

Write exactly:

- `app/slug_bzlmod_v2/src/repo_file.rs`; and
- `app/slug_bzlmod_v2/src/repository_ignore.rs`.

Against Rust base `e4ee0a8e`, cap `repo_file.rs` at 120 production plus
170 test lines and 2,600 physical lines; cap `repository_ignore.rs` at 160
production plus 210 test lines and 3,200 physical lines. Aggregate semantic
growth is capped at 660 and combined physical size at 5,800. Current physical
bases are 2,281 and 2,783. Keep tests colocated; neither owner requires a split.

## Frozen implementation contract

Add structurally distinct crate-private
`HostRouteRepoFileObservationKey` and
`HostRouteRepositoryIgnoreObservationKey` newtypes around the corresponding
legacy identities. Add one compact observed carrier per key containing exactly
one semantic Result Arc of the legacy value type plus one Arc-backed
`PathObservationEpoch`. Carriers are `Allocative`/cheaply cloneable and
expose only borrowed crate-visible accessors. Do not export them from
`slug_bzlmod_v2::lib`.

Give each legacy/observed pair one private mode-aware driver. Legacy computes
only `HostRepositorySourceFileKey`; observed computes only
`HostRepositorySourceFileObservationKey`. Neither sibling computes the other
or constructs `ExternalRepositoryPackageLookupKey` or an upper loading key.

Values are
`SourcePreparationOutcome<Result<Carrier, ObservedPathFrontierError>>`.
Need returns immediately with no carrier. Typed source/parser/epoch outer
remains outer. Semantic policy/source/parse/evaluation/ignore errors remain in
the carrier Result and are valid/equal only when Complete.

The routed REPO driver preserves policy projection before routed
`REPO.bazel` source before evaluation. A policy projection failure completes
semantically with an empty epoch and does not activate source. Observed source
Need/outer propagates; every completed source epoch is retained before source
semantics are inspected. Missing source produces the existing empty REPO value.
Source and evaluation errors retain that decisive source prefix. Legacy value,
error text, equality and event output remain unchanged.

The routed ignore driver preserves routed REPO before routed
`.bazelignore` source before parser observations. Merge each completed epoch
before semantic inspection with stable left-first
`PathObservationEpoch::from_shared`. Equal duplicates retain the earlier
exact Arc; mismatch/conflict is typed outer. A semantic REPO terminal retains
only the REPO prefix. Missing/directory ignore source preserves existing empty
behavior. Parser-specific operations, including WindowsLongPath variants,
merge last. Parser Need/outer yields no parent carrier; parser semantic errors
retain the full reached prefix.

## Events, memory and compatibility

The legacy routed REPO key remains its family's sole local Complete batch
owner. The observed routed REPO sibling owns exactly one corresponding local
Complete batch and stores none on Need, typed outer, or cancellation. The
routed ignore siblings store no batch. Source/parser children keep existing
ownership. Preserve cold child-before-parent order, semantic-error batches,
cancellation discard, and warm suppression.

Retain no route graph, parser vector, prefix list, queue, store, cache, interner,
lock, task or direct Host read. Evaluation buffers, union inputs and parser
scratch are compute-local. Completed keys retain only the semantic Result Arc,
Arc-backed epoch, and the existing DICE-owned REPO event batch.

Routed REPO/ignore values, errors, ignored-prefix behavior, UTF-8 modes and
events remain exact. Structural observed identity, carrier association and
typed outer are Slug-native. External package lookup/source/load, recursive
external `.bzl`, loading query, multi-build, one-shot evaluation and exact
identity bytes remain deferred.

## Discriminating proof and validation

Add focused colocated proof for:

- structural observed identity, exact legacy semantic/event parity,
  Complete-only equality/validity, and both family-isolation directions;
- policy-before-source and REPO-source-before-ignore-source-before-parser
  activation, empty/decisive prefixes, exact demand/value/`Arc::ptr_eq`
  membership, equal-duplicate first Arc and union mismatch/conflict;
- source/parser Need, injected typed outer, semantic errors, cancellation with
  no batch, recovery and child-before-parent event order;
- missing/directory/regular/special/symlink REPO and ignore files, ignore
  prefix and parser path operations, UTF-8/evaluation errors, warm suppression,
  edit/delete/recreate and A/B/A; and
- zero upper lookup/package activation and compact post-return retention.

Run serially:

1. focused routed REPO/ignore tests, cancellation alone, then their
   default-parallel batch;
2. full `slug_bzlmod_v2`, `slug_loading_v2`, and `slug_query_v2` suites;
3. established `slug_core_v2` library/runtime checks, recording only unchanged
   inherited baselines;
4. `cargo fmt --all -- --check`, `cargo check -p slug_bzlmod_v2`,
   `git diff --check e4ee0a8e`, exact semantic/physical accounting, Buck2
   retention scan, and AI cleanup categories 1-9.

Require independent latest-diff implementation review. After ACCEPT, commit
this Rust packet and schedule exactly one docs-only external package
source/load frontier design. Do not activate query or close M1.

## STOP / REPLAN

STOP on any other file; Cargo, BUILD, fixture, oracle or generated-file writes;
public export; upper lookup/package/loading/query activation; a third key
family; computing both source families; reconstructed Result Arcs; semantic
inspection before epoch union; partial carrier on Need/outer; moved/duplicate
event authority; retained scratch or a new store/cache/interner/lock/task/Host
read; behavior drift; nondiscriminating proof; cap excess; multiple successors;
or M1 closure.

`REPLAN` if the existing routed REPO key cannot remain the event owner, parser
epochs cannot compose after both source epochs, another file/owner is required,
or the frozen caps cannot build.

## Immediate predecessor

`7f60a5c4` independently accepts the docs-only owner audit and frozen design
from Rust base `e4ee0a8e`. It selects the two route-local producer siblings as
the uniquely smaller prerequisite before package lookup because per-package
composition would duplicate route-wide policy work, move REPO event ownership,
and miss direct-local include re-entry.
