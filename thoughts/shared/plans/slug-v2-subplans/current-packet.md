# Current Slug V2 Packet

Packet: `WP-2A-m1-host-glob-frontier-design`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Accepted predecessors: `5816e435`, `daf5eef9`
Result: freeze the singleton root-package-all build frontier as the next
complete retained consumer above observed root-package loading.

## Selected natural owner

The live graph has two direct `RootPackageLoadKey` consumers. Cquery uses one
rdeps seed package transiently for target-existence validation and retains no
package result. `BuildCommandRootKey` retains each loaded package inside
`BuildRequestedTarget` and its command Result, so it is the uniquely smallest
next retained semantic owner.

The bounded complete identity is exactly one root-repository
`TargetPattern::PackageAll`. It depends only on the root-module anchor and root
package and finishes as `BuildTargetCompletion::LoadedOnly` with an empty action
closure. Empty roots are only the accepted anchor frontier. Starlark rules,
exported files, multiple targets, external targets and cquery add distinct
analysis/source/aggregation/repository/transient frontiers and stay deferred.

## Frozen carrier and driver

Add private `BuildCommandRootObservationKey(BuildCommandRootKey)`. Its
constructor admits only the singleton root package-all structural identity and
has a distinct Display. `ObservedBuildCommandRoot` retains exactly:

- one `Arc<Result<BuildCommandEvaluation, BuildCommandError>>`; and
- one Arc-backed `PathObservationEpoch`.

Its Value is `SourcePreparationOutcome<Result<ObservedBuildCommandRoot,
ObservedPathFrontierError>>` with complete-only equality/validity. Derive
`Allocative` and use `Dupe` only for cheap Arc-backed values. Do not add a
retained map, vector, string, interner, cache or hash wrapper. This reuses the
existing Buck2-derived DICE, immutable Arc and memory-accounting patterns; no
Stage 9 ledger import is needed.

Expose `RootPackageLoadObservationKey` and `ObservedRootPackageLoad` only as
doc-hidden sealed loading API required by core. One shared Legacy/Observed
singleton-package-all driver selects matching anchor and package families.
Legacy enters it only for that same structural identity; all other legacy
branches retain their current path. Neither sibling computes the other and all
public/native command callers remain legacy.

## Order, terminals and events

Compute and union the observed anchor before the observed root package. Union
each completed child epoch before inspecting its semantic Result. The package
child includes its complete anchor/BUILD/direct-`.bzl`/glob closure, so the
duplicate anchor keeps the first exact Arc.

An anchor semantic error retains the anchor epoch. A package semantic error
retains anchor plus the package's decisive prefix. Need and typed child/union
outer error return no carrier or event, and cancellation publishes nothing.
Success retains the full epoch and the unchanged loaded-only command result.

Anchor and package event batches remain child-owned. The build sibling stores
no event data. Child values, target projection, loaded-only branch state and
union construction remain compute-local.

## Required proof

Add discriminating proof for:

- legacy/observed singleton package-all semantic parity and unchanged output;
- exact anchor-then-package observation Arcs, including duplicate first-Arc;
- anchor/package semantic prefix, Need and typed outer no-carrier/event polarity;
- strict anchor/package/build family and public-caller isolation;
- complete-only equality/validity, warm reuse, edit/delete/recreate and A-B-A;
- cancellation publication suppression and recovery; and
- unchanged legacy behavior for empty, Starlark, exported, multi-target,
  external and cquery paths.

Reuse accepted root anchor/package lifecycle/event evidence and pinned Bazel
package-pattern behavior. No fixture or oracle is required.

## Authority and caps

This design packet is docs-only. Write only canonical, this manifest and Stage
2 under 40/320/280/640 net-line caps. Require exact ledger accounting,
cross-document consistency, independent natural-owner/retention review and
`git diff --check`.

The future implementation may write only:

- `app/slug_loading_v2/src/bzl_module.rs`;
- `app/slug_loading_v2/src/lib.rs`; and
- `app/slug_core_v2/src/runtime/dice.rs`.

Against `daf5eef9`, formatted caps are 24 net/6,214 physical for Bzl module,
4/82 for loading lib, 260 production plus 420 test net/13,730 physical for core
dice, and 708 aggregate net Rust lines. Completion-only scheduling may write
canonical/current/Stage 2 under 180 aggregate net lines. Require independent
ownership, retention and nine-category cleanup review.

## Compatibility boundary

Existing singleton root package-all package, output and child-event behavior
remains exact. Carrier association, stable first-Arc union and typed outer
errors are Slug-native. Analyzed/exported/multi-target/external/cquery frontier
composition, public/core activation, repository materialization,
native-Windows raw-byte ordering and exact Bazel identity bytes remain
unsupported/deferred.

## STOP / REPLAN

STOP on any other file; Cargo, fixture or oracle writes; command/public caller
activation; analyzed/exported/multi/external/cquery composition; a partial
frontier; reconstructed Host reads; changed legacy semantics, output, events or
dependency order; duplicated full build driver; another retained collection,
cache, graph, store, lock or task; missing `Allocative`; or cap excess.

`REPLAN` if singleton package-all has another unobserved semantic input, the
shared narrow driver changes other legacy branches, loading visibility cannot
remain sealed, child event ownership changes, or proof needs another file/key/
seam/oracle.

## Immediate successor

On independent acceptance schedule only the bounded private singleton
root-package-all build-frontier implementation. Do not combine any caller/public
cutover or another build/cquery/repository frontier.
