# Current Slug V2 Packet

Packet: `WP-6-m2-root-cquery-starlark-label-implementation`
Milestone: M2 analysis graph with the first configuration-opaque M4 consumer
Owner: `slug-v2-subplans/06-analysis-toolchains-and-actions.md`
Role: bounded implementation of one root Starlark-label cquery
Evidence: accepted recursive configured analysis; exact retained Bazel 9.2
success/missing/recovery; parallel Terra live/source audits; reserved Sol
acceptance of the typed error, command-root, output, wire, and lifecycle design.

Implement only:

`cquery <one root literal> --output=starlark --starlark:expr=str(target.label)`

Exact production allowlist:

- `app/slug_analysis_v2/src/dice.rs`
- `app/slug_analysis_v2/src/lib.rs`
- `app/slug_commands_v2/src/cquery.rs`
- `app/slug_core_v2/src/runtime/dice.rs`
- `app/slug_core_v2/src/runtime/mod.rs`
- `app/slug_cli_v2/src/commands/cquery.rs`
- `app/slug_server_v2/src/lib.rs`
- `app/slug_server_v2/src/server.rs`

Exact test allowlist:

- `app/slug_analysis_v2/tests/root_analysis.rs`
- `app/slug_commands_v2/tests/commands.rs`
- the existing test module in `app/slug_core_v2/src/runtime/dice.rs`
- `app/slug_cli_v2/tests/cli.rs`
- `app/slug_server_v2/src/tests.rs`

Caps: at most 650 formatted net production lines, 600 formatted net test lines,
and 1,250 total. No fixture, oracle, harness, plan, ledger, Cargo manifest, or
lockfile edit belongs to the implementation diff.

Required implementation:

1. Replace string-only `AnalysisError` storage with an allocative typed
   `TargetNotFound { label, build_file }` versus ordinary message split,
   constructed at the existing loaded-package target lookup. Preserve all
   current display text and prevent dependency misses from being classified as
   the requested root miss.
2. Parse exactly one root `TargetPattern::Single`, explicit
   `--output=starlark`, and exactly one
   `--starlark:expr=str(target.label)`, plus one optional output base and the
   existing bzlmod inputs. Reject all other arguments before analysis with
   structured exit-2 errors.
3. Add a private non-DICE `CqueryCommandRoot` that directly computes the
   existing `RootConfiguredTargetAnalysisKey` through `NativeCommandRoot`.
   Keep `first-build` internal. Add no cquery DICE key, pre-analysis package
   load, evaluator, graph, or second configured identity.
4. On success, retain the accepted `AnalysisResult` and publish only its
   canonical label plus newline. On matching direct root miss, publish exit 1,
   empty stdout, and the exact accepted three-line Bazel 9.2 stderr. Other
   analysis failures stay typed generic failures. No JSON success envelope.
5. Add one-shot and retained-daemon entry points with a dedicated additive
   `CqueryRequest { target, bzlmod }`, `DaemonRequest::Cquery`, dispatcher, and
   send helper. Do not inherit loading-query expression/order/graph fields.
   Keep `invalidated_files` as response metadata.

Mandatory evidence:

- parser positive case plus missing/duplicate/alternate output/expression,
  Starlark-file, passthrough, multi-label, pattern, external, and query-flag
  rejections;
- typed direct-root missing with preserved legacy display and dependency-miss
  separation;
- exact one-shot/daemon success bytes, exit-1 missing bytes, and missing-to-
  recovery with zero invalidations absent an edit;
- exact `RootConfiguredTargetAnalysisKey` activation identities/counts for
  cold, warm, missing, recovery, BUILD edit, and `.bzl` edit;
- explicit zero action execution and zero REAPI calls; additive wire round trip
  and malformed-wire behavior.

Validate serially. Clean stale `slugd` before and after daemon tests. Run
focused analysis, commands, core, server, and CLI tests; full affected crates;
`cargo build -p slug_cli_v2` before binary daemon tests; GNU-Windows no-run
checks for affected libraries; `cargo fmt --all -- --check`;
`scripts/v2_archive_status.sh`; scope/cap/forbidden evaluator, key, display-
parse, filesystem, action, and REAPI greps; and `git diff --check`.

Stop and `REPLAN` on a ninth production file, cap breach, new DICE key, direct
filesystem read in analysis, display-text parsing, second load/analysis,
general Starlark evaluation, observable configuration token, fixture edit,
action/REAPI execution, or lock across DICE compute.
