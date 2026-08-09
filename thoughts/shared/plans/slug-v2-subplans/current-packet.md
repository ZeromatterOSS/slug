# Current Slug V2 Packet

Packet: `WP-6-m4-configured-query-successor-audit-2`
Milestone: M4 configured query
Owner: `slug-v2-subplans/06-analysis-toolchains-and-actions.md`
Cross-owner: `slug-v2-subplans/08-ruleset-and-command-conformance.md`
Result: read-only selection audit after accepted ordered label/Starlark-label sets.

## Observable slice

Select one exact next M4 behavior supported by the retained configured analysis
graph/providers and Buck2-derived query evaluator. Prefer direct reuse of
existing semantic state and evaluator paths over new command-specific logic.

## Ownership and stops

Provider projection is gated on a complete qualified-provider dictionary and
builtin value boundary. Configured `kind`/`attr` are gated on retained
configured metadata. `deps` is gated on Bazel-observable host-platform and
constraint nodes. Add no second graph/parser/evaluator, target patterns,
external repositories, exact-hash approximation, JVM/Java, CI, or compatibility
surface.

## Validation

Return one bounded functional implementation packet with exact observable,
owner, file/test allowlist, reusable or required Bazel 9.2 evidence, validation,
and stops. Do not commit the audit alone; bundle minimal scheduling state with
its selected implementation.
