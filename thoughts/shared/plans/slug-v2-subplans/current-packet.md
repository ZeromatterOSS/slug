# Current Slug V2 Packet

Packet: `WP-6-m38-cquery-kind-rdeps-direct`
Milestone: M4 configured query
Owner: `slug-v2-subplans/06-analysis-toolchains-and-actions.md`
Result: implementation after accepted executable direct reverse traversal.

## Observable slice

Admit exactly
`kind(<one regex word>, rdeps(<one concrete root-repository universe root>,
<one concrete root-repository seed>[, <signed Java-int reverse depth>]))
--noimplicit_deps`. Compile the regex before any operand activation, then run
reverse traversal and the existing configured target-kind projection. Regex is
the accepted Slug-native boundary; other semantics are exact Bazel 9.2.

## Ownership and stops

Reuse M36 deterministic regex preflight/no retained regex, M35 direct reverse
traversal, full configured/null identity, reverse depth, existing kind
projection/fail-closed unsupported-kind boundary, and selected-induced output.
Production is limited to `app/slug_query_v2/src/expr.rs` and `generic.rs`; core
already invokes the shared first-statement preflight. Add no state, keys,
caches, adjacency, interning, locks, or retained representations.

Keep every other/nested wrapper, non-direct universes, multiple seeds,
same-package reverse, paths, default implicit/tool/external/factored topology,
parser/vendor changes, exact hashes, JVM/Java, and CI stopped.

## Validation

Use pinned Bazel 9.2 `KindFunction`/`RegexFilterExpression`, accepted delegation
and kind evidence. Prove invalid regex has no activation; valid resolve →
unbounded universe → reverse → kind order; reverse failure prevents projection;
ordinary-rule versus alias selection, duplicate configured keys, depths, empty
and selected-induced graph; stopped shapes reject; and command/server plus
rebuilt CLI daemon symmetry. Preserve unsupported-Platform assertions. Run
formatting, archive, diff, and daemon checks serially. Caps: 100 production /
300 test / 400 total Rust lines. Bundle bookkeeping with the functional commit.
