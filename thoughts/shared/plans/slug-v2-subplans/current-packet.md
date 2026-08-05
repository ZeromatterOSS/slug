# Current Slug V2 Packet

Packet: `WP-6-m2-production-native-conversion-schedule-driver-design`
Milestone: M2 authoritative target configuration
Owner: `slug-v2-subplans/06-analysis-toolchains-and-actions.md`
Result: docs-only audit and freeze of the production native conversion schedule
driver boundary.

## Goal

Audit and freeze the full production command entrypoint that will own native
conversion scheduling, without implementing it.

## Required design record

Record the parse batches, default memoization, priority acceptance, expansion,
and policy events that can produce converter calls; distinguish request from
attempt/retry lifetime; and bind the first configuration input and preflight
stale-Windows filtering. Freeze the one-way core-to-configuration direction.

This is a design boundary only: it does not authorize native capture, a
converter, a driver, DICE, or configured-target work. Native capture remains
**REPLAN** and the user-approved configured-target-cycle deferral remains
unchanged.

## Allowed paths

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`
- `thoughts/shared/plans/slug-v2-subplans/06-analysis-toolchains-and-actions.md`
- `thoughts/shared/plans/slug-v2-subplans/current-packet.md`

## Required tests and validation

Pin the design against the retained converter and Host evidence. Run formatting,
archive, scope, cap, no-Cargo, and `git diff --check` gates.

## Stop conditions

Do not edit Rust or Cargo. Do not add native capture, converter, driver, DICE,
command/configured-target behavior, fixtures, or generated output. Stop and
REPLAN on any need for a core bridge, reverse dependency, or a new cycle.

## Diff budget

- Documentation: at most 120 net lines.
- Total: at most 120 net lines; no Rust, Cargo, fixture, generated, baseline,
  or unrelated changes.
