# Current Slug V2 Packet

Packet: `WP-6-m2-label-map-and-flag-alias-converter-implementation`
Milestone: M2 authoritative target configuration
Owner: `slug-v2-subplans/06-analysis-toolchains-and-actions.md`
Result: private exact 39/0 label-route converter extension.

## Goal

Implement LabelMap and FlagAlias while retaining their exact source-closed
conversion scope. Do not activate downstream normalization or command aliases.

## Required design record

Extend only the private converter with exact 25-character trim logic, without a
Guava/JDK/regex/dependency. LabelMap retains an Arc ordered slice of
`(CompactString, Option<ResolvedOptionLabel>)`; FlagAlias retains each
unnormalized `(CompactString, ResolvedOptionLabel)` occurrence. Derive
`Allocative` and use `Dupe` only on Arc wrappers. Use the existing
`OptionLabelContext` and mapping-free labels; retain no map/cache/interner or
downstream aggregation. Preserve validation order, but return private `Invalid`;
user-facing diagnostics remain deferred. Keep LabelMap/FlagAlias normalization,
command expansion, loader behavior, and user-approved configured-target cycles
untouched.

## Allowed paths

- `app/slug_configuration_v2/src/native/label_convert.rs`
- `app/slug_configuration_v2/src/native/tests.rs`
- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`, terminal only
- `thoughts/shared/plans/slug-v2-subplans/06-analysis-toolchains-and-actions.md`, terminal only
- `thoughts/shared/plans/slug-v2-subplans/current-packet.md`, terminal only

## Required tests and validation

Test all three contexts/defaults/malformed inputs, Unicode trim/order/duplicate
behavior, ASCII `\w`, prefix gate, mapping/non-visible labels, and the exact
39/0 partition. Run focused test/check, GNU-Windows no-run, formatting, archive,
scope, cap, and diff gates.

## Stop conditions

Stop on external grammar, mixed/Host/regex work, new dependency/context/public
API, map/interner/cache, command/loading/DICE/normalization/checksum/wire/
configured-target work, an outside file, or cap. Do not edit Cargo, `mod.rs`,
identity, registry, convert/defaults/value/cache, or create fixtures/probes/
artifacts.

## Diff budget

- Production: 280; tests: 440; documentation: 100; total formatted net: 820.
  No Cargo, fixture, generated, baseline, or unrelated changes.
