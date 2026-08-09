# Current Slug V2 Packet

Packet: `WP-4-8-m3-attr-two-package-observable-candidate-oracle-generation`
Milestone: M3 query / Stage 4 exact oracle generation
Owner: `slug-v2-subplans/04-starlark-loading-and-build-packages.md`
Cross-owner: `slug-v2-subplans/08-ruleset-and-command-conformance.md`
Result: generate and validate the isolated Bazel 9.2 loading oracle from the
reviewed 165-record ordinary-`attr()` manifest without inference or drift.

## Background and boundary

The Stage 4 owner freezes all 165 bindings, constructor fills, support targets,
nine negative-only controls, three external null-deprecation baselines, and
exact lane-7 and test-suite discriminators. Its authoritative LF-terminated
record stream has vector
`13/7/5/4/3/3/3/6/11/12/16/3/23/5/10/16/15/10` and SHA-256
`3352106d79edef976c998b5423b2ee6686c7c5bda9540d27b66fe6e61566faf2`.
Independent correction rereview returned `ACCEPT`; generation may transcribe
that manifest but may not reinterpret it.

## Required generation

- Recompute the authoritative record count, vector, and digest before writing
  a fixture row; stop if any differs.
- Add only the new fixture TOML/expected record, the five-file canonical payload
  projection (`MODULE.bazel`, main `attr/defs.bzl` and `attr/BUILD.bazel`, local
  module/leaf BUILD files), Python derived global/projection integrity, and the
  Rust global SHA plus the accepted 275-to-285 entry-count update. Do not add a
  Rust projection.
- Produce exactly 18 query rows, 165 globally unique positive/negative pairs,
  330 probe instances, and the nine named negative-only controls. Exact stdout
  lists every `_yes` once and no `_no`; lane 7, suite closure, macro/direct
  provenance, canonical-main external loads, transition allowlist, and all
  native loading-only additions/removals follow the manifest literally.
- Keep the accepted cap: +7 payload-expanded regular files, +5 directories,
  zero links, and at most +2,400 logical lines. Preserve all fourteen existing
  projections and the isolated root-without-`BUILD.bazel` layout.
- Run update and clean replay independently with pinned Bazel 9.2; both must
  freeze `@@ext+//leaf:label`, all 18 exact rows, payload metadata/integrity,
  the protected 29-row CLI suite, and the two generated-kind CLI/server cases.

## Boundary and review

Use pinned Bazel only as the external oracle. Production remains Rust-native:
add no production Rust query projection, graph/DICE/regex state, configured
analysis, toolchain resolution, JVM, Java source/bytecode/helper, or production
Bazel delegation. Run the fixture hygiene review required at this growth
boundary, then obtain independent review of the generated evidence before
activation is scheduled.

## Stops

Stop and `REPLAN` before generation if the manifest cannot be transcribed
literally, if count/vector/digest or five-file/two-package arithmetic changes,
if another material contract correction is required, or if any row needs
configured analysis, an unbounded registry, JVM/Java, or production Bazel
delegation.
