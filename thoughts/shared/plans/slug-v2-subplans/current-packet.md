# Current Slug V2 Packet

Packet: `WP-2A-m1-external-package-source-load-observation-design`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Scheduling base: `7bc9e1da`
Rust base: `e4555dca`
Result: freeze the smallest complete observed external-package BUILD and
`.bzl` source/load frontier required before loading-query publication; do not
implement it.

## Authority and caps

Write only:

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`;
- this manifest;
- `thoughts/shared/plans/slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`; and
- `.codex/skills/slug-agent-orchestration/references/routing-log.md`.

Cap net growth at 40 canonical, 220 manifest, 180 Stage 2, 30 routing and 470
aggregate lines against `7bc9e1da`.

## REPLAN evidence and natural-owner audit

The loading-query design cannot freeze `RootQueryCommandKey` yet.
`ExternalUnconfiguredPackageGraphKey` reaches
`RepositoryPackageLoadKey -> RepositoryPackageSourceKey`. That source reaches
`ExternalRepositoryPackageLookupKey`, `HostRouteRepositoryIgnoreKey`,
`HostRepositoryPathKey`, `HostRepositorySourceFileKey` and direct-local
module support. External BUILD loads additionally reach
`ExternalBzlModuleEvalKey -> HostRepositorySourceFileKey`.

These values retain routes, resolved paths, bytes, semantic package/module
values and event batches, but no complete `PathObservationEpoch`.
`HostRepositoryPathKey` computes legacy `ResolvedPathKey`; selected source
bytes compute direct `PathObservationKey`. A query terminal cannot reconstruct
their exact shared Result Arcs, and computing an observed family beside the
legacy keys would duplicate source and event authority.

Audit the complete live chain in:

- `slug_bzlmod_v2::source_preparation` for materialization, routed path
  resolution, selected source bytes and direct-local module preparation;
- `slug_bzlmod_v2::{repo_file,repository_ignore,host_package}` for routed
  REPO/ignore, BUILD-marker lookup and selected BUILD source;
- `slug_loading_v2::bzl_module` for external BUILD evaluation, recursive
  external `.bzl` loading and package completion; and
- their crate-root exports and existing focused test owners.

Choose the first parent or cohesive sibling family that can expose one complete
external package semantic Result Arc plus every exact selected path Result Arc
without computing a second legacy subtree. If BUILD and recursive `.bzl`
paths require more than one semantic owner, freeze their ordered carrier
composition explicitly. Select a uniquely smaller prerequisite if the source
substrate itself must land before package loading; otherwise `REPLAN`.
Do not broaden into root-package, query evaluation or public activation.

## Contract to freeze

Use private structural observation-key siblings and shared mode-aware drivers.
Legacy callers compute only legacy children and preserve current values/events;
observed callers compute only observed children. The final observed external
package-load value must retain exactly its semantic
`Arc<Result<LoadedPackage, RepositoryPackageLoadError>>` plus one Arc-backed
`PathObservationEpoch`. Intermediate carriers remain dependency-owned once
their epochs have been merged into the parent.

Freeze deterministic left-first epoch order for materialization/resolution,
REPO and ignore inputs, BUILD marker probes, selected BUILD bytes, external
load resolution and recursive `.bzl` bytes. Duplicate equal demands preserve
the first exact Arc. Need publishes no carrier; typed
`ObservedPathFrontierError` remains outer; semantic source/load errors remain
Complete with the decisive prefix epoch. Specify ordered/joined outer, Need and
semantic precedence at every batch rather than relying on `?` short-circuit.

The existing REPO, BUILD and `.bzl` evaluation children remain the sole event
owners. Observed parents store no duplicate batch. Cancellation or typed outer
publishes nothing; semantic completion preserves existing successful-child
batch order; warm replay is suppressed. Retained values add no materialization
request graph, closure vector, include queue, map, store, cache, lock, task or
direct Host read.

## Compatibility, proof and future boundary

External package values, target kinds, apparent/canonical labels, BUILD
basename priority, REPO/ignore semantics, source bytes, errors, exit codes and
child event text/order remain exact. Observation siblings, typed outer and
carrier association are Slug-native. Root query publication, multi-build,
one-shot evaluation, broader repository kinds and exact identity bytes remain
deferred.

Require discriminating proof for direct local package success, missing and
wrong-kind BUILD files, REPO and ignore edits, unsupported module/load cycles,
recursive external `.bzl` success/error, compatible and incompatible Need,
typed outer versus Need/semantic, exact demand/value/`Arc::ptr_eq` membership,
first-Arc duplicates, cold child order, warm suppression, pending cancellation
and recovery, edit/delete/recreate/A-B-A, complete-only equality/validity,
reverse family nonactivation and compact post-return retention. Prove the later
query consumer can receive a complete carrier while it remains unactivated.

Freeze the exact future Rust/test allowlist, per-file production/test/physical
caps, any necessary large-file split, validation commands and exactly one
bounded implementation successor only after the audit proves the cohesive
owner. Use current physical sizes and semantic sections as the cap bases.

## STOP / REPLAN

STOP on Rust, Cargo, BUILD, fixture, oracle or generated-file writes; public
query activation; root-package or one-shot changes; computing legacy and
observed source/load families together; duplicate event ownership; epoch or
Result-Arc reconstruction; partial carrier on Need/outer; retained scratch or a
new store/cache/lock/task/Host read; missing future allowlist/caps; multiple
successors; docs cap excess; or M1 closure. `REPLAN` if no bounded cohesive
source/load frontier exists, BUILD and external `.bzl` carriers cannot compose
without duplicated evaluation, another crate is required beyond the frozen
future boundary, or exact child Arcs cannot survive projection.

## Immediate predecessor

`7bc9e1da` schedules and audits loading-query observed publication after
accepted route implementation `e4555dca`. The audit proves the query owner is
still incomplete and authorizes this docs-only prerequisite design.
