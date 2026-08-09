# Current Slug V2 Packet

Packet: `WP-6-m4-cquery-executables-nontest-successor-audit`
Milestone: M4 configured query
Owner: `slug-v2-subplans/06-analysis-toolchains-and-actions.md`
Cross-owner: `slug-v2-subplans/08-ruleset-and-command-conformance.md`
Result: read-only activation audit after accepted non-test executable analysis.

## Observable slice

Freeze the smallest exact configured `executables(expr)` activation over the
now-analyzable non-test rule surface. Resolve the known cquery analysis-failure
exit-2 versus Bazel exit-1 classification without broadening protocol state.

## Ownership and stops

Reuse retained `AnalysisResult.rule_capability` and the sole recursive query
fold. Do not reload packages, create metadata caches/keys, use `test_kind` as
the predicate, or activate configured test-rule success. Test-rule runfiles,
general providers, traversal, attrs, patterns/externals, exact hashes,
JVM/Java, CI, and shims remain outside the packet.

## Validation

Return one bounded implementation contract covering the shared evaluator,
configured filter, exact non-test oracle rows, typed analysis-error mapping,
lifecycle/equality evidence, allowlist, validation, and hard stops. Do not
commit the audit alone.
