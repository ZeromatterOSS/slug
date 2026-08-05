# Current Slug V2 Packet

Packet: `WP-6-m2-windows-option-path-per-converter-call-observation-identity-design`
Milestone: M2 authoritative target configuration
Owner: `slug-v2-subplans/06-analysis-toolchains-and-actions.md`
Result: docs-only producer-free Windows observation identity, selection, and
rollback design.

## Goal

Design occurrence-keyed Windows option-path observation identity that can later
preserve one exact result per converter call without changing generic path
observation semantics.

## Required design record

Freeze a producer-free key, selection, and rollback contract for Windows
option-path long-name observations keyed by converter occurrence. It must retain
distinct calls with equal raw UTF-16 and their distinct resolved/fallback
outcomes, fit the accepted call-ID Host schema, and leave generic
`PathObservationDemand` identity/equality unchanged.

This is a design boundary only: it does not authorize Rust, Cargo, fixtures,
DICE, native capture, a converter, a driver, or configured-target work. Native
capture remains **REPLAN** and the user-approved configured-target-cycle
deferral remains unchanged.

## Allowed paths

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`
- `thoughts/shared/plans/slug-v2-subplans/06-analysis-toolchains-and-actions.md`
- `thoughts/shared/plans/slug-v2-subplans/current-packet.md`

## Required tests and validation

Pin the design against the accepted Host schema and generic observation
identity/rollback evidence. Run formatting, archive, scope, cap, no-Cargo, and
`git diff --check` gates.

## Stop conditions

Do not edit Rust or Cargo. Do not change generic observation identity, add
native capture, converter, driver, DICE, command/configured-target behavior,
fixtures, or generated output. Stop and REPLAN on any need for a reverse
dependency, new crate, or cycle.

## Diff budget

- Documentation: at most 120 net lines.
- Total: at most 120 net lines; no Rust, Cargo, fixture, generated, baseline,
  or unrelated changes.
