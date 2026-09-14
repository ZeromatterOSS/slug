# Slug V2 Implementation Worker Template

Use only the core fields plus applicable conditional sections. Do not emit
placeholder sections or `none with a reason`.

```text
Task: <one bounded observable result>
Owner: <milestone, manifest path, packet ID>
Baseline: <branch, HEAD, dirty ownership>
Validation tier: <docs | oracle | private/local | public/cross-crate | DICE/daemon/platform>

Read:
- AGENTS.md and the packet manifest, including linked applicable contracts
- <owner section only for a reserved decision or unresolved contradiction>
- <accepted oracle or exact Bazel 9.2 source>
- <production/test files in scope>
- <matching Stage 9 row only when reuse/representation changes>

Required result:
<exact acceptance condition>

Allowed files:
<exact paths>

Forbidden:
- Unapproved public API, DICE key/lock, identity, ownership, formatter, regex,
  stage-boundary, dependency, or destructive change
- Direct filesystem discovery outside the observation owner
- Unrelated cleanup, function activation, assertion weakening, or broad
  shared-target Cargo work

Focused validation:
<minimum commands and exact pass condition>

Stop if:
- Dirty ownership overlaps
- Accepted evidence contradicts the packet
- A reserved decision or scope expansion is required
- Completion would weaken an assertion or require a new design/permission

Recovery:
- Correct invocation/environment and implementation failures within the accepted
  contract using the orchestration skill; a changed failure class alone is not
  REPLAN. Preserve valid evidence and unfinished source in the review worktree.
- Report new dependencies/ownership decisions to the root rather than guessing.

Return:
- Changed files and behavior
- Commands and exact results
- Evidence used
- Residual/unsupported boundary
- Any stop condition
```

## Inherited requirements

The packet author owns the guide's readiness checklist. Pass the worker only
applicable packet invariants, exact owner/source sections and evidence handles;
do not copy the authoring checklist into this prompt or require a full owner
plan, guide, history or preserved patch read. Flag an omitted material risk to
the root. Workers revise the contract only when explicitly assigned that task.

An accepted discriminating Bazel 9.2 oracle is sufficient; do not add another
fixture unless the packet names a missing behavior. Workers run focused tests.
The root runs only the named validation tier, owns documentation/commits, and
reserves broad suites for milestone/integration checkpoints. Use quiet commands
where supported and return command, exit status, test count, and only relevant
failure output.
