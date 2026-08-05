# Current Slug V2 Packet

Packet: `WP-6-m2-fixed-regex-default-seed-implementation`
Milestone: M2 authoritative target configuration
Owner: `slug-v2-subplans/06-analysis-toolchains-and-actions.md`
Result: exact private value/default/cache seeds for the three annotated
RegexFilter defaults without admitting a general regex route.

## Goal

Materialize exactly the two `-.*` defaults and the one
`-/javatests[/:],-/test/java[/:]` default as finite private RegexFilter seeds.
Retain raw original input separately from semantic equality, render the exact
canonical generated patterns, and leave every explicit regex occurrence
Unsupported.

## Required design record

Use `RegexFilterDefaultSeed { original_input, semantic }`, where the finite
semantic discriminator alone owns equality/order/cache identity. `ExcludeAll`
renders `-(?:(?>.*))`; `InstrumentationDefault` renders
`-(?:(?>/javatests[/:])|(?>/test/java[/:]))`. Derive `Allocative`; retain owned
`CompactString`; add no Arc, Dupe, interner, map, cache storage/integration, or
regex dependency.

## Allowed paths

- `app/slug_configuration_v2/src/native/value.rs`
- `app/slug_configuration_v2/src/native/defaults.rs`
- `app/slug_configuration_v2/src/native/cache_grammar.rs`
- `app/slug_configuration_v2/src/native/tests.rs`
- the canonical, Stage 6, and current-packet scheduling documents for terminal
  disposition only

## Required tests and validation

Test exact three-descriptor selection; original retention separate from
semantic equality/order; both exact scalar cache texts and outer escaping; all
explicit values remaining Unsupported; the unchanged Runs seed/cache and
287/8/5/41 partition; `Allocative`; and forbidden source surfaces. Run focused
crate tests/check, GNU-Windows tests check, formatting, archive, scope, cap,
no-Cargo, and diff gates.

## Stop conditions

Stop with REPLAN on dynamic RegexFilter construction, arbitrary explicit input,
Pattern/Matcher/compiler/matching/diagnostics, public reversal or predicate
activation, coverage replacement, normalization/configuration/checksum/DICE,
Host/context/loader/wire ownership, a new dependency/utility, or equality that
requires original input. Do not edit registry, convert, Cargo/lockfiles,
fixtures, generated data, or create probes/artifacts.

## Diff budget

- Production Rust: at most 150 net lines.
- Test Rust: at most 240 net lines.
- Documentation: at most 100 net lines.
- Total: at most 490 formatted net lines.
