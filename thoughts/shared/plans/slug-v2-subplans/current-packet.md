# Current Slug V2 Packet

Packet: `WP-5-m1-external-repository-test-rule-query-design`
Milestone: M1, one semantic loading spine
Owner: `slug-v2-subplans/05-bzlmod-checkpoint-evidence-3.md`
Evidence: accepted direct local-override external query route and native target
projections; accepted root Starlark rule/test metadata and suite membership;
pinned Bazel 9.2 loading/query sources must be refreshed only for the new
external-load boundary
Validation tier: design/source/oracle evidence only, with one independent
reserved-boundary pre-review before any implementation

Design the smallest exact Rust-only observable slice for one direct
`local_path_override` external package that loads a same-repository `.bzl`
file defining a Starlark test rule. Freeze literal query, `label_kind`, retained
rule capability/test metadata, canonical identity, apparent rendering,
loading-event publication, and warm/edit/recovery invalidation semantics.
Decide explicitly whether one direct native `test_suite` member edge to that
test rule is part of the same atomic slice or a separate follow-on.

Read the live external repository loading/query owners and the accepted root
Starlark loading/query tests before proposing representation. Collect bounded
Bazel 9.2 evidence for the exact success output, dependency/load identity,
test metadata relevant to query, and any suite-member behavior proposed.
Prefer an existing fixture only if it remains discriminating after the sixth
external-query hygiene checkpoint; otherwise specify the minimum isolated
fixture growth and why it is required.

The design must name exact production/test/oracle allowlists, existing DICE
keys and route owners to reuse, equality/invalidation expectations, lifecycle
events, diagnostics, focused direct dependents, and stop gates. It must keep
configuration, analysis, actions, execution, external globs/patterns,
cross-package/repository loads, repository rules/extensions, visibility
content evaluation, JVM, Java bytecode, and Bazel delegation out of Slug
architecture.

This packet changes only the owner plan, canonical scheduling row, and this
manifest. Do not edit Rust, Cargo metadata, fixtures, or expected outputs.
Stop with `REPLAN` if exact external `.bzl` identity/load resolution requires a
new public cross-crate identity, DICE ownership model, filesystem bypass, or
unbounded discovery. Because external load identity is a reserved boundary,
obtain one independent pre-review before authorizing implementation.
