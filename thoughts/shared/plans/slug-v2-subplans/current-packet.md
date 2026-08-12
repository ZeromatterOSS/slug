# Current Slug V2 Packet

Packet: `WP-8-m5-filewrite-aquery-multi-action-order-evidence-design`
Milestone: M5 expansion
Owner: `slug-v2-subplans/08-ruleset-and-command-conformance.md`
Result: freeze the exact multi-action FileWrite text ordering evidence or
record a bounded `REPLAN`.

## Design question

Determine whether the accepted one-literal aquery command can widen from
exactly one FileWrite to a finite closure of FileWrite actions without
inventing Bazel ordering or container semantics. Keep the accepted command
root, raw-wire validation, bzlmod transport, retained build DICE evaluation,
per-action formatter, identity-domain separation, and terminal classifications
unchanged.

## Read-only scope

Inspect the accepted command-root implementation, retained
`BuildCommandEvaluation::action_closure` ordering and ownership, existing
FileWrite action tests, accepted `action-query-identity-evidence` and
`aquery-action-shape` artifacts, and pinned Bazel 9.2 source for the text
action-graph formatter/order boundary. Freeze the smallest discriminator matrix
that distinguishes declaration order, dependency-before/after-root order, and
shared/diamond closure deduplication. Classify every field and ordering rule as
exact, Slug-native, or unsupported/deferred.

Select at most one bounded evidence/implementation successor with explicit
file allowlist, line caps, failure boundaries, and lifecycle proof. If existing
retained ordering cannot be justified by pinned source plus a small Bazel 9.2
matrix, record `REPLAN` rather than broadening the parity claim.

## Validation

This packet is design-only: no Rust, fixture, expected oracle, Cargo, lockfile,
Bazel execution, daemon process, JVM/Java, REAPI, or CI changes. Confirm the
accepted one-action command remains untouched, cite the exact existing evidence
and source obligations, run documentation/diff checks, and require independent
design review. A material correction consumes the sole correction budget.

## Stops

Do not add query functions, multiple roots, external labels, non-text output,
file contents, compilation/root-setting flags, action execution, new DICE
state, action reconstruction, retained identity, exact Bazel checksum/
ActionKey bytes, or unrelated Stage 4/8 breadth.
