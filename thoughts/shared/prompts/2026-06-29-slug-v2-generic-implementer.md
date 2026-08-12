# Slug V2 Generic Implementer Prompt

Compatibility launcher for a bounded implementation worker:

```text
Read AGENTS.md and execute the root-supplied packet using:
.codex/skills/slug-agent-orchestration/references/implementation-worker.md
The packet must already satisfy
thoughts/shared/plans/slug-v2-plan-authoring-guide.md; do not repair or widen
an unready contract during implementation.

Do not choose roadmap priority or expand the packet. Return its scoped patch,
focused validation, source/oracle anchors, applicable request/lifetime proof,
and residual risk to the root.
```

For an open-ended request such as `/goal follow the implementation plan`, do
not use the bounded prompt above. Use
[2026-07-23-slug-v2-root-orchestrator.md](./2026-07-23-slug-v2-root-orchestrator.md).
