# Current Slug V2 Packet

Packet: `WP-6-m5-toolchain-filewrite-text-formatter-design`
Milestone: M5 entry
Owner: `slug-v2-subplans/06-analysis-toolchains-and-actions.md`
Result: read-only text formatter and Slug-native token handoff design.

## Observable slice

Design the first text-only aquery handoff over the accepted toolchain-backed
`ConfiguredActionView`. Freeze exact field order, punctuation, owner/platform
labels, inputs, executable bit, and output-relative path semantics from pinned
Bazel 9.2 evidence while classifying configuration, configured output root, and
action identity as explicit Slug-native projections.

## Ownership and stops

Define collision-safe, tagged, versioned projection inputs so C0/C1/C0,
P0/P1/P0, content A/B/A, and output path A/B/A preserve the equality/change
relationships in `f00e99db`. Do not call any value a Bazel checksum, Bazel
configured root, or ActionKey. Preserve exact REAPI/CAS digest separation.

Keep the vendored Buck2 `starlark-rust` parser/evaluator unchanged for BUILD and
`.bzl` semantics. Aquery syntax is the separate query language and must reuse
the existing Buck2-derived `QueryExpression` parser. Add no parser.

## Validation

Audit existing configuration projections and choose one bounded implementation
successor or return `REPLAN`. No Rust, tests, fixtures/oracle reruns, aquery
command/root/wire, formatter implementation, action execution, REAPI identity
reuse, exact Bazel-byte work, parser/vendor changes, JVM/Java, or CI. Caps: 0
production / 0 test / 180 bookkeeping lines. Bundle with the next functional
commit; no standalone documentation commit.
