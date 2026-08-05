# Current Slug V2 Packet

Packet: `WP-6-m2-repository-label-conversion-route-split-design`
Milestone: M2 authoritative target configuration
Owner: `slug-v2-subplans/06-analysis-toolchains-and-actions.md`
Result: docs-only label-route split and bounded successor decision.

## Goal

Detach the 41 supplied repository/package-label routes from the terminal five
Host routes, then decide the bounded label-only converter successor.

## Required design record

Use the accepted `LabelConversionContext` and `ResolvedOptionLabel` boundary to
partition the 41 supplied label routes without adding a context, loader, or
Host/capture dependency. Preserve conversion before normalization and decide
whether a bounded label-only converter can own the successor.

This is design-only. It authorizes no Rust, Cargo, fixtures, source lookup,
Host/capture, DICE, command, normalization, checksum, wire, or configured-target
work. Stop with REPLAN on a Host/capture dependency, a new context or loader,
a reverse edge/cycle, or violation of conversion-before-normalization. The
user-approved configured-target-cycle deferral remains unchanged.

## Allowed paths

- `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`
- `thoughts/shared/plans/slug-v2-subplans/06-analysis-toolchains-and-actions.md`
- `thoughts/shared/plans/slug-v2-subplans/current-packet.md`

## Required tests and validation

Record the 41-versus-five route split and successor decision. Run archive,
scope, cap, no-Cargo, and `git diff --check` gates.

## Stop conditions

Do not edit Rust or Cargo, create probes/artifacts, add source discovery,
Host/capture, DICE, command, normalization, checksum, wire, configured-target
behavior, fixtures, or generated output. Stop and REPLAN on the listed boundary
violations.

## Diff budget

- Documentation: at most 140 net lines.
- Total: at most 140 net lines; no Rust, Cargo, fixture, generated, baseline,
  or unrelated changes.
