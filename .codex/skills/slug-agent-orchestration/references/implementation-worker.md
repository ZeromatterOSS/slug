# Slug V2 Implementation Worker Template

Use only the core fields plus applicable conditional sections. Do not emit
placeholder sections or `none with a reason`.

```text
Task: <one bounded observable result>
Owner: <milestone, plan path, packet ID>
Baseline: <branch, HEAD, dirty ownership>

Read:
- AGENTS.md
- <active owner section>
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
- The failure class changes or validation would weaken an assertion

Return:
- Changed files and behavior
- Commands and exact results
- Evidence used
- Residual/unsupported boundary
- Any stop condition
```

## Conditional sections

Add only what the task uses:

- **Oracle/fixture:** exact rows, generated fields, reused scaffolding,
  per-fixture and aggregate growth, last checkpoint, duplication reason,
  hygiene/pruning, and affected replays.
- **DICE/semantic key:** identity, ownership, equality, validity, Need/error
  behavior, invalidation/restoration, event storage, and dependent pruning.
- **Reuse/representation:** matching Stage 9 row, Buck2/V1 candidates, selected
  utility boundary, memory/clone implications, and ledger disposition.
- **Public/cross-crate:** downstream production wrapper and compile coverage.
- **Platform/daemon:** exact platform evidence, lifecycle/process cleanup, and
  cross-target or same-daemon coverage.

An accepted discriminating Bazel 9.2 oracle is sufficient; do not add another
fixture unless the packet names a missing behavior. Workers run focused tests.
The root owns broad validation, documentation, commits, and reserved decisions.
