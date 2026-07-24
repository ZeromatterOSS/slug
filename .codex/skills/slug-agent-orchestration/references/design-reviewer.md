# Slug V2 Independent Reviewer Template

```text
Review packet <ID> as an independent Bazel-parity and architecture gate.

Read:
- The approved packet
- AGENTS.md and the owner gate
- The actual diff, not only the worker summary
- The named Bazel 9.2.0 source/tests and generated oracle
- Relevant DICE ownership documentation where applicable
- Focused and downstream validation evidence

Do not implement, edit files, or broaden the packet.

Check:
1. Does the representation encode Bazel semantics rather than fixture output?
2. Are identity, ownership, semantic equality, and invalidation complete?
3. Are ordering, deduplication, diagnostics, and formatter behavior exact?
4. Are negative, external-label, generated-target, and lifecycle boundaries
   discriminatingly tested?
5. Is semantic discovery DICE-owned with no direct filesystem or fresh graph?
6. Does the patch activate only the named surface?
7. Is there at least one source-derived adversarial case beyond the happy path?
8. Are retained compact utilities used appropriately on hot paths?
9. Does downstream validation cover every changed public/cross-crate boundary?
10. Is any acceptance claim broader than the actual fixture/test set?
11. For oracle work, is every copied registry/module subtree, mutation,
    manifest, expected field, and negative assertion necessary to discriminate
    the claimed behavior?
12. Has the fixture-growth checkpoint fired, and if so was a bounded hygiene
    review completed before adding more fixture breadth?

Return exactly one verdict:

ACCEPT
<one compact reason and residual risk>

or

REVISE
1. <file:line, violated oracle/contract, smallest correction, required test>
...

or

REPLAN
<architecture/parity contradiction and the decision required before resuming>

List at most five material blockers. Do not include optional style suggestions
unless they conceal correctness, performance, ownership, or maintainability
risk.
```
