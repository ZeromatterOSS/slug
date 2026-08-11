# Current Slug V2 Packet

Packet: `WP-6-m5-filewrite-semantic-identity-design-retry`
Milestone: M5 entry
Owner: `slug-v2-subplans/06-analysis-toolchains-and-actions.md`
Result: freeze one complete FileWrite semantic identity design.

## Observable slice

Design a collision-safe tagged structural identity for the admitted
toolchain-backed FileWrite view. Include configured owner, typed output, Write
content and executable bit, default exec group, selected platform configured
key, normalized exec properties, and the complete ordered
Platform-to-ConstraintValue-to-ConstraintSetting chain.

## Ownership and stops

Keep semantic identity separate from any graph-local formatter token, Bazel
ActionKey/checksum, configuration hash, output-root hash, and REAPI digest.
Specify domain/version tags, unambiguous field framing, collection order, and
which existing retained values are borrowed versus projected. Select at most
one bounded implementation successor or return `REPLAN`.

Keep the vendored Buck2 `starlark-rust` parser/evaluator unchanged for BUILD and
`.bzl` semantics. Aquery syntax is the separate query language and must reuse
the existing Buck2-derived `QueryExpression` parser. Add no parser.

## Validation

Audit only the accepted configured-action and closure-resolved platform views.
No Rust, tests, hash implementation, formatter, aquery command/root/wire,
execution, new DICE key/state, REAPI reuse, oracle rerun, parser/vendor,
exact-Bazel-byte work, JVM/Java, or CI changes. Cap: 180 bookkeeping lines.
Require independent design review and bundle bookkeeping with the next
functional commit; no standalone documentation commit.
