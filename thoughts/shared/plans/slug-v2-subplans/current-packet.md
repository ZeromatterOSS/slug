# Current Slug V2 Packet

Packet: `WP-4-8-m3-attr-typed-attribute-string-design`
Milestone: M3 query / Stage 4 loading prerequisite
Owner: `slug-v2-subplans/04-starlark-loading-and-build-packages.md`
Cross-owner: `slug-v2-subplans/08-ruleset-and-command-conformance.md`
Result: freeze the smallest exact typed attribute-string projection required by
the sole remaining default query function, `attr`.

## Background and boundary

M3 now implements 15 of Bazel 9.2's 16 default loading-query functions.
`attr(WORD, WORD, EXPR)` is the only registry gap. Its existing
`QueryAttribute` projection retains only name, reachable labels, and
explicitness, while loading already owns a richer typed `CoercedAttributeValue`
tree. Bazel's `TargetUtils.getAttrAsString` matches a universe of type-specific
string projections, including every configurable branch; this cannot be
inferred from dependency labels.

This packet is documentation and source audit only. Design the smallest total,
immutable Stage 4 projection and Stage 8 accessor contract for values already
admitted by V2. Do not implement or activate `attr`, extend loading syntax, or
add an oracle in this packet.

## Required design

- Audit pinned Bazel 9.2 `AttrFunction`, `RegexFilterExpression`,
  `TargetUtils.getAttrAsString`, attribute mapper/type formatting, and selector
  traversal at immutable commit
  `8220c6198837d5c13d53fea211cf3282aa12408a`.
- Inventory every currently admitted V2 coerced value alternative and
  provenance/default state: null, scalar/list strings, scalar/list labels and
  outputs, both dict orientations, selector branches/defaults, concatenation,
  and any other live alternative found by the audit.
- Freeze exact per-alternative string values, order, qualification, quoting,
  null suppression, branch union/dedup behavior, explicit/default behavior,
  and invalid/unrepresented-value failure.
- Place the compact immutable projection at the loading/query ownership seam;
  define structural equality and same-DICE invalidation/reuse obligations for
  every semantic and formatting-only transition. Reuse current compact
  representations and add no new DICE key.
- Preserve the accepted Rust-native regex contract: compile once before operand
  evaluation, find/search each projected string, and filter streamed candidate
  deliveries. Regex design itself is closed and out of scope.
- End in `ACCEPT` only if one bounded representation implementation and one
  later query activation can be named truthfully; otherwise record `REPLAN`
  with the exact missing evidence or representation boundary.

## Files and evidence

Edit only:

- `thoughts/shared/plans/slug-v2-subplans/04-starlark-loading-and-build-packages.md`;
  and
- `thoughts/shared/plans/slug-v2-subplans/08-ruleset-and-command-conformance.md`.

Read-only inputs may include pinned Bazel source, existing accepted oracle
fixtures/records, `app/slug_loading_v2/src/{attrs.rs,package.rs}`, and
`app/slug_query_v2/src/{graph.rs,expr.rs,generic.rs,loading_environment.rs}`.
Add no Rust, Cargo/lockfile, BUILD, fixture, oracle, canonical-plan, manifest,
or routing-log change during the design packet. Obtain an independent
cross-crate representation/identity review before scheduling implementation.

## Stops

Stop and `REPLAN` on a need for a live/frozen Starlark heap, a currently
unrepresented value kind without a bounded typed extension, uncertain
formatter/order/quoting semantics, configuration evaluation, query-time
filesystem or Starlark reads, a new DICE key/lock, an unbounded cross-crate
representation change, regex redesign, cquery/aquery breadth, or any JVM,
Java source/bytecode/helper integration or Bazel delegation. Pinned source and
ordinary external Bazel-oracle evidence may inform semantics; no Java artifact
or runtime becomes Slug architecture.
