# Current Slug V2 Packet

Packet: `WP-6-m2-label-seven-route-converter-implementation`
Milestone: M2 authoritative target configuration
Owner: `slug-v2-subplans/06-analysis-toolchains-and-actions.md`
Result: private extension for six literal-default routes and one exact
label-to-string route.

## Goal

Extend the existing private label converter with `host_platform`, five Proto
literal defaults, and `LabelToStringEntry`; retain `LabelMap` and `FlagAlias`
as Unsupported.

## Required design record

Use the existing `OptionLabelContext`/mapping-free `ResolvedOptionLabel` only.
Table the six exact literals and their empty/default behavior: host empty uses
its host-platform default; three Proto EmptyToNull empty inputs are `None`; two
Proto Label empty inputs use ordinary parsing. `LabelToStringEntry` accepts one
`=`, nonempty sides, context-parsed lhs, and exact untrimmed `CompactString`
rhs. The source's fixed delimiter diagnostic is recorded in the terminal
evidence; this private kernel returns `LabelConvertError::Invalid` for each
delimiter-shape failure, and user-facing diagnostic projection remains deferred.
No mapping retention or cache/normalization is allowed. User-approved
configured-target cycles remain deferred.

## Allowed paths

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`, terminal only
- `thoughts/shared/plans/slug-v2-subplans/06-analysis-toolchains-and-actions.md`, terminal only
- `thoughts/shared/plans/slug-v2-subplans/current-packet.md`, terminal only
- `app/slug_configuration_v2/src/native/label_convert.rs`
- `app/slug_configuration_v2/src/native/tests.rs`

## Required tests and validation

Test the exact seven/two partition, all contexts/defaults/empty cases, exact
literal bytes, delimiter matrix/RHS whitespace, map/alias Unsupported, and Arc/
`Allocative` behavior where applicable. Run focused test/check, GNU-Windows
no-run, formatting, archive, scope, cap, and diff gates.

## Stop conditions

Stop on external grammar, mixed/terminal-Host/regex work, a new dependency/context/public
API, map/interner/cache, command/loading/DICE/normalization/checksum/wire/
configured-target work, outside files, or a cap. Do not edit Cargo, `mod.rs`,
identity, registry, convert/defaults/value/cache, or create fixtures/probes/
artifacts.

## Diff budget

- Production: 240; tests: 420; documentation: 100; total formatted net: 760.
  No Cargo, fixture, generated, baseline, or unrelated changes.
