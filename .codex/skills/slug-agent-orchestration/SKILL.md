---
name: slug-agent-orchestration
description: Orchestrate Slug plan execution across lower-cost Codex agents while preserving Bazel parity, plan ownership, and validation quality. Use whenever a user asks to follow or continue the implementation plan or roadmap (including a simple `/goal follow the implementation plan`), and when splitting work into agent packets, choosing Terra medium/high or Sol review, validating delegated work, or recording routing outcomes.
---

# Slug Agent Orchestration

Route each bounded packet to the least-cost model likely to complete it once.
Keep the root agent responsible for architecture, dirty-worktree safety,
integration, validation, commits, plan state, and final user communication.

For an open-ended plan-following goal, the root is the persistent orchestrator.
Use a high-capability Sol root when the surface permits choosing the root model.
Do not spawn a second standing Sol-high orchestrator. Use Terra workers for
bounded work and Sol low as an on-demand independent reviewer.

## Required Context

1. Read `AGENTS.md`,
   `thoughts/shared/prompts/2026-07-23-slug-v2-root-orchestrator.md`, the
   canonical V2 plan, its **Live Status** table, and the owning subplan.
2. Read `references/routing-log.md` before choosing a model when it contains an
   analogous task.
3. Read any task-triggered repo skill in the root agent before delegating.
4. Check `git status --short --branch`, inspect dirty diffs, and identify files
   other agents own.
5. Inspect live agent and Cargo/slugd processes before retrying validation or
   assigning overlapping work.
6. Select exactly one packet from Live Status or the current owner gate. Do not
   reconstruct priority from older checkpoint prose.

## Root Orchestrator Policy

The root:

- chooses the critical-path packet and writes its exact contract;
- owns architecture, public/cross-crate interfaces, DICE ownership and locks,
  stage boundaries, destructive actions, and dirty-worktree integration;
- inspects the actual worker diff and source/oracle anchors;
- adds a source-derived adversarial regression for ordering, identity,
  equality, invalidation, provenance, or formatting changes;
- runs downstream and broad validation serially;
- requests terminal review where required;
- edits Live Status, owner-plan evidence, and the routing log once; and
- commits only an accepted packet.

Normal implementation workers do not edit `AGENTS.md`, the canonical plan,
owner subplans, prompts, skills, the routing log, or Git commits. A packet may
delegate those files only when its sole purpose is a named documentation or
process change.

When a packet has already begun and owns cleanly identified dirty files, finish
or preserve it to a safe boundary. Otherwise, a red M0 or another baseline
blocker in Live Status precedes new feature work.

## Route by Complexity

Use this table as the default, then adjust from logged evidence.

| Route | Best fit | Avoid |
|-------|----------|-------|
| Root only | Tiny read-only checks, one-file mechanical edits, tasks where coordination costs more than execution | Using delegation merely because a slot is available |
| Terra medium | Default bounded worker: source archaeology, fixtures, focused tests, one-abstraction Rust changes | Unresolved cross-crate architecture, subtle DICE ownership, or routine plan editing |
| Terra high | Difficult but bounded implementation/debugging: multi-file Rust, Starlark/query graph work, async invalidation, complex test migration | Open-ended redesign without an approved boundary |
| Sol low | Architecture/parity review, Bazel-source adjudication, cross-stage interface review, independent review of a risky patch, diagnosis after a concrete worker miss | Routine implementation that Terra can finish directly |
| Sol high review | Escalation after a concrete unresolved reviewer/root miss or a genuinely new architecture boundary | Standing second orchestrator or speculative routine review |

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

Use `references/implementation-worker.md` as the worker template. It requires
exact allowed files, a Bazel oracle/source contract, semantic and invalidation
checks, focused validation, and hard stop conditions.

Use `references/design-reviewer.md` for independent review. A reviewer returns
exactly `ACCEPT`, `REVISE`, or `REPLAN`, with concrete blockers only.

