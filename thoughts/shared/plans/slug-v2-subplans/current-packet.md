# Current Slug V2 Packet

Packet: `WP-6-m5-filewrite-aquery-text-formatter-design-retry`
Milestone: M5 entry
Owner: `slug-v2-subplans/06-analysis-toolchains-and-actions.md`
Result: freeze the first FileWrite aquery text formatter handoff.

## Observable slice

Reconcile the pinned Bazel 9.2 FileWrite text evidence with the accepted
configured action view, closure-resolved platform semantics, and exact Slug
semantic identity. Freeze the first formatter's field order, punctuation,
labels, inputs, mnemonic, configuration, output path, and action-token spelling,
classifying every field as exact Bazel-shaped text or an explicit Slug-native
projection.

## Ownership and stops

The exact canonical bytes remain the only FileWrite semantic identity. Any
short graph-local display token must be derived from the complete identity,
domain/version separated, named as a non-identity projection, and never reused
as Bazel ActionKey/checksum, configuration/output-root hash, REAPI digest, DICE
key equality, or cache identity. Keep ordinary no-toolchain/non-Write shapes
fail-closed.

Keep the vendored Buck2 `starlark-rust` parser/evaluator unchanged for BUILD and
`.bzl` semantics. Aquery syntax is the separate query language and must reuse
the existing Buck2-derived `QueryExpression` parser. Add no parser.

## Validation

Read only the accepted FileWrite evidence and live retained views. Select at
most one bounded implementation successor or return `REPLAN`. No Rust, tests,
oracle rerun, formatter implementation, aquery command/root/wire, execution,
new DICE key/state, REAPI reuse, parser/vendor, exact Bazel identity-byte work,
JVM/Java, or CI. Cap: 180 bookkeeping lines. Require independent design review
and bundle bookkeeping with the next functional commit; no standalone
documentation commit.
