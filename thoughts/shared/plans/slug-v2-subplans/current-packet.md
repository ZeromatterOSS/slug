# Current Slug V2 Packet

Packet: `WP-2A-m1-cquery-observed-publication-design`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Scheduling base: `0e919d2b`
Rust base: `941db0d0`
Result: freeze the smallest complete observed-family cutover inside the existing
public `CqueryCommandRoot`, without changing Rust or widening cquery syntax.

## Authority and caps

Write only:

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`;
- this manifest;
- `thoughts/shared/plans/slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`; and
- `.codex/skills/slug-agent-orchestration/references/routing-log.md`.

Documentation caps against `0e919d2b` are 40 canonical, 180 Stage 2, 180
manifest, 30 routing-log and 430 aggregate net lines.

## Design boundary

Audit and freeze the existing core-owned `CqueryCommandRoot` path from public
constructor through:

1. ordered root preparation;
2. root configured analysis;
3. `deps()` recursive configured-analysis closure;
4. `rdeps()` seed-package validation;
5. query evaluation and error projection; and
6. generic native-demand selection, retry, event replay and acceptance.

Use the accepted `prepare_configured_node_analysis_observed`,
`ConfiguredNodeAnalysisObservationKey` and
`RootPackageLoadObservationKey` at every matching edge. Freeze one private
mode-aware or observed-only driver owned by `CqueryCommandRoot`; do not add a
second public command, a side store, or a DICE root that merely wraps the same
work. The design must decide the exact outer/Need/semantic precedence for
ordered roots and joined deps batches, deterministic first-error order,
cancellation, local child event ownership, selected-epoch lifetime and warm
replay.

The cquery terminal retains only its existing semantic Result Arc and existing
semantic target/analysis collections. Observed child epochs stay
dependency-owned; the accepted native snapshot retains selected exact Result
Arcs. Cquery has no source certificate or request-revision finalizer. No new
carrier, retained collection/cache/interner/lock/task, direct Host read or
event owner is admitted.

## Audit result and exclusions

The predecessor audit selected cquery because its private root already owns all
preparation, analysis, deps/rdeps, evaluation and publication sequencing, and
the observed analysis/package seams cover every legacy edge. Multi-target build
requires a separate aggregate source-certificate/revision decision; loading
query crosses `slug_query_v2` graph/environment owners; and
`evaluate_workspace_targets` remains a separate one-shot migration adapter.
Those are not prerequisites for the bounded cquery cutover.

Existing public cquery results, output bytes, errors, exit classes and events
remain exact. The internal observed-family cutover, typed outer failure and
selected-epoch association are Slug-native. New expressions, external labels,
implicit/tool traversal, exact Bazel identity bytes, query/aquery, build
aggregation, repository/materializer breadth and the one-shot adapter remain
unsupported/deferred.

## Future implementation envelope

The design may authorize only:

- `app/slug_core_v2/src/runtime/dice.rs`; and
- `app/slug_core_v2/src/runtime/tests/cquery_command_tests.rs` (new).

Against `941db0d0`, semantic caps are 160 production, 300 test and 460
aggregate net lines. Physical caps are 12,435 for `dice.rs`, 1,200 for the
new test file and 13,635 combined. The design must freeze any line-identical
test relocation and may lower these caps; it may not raise or add a Rust file.

Require discriminating proof for zero legacy package/configured-analysis
activation across direct roots, multi-root sets, deps and rdeps; exact public
outputs/errors/events; cold child-before-result order and warm suppression;
outer > Need > semantic precedence with deterministic sibling ordering;
semantic error sidecars; cancellation/no publication and recovery;
default/explicit/edit/restored root settings; recursive/null/delegating/
platform/toolchain closure; stable selected Arcs through retry; no cquery
carrier/revision; and unchanged build/query/aquery/one-shot activation.

Validation must include focused cquery/native-demand tests, complete core,
analysis and loading suites, formatting, direct check, diff/archive gates,
Buck2 retention and AI cleanup scans, exact cap accounting and independent
implementation review.

## STOP / REPLAN

STOP on Rust, Cargo/BUILD, fixture/oracle/generated writes in this design;
another file or public API; cquery syntax/output/error/event drift; a legacy or
second package/configured-analysis family; duplicate root/driver/event owner;
partial carrier or invented source certificate/revision; retained state,
direct Host read or cap excess. `REPLAN` if observed outer/Need/semantic order
cannot be complete in the existing root, if query-crate changes are required,
or if the focused implementation cannot fit the two-file envelope. Acceptance
schedules exactly one bounded implementation and does not close M1.

## Immediate predecessor

`0e919d2b` records accepted neutral implementation `941db0d0` and activates
the post-neutral owner audit. The audit found cquery smaller than build
aggregation, loading query or the one-shot adapter because it can reuse the
accepted observed package/configured-analysis family without another owner.
