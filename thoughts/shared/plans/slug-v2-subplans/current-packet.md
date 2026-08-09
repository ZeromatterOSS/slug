# Current Slug V2 Packet

Packet: `WP-4-8-m3-attr-test-timeout-manifest-correction`
Milestone: M3 query / Stage 4 exact manifest correction
Owner: `slug-v2-subplans/04-starlark-loading-and-build-packages.md`
Cross-owner: `slug-v2-subplans/08-ruleset-and-command-conformance.md`
Result: correct only `l11_a003`'s invalid negative test-size source, recompute
the authoritative manifest digest, and preserve the reviewed source-template
blockers for the next executable-design retry.

## Background and boundary

The Stage 4 record stream has 165 rows with vector
`13/7/5/4/3/3/3/6/11/12/16/3/23/5/10/16/15/10` and SHA-256
`8ae8899e0debb42369bc6453e4f1aad7b3cbca9940aa563993a3db35eca1ff9e`.
The first exact source-template diff passed all 18 primary commands in two
roots, but independent review found five hidden construction mismatches. During
the one allowed correction, Bazel 9.2 rejected the frozen `l11_a003_no`
construction `size="short"` with `size 'short' is not a valid size` and computed
`timeout 'illegal' is not a valid timeout`. Bazel's computed timeout `short`
comes from valid size `small`; changing the source template alone would violate
the semantic row. The complete unaccepted template diff and temporary roots
were removed.

## Required correction

- Change only `l11_a003`'s negative operand from `size=short,timeout=short` to
  `size=small,timeout=short`. The timeout remains a computed default; preserve
  the ID, attr spelling, regex, positive `size=medium,timeout=moderate`, labels,
  rule classes, expectation, vector, and all other records.
- Re-extract all 165 LF records, calculate the new SHA-256, and update every
  current Stage 4/8 checksum reference. Prove unique IDs and the lane vector are
  unchanged.
- Record the exact Bazel 9.2 invalid-size diagnostic and the valid
  `size=small` computed-timeout positive evidence. Obtain independent review of
  the one-row latest diff.
- Preserve for the source-template retry all four other review corrections:
  paired lane-1 supports; package-derived notice licenses; `legacy_macro`
  generator provenance for native lane-13 negatives; and exact suite/manual
  tag closure. Those are source-template obligations, not changes in this
  packet.

## Boundary and review

Edit only the Stage 4 and Stage 8 owner plans. Add no fixture, payload, expected
record, source template, Python, Rust, Cargo/lockfile, BUILD, graph/DICE/regex
state, configured analysis, toolchain resolution, JVM, Java
source/bytecode/helper, or production Bazel delegation.

## Stops

Stop and `REPLAN` if any record other than `l11_a003` must change, if `small`
does not produce computed timeout `short`, if the vector or five-file/two-package
architecture changes, or if fixture/code/JVM/configured-analysis work is needed.
