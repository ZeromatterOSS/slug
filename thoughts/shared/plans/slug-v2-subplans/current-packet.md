# Current Slug V2 Packet

Packet: `WP-6-m2-host-conversion-inputs-event-schema-correction`
Milestone: M2 authoritative target configuration
Owner: `slug-v2-subplans/06-analysis-toolchains-and-actions.md`
Result: producer-free Host-fact event-schema correction only.

## Goal

Correct `HostConversionInputs` so its facts preserve actual converter-call
identity without adding a converter, core bridge, or Host read.

## Required implementation

Add dense checked `ConverterCallId(u32)`. Give every `HomeFact` its call ID and
every Windows fact its call ID, raw UTF-16, and resolved/fallback outcome.
Require call IDs to be strictly ascending and unique within each stream; permit
duplicate Windows raw values and distinct outcomes, and permit the same call ID
in both streams. Retain optional shared AutoCPU/path-flavor/capacity facts and
publish only complete successful schedules.

## Allowed paths

- `app/slug_configuration_v2/src/native/host.rs`
- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`
- `thoughts/shared/plans/slug-v2-subplans/06-analysis-toolchains-and-actions.md`
- `thoughts/shared/plans/slug-v2-subplans/current-packet.md`

## Required tests and validation

Test dense IDs; stream ordering/duplicates; same-ID home/Windows facts;
duplicate raw Windows values with distinct outcomes; optional process facts;
Arc-backed structural equality, ordering, and hashing; and invalid streams.
Run focused configuration tests/check, GNU-Windows no-run, formatting, archive,
scope, cap, no-Cargo, and `git diff --check`.

## Stop conditions

Do not edit Cargo, core, server, converters, DICE, drivers, command/configured-
target behavior, fixtures, or generated output. Stop and REPLAN on any need for
native capture, a production driver, a core bridge, a reverse dependency, or a
cycle. Native capture and user-approved configured-target-cycle deferral remain
unchanged.

## Diff budget

- Production Rust: at most 150 net lines.
- Test Rust: at most 220 net lines.
- Documentation: at most 120 net lines.
- Total: at most 490 net lines; no Cargo, fixture, generated, baseline, or
  unrelated changes.
