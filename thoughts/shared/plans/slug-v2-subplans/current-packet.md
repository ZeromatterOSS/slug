# Current Slug V2 Packet

Packet: `WP-5-m1-external-repository-rule-query-design`
Milestone: M1, one semantic loading spine
Owner: `slug-v2-subplans/05-bzlmod-checkpoint-evidence-3.md`
Evidence: accepted direct local-override external source-file query, complete
route hashing, native materialization/path retries, canonical semantics with
apparent output, and end-to-end no-legacy activation guards
Validation tier: read-only live-seam and Bazel 9.2 design audit

Design only the smallest follow-on query slice for one native rule in the same
external package, starting with `filegroup` over local exported/source files.
Freeze exact Bazel output, graph identity, same-package edge projection,
missing-edge diagnostics, invalidation/event behavior, and the production/test
allowlist before any Rust or fixture edit.

Preserve the accepted `RootRepositoryRoute`, `HostRepositorySourceFileKey`,
typed command retry/publication owner, canonical identity, apparent rendering,
and complete `RepoSpec` hash. Reuse existing loading/query rule
representations where their ownership is exact.

Stop on transitive repository mapping, cross-package edges, registry
transport, repository rules/extensions, external `.bzl` loads, glob
traversal, patterns, canonical-label input, build/execution behavior, JVM,
Java bytecode, or Bazel delegation. Finish the design with one independent
terminal review.
