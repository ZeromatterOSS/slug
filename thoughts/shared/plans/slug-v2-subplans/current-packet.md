# Current Slug V2 Packet

Packet: `WP-6-m31-cquery-reverse-delegation-normalization-design`
Milestone: M4 configured query
Owner: `slug-v2-subplans/06-analysis-toolchains-and-actions.md`
Result: read-only design after forward-filter closure.

## Observable slice

Design the exact retained semantic relation required for Bazel 9.2
`rdeps(deps(<one concrete root>), <one concrete label>)` to unwind delegated
configured targets. Explain why a reverse scan of authoritative forward alias
edges incorrectly includes `alias_inner` in the accepted delegation fixture.

## Ownership and stops

Reuse the sole configured analysis key/result, full configured/null identity,
authoritative edges, and accepted delegation oracle. Identify whether Slug must
retain a resolved configured-target value key, a delegation-equivalence link,
or another smaller request-replayable fact. Do not guess from alias edge shape,
labels, providers, or configuration tokens.

Do not implement Rust, add a reverse adjacency/graph/key/cache, modify DICE
ownership, or change fixtures/oracles/parser/vendor/wire/output. Keep general
reverse/path breadth, `some(deps)`, implicit/tool/external/factored topology,
exact hashes, JVM/Java, and CI stopped.

## Validation

Use pinned Bazel 9.2 `RdepsFunction`, configured-target lookup, post-analysis
reverse-deps, delegation handling/value-key owners, and the accepted delegation
payload. Produce one bounded implementation packet with exact representation,
invalidation, memory, and test ownership, or `REPLAN`. No standalone
documentation commit: bundle this design bookkeeping with the next functional
packet only after independent review accepts it.
