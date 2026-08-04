# Current Slug V2 Packet

Packet: `WP-5-m1-external-repository-test-suite-query-design`
Milestone: M1, one semantic loading spine
Owner: `slug-v2-subplans/05-bzlmod-checkpoint-evidence-3.md`
Evidence: accepted direct local-override external source-file, filegroup,
direct alias, and config-setting queries; accepted root-package test-suite
loading/query representation; Bazel 9.2 query oracle infrastructure; complete
route hashing and end-to-end no-legacy guards
Validation tier: design-only source/oracle reconciliation plus independent
terminal design review

Design only the next bounded external native `test_suite` query slice. Do not
edit Rust, Cargo metadata, fixtures, generated oracle JSON, protocol, or any
implementation file. Start from the live retained
`PackageTargetKind::TestSuite`, `TestSuiteMembership`, native capability and
metadata, root graph projection, and existing query `labels`, `deps`, `tests`,
and `label_kind` consumers. Reproduce Bazel 9 semantics in Rust; do not add an
external `.bzl` load or another rule family merely to populate a suite.

Collect exact Bazel 9.2 evidence for explicit-empty and omitted native suites
in a direct local-override external package. Probe literal output,
`--output=label_kind`, `labels(tests, ...)`, `deps(...)`, and `tests(...)`,
including whether empty output and attribute explicitness provide a useful
discriminating slice. Reconcile that evidence with accepted root test-suite
fixtures and pinned Bazel source for explicit/implicit membership where the
live oracle cannot expose retained provenance directly.

The proposed implementation contract must state:

- which explicit and implicit suite forms are accepted without external
  Starlark/native test-rule breadth;
- exact `tests` and `$implicit_tests` attributes, explicitness, ordinary-edge
  order/deduplication, native rule capability, test metadata, and visibility;
- canonical semantic labels, selected apparent rendering, and whether any
  same-package member remapping or source synthesis is permitted;
- the exact literal, `label_kind`, `labels`, `deps`, and `tests()` surface that
  becomes active;
- cold/warm/BUILD-edit invalidation and event publication, with source-file
  changes remaining unobserved unless evidence proves otherwise;
- stop gates for suite members, suite chaining/cycles, other rule kinds,
  cross-package/repository labels, nontrivial visibility, patterns, loads,
  globs, configuration, analysis, build, and execution; and
- the exact production/test/fixture allowlist plus serial validation and
  independent implementation-review requirements.

Prefer a useful one-file private graph projection. If Bazel parity requires a
test-rule owner, external load, new retained representation/key, package or
repository discovery, configuration/analysis, filesystem observation, or
another architectural owner, record the exact unsupported boundary and
`REPLAN` instead of broadening the packet. Finish by appending the proposed
contract and evidence to the owner plan, advancing this manifest only after
independent terminal design review, and making no implementation change.
