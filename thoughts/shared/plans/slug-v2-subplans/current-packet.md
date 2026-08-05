# Current Slug V2 Packet

Packet: `WP-6-m2-label-nine-route-source-closure-evidence`
Milestone: M2 authoritative target configuration
Owner: `slug-v2-subplans/06-analysis-toolchains-and-actions.md`
Result: pinned Bazel 9.2 source evidence and bounded successor decision for the
nine deferred label routes.

## Goal

Pin the exact six symbolic-default spellings/routes and the complete
`LabelMap`, `LabelToStringEntry`, and `FlagAlias` grammars, defaults, errors,
ordering, duplicate, and alias-validation behavior from Bazel 9.2. Decide
whether those nine routes have a bounded successor.

## Required design record

Use official/pinned local Bazel 9.2 source only. Keep `RunUnder` and
`CustomFlag` mixed, the five Host routes Unsupported, and the eight Java-regex
routes deferred. The user-approved configured-target-cycle deferral remains
unchanged.

## Allowed paths

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`
- `thoughts/shared/plans/slug-v2-subplans/06-analysis-toolchains-and-actions.md`
- `thoughts/shared/plans/slug-v2-subplans/current-packet.md`

## Required tests and validation

Record exact pinned source anchors and source-derived discriminators. Run source
provenance, archive, scope, cap, no-Cargo, and `git diff --check` gates.

## Stop conditions

Stop with REPLAN on a mixed, Host, regex, or JVM need; unclosed grammar; a new
context/loader; or a reverse edge/cycle. Do not edit Rust, Cargo, fixtures, or
create probes/artifacts, or do command, loading, DICE, normalization, checksum,
wire, or configured-target work.

## Diff budget

- Documentation and total: at most 180 net lines. No Rust, Cargo, fixture,
  generated, baseline, or unrelated changes.
