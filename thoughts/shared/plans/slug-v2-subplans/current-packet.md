# Current Slug V2 Packet

Packet: `WP-6-m2-root-cquery-starlark-label-boundary-design`
Milestone: M2 analysis graph with the first configuration-opaque M4 consumer
Owner: `slug-v2-subplans/06-analysis-toolchains-and-actions.md`
Role: design-only command, DICE, error, output, and lifecycle boundary
Evidence: accepted recursive configured analysis; exact retained Bazel 9.2
Starlark-expression success/missing/recovery evidence; accepted general
configuration-identity REPLAN.

Do not edit Rust, tests, fixtures, generated oracle records, wire schema, or
harness code. Design the smallest exact implementation for only:

`cquery <one root literal> --output=starlark --starlark:expr=str(target.label)`

The design must settle these ownership points before implementation:

1. Parse and retain exactly one root-repository `TargetPattern::Single`.
   Require explicit `--output=starlark` and exactly one
   `--starlark:expr=str(target.label)`. Reject missing/duplicate values, every
   alternate expression/output, `--starlark:file`, patterns, multiple labels,
   and external labels before analysis.
2. Construct the existing internal target configuration without making
   `first-build` observable, then drive the existing
   `RootConfiguredTargetAnalysisKey` directly through `NativeCommandRoot`.
   Add no cquery DICE key, command-owned graph, package pre-analysis, evaluator
   call, or second configured-target identity.
3. Project success only from the returned `AnalysisResult.key().label()` and
   emit exact `@@//parent:parent\n`-shaped canonical-label stdout. Recognizing
   the one accepted expression is a bounded command contract, not permission
   for partial general Starlark evaluation.
4. Preserve typed Needs, root anchor/loading errors, analysis errors, event
   capture, terminal selection, and accepted publication. Specify a typed
   missing-target path that can reproduce the accepted Bazel 9.2 diagnostic
   without parsing display text, duplicating package loading, or widening the
   analysis graph.
5. Give one-shot and retained daemon paths the same normalized request and
   terminal semantics. A dedicated `CqueryRequest`/`DaemonRequest::Cquery`
   variant is a public schema change; enumerate exact compatibility/round-trip
   tests and do not reuse loading-query-only order/graph/strict-suite fields.
6. Pin exact success/missing exit codes, stdout, CLI stderr/JSON policy, runtime
   mode, and daemon `invalidated_files` behavior against both Bazel evidence
   and established Slug command conventions. Any contradiction requires
   `REPLAN`, not an invented hybrid.
7. Prove cold success, warm equality/reuse, missing target, same-daemon
   recovery, BUILD/.bzl provider edits, and no REAPI/action execution. Use
   exact activation/event assertions for the existing root analysis key.

Return `ACCEPT` only with a complete file allowlist, production/test/total line
caps, exact test inventory, serial validation commands, and no utility or DICE
ownership ambiguity. Obtain reserved review before authorizing Rust.

Stops: no default/explicit `label`, arbitrary Starlark expression/file,
provider formatter, query expression, pattern, external repository,
configuration, transition, toolchain, aquery, action execution, REAPI, or cycle
work; no hard-coded output label; no display-text error parsing; no lock across
DICE computation; no credential inspection.
