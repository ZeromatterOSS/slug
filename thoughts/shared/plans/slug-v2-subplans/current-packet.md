# Current Slug V2 Packet

Packet: `WP-4-8-m3-attr-five-source-template-oracle-design-retry-2`
Milestone: M3 query / Stage 4 executable oracle design
Owner: `slug-v2-subplans/04-starlark-loading-and-build-packages.md`
Cross-owner: `slug-v2-subplans/08-ruleset-and-command-conformance.md`
Result: freeze and validate exact five-file source and 18 command/output bytes,
including every reviewed hidden construction without changing the accepted
165-row semantic manifest.

## Background and boundary

The accepted manifest has 165 rows with vector
`13/7/5/4/3/3/3/6/11/12/16/3/23/5/10/16/15/10` and SHA-256
`99b772e6a8a19540ad379792fe5db7c8683d50d6e8af282ba55766585242300d`.
Pinned source and reviewed Bazel 9.2 matrix evidence establish the exact
package-license construction: retain `default_package_metadata`, add BUILD-only
`licenses(["notice"])`, and omit explicit notice only on the six named
package-derived filegroups. Starlark normal remains attr-absent and
`config_setting` remains `[none]`. The earlier empty result was an over-escaped
regex, not a Bazel semantic boundary.

## Required design

- Freeze complete LF-exact bodies, byte counts, SHA-256s, and roles for only
  `MODULE.bazel`, `attr/defs.bzl`, `attr/BUILD.bazel`,
  `modules/ext/MODULE.bazel`, and `modules/ext/leaf/BUILD.bazel`; no root BUILD.
- Apply every accepted construction: pair-specific lane-1 supports; one
  package-level `licenses(["notice"])` and no explicit notice on exactly
  `l02_a005_yes`, `l02_a006_no`, `l09_a005_yes`, `l13_a017_yes`,
  `l14_a003_yes`, `l15_a002_no`; medium/small computed timeout; one named
  `legacy_macro` for Starlark/native provenance; suite tags; all-other-test
  manual tags; separate executable/test attr dictionaries; corrected transition
  allowlist and external baselines.
- Freeze 18 literal primary expressions and normalized outputs, complete source
  declarations and supports, plus the 165-ID/nine-control bijection.
- In two fresh disposable roots/output bases, run all primary commands and
  focused hidden probes for lane-1 supports, license provenance, computed
  timeout, macro function, and tag closure. Primary outputs must be identical,
  contain every `_yes` once/no `_no`, and retain canonical external/tool labels.
- Refresh exact file/total bytes and lines and future generation cap. Remove all
  temporary roots/outputs/lockfiles/helpers and obtain full independent review.

## Boundary and review

Edit only Stage 4 and Stage 8. Temporary source material remains outside the
checkout. Add no fixture, payload, expected record, generated source, Python,
Rust, Cargo/lockfile, BUILD, graph/DICE/regex state, configured analysis,
toolchain resolution, JVM, Java source/bytecode/helper, or production Bazel
delegation. Use ordinary RC discovery without reading/copying the private RC.

## Stops

Stop and `REPLAN` if the accepted semantic manifest must change, any reviewed
construction fails a focused probe, five files/two packages are insufficient,
the two roots differ, or fixture/code/JVM/configured-analysis/toolchain-resolution
work is needed.
