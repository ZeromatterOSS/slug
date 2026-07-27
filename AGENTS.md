# AGENTS.md

Project-wide instructions for AI agents on slug.

## Bazel target

**Bazel 9 parity only.** Slug matches Bazel 9 success, failure, diagnostics,
outputs, and paths; it does not preserve Bazel 8 or prototype behavior.

- Canonical plan:
  `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`.
  V1 plans are archive material unless the V2 plan requests them.
- Reproduce observable semantics in Rust, not Bazel's machinery. JVM bytecode,
  an embedded JVM, and delegation to Bazel/Java are oracle tools only unless
  the user explicitly approves them as Slug architecture. Record an unsupported
  boundary or `REPLAN` when no bounded exact Rust implementation exists.
- Port `@bazel_tools` content verbatim from upstream; do not invent it.
- This is a prototype with no compatibility surface. No shims, deprecations, or
  migration support unless the user asks.

## Repo workflow for agents

Start from the live checkout, not from memory.

- A request to follow or continue the implementation plan, including
  `/goal follow the implementation plan`, must use
  `.codex/skills/slug-agent-orchestration/SKILL.md`.
- If the user names a prompt or plan, read that prompt/plan before editing.
  Prompts live in `thoughts/shared/prompts/`; subplans live in
  `thoughts/shared/plans/slug-v2-subplans/`. V1 plans are available through the
  archive refs named in `V1_ARCHIVE.md`, not as active root plans.
- Repo-local skills live in `.codex/skills/`. For V2 hot-path utilities,
  Buck2-derived data structures, interning, hashing, compact collections, or
  memory-accounting work, read
  `.codex/skills/slug-buck2-utility-reuse/SKILL.md` before editing.
- Check `git status --short` and inspect dirty diffs before making changes.
  Treat dirty files as active user/agent state unless the user says otherwise.
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

- Every parity change needs accepted discriminating Bazel 9.2 oracle evidence
  or a pinned-source regression. Reuse existing evidence; add coverage only for
  a demonstrated gap.

## NOT in scope

- Bazel 8.x compat. `.bazelversion=8.x` → upgrade it.
- WORKSPACE files. Removed in Bazel 9. Unsupported.
- Legacy toolchain resolution. Bzlmod-only.
