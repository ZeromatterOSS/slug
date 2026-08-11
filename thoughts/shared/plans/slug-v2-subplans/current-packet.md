# Current Slug V2 Packet

Packet: `WP-6-m34-cquery-reverse-successor-audit`
Milestone: M4 configured query
Owner: `slug-v2-subplans/06-analysis-toolchains-and-actions.md`
Result: read-only successor audit after accepted inner-depth normalization.

## Observable slice

Select the next smallest exact Bazel 9.2 configured reverse-query behavior after
`rdeps(deps(root[, syntactic_depth]), seed[, reverse_depth])`. The optional inner
depth is syntax only here: Bazel re-closes the universe transitively from root,
so zero, positive, maximum, and omitted inner depths have identical results.

## Ownership and stops

Reuse the normalized configured graph, full configured/null identity,
universe-first loading validation, and request-local traversal. Do not implement
Rust in this audit or add state, keys, caches, adjacency, vendor changes, JVM,
exact hashes, or CI. Keep general universes, wrappers, path functions, and
default implicit/tool/external/factored topology stopped unless exact bounded
source and oracle evidence selects one.

## Validation

Read pinned Bazel 9.2 source and accepted oracle evidence, then name one bounded
implementation packet with exact semantics, stops, caps, and discriminating
tests, or record `REPLAN`. Bundle this bookkeeping with the next functional
commit; no standalone documentation commit.
