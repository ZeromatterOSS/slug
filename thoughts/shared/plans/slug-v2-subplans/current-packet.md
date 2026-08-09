# Current Slug V2 Packet

Packet: `WP-6-m4-configured-query-successor-audit`
Milestone: M4 configured query
Owner: `slug-v2-subplans/06-analysis-toolchains-and-actions.md`
Cross-owner: `slug-v2-subplans/08-ruleset-and-command-conformance.md`
Result: read-only selection audit after accepted configured set algebra.

## Observable slice

Select the smallest exact next M4 behavior after accepted configured set
algebra. Audit the retained `AnalysisResult` graph/providers, current public
cquery modes, existing Bazel 9.2 evidence, and the Buck2-derived generic query
evaluator. Prefer direct reuse of retained parser/evaluator code and semantic
state already owned by analysis.

## Ownership and stops

Do not select `deps` unless the packet first accounts for Bazel-observable host
platform and constraint nodes; the current direct-dependency surface is known
incomplete. Do not invent state or add a second graph/parser/evaluator. Exact
configuration hashes, target patterns, external repositories, JVM/Java, CI,
and compatibility shims remain excluded.

## Validation

Return one bounded implementation packet with an exact observable, owner,
allowlist, tests, and stop conditions. Reuse accepted evidence where it
discriminates; obtain fresh Bazel 9.2 evidence only for a demonstrated gap.
Do not commit this audit as documentation-only work: carry its bookkeeping with
the selected functional implementation.
