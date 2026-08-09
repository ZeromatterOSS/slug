# Current Slug V2 Packet

Packet: `WP-4-8-m3-attr-five-source-template-oracle-design-retry`
Milestone: M3 query / Stage 4 executable oracle design
Owner: `slug-v2-subplans/04-starlark-loading-and-build-packages.md`
Cross-owner: `slug-v2-subplans/08-ruleset-and-command-conformance.md`
Result: freeze and validate exact five-file source and 18 command/output bytes,
incorporating all retained review corrections without changing the corrected
165-row semantic manifest.

## Background and boundary

The corrected Stage 4 record stream has 165 rows with vector
`13/7/5/4/3/3/3/6/11/12/16/3/23/5/10/16/15/10` and SHA-256
`99b772e6a8a19540ad379792fe5db7c8683d50d6e8af282ba55766585242300d`.
The independently accepted correction makes `l11_a003_no` use valid
`size=small` with computed `timeout=short`. The prior unaccepted source diff was
removed, but its review leaves four exact source obligations: paired lane-1
supports, package-derived notice licenses, lane-13 `legacy_macro` provenance,
and complete suite/manual tag closure.

## Required design

- Rebuild one authoritative LF-exact manifest for `MODULE.bazel`,
  `attr/defs.bzl`, `attr/BUILD.bazel`, `modules/ext/MODULE.bazel`, and
  `modules/ext/leaf/BUILD.bazel`, with complete bodies, byte counts, SHA-256s,
  and no root `BUILD.bazel`.
- Apply all retained fidelity corrections: lane-1 pair-specific constraint and
  toolchain supports; Bazel's package-level notice license mechanism with no
  explicit notice on package-derived operands; `l11_a003` medium/small sizes
  with computed moderate/short timeouts; one named `legacy_macro` producing the
  required Starlark and native lane-13 provenance; `tags=[suite]` on the named
  suite rows; and `tags=[manual]` on every test probe except the singleton
  implicit member.
- Freeze all 18 literal query expressions and normalized outputs, every source
  declaration/support/macro call, and the 165-ID/nine-control bijection. Keep
  test and executable attr dictionaries separate because the test base owns
  `args`.
- In each of two fresh disposable roots, run all 18 primary queries plus hidden
  focused probes that discriminate every retained correction. Exact normalized
  primary outputs must be byte-identical, list every `_yes` once and no `_no`,
  and retain `@@ext+//leaf:label` and the corrected transition allowlist.
- Refresh file hashes, total bytes/lines, and the future generation cap. Remove
  all temporary roots/output bases/lockfiles/helpers and obtain independent
  review of the full latest diff.

## Boundary and review

Edit only Stage 4 and Stage 8. Temporary sources stay outside the checkout.
Add no fixture, payload, expected record, generated source, Python, Rust,
Cargo/lockfile, BUILD, graph/DICE/regex state, configured analysis, toolchain
resolution, JVM, Java source/bytecode/helper, or production Bazel delegation.
Use pinned Bazel only as the external oracle and ordinary RC discovery without
reading or copying the private home RC.

## Stops

Stop and `REPLAN` if the corrected manifest must change, any retained review
blocker cannot be represented within five files/two packages, hidden probes or
primary outputs fail, output varies across roots, or fixture/code/JVM/configured
analysis/toolchain-resolution work is needed.
