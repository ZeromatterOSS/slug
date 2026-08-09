# Current Slug V2 Packet

Packet: `WP-4-8-m3-attr-two-package-observable-candidate-oracle-design`
Milestone: M3 query / Stage 4 loading evidence design
Owner: `slug-v2-subplans/04-starlark-loading-and-build-packages.md`
Cross-owner: `slug-v2-subplans/08-ruleset-and-command-conformance.md`
Result: design the smallest isolated two-package Bazel 9.2 ordinary-`attr()`
fixture that preserves the corrected 165-atom discriminator ledger.

## Background and boundary

The isolated generation draft was removed before Bazel ran. Pinned Bazel 9.2
proves `deprecation` is a computed package default and explicit Starlark `None`
does not suppress it. A single `//attr` package cannot expose both lane 9's
package-derived `deprecation="deprecated"` and lane 2's same-schema null
control. The corrected 18-lane vector remains
`13/7/5/4/3/3/3/6/11/12/16/3/23/5/10/16/15/10`, totaling 165 pairs; no
positive reuse or semantic conflation is allowed.

## Required design

- Select the smallest isolated layout with one positive-default package and
  one baseline package. Prove whether the existing external leaf can honestly
  host the baseline; otherwise admit exactly one additional main-repository
  BUILD file. Preserve macro-location, base-setting/transition, native-class,
  nonrule, and canonical external evidence.
- Remap every affected pair label and exact stdout without changing the 165
  semantic atoms or 18 commands. Lane 2's null-deprecation target must retain
  the same rule schema as its positive target; an attribute-removal class is
  not a substitute.
- Recalculate exact virtual file/directory/encoded-entry totals, aggregate
  payload arithmetic, physical plus expanded fixture growth, and the logical
  line cap from hygiene reset `51540963`.
- Reprove from the actual consumer call graph that the unique fixture remains
  absent from Rust `PROJECTIONS` and every CLI/server semantic case. Retain
  exact anchored `@@ext+//leaf:label`, loading-only native toolchain evidence,
  and the permanent Rust-native/no-JVM boundary.

## Files

Edit only the Stage 4 and Stage 8 owner plans. Read pinned Bazel source, the
corrected ledger, stopped-generation record, payload format, and fixture
consumers without edits. Add no fixture, payload, expected record, Rust,
Cargo/lockfile, BUILD, canonical-plan, manifest, routing-log, generated content,
JVM/Java artifact, or production Bazel delegation during this design packet.
Obtain independent oracle-design review before generation.

## Stops

Stop and `REPLAN` if the baseline cannot preserve same-schema null semantics,
if isolation requires a Slug projection/consumer, if the corrected ledger
needs another semantic change, or if the design requires configured analysis,
an unbounded registry, production Rust/graph/DICE/regex changes, JVM/Java, or
Bazel semantic delegation.
