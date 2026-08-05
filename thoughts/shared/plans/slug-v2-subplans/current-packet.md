# Current Slug V2 Packet

Packet: `WP-6-m2-run-under-and-custom-flag-source-closure-evidence`
Milestone: M2 authoritative target configuration
Owner: `slug-v2-subplans/06-analysis-toolchains-and-actions.md`
Result: pinned source-only closure and bounded-successor decision for the two
remaining command-tokenized routes.

## Goal

Pin Bazel 9.2 RunUnder converter/value/default/error/rendering and its
source-equivalent ShellUtils tokenization/original-suffix/context split. Pin
CustomFlag raw-define versus label `/...` canonicalization/default/error/context,
then decide a bounded successor.

## Required design record

Use source evidence only; preserve the distinction between conversion and later
command activation, normalization, loader, checksum, wire, DICE, and configured
target behavior. User-approved configured-target-cycle deferral remains explicit.

## Allowed paths

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`
- `thoughts/shared/plans/slug-v2-subplans/06-analysis-toolchains-and-actions.md`
- `thoughts/shared/plans/slug-v2-subplans/current-packet.md`

## Required tests and validation

Record pinned source provenance and exact behavior discriminators. Run source,
archive, scope, cap, no-Cargo, and `git diff --check` gates.

## Stop conditions

Stop with REPLAN on a JVM need, unclosed tokenization/grammar, new context or
loader, reverse edge/cycle, or command ownership ambiguity. Do not edit Rust,
Cargo, fixtures, or create probes/artifacts, or implement command/loading/DICE/
normalization/checksum/wire/configured-target behavior.

## Diff budget

- Documentation and total: at most 220 net lines. No Rust, Cargo, fixture,
  generated, baseline, or unrelated changes.
