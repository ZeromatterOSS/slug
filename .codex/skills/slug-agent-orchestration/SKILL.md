---
name: slug-agent-orchestration
description: Run Slug V2 implementation-plan or roadmap goals through bounded, reviewed work packets. Use for `/goal follow the implementation plan`, next-packet selection, delegation, model routing, integration, or routing records.
---

# Slug Agent Orchestration

The root owns priority, architecture, worktree safety, integration, status, and
commits. Delegate bounded work to the least-cost capable agent.

## Start a Plan Goal

1. Read `AGENTS.md` and the canonical plan's **Live Status** plus current
   packet. Historical plan prose is evidence, not scheduling state.
2. Read only the current owner plan's goal/current-priority/acceptance sections
   and the exact active packet heading. Search for the packet ID; do not load an
   entire long evidence history by default.
3. Use the routing table below. Search `references/routing-log.md` for an
   analogous packet and read only the matching recent rows.
4. Check `git status --short --branch` and dirty diffs. Inspect live
   agent/Cargo/slugd processes only before overlapping work, retry, or
   daemon-sensitive validation.
5. Continue a clearly owned active packet; otherwise select exactly one packet
   from Live Status. Clear a red M0 or other named baseline blocker before
   beginning another feature packet. Read only matching Stage 9 rows unless
   reuse scope expands.

## Routing

| Route | Use |
|-------|-----|
| Root only | Small read-only or mechanical work where delegation costs more |
| Terra medium | Default audit, oracle fixture, focused tests, one abstraction |
| Terra high | Approved difficult multi-file Rust, DICE, Starlark, or query work |
| Sol low | Pre-review of reserved decisions and final risky-patch review |
| Sol high review | Concrete unresolved miss or genuinely new architecture |

For long autonomous runs, keep one high-capability Sol root when selectable;
do not add a standing second orchestrator. Explicit worker overrides use
`gpt-5.6-terra` at medium/high or `gpt-5.6-sol` at low, with
`fork_turns="none"` or a small task-local fork.

Default to one write worker. Additional workers must be read-only or own
disjoint files. Never run parallel Cargo commands against one target directory.

## Packet Contract

Use `references/implementation-worker.md`. Every packet has:

- one owner gate and observable result;
- exact allowed files and exclusions;
- Bazel 9.2 oracle/source anchors;
- relevant Stage 9/Buck2/V1 reuse decisions;
- focused validation and stop conditions; and
- a residual-risk report.

The root retains new DICE keys/locks, public or cross-crate APIs, identity and
ownership models, formatter semantics, regex engines, stage boundaries, and
destructive actions. Obtain Sol review before implementing such a decision.

Workers normally edit only named source/test/fixture files and run focused
tests. The root inspects the diff, verifies the oracle, adds a discriminating
case when identity/equality/invalidation/ordering/formatting is involved, and
owns downstream and broad validation plus documentation and commits.

## Acceptance

Check the applicable behavior, not every item mechanically:

- exact Bazel success/failure, diagnostics, ordering, and output;
- identity, ownership, semantic equality, reuse, and invalidation;
- create/edit/delete/recreate and unsupported/external/generated boundaries;
- DICE-owned discovery without filesystem or fresh-graph bypass;
- activation limited to the named surface and compact hot-path utilities; and
- downstream coverage for changed interfaces.

Use `references/design-reviewer.md` for reserved or risky boundaries. The
verdict is `ACCEPT`, `REVISE`, or `REPLAN`. Allow one focused correction after
a concrete miss. A second material correction ends the packet in `REPLAN`.

## Root Validation and Closeout

Run only what the risk requires, serially:

1. focused owner tests;
2. downstream/public-wrapper tests for changed interfaces;
3. named comparisons through `tools/v2_oracle`;
4. risk-appropriate broad Cargo suites, serialized against the shared target
   directory;
5. `cargo fmt --check` and `git diff --check`; and
6. daemon tests in a socket-capable environment, with stale `slugd` cleanup
   before and after.

Do not weaken environment-limited tests. After a terminal result, update Live
Status and compact owner evidence once, append one terminal packet rollup to
the bounded routing log, and commit only accepted work.

## Log Use

`references/routing-log.md` is append-only evidence, not routine startup
context. Search it for an analogous packet and read only the relevant rows.
When it needs archival, preserve old rows under a dated history file before
trimming the active log.
