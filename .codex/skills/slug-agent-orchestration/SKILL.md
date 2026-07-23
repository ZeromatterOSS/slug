---
name: slug-agent-orchestration
description: Orchestrate Slug work across lower-cost Codex agents while preserving Bazel parity, plan ownership, and validation quality. Use when splitting a Slug task into agent packets, choosing between Terra medium/high and Sol low, running parallel discovery or implementation, reviewing delegated work, or recording model effectiveness and token/cost observations for future routing.
---

# Slug Agent Orchestration

Route each bounded packet to the least-cost model likely to complete it once.
Keep the root agent responsible for architecture, dirty-worktree safety,
integration, validation, and final user communication.

## Required Context

1. Read `AGENTS.md`, the canonical V2 plan, and the owning subplan.
2. Read `references/routing-log.md` before choosing a model when it contains an
   analogous task.
3. Read any task-triggered repo skill in the root agent before delegating.
4. Check `git status --short --branch` and identify files other agents own.

## Route by Complexity

Use this table as the default, then adjust from logged evidence.

| Route | Best fit | Avoid |
|-------|----------|-------|
| Root only | Tiny read-only checks, one-file mechanical edits, tasks where coordination costs more than execution | Using delegation merely because a slot is available |
| Terra medium | Default bounded worker: source archaeology, fixtures, docs, focused tests, one-abstraction Rust changes, plan/evidence updates | Unresolved cross-crate architecture or subtle DICE ownership |
| Terra high | Difficult but bounded implementation/debugging: multi-file Rust, Starlark/query graph work, async invalidation, complex test migration | Open-ended redesign without an approved boundary |
| Sol low | Architecture/parity review, Bazel-source adjudication, cross-stage interface review, independent review of a risky patch, diagnosis after a concrete worker miss | Routine implementation that Terra can finish directly |

Prefer one Terra-medium worker. Use Terra high only when the packet itself is
complex, not because the overall project is complex. Use Sol low as a concise
reviewer or adjudicator, not as a standing second implementation team.

When the orchestration surface accepts explicit overrides, use
`model="gpt-5.6-terra"` with `reasoning_effort="medium"` or `"high"`, and
`model="gpt-5.6-sol"` with `reasoning_effort="low"`. Pair an explicit model
override with `fork_turns="none"` or a small positive turn count.

## Partition Work

Delegate only concrete, independent packets. Give each packet:

- one outcome and owner plan;
- exact allowed files or a read-only scope;
- source/oracle anchors;
- explicit exclusions and stop conditions;
- focused validation and expected evidence; and
- a request to report residual risk, not to expand scope.

Do not delegate two write packets that may edit the same files. Do not run
parallel Cargo commands against one target directory. Keep architecture,
cross-crate API choices, DICE ownership/locking decisions, destructive actions,
and final integration with the root unless the plan explicitly delegates the
decision.

## Minimize Tokens and Cost

- Spawn with `fork_turns="none"` or the smallest useful recent-turn count.
  Supply task-local paths and facts instead of duplicating the conversation.
- Reference local files; do not paste source that the worker can read.
- Start one worker. Add parallel workers only for genuinely independent work
  that shortens the critical path.
- Ask for compact findings, exact paths/lines, commands, and a patch or decision.
- Allow one focused correction after a concrete miss. Escalate Terra medium to
  Terra high, or request Sol-low adjudication, only with the failed evidence.
- Stop agents that drift, duplicate another packet, or cannot advance the
  acceptance gate.
- Never invent token counts. Record exact usage when exposed; otherwise write
  `not exposed` and use the qualitative cost band.

## Orchestration Loop

1. Classify the task and choose the smallest useful packet.
2. Select the route using the table and prior log entries.
3. Spawn with minimal context and continue useful root work in parallel.
4. Inspect the worker's actual diff/findings; do not accept its summary alone.
5. Run focused owner tests, then broader checks only when justified.
6. Use Sol low for a compact independent review when the packet changes
   architecture, parity interpretation, DICE ownership, or a broad interface.
7. Update the owner plan/evidence and the routing log after a meaningful
   delegation.

## Packet Prompt

```text
Task: <one bounded result>
Owner: <plan/gate>
Read: <exact files/source refs>
Scope: <allowed files or read-only>
Do not: <exclusions>
Oracle: <Bazel source/test or fixture>
Validate: <exact commands/pass condition>
Return: <compact result, changed files, validation, residual risk>
Stop if: <architecture ambiguity, dirty overlap, changed failure class>
```

## Record Outcomes

Update `references/routing-log.md` after a delegated packet completes, is
stopped, or requires escalation. Record:

- task class and packet;
- model and reasoning effort;
- context strategy and parallelism;
- exact tokens/cost if exposed, otherwise `not exposed` plus a cost band;
- outcome, validation, rework, and escalation;
- recommendation for the next analogous task.

Keep event rows append-only. Update the short recommendations section when at
least two results support a routing change or one result exposes a serious
failure mode.
