# AGENTS.md

Project-wide instructions for AI agents on slug.

## Bazel version target

**Bazel 9 parity only.** No back-compat for older Bazel or slug's earlier prototype behaviour.

- Current canonical plan:
  `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`.
  The January roadmap and numbered V1 subplans are archive/reference material
  unless the V2 plan explicitly asks an agent to extract or compare them.
- Bazel 9 removes symbol (`CcInfo`, `PyInfo`, `ProtoInfo` from globals) → slug removes too. No deprecation, no shim.
- Bazel 9 changes lockfile/WORKSPACE/Starlark API → slug matches exact. Not superset, not subset.
- Bazel 9 errors on pattern (native `cc_library` without `load("@rules_cc//...")`) → slug errors same message shape.
- `@bazel_tools` content: port verbatim from upstream `src/<path>/BUILD.tools`. No invention, copy exact.

## Rationale

Prototype. No external users of slug's Starlark surface. Break any slug workspace for parity — fine. No migration guides, no deprecation flags, no compat shims unless user asks.

Cite Bazel source of truth for parity decisions:

- Symbol removal: `src/main/java/com/google/devtools/build/lib/analysis/BaseRuleClasses.java` (EmptyRule pattern) + relevant `rules-*.java` registry.
- `@bazel_tools` content: `src/main/java/.../BUILD.tools` + `embedded_tools/` layout in installed Bazel.
- Lockfile format: `src/main/java/com/google/devtools/build/lib/bazel/bzlmod/` (version, digest encoding, repo spec schema).

## "Parity" concretely

- Bazel 9 errors → slug errors, same kind.
- Bazel 9 output path → slug output path, same. V2 does not inherit the V1
  `buck-out` exception unless a deliberate Slug extension plan says so.
- Bazel 9 MODULE.bazel builds → slug builds, same result.
- Bazel 9 fails → slug fails. Workarounds masking a Bazel 9 failure = bugs.

## Repo workflow for agents

Start from the live checkout, not from memory.

- A request to "follow the implementation plan", continue the roadmap, pursue
  the current goal, or otherwise choose the next V2 work is an orchestration
  task. Before selecting or delegating work, read
  `.codex/skills/slug-agent-orchestration/SKILL.md` and
  `thoughts/shared/prompts/2026-07-23-slug-v2-root-orchestrator.md`.
  This applies even when the initial request is only `/goal follow the
  implementation plan`.
- For plan-following goals, use the canonical plan's **Live Status** table as
  the scheduling authority. Historical checkpoint prose is evidence, not the
  current queue. Finish an already-active owned packet to a safe boundary, then
  clear any baseline blocker named by Live Status before starting another
  feature packet.
- The root orchestrator owns architecture, worktree safety, plan/status and
  routing-log edits, broad validation, commits, and final communication.
  Implementation workers normally edit only their explicitly allowed source,
  test, and fixture files. They return evidence to the root rather than editing
  plans or committing.
- Default to one write worker. A concurrent second worker must be read-only or
  own completely disjoint files. Allow one focused correction after a concrete
  miss; a second material correction requires replanning rather than an
  open-ended repair loop.
- Read this file, then the current roadmap entry under
  `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`, then the
  relevant V2 subplan under `thoughts/shared/plans/slug-v2-subplans/`.
  Read V1 plans only when the V2 plan names them as extraction/reference
  material.
- If the user names a prompt or plan, read that prompt/plan before editing.
  Prompts live in `thoughts/shared/prompts/`; subplans live in
  `thoughts/shared/plans/slug-v2-subplans/`. V1 plans are available through the
  archive refs named in `V1_ARCHIVE.md`, not as active root plans.
- Repo-local skills live in `.codex/skills/`. For V2 hot-path utilities,
  Buck2-derived data structures, interning, hashing, compact collections, or
  memory-accounting work, read
  `.codex/skills/slug-buck2-utility-reuse/SKILL.md` before editing.
- When splitting work across agents or choosing a lower-cost model, follow
  `.codex/skills/slug-agent-orchestration/SKILL.md`. Route each bounded packet
  to the least-cost capable agent, keep context forks small, verify delegated
  diffs in the root, and add one routing-log rollup only when the packet reaches
  `ACCEPT`, `REPLAN`, or a genuine stop.
- Check `git status --short` and inspect dirty diffs before making changes.
  Treat dirty files as active user/agent state unless the user says otherwise.
- Prefer focused owning-abstraction tests before broad SDK or repo-wide smokes.
  Use broad smokes only after the local bug class is understood.
- Do not run multiple `cargo build` or `cargo test` commands in parallel when
  they share the same target directory; Cargo lock contention obscures signal.
- If a Rust change affects the V2 `slug` binary path used by oracle tests,
  rebuild it with `cargo build -p slug_cli_v2` before invoking the binary named
  by `SLUG_V2_BIN`.
- Clean stale `slugd` processes before and after Slug smokes or focused
  daemon-sensitive tests.
- Bazel commands may use ordinary RC discovery and the user's `~/.bazelrc` for
  BuildBuddy authentication. Never inspect, print, copy, or commit its contents;
  no credential or derived secret material may enter this checkout or Git.

## DICE and incremental state

DICE is Slug's incremental computation graph. Use it for semantic build state
that must be cached, invalidated, replayed, or shared across requests. Before
editing DICE keys, ownership, or any locking around a DICE compute, read
[docs/developers/dice.md](docs/developers/dice.md) — it covers the ownership
principles and the silent-deadlock hazard of holding a lock across a DICE
computation.

## Validation expectations

- Every parity fix needs either observed Bazel 9 behavior or a citation from a
  local Bazel source checkout.
- Add or strengthen the narrow regression first, then implement the fix.
- Same-daemon behavior matters: create/edit/delete transitions, lockfile
  changes, environment changes, repository mapping changes, and materialized
  output changes should invalidate or replay for a clear reason.
- Repository materialization tests should compare against the helper or manifest
  format that writes the marker/output state; avoid hard-coding stale marker
  formats in new tests.
- Update the owning plan with compact evidence when a result changes the
  project state. Do not use a passing real-world target as proof that structural
  acceptance criteria are complete unless the plan says so.
- New implementation work should add or strengthen the Bazel oracle fixture
  first, then port or write code until Slug V2 matches that fixture.

## NOT in scope

- Bazel 8.x compat. `.bazelversion=8.x` → upgrade it.
- WORKSPACE files. Removed in Bazel 9. Unsupported.
- Legacy toolchain resolution. Bzlmod-only.
