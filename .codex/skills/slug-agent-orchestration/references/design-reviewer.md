# Slug V2 Independent Reviewer Template

```text
Review packet <ID> as an independent Bazel-parity and architecture gate.
Phase: <design | final>
Baseline: <source/design revision; for final, accepted design and diff base>

Read:
- The proposed contract (design) or approved packet (final)
- AGENTS.md
- The design changes (design) or actual implementation diff (final)
- The cited Bazel 9.2 source/oracle evidence
- Relevant DICE ownership documentation where applicable
- Cited evidence and compact validation results available for this phase

Do not implement, edit files, or broaden the packet.
For a correction rereview, read only the correction diff, affected evidence,
and the prior blocker. Do not reconstruct the full packet.
Tests/evidence-only corrections do not automatically require architecture
re-review. Reassess acceptance if coverage, assertions, provenance or failure
attribution changes even when the contract is unchanged. Inspect recorded
validation output; rerun only missing, stale, or suspect evidence.

For design, map material invariants to semantic owners, pinned source/evidence,
and planned discriminators. Identify prerequisites and unresolved decisions;
require enough evidence to choose a sound contract, not an implementation or
executed implementation tests. ACCEPT freezes the design only.

For final, map each material invariant to the actual change and recorded
validation appropriate to the packet's tier (docs use source/structure checks).
Reuse accepted design/source evidence; inspect the implementation delta and its
proof. Counts, line caps and hashes alone do not establish semantics. An
invocation failure needs the smallest missing validation, not architecture
REPLAN. ACCEPT approves only the stated, evidenced scope.

Check the packet's applicable invariants and inherited contracts. If a material
risk is missing, consult the relevant sections of the plan-authoring guide's
required record, complexity or performance gates; do not repeat its checklist.
In particular, reject claims broader than their evidence, weakened assertions,
unmodeled semantic inputs and owner bypasses. For oracle changes, verify that
copied content and mutations discriminate the claimed behavior and preserve
provenance; apply the skill's fixture-growth checkpoint when triggered.

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
