# Current Slug V2 Packet

Packet: `WP-2A-m1-host-glob-frontier-design`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Accepted predecessors: `b9fda97d`, `f5a9b249`, `bd4fb8db`, `dc6f6e02`,
`c271b07c`, `2bccb48e`
Result: audit and freeze the smallest complete loading-side owner above the
accepted observed Host-glob traversal.

## Design task

Audit the live `RootPackageLoadKey` chain in exact dependency order:
root-module anchor, selected BUILD source, direct recursive Host `.bzl` loads,
synchronous package attempts, adapter projection and first-seen Host-glob
requests. Account for attempt replay, prepared-request insertion order, event
capture, semantic errors, Need, outer frontier errors and cancellation.

Determine whether one private observed `RootPackageLoadKey` sibling can
consume the already accepted observed anchor, source, Host-`.bzl` and
traversal families without losing an epoch. If the adapter projection is a
uniquely smaller natural prerequisite, freeze only that prerequisite; do not
publish a partial package certificate or reconstruct observations above it.

The candidate retained carrier is one semantic package Result Arc plus one
Arc-backed `PathObservationEpoch`. Prepared glob maps, AST/evaluator/module
state, event batches, child carriers, union scratch and replay control remain
compute-local. Existing completed-event ownership must remain exact; Need,
outer errors and cancellation publish no carrier or event data.

## Required audit and proof plan

- map every anchor/source/direct-load/glob dependency to its accepted observed
  sibling and prove no semantic input remains legacy;
- freeze deterministic anchor -> source -> direct-load source order ->
  first-seen glob-request order, with stable first-Arc epoch union;
- preserve synchronous replay, include/exclude and allow-empty behavior,
  semantic error/Need precedence, final package value and event ordering;
- define prefix-bounded semantic/outer/Need algebra for every child family;
- preserve strict Legacy/Observed family isolation and keep public callers
  legacy until a later activation packet;
- classify DICE-retained, evaluator/attempt scratch and event lifetimes,
  equality cutoff, invalidation, cancellation and warm/A-B-A behavior; and
- cite pinned Bazel 9.2 `PackageFunction`/`UnixGlob` tests or existing
  accepted evidence for every exact claim.

The future proof must discriminate no-glob, one/multiple/repeated include and
exclude requests, recursive `.bzl` plus glob aggregation, package/glob errors,
Need and outer errors at each rank, exact Arc/order, replay-event nonduplication,
complete-only equality/validity, warm/A-B-A, cancellation and family/caller
activation.

Read only the bounded loading package/Host-glob owners and tests plus the
already accepted observed anchor, source, Host-`.bzl` and traversal owners.
Do not run or add an oracle unless the audit finds a specific evidence gap.

## Authority and validation

This packet is docs-only. Write only:

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`;
- this manifest; and
- `thoughts/shared/plans/slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`.

Caps are 40 canonical, 320 manifest, 280 Stage 2 and 640 aggregate net lines.
Require source/reference verification, plan consistency, exact ledger
accounting, independent design review and `git diff --check`.

## Compatibility boundary

Existing admitted package evaluation, event, glob request/replay, matching,
ordering and diagnostic behavior remain exact. Carrier association, epoch
aggregation and exact-Arc identity are Slug-native. Public/core cutover,
repository package globbing/materialization, native-Windows raw-byte ordering
and exact Bazel identity bytes remain unsupported/deferred.

## STOP / REPLAN

STOP on Rust, Cargo, fixture or oracle writes; public/core/repository/
materializer activation; a partial certificate; reconstructed or historical
Host reads; duplicated evaluation; changed request/event/error order; retained
AST/evaluator/prepared/request collections; another cache/graph/store/lock; or
docs cap excess.

`REPLAN` to exactly one smaller docs-only natural-owner prerequisite if the
adapter erases an epoch that cannot be passed through locally, a child frontier
is incomplete, family isolation requires duplicated package drivers, or proof
needs another code owner/seam/oracle.

## Immediate successor

On acceptance schedule exactly one bounded private loading-frontier
implementation, or exactly one uniquely smaller docs-only prerequisite. Do not
combine public/core cutover, repository globbing or materialization.
