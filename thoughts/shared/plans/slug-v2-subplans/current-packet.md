# Current Slug V2 Packet

Packet: `WP-6-m2-label-map-and-flag-alias-library-semantics-evidence`
Milestone: M2 authoritative target configuration
Owner: `slug-v2-subplans/06-analysis-toolchains-and-actions.md`
Result: source-only library-semantics closure for the remaining LabelMap and
FlagAlias routes.

## Goal

Pin Guava 33.5.0 `Splitter`/`CharMatcher` trimming, order, and duplicate
behavior, plus JDK 25 `Pattern` `\w` domain and exact FlagAlias validation and
diagnostics. Preserve the distinction between conversion and later command
alias expansion/C normalization, then decide a bounded successor.

## Required design record

Use source evidence only; do not infer Java/Guava behavior. Keep the converter
separate from downstream command alias expansion and C normalization. The
user-approved configured-target-cycle deferral remains explicit.

## Allowed paths

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`
- `thoughts/shared/plans/slug-v2-subplans/06-analysis-toolchains-and-actions.md`
- `thoughts/shared/plans/slug-v2-subplans/current-packet.md`

## Required tests and validation

Record pinned source provenance and exact behavioral discriminators. Run source
provenance, archive, scope, cap, no-Cargo, and `git diff --check` gates.

## Stop conditions

Stop with REPLAN on missing Guava/JDK authority, a JVM need, unclosed grammar,
new context/loader, reverse edge/cycle, or command/normalization ownership
ambiguity. Do not edit Rust, Cargo, fixtures, or create probes/artifacts, or do
command, loading, DICE, normalization, checksum, wire, or configured-target work.

## Diff budget

- Documentation and total: at most 180 net lines. No Rust, Cargo, fixture,
  generated, baseline, or unrelated changes.
