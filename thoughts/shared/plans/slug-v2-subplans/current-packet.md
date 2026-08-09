# Current Slug V2 Packet

Packet: `WP-4-8-m3-attr-transition-allowlist-manifest-correction`
Milestone: M3 query / Stage 4 exact manifest correction
Owner: `slug-v2-subplans/04-starlark-loading-and-build-packages.md`
Cross-owner: `slug-v2-subplans/08-ruleset-and-command-conformance.md`
Result: correct only the disproved `l12_a003` transition-allowlist label,
recompute the authoritative manifest digest, and return to exact source-template
design without changing the 165-row semantic family.

## Background and boundary

The Stage 4 record stream has 165 rows with vector
`13/7/5/4/3/3/3/6/11/12/16/3/23/5/10/16/15/10` and SHA-256
`3352106d79edef976c998b5423b2ee6686c7c5bda9540d27b66fe6e61566faf2`.
Disposable five-source synthesis proved the Starlark loads and then exposed one
manifest error: ordinary Bazel 9.2 query renders the admitted transition's
`$allowlist_function_transition` as
`@@bazel_tools//tools/allowlists/function_transition_allowlist:function_transition_allowlist`.
The frozen shorter label `@@bazel_tools//tools/allowlists:function_transition_allowlist`
selects nothing under its anchored regex. The source-template packet stopped
and removed all temporary material before checkout edits.

## Required correction

- Change only `l12_a003`'s anchored regex, positive rendered value, and support
  label to the exact observed canonical label above. Preserve its ID, attr
  spelling, rule classes, transition output, negative operand, and expectation.
- Re-extract all 165 LF-terminated records, prove the vector and unique IDs are
  unchanged, calculate the new SHA-256, and update every current Stage 4/8
  checksum reference. Do not alter another record.
- Record the exact Bazel 9.2 positive query evidence and the old-regex empty
  result. No new oracle is needed; reuse the accepted disposable-root evidence.
- Obtain one independent latest-diff review that the correction is exactly one
  row and preserves the five-file/two-package/source-template boundary.

## Boundary and review

Edit only the Stage 4 and Stage 8 owner plans. Add no fixture, payload, expected
record, generated source, Python, Rust, Cargo/lockfile, BUILD, source-template
body, graph/DICE/regex state, configured analysis, toolchain resolution, JVM,
Java source/bytecode/helper, or production Bazel delegation.

## Stops

Stop and `REPLAN` if any record other than `l12_a003` must change, if the vector
or two-package/five-file boundary changes, if the evidence does not distinguish
the old and new anchored labels, or if the correction needs fixture/code/JVM/
configured-analysis work.
