# Slug V2 Independent Reviewer Template

```text
Review packet <ID> as an independent Bazel-parity and architecture gate.

Read:
- The approved packet
- AGENTS.md
- The actual diff, not only the worker summary
- The cited Bazel 9.2 source/oracle evidence
- Relevant DICE ownership documentation where applicable
- Compact validation results

Do not implement, edit files, or broaden the packet.
For a correction rereview, read only the correction diff, affected evidence,
and the prior blocker. Do not reconstruct the full packet.
Treat tests-only or evidence-only correction as non-material unless it changes
the accepted contract or architecture. Inspect recorded validation output;
rerun commands only when evidence is missing, stale, or suspect.

Check only applicable risks:
1. Does the representation encode Bazel semantics rather than fixture output?
2. Are identity, ownership, semantic equality, and invalidation complete?
3. Are ordering, deduplication, diagnostics, and formatter behavior exact?
4. Are named negative and lifecycle boundaries discriminatingly tested?
5. Is semantic discovery DICE-owned with no direct filesystem or fresh graph?
6. Does the patch activate only the named surface?
7. If retained representation changed, are compact utilities appropriate?
8. Does downstream validation cover every changed public/cross-crate boundary?
9. Is any acceptance claim broader than the actual fixture/test set?
10. For oracle work only, is every copied registry/module subtree, mutation,
    manifest, expected field, and negative assertion necessary to discriminate
    the claimed behavior?
11. For oracle work only, has the fixture-growth checkpoint fired, and if so
    was a bounded hygiene review completed before adding more fixture breadth?
12. For daemon/input work, are request projections, observed inputs, source
    certificates, final validation, provisional cleanup, and overlapping
    sessions complete without fabricated filesystem snapshots?
13. For retained/cache/async work, are lifetime class, publication, cutoff,
    invalidation, eviction, cancellation, join, and shutdown release explicit?
14. Does every fallback name its violated invariant, deletion condition, owner,
    and permanence-prevention test?
15. If a touched module crosses the complexity trigger, does the packet either
    split it or justify one cohesive owner without mixing semantic,
    presentation, persistence, and transport concerns?
16. For a claimed hot-path improvement, do balanced measurements preserve
    exact outputs/RPCs and meet the declared metric threshold?

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