## Acceptance Checklist

The packet contract and root review must address, where applicable:

- exact Bazel success, failure, diagnostics, exit status, ordering, and output;
- representation identity, ownership, semantic equality, and invalidation;
- semantically equal reuse plus create/edit/delete/recreate transitions;
- external labels, generated targets, negative boundaries, and unsupported
  forms;
- DICE-owned discovery with no direct-filesystem or fresh-graph bypass;
- activation of only the named surface;
- compact Buck2-derived utilities on hot paths;
- production-wrapper and downstream coverage for interface changes; and
- at least one discriminating adversarial case not implied by happy-path rows.

Passing Cargo tests or matching a nondiscriminating fixture is not enough.

## Minimize Tokens and Cost

- Spawn with `fork_turns="none"` or the smallest useful recent-turn count.
  Supply task-local paths and facts instead of duplicating the conversation.
- Reference local files; do not paste source that the worker can read.
- Start one worker. Add parallel workers only for genuinely independent work
  that shortens the critical path.
- Ask for compact findings, exact paths/lines, commands, and a patch or decision.
- Allow one focused correction after a concrete miss. A second material
  correction ends the packet in `REPLAN`; do not continue open-ended repair.
  Escalate Terra medium to Terra high, or Sol low to Sol high, only with the
  failed evidence.
- Stop agents that drift, duplicate another packet, or cannot advance the
  acceptance gate.
- Never invent token counts. Record exact usage when exposed; otherwise write
  `not exposed` and use the qualitative cost band.

## Validation Ownership

Workers run only the focused commands named by their packet. The root owns:

- `cargo fmt --check` and `git diff --check`;
- downstream/public-wrapper compilation and tests;
- accepted-fixture comparisons through `tools/v2_oracle`;
- daemon-sensitive tests in an environment that permits Unix sockets;
- broad serialized Cargo suites; and
- process cleanup before and after daemon-sensitive validation.

Never retry a broad validation until the previous process/session is known to
have ended. Do not weaken a test because the current sandbox lacks a required
capability; record the environment limitation and run the required lane where
the capability exists.

## Orchestration Loop

1. Read Live Status and choose the smallest packet that advances or unblocks
   the critical path.
2. Complete a Terra-medium reuse/source/oracle audit when the boundary is not
   already approved.
3. Obtain Sol-low pre-review before any reserved architecture/parity choice.
4. Spawn one bounded Terra worker with `fork_turns="none"` and the stored
   implementation template.
5. Inspect the actual diff; verify the source/oracle and add an adversarial
   regression.
6. Run focused, downstream, and fixture validation serially.
7. Obtain Sol-low final review for reserved or risky boundaries.
8. Permit one focused correction and revalidation. On a second material miss,
   stop and replan.
9. After `ACCEPT`, update Live Status, compact owner evidence, and one routing
   rollup, then commit.
10. Re-read worktree and process ownership before choosing another packet.

## Stored Prompts

- Root session:
  `thoughts/shared/prompts/2026-07-23-slug-v2-root-orchestrator.md`
- Implementation worker: `references/implementation-worker.md`
- Independent reviewer: `references/design-reviewer.md`

Do not shorten a worker packet by removing required fields. Fill `none` with a
reason where a field does not apply.

## Record Outcomes

Update `references/routing-log.md` once after a packet reaches `ACCEPT`,
`REPLAN`, or a genuine stop. Do not add a row for every audit, review round, or
worker message. Record:

- task class and packet;
- model and reasoning effort;
- context strategy and parallelism;
- exact tokens/cost if exposed, otherwise `not exposed` plus a cost band;
- start/end time or wall time;
- oracle rows and focused/downstream test counts;
- outcome, review rounds, root corrections, rework, and escalation;
- recommendation for the next analogous task.

Keep event rows append-only. Update the short recommendations section when at
least two results support a routing change or one result exposes a serious
failure mode.
