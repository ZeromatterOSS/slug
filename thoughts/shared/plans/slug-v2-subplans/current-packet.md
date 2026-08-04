# Current Slug V2 Packet

Packet: `WP-5-m1-external-repository-rule-query`
Milestone: M1, one semantic loading spine
Owner: `slug-v2-subplans/05-bzlmod-checkpoint-evidence-3.md`
Evidence: accepted corrected external `filegroup` query design, direct
local-override external source-file query, root filegroup/source-edge Bazel
9.2 oracle, pinned assumed-input source semantics, complete route hashing,
native materialization/path retries, and end-to-end no-legacy guards
Validation tier: one-file private query projection plus focused public
query/core/CLI/server and two-row Bazel/Slug oracle extension

Implement only the external native `filegroup` projection in
`app/slug_query_v2/src/graph.rs`. Reuse the ordinary `LoadedPackage` and
`PackageTargetKind::Filegroup`; remap only root-context labels in the current
package to canonical `@@dep+`, retain route-specific apparent rendering, and
synthesize same-package source nodes without observing their physical files.

Project the existing `srcs` attribute and ordinary edges, but no query-visible
visibility attribute. The accepted observable slice is an external filegroup
literal, node-local `labels(srcs, ...)`, and forward `deps` within the same
package. Preserve route-specific DICE graph/load keys while keeping
`QueryLabel` equality/hash/order and diagnostics canonical.

Production allowlist: `app/slug_query_v2/src/graph.rs`. Tests may change only
`app/slug_query_v2/tests/loading_query.rs`, core runtime, CLI, and server
tests. Oracle changes are limited to the existing `module-local-override`
fixture TOML, `workspace/dep/BUILD.bazel`, and expected JSON; add no asset.

Prove exported-source reuse, absent implicit-source synthesis, exact
attribute/edge order and provenance, canonical node identity, apparent output,
route-specific caches, canonical missing diagnostics, other-rule/collision/
cross-edge/visibility stop gates, cold/warm/BUILD-edit events, physical-source
non-invalidation, and zero forbidden legacy/root/snapshot activation. Add
exact Bazel/Slug rows for `@dep//:files` and
`labels(srcs, @dep//:files)` while protecting both existing fixture commands.

Stop on a new representation, DICE/loading/source owner, source observation,
cross-package/repository route, reverse/package/provenance function breadth,
external loads/globs/patterns, registry transport, repository rules/extensions,
build/execution, JVM, Java bytecode, or Bazel delegation. Finish with the
serial focused/full validations and one independent terminal implementation
review required by the accepted owner-plan contract.
