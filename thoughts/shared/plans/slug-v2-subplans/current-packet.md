# Current Slug V2 Packet

Packet: `WP-6-m2-java-regex-route-source-closure-evidence`
Milestone: M2 authoritative target configuration
Owner: `slug-v2-subplans/06-analysis-toolchains-and-actions.md`
Result: source-only closure decision for the eight remaining Java-regex-backed
converter routes.

## Goal

Inventory and pin the eight deferred regex routes: `RegexFilter` (3),
`ExecutionInfoModifier` (1), `PerLabelOptions` (3), and `RunsPerTest` (1).
Establish Bazel 9.2 grammar, defaults, errors, ordering, duplicate handling,
rendering/cache behavior, and the exact JDK 25 `Pattern`/`Matcher` dependency;
then decide bounded subsets versus REPLAN.

## Required design record

Use pinned source only; do not run a JVM or introduce a regex runtime/dependency.
Keep Host routes terminal Unsupported. Preserve the accepted RunUnder renderer/
cache and full-Java-String REPLAN, command activation, loading, DICE,
normalization, checksum, wire, and user-deferred configured-target cycles.

## Allowed paths

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`
- `thoughts/shared/plans/slug-v2-subplans/06-analysis-toolchains-and-actions.md`
- `thoughts/shared/plans/slug-v2-subplans/current-packet.md`

## Required tests and validation

Record primary Bazel and JDK source anchors and a complete eight-route inventory.
Run source, archive, scope, cap, no-Cargo, and `git diff --check` gates.

## Stop conditions

Stop with REPLAN on an unclosed JDK/runtime regex dependency, renderer/cache
need, a Host/context/loader boundary, or any activation/command ownership need.
Do not edit Rust, Cargo, fixtures, or create probes/artifacts.

## Diff budget

- Documentation and total: at most 260 net lines. No Rust, Cargo, fixture,
  generated, baseline, or unrelated changes.
