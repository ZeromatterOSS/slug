# Current Slug V2 Packet

Packet: `WP-4-8-m3-attr-candidate-order-oracle-design`
Milestone: M3 query / Stage 4 loading prerequisite
Owner: `slug-v2-subplans/04-starlark-loading-and-build-packages.md`
Cross-owner: `slug-v2-subplans/08-ruleset-and-command-conformance.md`
Result: design the focused Bazel evidence that closes `attr` candidate order,
selector correlation, and total current-native attribute inventory.

## Background and boundary

Pinned Bazel 9.2 source closes leaf formatting, but the prior representation
design reached `REPLAN`: V2 loses selector-default position and some pre-sort
order before `QueryAttribute`, and it has no total typed projection for native
rules or the universal `name` attribute. This packet designs evidence only. It
does not generate the fixture, change representation, or activate `attr`.

## Required oracle design

- Select one existing query fixture if it can isolate the cases without copied
  scaffolding; otherwise justify one smallest new fixture.
- Freeze exact successful `attr` rows for default branch first/middle/last;
  one selector; concatenated selectors with equal key sets; concatenated
  selectors with distinct/overlapping key sets; string and list typed
  concatenation; duplicate candidates/elements; all admitted dict orientations;
  empty/null/effective schema defaults; `$implicit` lookup; universal `name`;
  main/external canonical labels; and every currently admitted native rule.
- Give every row a wrong-algorithm discriminator for branch-union versus typed
  combination, default-appended order, candidate dedup, post-sort formatting,
  apparent versus canonical external labels, and Starlark-only rule filtering.
- Pin command order, exact stdout/stderr/exit, immutable Bazel 9.2 provenance,
  fixture mutation/cleanup, expected row count, file allowlist, and bounded
  growth. Reuse leaf-format source facts rather than adding nondiscriminating
  rows.
- Freeze the later Stage 4 capture point before order normalization and the
  later Stage 8 request-local early-exit traversal, but authorize neither.

## Files

Edit only the Stage 4 and Stage 8 owner plans. Read existing fixtures/harness,
pinned Bazel source, and current loading/query representations without edits.
Add no fixture, oracle record, Rust, Cargo/lockfile, BUILD, canonical-plan,
manifest, or routing-log change during this design packet. Obtain independent
fixture/evidence review before scheduling generation.

## Stops

Stop and `REPLAN` if exact ordering/correlation cannot be isolated, if the
current native inventory is not finite, if a row requires configuration
analysis rather than loading-query aggregation, or if the evidence would add a
Java helper/artifact, JVM integration, bytecode, production Bazel delegation,
query-time filesystem/Starlark reads, a DICE key, regex redesign, or
cquery/aquery breadth. Bazel remains an external oracle only.
