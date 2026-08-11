# Current Slug V2 Packet

Packet: `WP-6-m37-cquery-executables-rdeps-direct`
Milestone: M4 configured query
Owner: `slug-v2-subplans/06-analysis-toolchains-and-actions.md`
Result: implementation after accepted filtered direct reverse traversal.

## Observable slice

Admit exactly
`executables(rdeps(<one concrete root-repository universe root>, <one concrete
root-repository seed>[, <signed Java-int reverse depth>])) --noimplicit_deps`.
Complete reverse traversal first, then apply the existing executable-non-test
capability predicate. Non-regex semantics are exact Bazel 9.2.

## Ownership and stops

Reuse M35 direct reverse traversal, universe-first seed validation, full
configured/null identity, reverse depth, existing executable predicate, and
selected-induced graph output. Production is limited to
`app/slug_query_v2/src/expr.rs` and `generic.rs`; M36 regex preflight remains
unchanged. Add no state, keys, caches, adjacency, interning, locks, or retained
representations.

Keep `filter`/`kind`/nested wrappers in this new shape, non-direct universes,
multiple seeds, same-package reverse, paths, default
implicit/tool/external/factored topology, parser/vendor changes, exact hashes,
JVM/Java, and CI stopped.

## Validation

Use pinned Bazel 9.2 `ExecutablesFunction`, the accepted executable-capability
oracle, and delegation evidence. Prove resolve → unbounded universe → reverse →
executables order; reverse failure prevents filtering; executable self-seed,
non-executable empty success, reverse depths, full keys, and selected-induced
graph; stopped shapes reject; and command/server plus rebuilt CLI daemon
symmetry. Run formatting, archive, diff, and daemon checks serially. Caps: 80
production / 260 test / 340 total Rust lines. Bundle bookkeeping with the
functional commit.
