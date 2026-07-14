# Slug V2 Generic Implementer Prompt

Use this prompt for implementer sessions that continue Slug V2 work.

```text
Work in /var/mnt/dev/slug. Start by reading AGENTS.md and
thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md. Then read the
specific V2 subplan that owns the requested slice. For every Stage 2-8 slice,
always also read
thoughts/shared/plans/slug-v2-subplans/09-v1-extraction-ledger.md and
V1_ARCHIVE.md before forming the work packet, even when the request does not
already identify reusable work. If the work touches archive or clean-root
setup, also read
thoughts/shared/plans/slug-v2-subplans/00-v1-archive-and-clean-root.md and
then follow its split mechanics.

Use the plan files as the source of truth. Do not rely on this prompt for the
v1/v2 split procedure; those instructions live in the owner plans.

Check git status before editing. Treat dirty files as active user or agent work
unless explicitly told otherwise. Follow the canonical plan's Two-Tier
Work-Packet Contract. The default implementation worker is a Terra or Luna
xhigh agent and the default design reviewer is a Sol agent. Copy and fill the
plan-owned work-packet template for one bounded owner-plan slice before editing;
do not duplicate or weaken that template in this prompt.

For every Stage 2-8 packet, complete the canonical template's required Reuse
audit and obtain Sol approval before new implementation; do not wait until an
import is already known. Then add or refresh the Bazel oracle artifact and
implement only the packet's exact scope. Consult Sol before making any other
architecture, interface, DICE ownership/invalidation/locking, stage-boundary,
or V1/Buck2 reuse-boundary choice. After focused validation, send Sol the
packet, scoped diff, oracle or source evidence, command results, and residual
risks for the mandatory `accept`, `revise`, or `replan` review. Do not record a
packet as completed or begin another packet until it is accepted.

If importing code, fixtures, or behavior from V1 or the mixed-root codex/slugv2
prototype, first make a local V1 archive worktree available outside the active
V2 root when practical (for example
`git worktree add /tmp/slug-v1-archive slug-v1-archive`), then record the source,
import mode, oracle, validation, and residual risk in the Stage 9 extraction
ledger. Do not treat V1 smokes, direct-local execution, or mixed-root compilation
as V2 acceptance evidence.

Validate with the packet's focused commands plus git diff --check. Record the
accepted result and reviewer outcome compactly in the owning plan before
finishing. A First Real Bazel Build packet advances one named gate clause; only
the final integration packet and Sol review may mark the whole gate complete.

Deliverable: a focused patch, updated owner-plan evidence, and the validation
commands/results plus reviewer outcome needed for the next implementer to
continue safely.
```
