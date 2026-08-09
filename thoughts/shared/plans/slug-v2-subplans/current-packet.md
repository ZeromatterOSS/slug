# Current Slug V2 Packet

Packet: `WP-8-m3-rust-native-regex-contract-design`
Milestone: M3 query
Owner: `slug-v2-subplans/08-ruleset-and-command-conformance.md`
Result: freeze the Rust-native regex boundary before query activation.

## Boundary

This is documentation and source audit only. Select and pin the existing
workspace Rust regex substrate, then define one explicit Slug-native
valid-Unicode contract. Java `Pattern` syntax edges, UTF-16 behavior, lone
surrogates, and exact Java diagnostics are deliberate non-goals under the
accepted Rust-only compatibility reset.

Preserve the exact Bazel 9.2 behavior around the matcher: `filter` searches
the printed label; `kind` searches the retained target-kind string; each
pattern compiles once; matching uses find/search rather than implicit full
anchoring; and operand evaluation, ordering, delivery, and existing graph
semantics remain unchanged.

## Required design

- Audit the locked crate source, version, enabled features, Unicode behavior,
  compile/search API, deterministic complexity controls, size limits, and
  error surface. Do not add or update a dependency in this packet.
- Freeze the admitted pattern syntax and valid-Unicode subject model,
  including literals, anchors, character classes, Unicode classes/case,
  repetition, grouping, alternation, escaping, and inline flags.
- Define stable compile diagnostics and explicit resource-limit failures.
  Invalid or resource-rejected patterns fail closed; exhaustion must never be
  reported as an ordinary non-match.
- Freeze compile-once ownership and the exact candidate strings used by
  `filter` and `kind`. Authorize one later joint implementation packet only if
  this contract closes.
- Keep `attr` deferred behind a separate complete typed attribute-string
  representation and equality design.

Use pinned Bazel 9.2 source evidence for compile-once, `Matcher.find`, and the
label/kind candidate strings. Use the locked Rust crate's own source/API as
authority for the named Slug-native dialect, Unicode model, errors, and
resource behavior. Define a discriminator matrix for literal/search/anchor,
Unicode and inline flags, invalid syntax, oversized patterns, explicit limit
failures, and deterministic repeated use. No Java execution or new oracle is
required or permitted for dialect-edge behavior.

## Files

Edit only:

- `thoughts/shared/plans/slug-v2-subplans/08-ruleset-and-command-conformance.md`.

Read-only inputs may include workspace `Cargo.toml`, `Cargo.lock`, the locked
regex crate source/documentation, and
`app/slug_query_v2/src/{expr,generic,graph}.rs`. Add no Rust, Cargo, lockfile,
fixture, oracle, canonical-plan, or routing-log changes during this packet.

Obtain an independent public-boundary review. Record `ACCEPT` or `REPLAN` in
the owner plan and schedule Rust only after acceptance.

## Stops

Stop and `REPLAN` if the selected crate cannot provide bounded deterministic
compile/search behavior or explicit resource failures. Stop on claims of Java
compatibility, UTF-16/lone-surrogate emulation, JVM/Java bytecode, helpers,
execution or delegation, `attr`, query-graph identity changes, DICE regex keys,
cquery/aquery breadth, Cargo/dependency edits, or silent resource failure as
non-match.
