# Slug V2 Generic Implementer Prompt

Use this prompt for implementer sessions that continue Slug V2 work.

```text
Work in /var/mnt/dev/slug. Start by reading AGENTS.md and
thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md. Then read the
specific V2 subplan that owns the requested slice. If the work touches archive,
clean-root setup, or imports from V1 or codex/slugv2, also read
thoughts/shared/plans/slug-v2-subplans/00-v1-archive-and-clean-root.md,
thoughts/shared/plans/slug-v2-subplans/09-v1-extraction-ledger.md, and
V1_ARCHIVE.md.

Use the plan files as the source of truth. Do not rely on this prompt for the
v1/v2 split procedure; those instructions live in the owner plans.

Check git status before editing. Treat dirty files as active user or agent work
unless explicitly told otherwise. Pick one bounded owner-plan slice, add or
refresh the Bazel oracle fixture first when applicable, then implement only the
code needed for that slice.

If importing code, fixtures, or behavior from V1 or the mixed-root codex/slugv2
prototype, first make a local V1 archive worktree available outside the active
V2 root when practical (for example `git worktree add C:\tmp\kuro-v1-archive slug-v1-archive` on Windows), then record the source, import mode, oracle, validation, and residual risk
in the Stage 9 extraction ledger. Do not treat V1 smokes, direct-local execution,
or mixed-root compilation as V2 acceptance evidence.

Validate with focused tests for the touched stage plus git diff --check. Record
compact evidence in the owning plan before finishing.

Deliverable: a focused patch, updated owner-plan evidence, and the validation
commands/results needed for the next implementer to continue safely.
```
