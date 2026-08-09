# Current Slug V2 Packet

Packet: `WP-4-8-m3-attr-atomic-discriminator-manifest-design`
Milestone: M3 query / Stage 4 exact oracle design
Owner: `slug-v2-subplans/04-starlark-loading-and-build-packages.md`
Cross-owner: `slug-v2-subplans/08-ruleset-and-command-conformance.md`
Result: freeze the complete stable-ID manifest for all 165 ordinary-`attr()`
discriminator atoms before fixture generation.

## Background and boundary

The corrected vector and two-package layout are accepted, but only lane 2's
`l02_a007` has a stable exact binding. Family prose and totals do not tell a
writer which ID owns each attr spelling, regex, typed yes/no value, absence,
support target, or expected label. Earlier drafts showed that inferring those
choices during generation can yield green yet incomplete evidence.

## Required manifest

- Add one authoritative 165-row table to the Stage 4 owner. IDs are contiguous
  within vector `13/7/5/4/3/3/3/6/11/12/16/3/23/5/10/16/15/10` and use
  `lNN_aMMM` names.
- Each row freezes: query attribute spelling; anchored whole-value regex;
  positive label, exact rule class/schema, declaration/default value and
  rendered candidate; negative label/operand, exact schema/value or reason for
  absence; expected positive/negative behavior; and named support targets.
- Mark source/generated/package-group, null output/run-under, `_private`, and
  other negative-only controls explicitly. Never invent a positive pair for
  them or reuse a positive label between atoms in a lane.
- Preserve lane 2 `l02_a007` positive main/default package and negative
  `@@ext+//leaf` same-schema baseline, canonical-main `.bzl` load, lane 6 exact
  external label, lane 9 direct/macro provenance, lane 12 transition output,
  all native additions/removals, and loading-only toolchain boundary.
- Derive a deterministic manifest checksum/count summary for Stage 8 so later
  generation can prove it transcribed the reviewed table without silently
  dropping or renumbering atoms.

## Files and review

Edit only Stage 4 and Stage 8 owner plans. Read the total schema ledger, pinned
Bazel 9.2 source, stopped attempts, payload format, and consumer boundary
without edits. Add no fixture, payload, expected record, Rust, Cargo/lockfile,
BUILD, canonical plan, manifest scheduler, routing log, generated content,
JVM/Java artifact, or Bazel delegation. Obtain independent review of the full
165-row latest text before scheduling generation.

## Stops

Stop and `REPLAN` if any accepted family lacks a finite exact atom binding, if
the vector or two-package architecture must change, if a row relies on
configured analysis/unbounded registry, or if the work would activate Slug,
add graph/DICE/regex state, JVM/Java, or production Bazel delegation.
