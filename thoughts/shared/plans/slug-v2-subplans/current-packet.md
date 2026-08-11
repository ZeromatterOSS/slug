# Current Slug V2 Packet

Packet: `WP-8-m5-filewrite-aquery-command-root-design`
Milestone: M5 entry
Owner: `slug-v2-subplans/08-ruleset-and-command-conformance.md`
Result: freeze the first command/root consumer of accepted FileWrite text.

## Observable slice

Reconcile the existing aquery CLI and request placeholders, Buck2-derived
`QueryExpression` parser, build-command action closure, resolved FileWrite
semantic views, accepted per-action formatter, and daemon response shape.
Select at most one bounded main-repository single-root `--output=text`
consumer and freeze its request, evaluation, selection, ordering, join/final
newline, diagnostics, and exit semantics.

## Ownership and stops

Stage 8 owns expression/root evaluation, command and protocol wiring, container
ordering, block joining, and final output. Stage 6 continues to own resolved
action semantics and per-action formatting. Reuse the retained action closure
and formatter directly; do not reconstruct actions, duplicate identity, or add
a command-owned analysis graph.

Aquery remains the separate query language and must reuse the existing
`QueryExpression` parser. Keep recursive/external/multi-root expressions,
operators or functions not already justified by the selected slice,
non-default formats, non-FileWrite actions, executable writes, file contents,
ordinary no-toolchain, and unresolved shapes explicitly unsupported or
fail-closed.

## Validation

This packet is read-only. Inspect the accepted Bazel 9.2 FileWrite evidence and
live command, query, protocol, action-closure, and formatter sources. Produce
one exact handoff or `REPLAN`, classifying Bazel-exact command behavior versus
explicit Slug-native fields. Select at most one bounded implementation
successor. Add no Rust, tests, fixture growth, oracle rerun, command/wire
implementation, execution, DICE state, parser/vendor, REAPI reuse, exact Bazel
identity-byte work, JVM/Java, or CI. Cap: 200 bookkeeping lines. Require
independent design review and bundle bookkeeping with the next functional
commit; no standalone documentation commit.
