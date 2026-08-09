# Current Slug V2 Packet

Packet: `WP-4-8-m3-attr-observable-candidate-oracle-design`
Milestone: M3 query / Stage 4 loading prerequisite
Owner: `slug-v2-subplans/04-starlark-loading-and-build-packages.md`
Cross-owner: `slug-v2-subplans/08-ruleset-and-command-conformance.md`
Result: design the focused Bazel evidence that closes every observable `attr`
candidate combination, formatting, and total current-native attribute boundary.

## Background and boundary

Pinned Bazel 9.2 source proves candidate traversal order, default position, and
duplicate-candidate multiplicity are internal and unobservable: `attr()` emits
only a target when any candidate matches. Whole typed candidate values remain
observable. V2 loses their non-label shape and some list/map order before
`QueryAttribute`, and it has no total projection for native rules or universal
`name`. This packet designs evidence only. It does not generate the fixture,
change representation or graph breadth, or activate `attr`.

## Required oracle design

- Select one existing query fixture if it can isolate the cases without copied
  scaffolding; otherwise justify one smallest new fixture.
- Freeze paired positive/negative `attr` rows for one selector; equal-key-set
  correlation; distinct and overlapping-key-set cross-products; string/list
  typed concatenation; duplicate elements inside one typed value; all admitted
  dict orientations; pre-normalization list/map order; empty/null/effective
  schema defaults; `$implicit`; universal `name`; main/external canonical label
  leaves; and every retained attribute of every currently loadable native rule.
- Give the matrix exact wrong-algorithm discriminators for branch union versus
  typed combination, equal-set correlation versus cross-product, post-sort
  formatting, apparent versus canonical external labels, null-as-empty, and
  Starlark-only rule filtering. State explicitly that no row may claim candidate
  position or equal-candidate multiplicity.
- Inventory `filegroup`, `alias`, `config_setting`, `test_suite`,
  `constraint_setting`, `constraint_value`, `platform`, `toolchain_type`, and
  `toolchain` as rules; prove `package_group`, source/BUILD/exported files, and
  generated files are excluded. Isolate the existing native-toolchain query-
  graph rejection as a later prerequisite rather than silently omitting it.
- Pin command order, exact stdout/stderr/exit, immutable Bazel 9.2 provenance,
  fixture mutation/cleanup, expected row count, file allowlist, and bounded
  growth. Reuse leaf-format source facts rather than adding nondiscriminating
  rows.
- Freeze the later Stage 4 typed capture point before value-internal order
  normalization and the later Stage 8 request-local existential early-exit
  traversal, but authorize neither. The representation need not retain
  candidate order or duplicate equal candidates.

## Files

Edit only the Stage 4 and Stage 8 owner plans. Read existing fixtures/harness,
pinned Bazel source, and current loading/query representations without edits.
Add no fixture, oracle record, Rust, Cargo/lockfile, BUILD, canonical-plan,
manifest, or routing-log change during this design packet. Obtain independent
fixture/evidence review before scheduling generation.

## Stops

Stop and `REPLAN` if observable correlation or value-internal ordering cannot
be isolated, if the current native inventory is not finite, if a row requires
configuration analysis rather than loading-query aggregation, or if the
evidence would add a Java helper/artifact, JVM integration, bytecode,
production Bazel delegation, query-time filesystem/Starlark reads, a DICE key,
regex redesign, or cquery/aquery breadth. Bazel remains an external oracle only.
