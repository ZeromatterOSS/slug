# Current Slug V2 Packet

Packet: `WP-4-8-m3-attr-five-source-template-oracle-design`
Milestone: M3 query / Stage 4 executable oracle design
Owner: `slug-v2-subplans/04-starlark-loading-and-build-packages.md`
Cross-owner: `slug-v2-subplans/08-ruleset-and-command-conformance.md`
Result: freeze and independently validate the exact five source bodies and 18
query argv/stdout bindings needed to generate the reviewed ordinary-`attr()`
oracle without source-level inference.

## Background and boundary

The corrected Stage 4 record stream freezes all 165 semantic bindings,
constructor fills, supports, negative-only controls, and discriminators. It has
vector
`13/7/5/4/3/3/3/6/11/12/16/3/23/5/10/16/15/10` and SHA-256
`8ae8899e0debb42369bc6453e4f1aad7b3cbca9940aa563993a3db35eca1ff9e`.
The focused transition-allowlist correction is independently accepted.
Generation remains blocked because semantic shorthand such as
`select(same_keys=...)` does not freeze complete rule definitions, support
declarations, selector dictionaries, macro bodies/locations, or source bytes.

## Required design

- Add to the Stage 4 owner one authoritative LF-exact source-template manifest
  for `MODULE.bazel`, `attr/defs.bzl`, `attr/BUILD.bazel`,
  `modules/ext/MODULE.bazel`, and `modules/ext/leaf/BUILD.bazel`. Freeze each
  complete body, byte count, SHA-256, and role; retain no root `BUILD.bazel`.
- Freeze every callable signature and attr declaration, mandatory support,
  selector dictionary/default, identity transition, legacy macro body/call
  placement, native declaration, package default, and negative-only producer.
  Test and executable rule attr dictionaries remain separate because Bazel's
  test base already owns `args`. Map every exact source declaration back to the
  unchanged semantic manifest.
- Freeze literal 18 Bazel argv expressions and normalized stdout records, with
  every `_yes` exactly once and no `_no`; audit every record ID and each of the
  nine controls to one declaration and command.
- Materialize the proposed bodies only in temporary directories and run pinned
  Bazel 9.2 with ordinary RC discovery. All 18 commands must pass in two
  distinct roots, reproduce `@@ext+//leaf:label`, and yield byte-identical
  normalized records before acceptance.
- Recalculate the eventual fixture/TOML/expected logical-line cap from the
  exact templates. Preserve five files/five directories, no links/mutations,
  the isolated fixture name, and absence from every Slug projection/consumer.

## Boundary and review

Edit only the Stage 4 and Stage 8 owner plans. Temporary source material and
Bazel outputs remain outside the checkout and are removed after validation.
Add no fixture, payload, expected record, generated source, Python, Rust,
Cargo/lockfile, BUILD, graph/DICE/regex state, configured analysis, toolchain
resolution, JVM, Java source/bytecode/helper, or production Bazel delegation.
Use pinned Bazel only as the external oracle and obtain independent review of
the complete templates and ID/declaration/command audit.

## Stops

Stop and `REPLAN` if the corrected semantic manifest must change, exact bodies
cannot stay within five files/two packages, a mapping is not bijective or a
command is nondiscriminating, temp-oracle output varies across roots, or any row
needs configured analysis, an unbounded registry, JVM/Java, or production Bazel
delegation.
