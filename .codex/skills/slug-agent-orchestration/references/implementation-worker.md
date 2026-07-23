# Slug V2 Implementation Worker Template

Fill every field. Use `none` with a reason rather than deleting a field.

```text
Task: <one bounded implementation result>
Owner gate: <milestone, stage, exact plan path, packet ID>
Baseline: <branch and exact HEAD>
Dirty ownership: <dirty paths and owners, or clean>

Read:
- AGENTS.md
- <owner subplan sections>
- <oracle fixture and expected result>
- <exact Bazel 9.2.0 source/test paths>
- <Stage 9 reuse rows and retained Buck2/V1 paths>
- <relevant production and test files>
- <any task-triggered repo skill already read by the root>

Required result:
<one observable result and its exact acceptance condition>

Allowed files:
<exact source/test/fixture paths>

Forbidden:
- Editing AGENTS.md, canonical plans, owner plans, prompts, skills, routing
  logs, or Git commits
- New DICE keys, locks, global registries, public APIs, identity models, or
  stage-boundary changes without a recorded reviewer decision
- Direct filesystem discovery outside the existing observation adapter
- Bazel compatibility other than 9.2.0
- Broad cleanup, unrelated refactoring, or additional function activation
- Weakening, deleting, fragment-matching, or normalizing exact assertions
- Broad or shared-target Cargo suites

Oracle contract:
<exact fixture rows or source-derived behavior>

Reuse decision:
- Candidates checked: <exact paths/Stage 9 rows>
- Decision: <adopt/port/rewrite/reference-only/reject for each>
- Approved boundary: <review verdict or none with reason>

Semantic checklist:
- Exact Bazel success and failure behavior
- Exact diagnostics, exit code, ordering, and formatter shape
- Identity and ownership remain structural, not inferred from printed output
- Semantic equality includes every field that must invalidate
- Formatting-only or semantically equal edits reuse computation
- Create/edit/delete/recreate transitions are covered where applicable
- External labels and unsupported forms fail at the reviewed boundary
- No hidden filesystem, fresh-DICE, or command-owned graph path
- Compact retained utilities are reused on hot paths
- Generated/source/rule identities and ownership remain distinct
- Production-wrapper/downstream behavior is covered if an interface changes

Focused validation:
<exact command, expected tests/rows, and pass condition>

Return:
1. Changed files and concise behavior summary
2. Commands run and exact pass/fail counts
3. Oracle/source anchors used
4. Residual risk and explicitly unsupported boundaries
5. Whether any stop condition was encountered

Stop immediately if:
- Dirty files overlap another owner
- The oracle contradicts the packet
- A reserved architecture decision is needed
- The observed failure class changes
- Scope would expand beyond allowed files/functions
- Validation would require weakening an assertion
```
