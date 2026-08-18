# Current Slug V2 Packet

Packet: `WP-2A-m1-root-package-all-build-frontier-implementation`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Accepted design: `5eb036c2`
Result: implement the private singleton root-package-all observed build sibling.

## Frozen scope

Implement private `BuildCommandRootObservationKey(BuildCommandRootKey)` only
for exactly one root-repository `TargetPattern::PackageAll`; its constructor
rejects every other identity and its `Display` is distinct from the legacy key.
`ObservedBuildCommandRoot` retains exactly one
`Arc<Result<BuildCommandEvaluation, BuildCommandError>>` plus one Arc-backed
`PathObservationEpoch`, derives `Allocative`, and uses complete-only equality/
validity. The key Value is exactly `SourcePreparationOutcome<Result<
ObservedBuildCommandRoot, ObservedPathFrontierError>>`.

Expose the accepted `RootPackageLoadObservationKey` and
`ObservedRootPackageLoad` only through the minimum doc-hidden loading API needed
by core. Add no public/native command caller. Cquery, analyzed/exported/
multi-target/external build paths and every repository/materializer path remain
unchanged.

One shared narrow Legacy/Observed singleton-package-all driver selects matching
root-module anchor and root-package families. Legacy enters the helper only for
the same singleton structural identity; every other legacy path stays on the
existing driver. Neither sibling computes the other.

## Order and terminals

Observed order is root-module anchor then root package. Union each completed
epoch before semantic inspection. Equal duplicate anchor observations retain
the first exact Arc.

Anchor semantic error retains the anchor epoch. Package semantic error retains
anchor plus its complete decisive package prefix. Need and typed child/union
outer error return no carrier or event, and cancellation publishes nothing.
Success returns the unchanged loaded-only evaluation with an empty action
closure and the full epoch.

Anchor and package events remain child-owned; the build sibling stores no event
data. Child values, loaded-only target construction and union scratch are
compute-local.

## Required proof

Add discriminating tests for semantic parity/output, exact anchor-then-package
Arcs and duplicate first-Arc, semantic/Need/outer/event polarity, strict family
and public-caller isolation, complete-only equality/validity, warm/edit/delete/
recreate/A-B-A, cancellation recovery, and unchanged empty/Starlark/exported/
multi/external/cquery legacy paths.

Reuse accepted lifecycle/event evidence and pinned Bazel package-pattern
behavior. No fixture or oracle is authorized.

## Authority and caps

Write only:

- `app/slug_loading_v2/src/bzl_module.rs`;
- `app/slug_loading_v2/src/lib.rs`; and
- `app/slug_core_v2/src/runtime/dice.rs`.

Against `daf5eef9`, formatted caps are 24 net/6,214 physical for Bzl module,
4/82 for loading lib, 260 production plus 420 test net/13,730 physical for core
dice, and 708 aggregate net Rust lines. No cap-only correction is authorized.
Completion-only scheduling may write canonical/current/Stage 2 under 180
aggregate net lines.

Retained state must remain one semantic Result Arc plus the existing Arc-backed
epoch, with `Allocative` and cheap-clone signaling. Add no standard retained
collection, interner, cache or hash surface. Require focused/full core and
loading validation, formatting, inherited Clippy/archive disposition, exact
scope/cap/artifact scans, `git diff --check`, Buck2-utility retention scan and
independent ownership/cleanup review.

## Compatibility boundary

Existing singleton root package-all package, output and child-event behavior
remains exact. Carrier association, stable first-Arc union and typed outer
errors are Slug-native. Every other build/cquery frontier, caller activation,
repository materialization, native-Windows raw-byte ordering and exact Bazel
identity bytes remain unsupported/deferred.

## STOP / REPLAN

STOP on any other file; Cargo, fixture or oracle writes; caller/public
activation; analyzed/exported/multi/external/cquery composition; partial
frontiers; direct/reconstructed Host reads; changed legacy output, events,
dependency/error order or semantics; duplicated full build driver; another
retained collection/cache/graph/store/lock/task; missing `Allocative`; or cap
excess.

`REPLAN` if singleton package-all has an unobserved input, the narrow shared
driver changes other legacy branches, the loading seam cannot remain sealed,
child event ownership changes, or proof needs another file/key/seam/oracle.

## Immediate successor

On acceptance return only to docs-only next-owner design using `5eb036c2` plus
the implementation commit. Do not combine caller/public cutover or another
build/cquery/repository frontier.
