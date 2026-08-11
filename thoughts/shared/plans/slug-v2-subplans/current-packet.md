# Current Slug V2 Packet

Packet: `WP-6-m36-cquery-filter-rdeps-direct`
Milestone: M4 configured query
Owner: `slug-v2-subplans/06-analysis-toolchains-and-actions.md`
Result: implementation after accepted direct-universe normalization.

## Observable slice

Admit exactly
`filter(<one regex word>, rdeps(<one concrete root-repository universe root>,
<one concrete root-repository seed>[, <signed Java-int reverse depth>]))
--noimplicit_deps`. Complete the universe and reverse traversal before applying
the existing label filter. Regex behavior remains the accepted Slug-native
valid-Unicode/Rust-regex boundary; non-regex graph semantics are exact Bazel 9.2.

## Ownership and stops

Reuse M35 direct-universe normalization, universe-first loading-only seed
validation, full configured/null identity, bounded normalized reverse traversal,
existing display-label filter, and selected-induced graph output. Production is
limited to `app/slug_query_v2/src/expr.rs` and `generic.rs`; add no state, keys,
caches, adjacency, interning, locks, or retained representations.

Keep `kind`/`executables`/nested filter or any other wrapper, `deps`/general/set/
multi-root/variable/external universes, multiple seeds, same-package reverse,
paths, default implicit/tool/external/factored topology, parser/vendor changes,
exact hashes, JVM/Java, and CI stopped.

## Validation

Use pinned Bazel 9.2 `RdepsFunction` and `RegexFilterExpression` plus accepted
delegation evidence. Prove universe → reverse → filter order; negative, zero,
positive, and omitted reverse depth; duplicate configured keys and aliases;
empty success and selected-induced graph; failures cannot be masked; stopped
shapes reject; and command/server plus rebuilt CLI daemon symmetry. Run
formatting, archive, diff, and daemon checks serially. Caps: 90 production / 300
test / 390 total Rust lines. Bundle bookkeeping with the functional commit.
