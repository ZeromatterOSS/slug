# Current Slug V2 Packet

Packet: `WP-6-m2-native-conversion-schedule-and-host-fact-redesign`
Milestone: M2 authoritative target configuration
Owner: `slug-v2-subplans/06-analysis-toolchains-and-actions.md`
Result: docs-only converter-call and producer-free Host-fact redesign; no Rust.

## Goal

Freeze the real command-owned schedule that determines whether a native
converter is called. Replace the invalid raw-deduplicated Host-fact assumption
before proposing a core-to-configuration bridge. Native capture remains
**REPLAN**.

## Required design record

Record each relevant default-validation checkpoint, `FieldOptionDefinition`
default memoization boundary, priority selection, single-value acceptance, and
expansion order that controls a converter call. Define per-accepted-occurrence
identity for fresh home and Windows raw/outcome facts: equal raw UTF-16 inputs
must remain distinct and may have different resolution outcomes. Classify exact
capacity eligibility through `ResourceConverter`, not descriptor shape.

Revise the producer-free configuration schema and identify the smallest future
bridge prerequisites: the command-owned event schedule, occurrence ordering,
error timing, and request/attempt lifetime. Preserve only core ->
configuration direction and explain why no DICE cycle or configured-target
cycle is introduced. Native capture and the production native-demand driver
remain absent and out of scope.

## Allowed paths

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`
- `thoughts/shared/plans/slug-v2-subplans/06-analysis-toolchains-and-actions.md`
- `thoughts/shared/plans/slug-v2-subplans/current-packet.md`
- routing only on `REPLAN`:
  `.codex/skills/slug-agent-orchestration/references/routing-log.md` and
  `.codex/skills/slug-agent-orchestration/references/routing-history-2026-08.md`

## Stop conditions

Do not edit Rust, Cargo, schemas, converters, DICE keys/computes, request or
native-demand drivers, command/configured-target behavior, fixtures, or
generated output. Stop and REPLAN if an exact schedule requires native capture,
a live production driver, a reverse configuration dependency, or a configured-
target cycle.

## Completion and next boundary

Complete only with the bounded schedule/schema redesign and synchronized
scheduling. Any implementation requires separate acceptance after native
capture/driver and converter-call prerequisites are proven.

## Diff budget

- Documentation: at most 160 net lines.
- No Rust, Cargo, fixture, generated, baseline, or unrelated changes.
