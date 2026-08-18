# Current Slug V2 Packet

Packet: `WP-2A-m1-host-glob-frontier-design`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Accepted predecessors: `b9fda97d`, `dc6f6e02`, `2bccb48e`
Result: freeze the complete private observed loading frontier at the natural
`RootPackageLoadKey` owner.

## Frozen design

Add private `RootPackageLoadObservationKey(RootPackageLoadKey)` with identical
structural fields and distinct Display. Its
`ObservedRootPackageLoad` retains exactly:

- one `Arc<Result<LoadedPackage, RootPackageLoadError>>`; and
- one Arc-backed `PathObservationEpoch`.

Its Value is `SourcePreparationOutcome<Result<ObservedRootPackageLoad,
ObservedPathFrontierError>>` with complete-only equality and validity.

One `Legacy | Observed` root-package driver preserves the existing serial
compute. Legacy selects only legacy anchor, source, Host-`.bzl` and traversal
families. Observed selects only their accepted observed siblings. Neither root
key computes the other, and all public/core callers remain legacy.

The Host-glob adapter is an ephemeral seam, not another DICE key or retained
owner. One shared adapter driver projects matching traversal families into the
existing semantic `HostGlobPrepared`; observed mode additionally passes the
exact traversal epoch upward. Make only the minimum traversal visibility/
borrowed accessors required by that seam.

## Exact order and terminals

Aggregate completed child epochs before inspecting semantic Results in this
existing serial order:

1. root-module loading anchor;
2. selected BUILD source;
3. direct Host-`.bzl` children in AST load order, each already carrying its
   recursive closure; and
4. first-demand Host-glob requests in evaluator replay order.

Reuse/generalize the existing stable Host-`.bzl` epoch-union helper so equal
duplicates retain the first exact Arc. A union conflict or typed child outer
error returns completed outer error without a carrier or parent event. A
semantic error returns the unchanged root semantic Result plus exactly the
completed prefix through its rank. Need returns immediately without carrier or
event; later ranks are not evaluated. Success retains the full epoch.
Cancellation publishes nothing.

Preserve the existing Slug-native synchronous package-attempt replay mechanism
unchanged. Insert only semantic `HostGlobPrepared` into the compute-local
`SmallMap`, union that request's epoch before rerunning the unchanged evaluator
attempt, and retain no pending attempt batch. Include/exclude, repeated request
reuse, allow-empty,
diagnostics and final package semantics remain unchanged.

## Event, memory and ownership

The shared root-package driver remains the sole parent package event authority.
Each sibling stores the same terminal local batch (or empty batch) only for
semantic Complete. Recursive Host-`.bzl` event batches remain child-owned.
Need, outer error and cancellation store no event data.

Prepared maps, AST/evaluator/module state, loaded-module vectors, attempt
control, child carriers, event batches, union scratch and epochs under
construction are compute-local. No evaluator borrow crosses an await. No new
cache, graph, store, lock, task, direct/historical Host read or retained
standard collection is authorized.

## Required proof

Add discriminating tests for:

- adapter semantic success/error, Need, outer error, exact path/epoch Arcs and
  strict traversal-family isolation;
- no-glob and complete anchor/source/direct-recursive-`.bzl`/multiple-glob
  package carriers with exact injected Arcs;
- AST load order and dynamic include-then-exclude first-demand order;
- repeated request reuse without duplicate computation or epoch/event loss;
- union-before-semantic prefix, Need and outer carrier/event polarity;
- terminal-attempt-only event replay and child event separation;
- legacy/observed package parity and zero cross-family/caller activation;
- complete-only equality/validity, warm reuse, create/edit/delete/recreate,
  A/B/A and cancellation recovery.

Reuse pinned Bazel 9.2 `PackageFunction.java:1001-1252`,
`PackageFunctionTest` glob order/invalidation/boundary tests, `UnixGlob`, and
accepted `glob-callable-contract`, `glob-directory-invalidation`,
`glob-package-boundaries`, Host-`.bzl` lifecycle/event and root-package
lifecycle evidence. No new fixture or oracle is needed.

## Authority and caps

Future implementation writes only:

- `app/slug_loading_v2/src/host_glob/traversal.rs`;
- `app/slug_loading_v2/src/host_glob/adapter.rs`;
- `app/slug_loading_v2/src/host_glob/adapter_tests.rs`;
- `app/slug_loading_v2/src/bzl_module.rs`; and
- `app/slug_loading_v2/src/host_package_load_tests.rs`.

Against `2bccb48e`, formatted caps are:

- traversal: 12 net lines, 790 physical;
- adapter: 170 net lines, 336 physical;
- adapter tests: 230 net lines, 634 physical;
- `bzl_module.rs`: 450 net lines, 6,365 physical;
- host-package tests: 650 net lines, 2,609 physical; and
- aggregate: 632 production, 880 tests and 1,512 total net Rust lines.

No cap-only correction is authorized. `bzl_module.rs` remains the cohesive
owner because attempt replay, event boundary, errors and RootPackageLoad live
together; splitting would widen private seams. Require independent
ownership/cohesion/retention and nine-category cleanup review.

## Validation and compatibility

Run focused adapter/root-package tests, full `slug_loading_v2`, direct
`slug_core_v2`, formatting, inherited Clippy/archive dispositions, exact
cap/scope/artifact/event/family scans and `git diff --check`.

Observable package evaluation, glob results, event ordering, matching and
diagnostics remain exact. Synchronous attempt control/replay, carrier
association, deterministic epoch union, first-Arc identity and typed outer
errors are Slug-native. Public/core
activation, repository package globbing/materialization, native-Windows
raw-byte ordering and Bazel identity bytes remain unsupported/deferred.

## STOP / REPLAN

STOP on any other file; `package.rs`, crate-root, Cargo, Bzlmod, core, caller,
repository-package, fixture or oracle changes; public activation; a partial
certificate; duplicated evaluator/package driver; changed request/event/error
order; retained AST/evaluator/prepared/request collections; direct Host reads;
another cache/graph/store/lock/task; or cap excess.

`REPLAN` if any observed child frontier is incomplete, the shared driver
changes legacy behavior, exact event ownership cannot be preserved, the
adapter needs another DICE key, proof needs another file/seam/oracle, or
`bzl_module.rs` cannot remain cohesive within cap.

## Immediate successor

On independent acceptance schedule only
`WP-2A-m1-host-glob-frontier-implementation` from the design commit. Do not
combine public/core cutover, repository package globbing or materialization.
