# Current Slug V2 Packet

Packet: `WP-2A-m1-root-module-anchor-frontier-loading-consumer-design`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Result: decide the smallest loading-side consumer of the accepted doc-hidden
root-module anchor frontier without attaching a partial certificate to a
broader package/load terminal or activating public/core publication.

## Accepted predecessor

Commit `c6e61d60` adds the callerless doc-hidden/public
`ObservedRootModuleLoadingAnchor` and
`RootModuleLoadingAnchorObservationKey`. The key computes only the private
observed root producer, forwards Need and outer errors, retains the exact
semantic-result and observation Arcs, owns no events, and leaves the live
public anchor and every loading/core caller unchanged.

The accepted implementation changes exactly `host_module.rs` and `lib.rs`:
97 production and 220 test lines, 317 total net, within all caps. Focused
observed-anchor proof passes 3/3, the unchanged public-anchor regressions pass
2/2, all 585 Bzlmod tests pass, and loading/core checks, formatting, diff and
artifact hygiene, plus independent ownership/cleanup review pass. Strict
Clippy and archive checks stop only on their recorded inherited baselines.

## Design questions

1. Map the exact loading chain from `RootPackageLoadKey`'s anchor-first
   dependency through root package source selection, source bytes, parse, load
   labels, recursive `.bzl` evaluation, glob/package listing, semantic errors,
   Need union, events, and final `LoadedPackage` publication.
2. Decide whether any bounded loading terminal can consume
   `RootModuleLoadingAnchorObservationKey` and retain its certificate without
   claiming completeness for mutable predecessors it does not own. Merely
   replacing the legacy anchor dependency and then dropping the epoch is not
   an accepted consumer.
3. Preserve anchor-first Need/error order. Keep
   `ObservedPathFrontierError` outer; do not launder it into
   `RootPackageLoadErrorInner::RootModule` or another public semantic error.
   Freeze the exact success/error/Need/cancellation carrier algebra before any
   implementation.
4. Prove the selected terminal's complete mutable predecessor frontier.
   Root-module observations, package-source selection negatives/bytes,
   recursive `.bzl` loads, and glob/directory inputs must either be included
   before sealing or shown to be outside that terminal. A partial certificate,
   ambient mutable dependency, or command-side reconstruction is forbidden.
5. Preserve event ownership: the observed root producer remains the sole root
   MODULE event owner, while the selected loading key may retain only its
   existing loading/evaluation batch. No path may store two equivalent root or
   package batches.
6. Preserve the one-way Bzlmod-to-loading dependency. Choose the natural loading
   producer/key/value, visibility, structural equality, request policy,
   invalidation, exact Arc reuse, and DICE lifetime. Retain no evaluator,
   transaction, event batch, source text, AST, horizon, glob walker, or
   materializer state as certificate authority.
7. Decide whether the uniquely smallest successor is one bounded loading-side
   implementation or one docs-only prerequisite such as package-source or
   recursive loading-frontier design. Do not combine independent consumers.
   Freeze its exact files, production/test/total caps, physical ceilings,
   proof, cleanup trigger, and successor.

Existing admitted serial anchor, package loading, error/Need/event order,
outputs, and exact Host observation values remain exact. Loading-side
certificate association, aggregation, and equality are Slug-native. Public
command overlap/final validation, package-source/BUILD/`.bzl`/glob frontiers
not proven complete here, routed/materialized repositories, and exact Bazel
identity bytes remain unsupported/deferred.

## Evidence, authority, and caps

Reuse `c6e61d60` and existing loading tests; no new Bazel oracle is needed
for this docs-only ownership decision. Use live source and DICE ownership as
authority, and record why any broader terminal is rejected as partial.

Write only:

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`;
- this manifest; and
- `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`.

Read only this packet and owner section, the plan-authoring guide,
`docs/developers/dice.md`, the Buck2 utility-reuse skill and matching Stage 9
Arc/`Dupe`/`Allocative` row, Bzlmod
`src/{host_module,host_package,lib}.rs`, loading
`src/{bzl_module,glob,keys,load_label,cycle_detector,lib}.rs`, loading
`src/host_glob/{mod,adapter,traversal}.rs`, workspace
`src/{lib,path_observation}.rs`, root `Cargo.toml`, app
`slug_{workspace,bzlmod,loading}_v2/Cargo.toml`, and directly referenced
focused tests.

Ledger caps are 40 canonical, 320 current, 280 Stage 2, and 640 total net
lines. No Rust, Cargo, test, oracle, or fixture write is authorized.

## STOP / REPLAN

STOP on every code/oracle write; changing the accepted anchor carrier or public
anchor; core/public caller edits; user API/wire/output/diagnostic behavior;
partial certificate publication; treating mutable package/source/load/glob
inputs as ambient; outer-error laundering; reverse dependency; direct/
reconstructed or historical Host reads; new graph/store/container/interner;
retained evaluator/event/source/AST/transaction/horizon/glob/materializer
state; routed/materialized repository, watcher, JVM, or combined consumers.

REPLAN if no bounded loading terminal has a complete frontier; consuming the
anchor would require a public error change or duplicate event owner; recursive
load/glob discovery cannot seal before completion; package-source or another
lower producer must first gain an observed carrier; a complete carrier needs a
new shared representation; or the first implementation cannot be bounded
independently.

## Immediate successor

On acceptance, activate only one bounded loading-side consumer implementation
or one uniquely required docs-only prerequisite. Do not combine package-source,
recursive `.bzl`/glob, core publication, public overlap, or another consumer.
