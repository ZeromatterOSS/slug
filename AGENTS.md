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
- Reproduce Bazel's observable semantics in Rust; do not reproduce Bazel's
  implementation machinery. JVM bytecode execution, an embedded JVM, and
  delegation to Bazel/Java are for oracle generation only unless the user
  explicitly approves them as Slug architecture. If exact behavior has no
  bounded Rust implementation, record the unsupported boundary or `REPLAN`.

## Repo workflow for agents

Start from the live checkout, not from memory.

- A request to follow or continue the implementation plan, including
  `/goal follow the implementation plan`, must use
  `.codex/skills/slug-agent-orchestration/SKILL.md`. Its compact startup path
  and `thoughts/shared/plans/slug-v2-subplans/current-packet.md` are the compact
  scheduling entrypoint. Canonical **Live Status** remains authoritative if
  they disagree.
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

- Every parity fix needs either observed Bazel 9 behavior or a citation from a
  local Bazel source checkout.
- Ensure a narrow regression already proves the behavior before implementing
  the fix. Reuse accepted evidence; add or strengthen it only for an actual
  coverage gap.
- Same-daemon behavior matters: create/edit/delete transitions, lockfile
  changes, environment changes, repository mapping changes, and materialized
  output changes should invalidate or replay for a clear reason.
- Repository materialization tests should compare against the helper or manifest
  format that writes the marker/output state; avoid hard-coding stale marker
  formats in new tests.
- Keep Bazel oracle fixtures discriminating and maintainable. Reuse deterministic
  fixture-local generators or immutable checked assets when that preserves
  isolation and provenance; do not duplicate registry/module scaffolding merely
  because an earlier fixture did. Remove unused modules, copied registries,
  mutations, manifests, expected fields, and negative assertions that do not
  affect the behavior being proved.
- Every parity implementation needs an accepted discriminating Bazel 9.2
  oracle or pinned-source regression. Reuse existing evidence when it already
  proves the behavior; add or strengthen an oracle only for an actual gap.

## NOT in scope

- Bazel 8.x compat. `.bazelversion=8.x` → upgrade it.
- WORKSPACE files. Removed in Bazel 9. Unsupported.
- Legacy toolchain resolution. Bzlmod-only.
