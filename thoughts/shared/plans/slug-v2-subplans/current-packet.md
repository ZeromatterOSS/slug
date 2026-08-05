# Current Slug V2 Packet

Packet: `WP-6-m2-label-only-30-route-converter-implementation`
Milestone: M2 authoritative target configuration
Owner: `slug-v2-subplans/06-analysis-toolchains-and-actions.md`
Result: private supplied-context converter for the closed 30-route cohort.

## Goal

Implement only the accepted fact-independent 30 label routes with a supplied
`OptionLabelContext`; do not activate command or configuration behavior.

## Required design record

Add a private configuration API that accepts the existing `OptionLabelContext`
and retains only `ResolvedOptionLabel`. It may handle ordinary `Label` (16),
`EmptyToNullLabel` (5), `LabelList` (6), `LabelOrderedSet` (1), and `LibcTop`
(2) routes and their literal/null/empty defaults. Mapping is borrowed for the
call, never retained. Conversion must finish before any list/set normalization.

The implementation may add only the forward, acyclic configuration-to-identity
dependency. It authorizes no Host/capture, command, loading, DICE,
normalization, checksum, wire, configured-target, source lookup, or new-context
work. The user-approved configured-target-cycle deferral remains unchanged.

## Allowed paths

- `app/slug_configuration_v2/Cargo.toml`
- `app/slug_configuration_v2/src/native/mod.rs`
- `app/slug_configuration_v2/src/native/label_convert.rs` (new)
- `app/slug_configuration_v2/src/native/tests.rs`
- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`, only if terminal
  disposition requires it
- `thoughts/shared/plans/slug-v2-subplans/06-analysis-toolchains-and-actions.md`,
  only if terminal disposition requires it
- `thoughts/shared/plans/slug-v2-subplans/current-packet.md`, only if terminal
  disposition requires it

## Required tests and validation

Test exact 30/9/2/5 membership plus eight regex routes; all three contexts with
mapped/non-visible labels; list order/empty omission; ordered-set convert-all,
first-wins, and late-invalid atomic error; every admitted default; LibcTop
`default` and `//`-to-`everything`; Eq/Ord/Allocative/Arc `Dupe`; and no
normalization/cache/wire/loader behavior. Run focused tests/check, GNU-Windows
no-run, formatting, archive, scope, cap, no-Cargo-lock, and diff gates.

## Stop conditions

Stop on a symbolic, composite, mixed, or Host route; a new context; public API;
mapping retention; map/interner/cache/global; reverse edge/cycle; command,
loading, DICE, normalization, checksum, wire, or configured-target work; any
outside file or cap; or a Cargo.lock/root/identity/registry/convert/defaults/
value/cache edit. Do not create probes or artifacts.

## Diff budget

- Production Rust: at most 320 net lines.
- Test Rust: at most 600 net lines.
- Documentation: at most 100 net lines.
- Total: at most 920 formatted net lines; no Cargo.lock, root, identity,
  registry, convert, defaults, value, cache, fixture, generated, baseline, or
  unrelated changes.
