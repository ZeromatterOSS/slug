# Current Slug V2 Packet

Packet: `WP-4-8-m3-attr-five-source-template-oracle-design`
Milestone: M3 query / Stage 4 executable oracle design
Owner: `slug-v2-subplans/04-starlark-loading-and-build-packages.md`
Cross-owner: `slug-v2-subplans/08-ruleset-and-command-conformance.md`
Result: freeze and independently validate the exact five source bodies and 18
query argv/stdout bindings needed to generate the reviewed ordinary-`attr()`
oracle without source-level inference.

## Background and boundary

The accepted Stage 4 record stream still freezes all 165 semantic bindings,
constructor fills, supports, negative-only controls, and discriminators. It has
vector
`13/7/5/4/3/3/3/6/11/12/16/3/23/5/10/16/15/10` and SHA-256
`3352106d79edef976c998b5423b2ee6686c7c5bda9540d27b66fe6e61566faf2`.
Generation preflight nevertheless stopped before writes: fields such as
`select(same_keys=...)` are semantic shorthand, not valid Starlark, and the
manifest does not freeze complete rule definitions, support declarations,
selector dictionaries, macro bodies/locations, or source-file bytes. Filling
those gaps during generation would violate the no-inference contract.

## Required design

- Add to the Stage 4 owner one authoritative LF-exact source-template manifest
  for `MODULE.bazel`, `attr/defs.bzl`, `attr/BUILD.bazel`,
  `modules/ext/MODULE.bazel`, and `modules/ext/leaf/BUILD.bazel`. Freeze each
  complete body, byte count, SHA-256, and its role; retain no root `BUILD.bazel`.
- Freeze every callable signature, attr declaration, mandatory support target,
  selector key/value/default dictionary, identity-transition implementation,
  legacy macro body and call placement, native declaration, package default,
  and negative-only producer. The semantic 165-record manifest remains
  unchanged and must map bijectively to the exact declarations.
- Freeze the literal 18 Bazel argv expressions and normalized stdout records,
  with every `_yes` exactly once and no `_no`; include a deterministic audit
  tying every record ID and each of the nine controls to a source declaration
  and one command.
- Materialize the proposed five bodies only in a temporary directory and run
  pinned Bazel 9.2 with ordinary RC discovery. All 18 commands must pass in two
  distinct roots, reproduce `@@ext+//leaf:label`, and yield byte-identical
  normalized records before the templates can be accepted.
- Recalculate the eventual fixture/TOML/expected logical-line cap from the
  exact templates. Preserve five files/five directories, no links/mutations,
  the isolated fixture name, and absence from every Slug projection/consumer.

## Boundary and review

Edit only the Stage 4 and Stage 8 owner plans. Temporary source material and
Bazel outputs must remain outside the checkout and be removed after validation.
Add no fixture, payload, expected record, generated source, Python, Rust,
Cargo/lockfile, BUILD, graph/DICE/regex state, configured analysis, toolchain
resolution, JVM, Java source/bytecode/helper, or production Bazel delegation.
Use pinned Bazel only as the external oracle and obtain independent review of
the complete source templates and ID/declaration/command audit.

## Stops

Stop and `REPLAN` if the semantic record manifest must change, if exact source
bodies cannot stay within the five-file/two-package boundary, if a mapping is
not bijective or a command is nondiscriminating, if temp-oracle output is not
stable across roots, or if any row needs configured analysis, an unbounded
registry, JVM/Java, or production Bazel delegation.
