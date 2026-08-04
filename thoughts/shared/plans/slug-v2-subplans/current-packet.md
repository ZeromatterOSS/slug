# Current Slug V2 Packet

Packet: `WP-5-m1-external-repository-starlark-rule-query-design`
Milestone: M1, one semantic loading spine
Owner: `slug-v2-subplans/05-bzlmod-checkpoint-evidence-3.md`
Evidence: accepted direct local-override external query route and native target
projections; accepted root Starlark rule loading/query representation; accepted
REPLAN proving external test rules require an unavailable test-base dependency
closure; live Bazel 9.2 non-test evidence and external-load source anchors must
be frozen for this narrower boundary
Validation tier: design/source/oracle evidence only, with one independent
reserved-boundary pre-review before any implementation

Design the smallest exact Rust-only observable slice for one direct
`local_path_override` external package that loads a same-repository `.bzl`
file defining one dependency-free non-test Starlark rule. Freeze literal query,
`label_kind`, self-only `deps`, retained rule capability/frozen implementation,
canonical identity, apparent rendering, loading-event publication, load-cycle
diagnostics, and warm/edit/recovery invalidation semantics.

Read the live external repository loading/query owners, `.bzl` cycle detector,
and accepted root Starlark loading/query tests before proposing representation.
Collect bounded Bazel 9.2 evidence for exact literal, kind, and dependency
output plus same-repository direct/transitive load, missing-load, cycle, and
edit/recovery behavior needed to discriminate the owner.
Prefer an existing fixture only if it remains discriminating after the sixth
external-query hygiene checkpoint; otherwise specify the minimum isolated
fixture growth and why it is required.

The design must name exact production/test/oracle allowlists and freeze a
private route-keyed external Bzl-module identity using the existing
`RootRepositoryRoute` and typed canonical package/target label. It must reuse
`HostRepositorySourceFileKey`, complete-only equality/validity, retained
module lifetimes, and local complete event publication without reusing the
root-only `HostBzlModuleEvalKey`. Specify the matching private cycle-detector
node/guard, invalidation expectations, diagnostics, focused direct dependents,
and stop gates. Keep configuration, test rules/test suites, generated outputs,
analysis, actions, execution, external globs/patterns, cross-package/repository
loads, repository rules/extensions, visibility content evaluation,
`@bazel_tools` graph activation, JVM, Java bytecode, and Bazel delegation out
of Slug architecture.

This packet changes only the owner plan, canonical scheduling row, and this
manifest. Do not edit Rust, Cargo metadata, fixtures, or expected outputs.
Stop with `REPLAN` if exact external `.bzl` identity/load resolution requires a
new public cross-crate identity, a new ownership model outside the accepted
loading owner, filesystem bypass, unbounded discovery, any implicit dependency
beyond the rule itself, or test-base/tool repository projection. Because
external load identity remains a reserved boundary, obtain one independent
pre-review before authorizing implementation.
