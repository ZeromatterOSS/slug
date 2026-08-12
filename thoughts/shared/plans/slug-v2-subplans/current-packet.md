# Current Slug V2 Packet

Packet: `WP-8-m5-filewrite-aquery-deps-owner-set-design`
Milestone: M5 expansion
Owner: `slug-v2-subplans/08-ruleset-and-command-conformance.md`
Result: freeze a bounded `deps()` owner-set/order compatibility strategy or
record `REPLAN`.

## Design question

Decide whether one `deps(<direct main-repository literal>)` expression can
reuse the accepted raw-wire request, typed build DICE evaluation, retained
configured action closure, resolved FileWrite views, and text formatter. Freeze
the owner membership/deduplication boundary separately from output order:
Bazel's configured-target owner set may be exact while Slug's deterministic
cross-owner traversal order must remain explicitly Slug-native unless pinned
source establishes a stronger contract.

## Read-only scope

Inspect the accepted aquery validator/command/wire, shared `QueryExpression`
AST, build-command closure construction and configured-edge kinds, resolved
FileWrite selection, the new root-order fixture's order-agnostic diamond row,
and pinned Bazel 9.2 aquery/query-set ownership sources already cited by Stage
8. Determine whether Slug's closure contains exactly the requested configured
dependency owners or also semantic-support nodes that need a source-backed
filter. Cover diamonds, aliases/generated files, toolchains, platforms,
constraints, configured transitions, shared equivalent actions across distinct
owners, and unsupported mixed action kinds.

Classify owner membership, per-owner declaration order, block framing, and
diagnostics as exact, Slug-native, or unsupported/deferred. Select at most one
bounded evidence/implementation successor with explicit file allowlist, line
caps, fail-closed boundaries, and one-shot/daemon A/B/A lifecycle proof. If the
existing closure cannot separate query-visible dependency owners from semantic
support without new retained state or wider query evaluation, record
`REPLAN`.

## Validation

This packet is design-only. Confirm the accepted literal command and generated
oracle evidence remain untouched. Run source/structure/diff checks and require
independent design review. Cap bookkeeping at 220 lines.

## Stops

Add no Rust, tests, fixture/expected evidence, Bazel execution, command/wire
fields, query-function activation, action reconstruction, DICE state, action
execution, contents, other action kinds/formats, retained identity changes,
exact Bazel identity bytes, JVM/Java, REAPI, or CI. One material correction
maximum; a second is `REPLAN`.
