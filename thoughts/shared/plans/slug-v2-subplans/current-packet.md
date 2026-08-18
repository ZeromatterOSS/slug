# Current Slug V2 Packet

Packet: `WP-2A-m1-external-repository-package-lookup-observation-design`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Scheduling base: `d451587f`
Rust base: `2a8dd968`
Result: freeze the uniquely smaller observed external package-marker lookup
producer before package source/load observation resumes.

## Authority

Write only:

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`;
- this manifest;
- `thoughts/shared/plans/slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`; and
- `.codex/skills/slug-agent-orchestration/references/routing-log.md`.

Do not write Rust, Cargo metadata, BUILD files, fixtures, oracles, generated
files, or callers. Docs caps are 40 net canonical, 220 Stage 2, 200 manifest,
30 routing, and 490 aggregate.

## Natural owner and future scope

`ExternalRepositoryPackageLookupKey` in `slug_bzlmod_v2::host_package` is the
first complete reusable owner after accepted route policy `2a8dd968`. It alone
owns canonical deletion policy, route ignore, and ordered `BUILD.bazel` then
`BUILD` marker selection. Both `RepositoryPackageSourceKey` and
`DirectLocalIncludePackageHorizonKey` consume this identity; inlining observed
selection above it would duplicate one subtree or leave the include horizon on
legacy inputs.

The future implementation writes exactly:

- `app/slug_bzlmod_v2/src/host_package.rs`; and
- new `app/slug_bzlmod_v2/src/host_package_observation_tests.rs`.

Keep production in the cohesive 4,567-line owner. Add only test glue:
`#[cfg(test)] mod observation_tests { use super::*;
include!("host_package_observation_tests.rs"); }`; move no existing body.
Against Rust base `2a8dd968`, cap `host_package.rs` at 240 production plus 8
test-glue lines and 4,850 physical lines; cap the new file at 360 tests and 380
physical lines. Aggregate semantic growth is capped at 608 and combined
physical size at 5,230.

## Frozen key and carrier

Add crate-private `ExternalRepositoryPackageLookupObservationKey` with
structurally distinct DICE identity/Display and the same validated
route/package constructor boundary. Add one crate-private observed carrier
with exactly the legacy semantic `Arc<Result<ExternalRepositoryPackageLookup,
ExternalRepositoryPackageLookupError>>` plus one Arc-backed
`PathObservationEpoch`, `Allocative`/cheap clone, and borrowed crate-visible
accessors for the later source/include consumers. Export nothing from `lib.rs`.

One private mode-aware driver must serve the legacy and observed keys. Legacy
selects only `HostRouteRepositoryIgnoreKey` and `HostRepositoryPathKey`;
observed selects only `HostRouteRepositoryIgnoreObservationKey` and
`HostRepositoryPathObservationKey`. Neither sibling computes the other,
package source/load, direct-local support, external `.bzl`, or query.

The driver returns Need immediately with no carrier. A typed observed child or
epoch failure remains typed outer with no carrier. Semantic policy, ignore,
and path errors remain inside Complete carriers. Only Complete is valid/equal.
Allocate exactly one semantic Result Arc for the parent terminal; never rebuild
child Result Arcs retained by its epoch.

## Frozen order and prefixes

Preserve invalid-package validation, canonical deleted-package projection,
route ignore, `BUILD.bazel`, then `BUILD` order. The prefix contract is:

1. invalid package, policy error, or canonical deleted-package membership has
   an empty epoch;
2. route-ignore semantic error or ignore match returning `Deleted` retains the
   complete observed route-ignore epoch;
3. union the completed route-ignore epoch before inspecting it;
4. union each completed marker-path epoch left-first before inspecting it, in
   `BUILD.bazel` then `BUILD` order;
5. a selected regular/special marker or decisive path error retains the prefix
   through that marker; and
6. wrong-kind/missing markers continue, while `NoBuildFile` retains the full
   ignore plus both-marker epoch.

Use stable `PathObservationEpoch::from_shared` unions. Equal duplicates retain
the earlier exact Arc; mismatch, operation mismatch, or conflict is typed
outer. No prefix vector, marker list, or union scratch survives compute.

## Events, compatibility, and proof

The lookup siblings own no event batch. Observed routed REPO remains the sole
local REPO batch owner; ignore/path children remain otherwise eventless.
Completed child events remain dependency-owned even if a later marker returns
Need/outer, while transaction cancellation/abort publishes no attempt. Preserve
cold child order, semantic-error events, recovery, and warm suppression.

External lookup values/errors, marker priority/kinds, deleted behavior and
events stay exact. Structural identity, carrier association, typed outer, and
exact-Arc epoch retention are Slug-native. Package source/load, direct-local
observed horizon, recursive external `.bzl`, query publication, multi-build,
one-shot, and identity bytes remain deferred.

Future proof must discriminate both `Deleted` origins; invalid/policy error;
ignore match/error; `BUILD.bazel` priority; regular/special/missing/wrong-kind
markers; path errors and `NoBuildFile`; exact empty/ignore/first-marker/full
demand/value/`Arc::ptr_eq` prefixes; first-Arc duplicates; Need and typed outer
at ignore and each marker; Complete-only validity; both family-isolation
directions; zero upper source/load/query activation; child-only event order;
real polled cancellation/recovery; warm reuse; create/edit/delete/recreate and
A/B/A priority changes; and compact retention.

Run focused tests individually and in their default-parallel batch, full
`slug_bzlmod_v2`, `slug_loading_v2`, and `slug_query_v2`, established
`slug_core_v2` library/runtime baselines, fmt/check/diff/accounting, Buck2
retention scan, AI cleanup categories 1-9, and independent latest-diff review.
Reuse accepted pinned source/oracles; add none.

## STOP / REPLAN

STOP on any other file; public export/caller activation; source/load/include/
query cutover; a third key family; mixed child families; semantic inspection
before union; partial/rebuilt epochs or Result Arcs; moved/new event owner;
retained collection/store/cache/interner/lock/task/direct Host read; existing
test movement; cap excess; multiple successors; or M1 closure.

`REPLAN` if the lookup owner cannot expose the complete route-ignore and both
marker epochs without another owner/file, if legacy parity requires duplicate
drivers, or if event publication cannot remain child-owned. After independent
design `ACCEPT`, schedule exactly one bounded implementation from `2a8dd968`;
after implementation `ACCEPT`, return directly to external package source/load
design. Do not activate query or close M1.
