# Current Slug V2 Packet

Packet: `WP-2A-m1-cquery-observed-publication-implementation`
Milestone: M1 one semantic spine
Owner: `slug-v2-subplans/02-rust-skeleton-and-runtime-substrate.md`
Rust base: `941db0d0`
Frozen design: `895996d5`
Result: switch the sole public `CqueryCommandRoot` to accepted observed
package/configured-analysis families without adding a root, carrier or revision.

## Authority and caps

Write only:

- `app/slug_core_v2/src/runtime/dice.rs`; and
- `app/slug_core_v2/src/runtime/tests/cquery_command_tests.rs` (new).

Against `941db0d0`, exclude only the frozen byte-identical test relocation
from semantic growth. Caps are 160 production, 300 test and 460 aggregate net
semantic lines; physical caps are 12,435 for DICE, 1,200 for the test file and
13,635 combined.

## Required implementation

Before semantic edits, move exact Rust-base DICE lines 9,210-10,042 into the
new file, beginning at
`cquery_executables_deps_filters_complete_closure_and_induces_edges` and
ending before `accepted_native_snapshot`. Insert nested
`mod cquery_command_tests { include!(\"tests/cquery_command_tests.rs\"); }`
at the old location; the child begins `use super::*;`. Change no relocated
body or parent fixture/visibility.

Keep the existing `CqueryCommandRoot`, public constructor and generic native
publication. Switch observed-only:

- direct root preparation and configured analysis;
- every joined deps configured-analysis frontier; and
- rdeps seed-package validation.

Use only `prepare_configured_node_analysis_observed`,
`ConfiguredNodeAnalysisObservationKey` and
`RootPackageLoadObservationKey`. Query preflight/evaluation, root/literal
order, projection, terminal construction and public output stay unchanged.

Inspect every ordered direct root and complete joined deps batch. Precedence is
first typed outer > combined compatible Need > first semantic error > ordered
success. Direct roots use request order; deps uses `compute_join` input order.
Nonsemantic preparation suppresses only that root's analysis. Rdeps retains
outer > Need > semantic. Incompatible Need union stays typed native failure.

Typed outer and cancellation publish no attempt events. Completed semantic
errors preserve successful-sibling sidecars and existing public error/exit
projection. Observed package/analysis keys are the only local event owners;
warm success suppresses replay. Add no carrier or revision: child epochs remain
dependency-owned and the accepted selected snapshot owns exact Result Arcs.
Retain only the existing semantic Arc/targets/analyses/events and compute-local
scratch.

## Compatibility and proof

Public cquery results, bytes, errors, exit classes and events remain exact.
Observed-family association, typed outer failure and selected-epoch ownership
are Slug-native. New syntax, external labels, implicit/tool traversal,
query/aquery, build aggregation, one-shot adapters, repository/materializer
breadth and exact Bazel identity bytes remain unsupported/deferred.

Require zero legacy package/configured-analysis activation for direct,
multi-root, deps and rdeps paths; exact public outputs/errors/events; cold
child-before-result order and warm suppression; mixed outer/Need/semantic
ordering; semantic sidecars; cancellation/recovery; default/explicit/edit/
restore settings; recursive/null/delegating/platform/toolchain closure; exact
selected Arc retry survival; no carrier/revision; and build/query/aquery/
one-shot nonactivation.

Run focused cquery/native-demand tests, complete core/analysis/loading suites,
formatting, direct check, diff/archive gates, exact accounting, Buck2 retention
and AI cleanup scans, and independent implementation review.

## STOP / REPLAN

STOP on any other file; changed relocated body; public API/syntax/output/error/
event drift; legacy/second package or configured-analysis family; duplicate
root/driver/event owner; carrier/revision invention; retained store/collection/
cache/interner/lock/task; direct Host read; Cargo/BUILD/fixture/oracle/generated
write; or cap excess. `REPLAN` if query-crate work, another Rust file, partial
terminal algebra or unbounded state is required. Acceptance returns to one
docs-only next-owner audit and does not close M1.

## Immediate predecessor

`895996d5` freezes the complete two-file design selected by the post-neutral
audit in `5c47033d`, using accepted neutral implementation `941db0d0` and
observed configured-analysis prerequisite `69d37ddb`.
