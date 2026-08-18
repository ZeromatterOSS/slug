# Current Slug V2 Packet

Packet: `WP-2A-m1-loading-query-observed-publication-design`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Scheduling base: `e4555dca`
Result: freeze the smallest complete observed-publication boundary for the
existing public loading-query command; do not implement it.

## Authority and caps

Write only:

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`;
- this manifest;
- `thoughts/shared/plans/slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`; and
- `.codex/skills/slug-agent-orchestration/references/routing-log.md`.

Cap net growth at 40 canonical, 220 manifest, 180 Stage 2, 30 routing and 470
aggregate lines against `e4555dca`.

## Natural-owner audit

Start from live `RootQueryCommandKey`, its sole `NativeCommandRoot`
implementation and public constructor. Trace every root-mode edge in
`slug_query_v2::{evaluator,loading_environment,graph}` before freezing the
design. The existing command key owns parsing, root anchor, dynamic query
evaluation, graph/output completion and one semantic Result Arc; generic
`drive_command` owns retry, selected-snapshot validation and publication.

Inventory at least:

- the root module anchor;
- root and external package graphs and package-load provenance;
- direct/transitive external repository routing;
- root recursive subtree discovery, package boundaries, directory listings and
  non-UTF-8 marker probes;
- BUILD companion boundary and marker resolution; and
- every load/build provenance edge reached by `buildfiles`,
  `loadfiles`, visibility, `siblings`, `deps`, `rdeps`, recursive
  patterns, generated files and label-kind completion.

The accepted observed anchor, root package load, package boundary, path
listing/resolution and repository route are candidates, not assumed proof of a
complete graph. In particular, prove whether external
`RepositoryPackageLoadKey` / `RepositoryPackageSourceKey` and their route
children already expose a complete exact-Arc epoch. If any selected path can
still reach a carrierless legacy Host observation, choose the uniquely smaller
producer prerequisite or `REPLAN`; do not hide it inside the command carrier.

Keep one-shot `evaluate_loading_query*` and non-root workspaces on their
existing legacy path. They are not native-demand publication callers.

## Contract to freeze if the owner is complete

Freeze one observed command terminal containing exactly one
`Arc<Result<QueryOutput, QueryError>>` plus one Arc-backed
`PathObservationEpoch`, with typed `ObservedPathFrontierError` outside the
semantic Result. Reuse the existing command identity unless the audit proves a
structurally distinct sibling is required. The native adapter must project the
exact semantic Arc and expose the complete epoch to generic selected-snapshot
validation before acceptance; query has no source certificate, request
revision or event owner.

Use one mode-aware query environment/graph driver wherever legacy one-shot and
observed native paths share semantics. Observed mode selects only observed
families and accumulates child epochs compute-locally in actual evaluator order.
Specify deterministic left-first duplicate-Arc selection and the precise
Need/typed-outer/semantic ordering for sequential stages and joined batches.
No partial carrier may accompany Need or typed outer. Semantic success or error
retains exactly the complete decisive epoch required by selection; later
dependency-owned child state must not make the carrier incomplete.

Observed package/module/path children remain the sole local event owners.
Cancellation or typed outer discards the attempt buffer; semantic completion
preserves the existing successful-child event behavior; warm acceptance replays
no child batch. The terminal and public accepted command retain no query
environment, graph scratch, candidate arena, traversal queue or new collection
beyond the existing semantic output and selected compact epoch.

## Compatibility, proof and future boundary

Public query labels, graph/order/label-kind output, errors, exit codes, policy,
external apparent/canonical routing and child event text/order remain exact.
Observed family identity, carrier association, typed outer and exact-Arc
selected validation are Slug-native. Multi-build certificate aggregation,
one-shot publication, wider query/aquery identities and exact Bazel identity
bytes remain deferred.

Require discriminating public proof for syntax/no-activation, direct and
recursive roots, external direct/transitive owners, load/build provenance,
visibility/package-group recursion, generated files and BUILD companions;
exact complete selected demand/value/`Arc::ptr_eq` equality; zero legacy
family activation; cold child order and warm suppression; compatible and
incompatible Need, typed outer versus Need/semantic, semantic error, pending
cancellation with no publication and recovery; edit/delete/recreate/A-B-A; and
legacy one-shot isolation. Include retention and AI-cleanup review.

Freeze the exact future Rust/test allowlist, semantic and physical caps, any
large-file split, validation commands and one bounded implementation successor
only after the live audit proves completeness. Prefer existing colocated query
and native-command owners; do not add a store, cache, lock, task, Host read or
second publication/event owner.

## STOP / REPLAN

STOP on Rust, Cargo, BUILD, fixture, oracle or generated-file writes; public
behavior changes; implementation; a second command/publication owner; source
certificate or revision work; unproved external-package observation
completeness; family/event drift; retained query scratch; missing future caps or
allowlist; multiple successors; docs cap excess; or M1 closure. `REPLAN` on
any carrierless legacy Host edge, incomplete selected epoch, error-order
contradiction, required file outside the frozen future allowlist, or no bounded
cohesive implementation.

## Immediate predecessor

`e4555dca` accepts the observed root-repository-route prerequisite from
frozen design `1ce16378`. It closes the route-family blocker selected by the
post-cquery audit and authorizes this design audit only.
