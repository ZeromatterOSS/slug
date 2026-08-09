# Current Slug V2 Packet

Packet: `WP-6-m4-configured-query-delegation-topology-oracle`
Milestone: M4 configured query
Owner: `slug-v2-subplans/06-analysis-toolchains-and-actions.md`
Cross-owner: `slug-v2-subplans/08-ruleset-and-command-conformance.md`
Result: second topology prerequisite after accepted toolchain graph evidence.

## Observable slice

Pin ordinary transitioned dependencies, alias delegation, input/output files,
package groups, reverse-dependency unwinding, and deterministic order.

## Ownership and stops

Design or extend one isolated Bazel 9.2 oracle without Rust. Distinguish
configured versus null-configuration nodes and direct versus delegated edges.
Do not infer topology from depth alone or add a query graph/harness shortcut.

## Validation

First perform a bounded source/fixture audit and freeze exact commands, output
patterns, allowlist, and caps. Stop on ambiguous delegation/order, unsupported
aspects/settings, exact-hash dependence, JVM/Java, or required Rust changes.
