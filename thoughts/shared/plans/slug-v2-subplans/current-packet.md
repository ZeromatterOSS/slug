# Current Slug V2 Packet

Packet: `WP-6-m4-configured-query-graph-ownership-design`
Milestone: M4 configured query
Owner: `slug-v2-subplans/06-analysis-toolchains-and-actions.md`
Cross-owner: `slug-v2-subplans/08-ruleset-and-command-conformance.md`
Result: reserved design after direct retained-state functions were exhausted.

## Observable slice

Design the complete configured node universe and edge ownership required for
exact traversal without creating a second analysis/query graph.

## Ownership and stops

Account for ordinary configured dependencies, selected toolchain implementation,
execution/target platforms, constraint nodes, aliases/non-rule nodes, and root
versus transitive discovery. Preserve DICE ownership/invalidation and current
Rust-native identity. Add no parallel graph/cache, command-local discovery,
filesystem bypass, exact Bazel hash claim, JVM/Java, CI, or shim.

## Validation

Return one bounded implementation sequence with node/edge owners, DICE keys and
equality, compact representations, Bazel evidence, allowlists, lifecycle and
activation tests, caps, and hard stops. Obtain reserved architecture review;
do not edit Rust in this design packet.
