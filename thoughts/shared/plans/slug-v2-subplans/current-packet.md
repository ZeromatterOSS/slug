# Current Slug V2 Packet

Packet: `WP-4-6-8-bazel-tools-test-closure-design`
Milestone: cross-stage M7 prerequisite design
Owner: `slug-v2-subplans/08-ruleset-and-command-conformance.md`
Result: freeze the complete pinned Bazel 9.2 source and ownership closure needed
before Slug can load `@@bazel_tools//tools/test`.

## Active design contract

The immutable built-in repository owner is accepted. It owns a versioned,
canonical `bazel_tools` route and the exact bytes, SHA-256, archive executable
state, and typed lookup terminals for a reviewed seven-file partial catalog.
It deliberately does not dispatch package, BUILD, or Bzl consumers.

Audit pinned Bazel 9.2.0 commit
`8220c6198837d5c13d53fea211cf3282aa12408a` and the live Slug graph to
freeze the smallest complete repository/source/package boundary that can load
`@@bazel_tools//tools/test` without pruning, synthesizing, or reading a Host
Bazel install. Trace the embedded MODULE registrations and all ordinary module,
repository-mapping, package, Bzl, label, config-setting, toolchain, filegroup,
rules_shell, platforms, and generated-source dependencies reached by that
package. Separate immutable built-in bytes from registry/resolution-owned
external repositories and name the DICE owner and invalidation edge for every
admitted input.

Decide whether one bounded implementation can reuse the existing route,
repository package/Bzl loaders, and configured-analysis representation. Freeze
the exact catalog expansion, source lookup/consumer dispatch, repository
mapping, Need/error ordering, equality/validity, lifecycle, and fail-closed
boundary. If no bounded Rust-native slice exists, record `REPLAN` rather than
inventing content or a parallel graph.

## Compatibility

Exact: verbatim pinned-source bytes and modes, content hashes, source-known
file/directory distinction, canonical labels/repository mappings, and admitted
package/Bzl/config/toolchain relationships. Slug-native: snapshot/manifest and
DICE type names, diagnostics, compact storage, path/configuration/action
identity bytes, and any cross-owner iteration order not guaranteed by Bazel.
Unsupported/deferred: TestProvider, TestRunnerAction, runfiles-tree
materialization, test execution/result analysis, BEP, coverage, unreviewed
embedded-tools packages, Windows, JVM/Java, and exact Bazel identity bytes.

## Scope, proof, and stops

This design packet may edit only:

- `thoughts/shared/plans/slug-v2-subplans/current-packet.md` and
  `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`;
- `thoughts/shared/plans/slug-v2-subplans/04-starlark-loading-and-build-packages.md`,
  `thoughts/shared/plans/slug-v2-subplans/06-analysis-toolchains-and-actions.md`,
  and
  `thoughts/shared/plans/slug-v2-subplans/08-ruleset-and-command-conformance.md`;
- `thoughts/shared/plans/slug-v2-subplans/09-v1-extraction-ledger.md` only if
  the final reuse decision changes an existing extraction row; and
- only if pinned Bazel evidence exposes a demonstrated checked-in oracle gap,
  these six existing fixture files:
  `tests/v2_oracle/fixtures/test-basic/fixture.toml`,
  `tests/v2_oracle/fixtures/test-basic/expected/oracle.json`,
  `tests/v2_oracle/fixtures/test-basic/workspace/MODULE.bazel`,
  `tests/v2_oracle/fixtures/test-basic/workspace/BUILD.bazel`,
  `tests/v2_oracle/fixtures/test-basic/workspace/pkg/BUILD.bazel`, and
  `tests/v2_oracle/fixtures/test-basic/workspace/pkg/defs.bzl`.

Cap bookkeeping at 240 net lines and any fixture correction at 180 net lines;
add no file or dependency. No Rust, Cargo/BUILD metadata, DICE key, package or
analysis implementation, command/Test/TestRunner, REAPI, BEP, JVM/Java,
Windows branch, generated source, second graph, Host observation, or runtime
source selection is authorized.

Require pinned-source and embedded-archive provenance, a complete transitive
closure ledger, repository-routing and DICE ownership diagrams in prose,
explicit accepted/rejected reuse decisions, exact/Slug-native/deferred
classification, a successor file allowlist/caps/tests/stops, source/structure,
credential, archive active-layout, fixture (if changed), and diff checks, plus
independent review. One bounded correction is allowed; a second material miss
is `REPLAN`. At `ACCEPT`, schedule only the reviewed successor.
