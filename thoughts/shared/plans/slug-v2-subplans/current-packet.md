# Current Slug V2 Packet

Packet: `WP-5-m1-external-dependency-free-starlark-rule-projection-implementation`
Milestone: M1, one semantic loading spine
Owner: `slug-v2-subplans/05-bzlmod-checkpoint-evidence-3.md`
Role: implementation worker
Evidence: accepted Bazel 9.2 dependency-free non-test external rule probe,
pinned explicit-public visibility representation, accepted external source/Bzl
ownership and package activation, route-aware query identity/provenance, and
independent loading/consumer design audits in the owner tail.

Implement only one disjoint extension to the current external loaded-package
gate: a package may contain exactly one recorded target, which must be a
`StarlarkRule` with exactly declared Public visibility, non-test and non-
executable capability, empty ordinary dependencies, and zero labels across all
dependency-reachable schema/value pairs. Preserve the current native
`ExportedFile`/`Filegroup` case. Reject the whole package before projection on
every other loaded-rule shape through private typed complete semantic reasons.

In the external graph, independently revalidate the same sole-target boundary
and project one `Rule("rule")` node with retained rule-class capability, no
edges or test metadata, existing dependency-reachable attributes, and an
explicit zero-label visibility attribute. Do not change loading environment,
provenance, candidates, outputs, identity, lifetime/event ownership, DICE, or
any public API.

The exact edit allowlist is `app/slug_loading_v2/src/bzl_module.rs`,
`app/slug_loading_v2/src/host_package_load_tests.rs`,
`app/slug_query_v2/src/graph.rs`,
`app/slug_query_v2/tests/loading_query.rs`, and
`app/slug_cli_v2/tests/cli.rs`. Total growth is at most `+550/-60`. No Cargo,
protocol, fixture, tool, generated-record, or sixth-file change is authorized.

Required focused coverage is accepted loading plus frozen lifetime/event
reuse; default/private/restricted visibility; test/executable capability;
ordinary, output, and every other reachable label; generated or additional
targets; graph defense in depth; all enabled consumers and four output modes;
empty `labels(visibility, ...)`; apparent labels and loading-file order; and
one-shot plus retained-daemon edit/delete/recreate recovery. Run focused
loading/query/CLI tests and direct checks, both affected loading/query GNU-
Windows no-run gates, rebuild `slug_cli_v2`, clean stale `slugd` before and
after direct lifecycle evidence, `cargo fmt --all -- --check`,
`scripts/v2_archive_status.sh`, exact scope/cap guards, and
`git diff --check`. Do not run Bazel, change a fixture, or run a workspace-wide
Cargo suite.

Stop with **REPLAN** rather than widening on mixed/additional targets,
default/nonpublic visibility, any reachable label or generated output,
test/suite/executable rules, cross-package/repository loads, globs, external
patterns, configuration, analysis/actions/execution, repository rules or
extensions, `@bazel_tools`, JVM, Java bytecode, Bazel delegation, a new
owner/key/lock/API, a sixth file, or cap excess. External test rules remain
blocked on the test-base/tool-repository closure.
