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

- A request to follow or continue the implementation plan, including
  `/goal follow the implementation plan`, must use
  `.codex/skills/slug-agent-orchestration/SKILL.md`. Its compact startup path
  and the canonical plan's **Live Status** are the scheduling authority.
- Read only the current owner plan sections and matching Stage 9 rows named by
  that startup path. Historical checkpoints and V1 plans are reference
  material, not routine context.
- If the user names a prompt or plan, read that prompt/plan before editing.
  Prompts live in `thoughts/shared/prompts/`; subplans live in
  `thoughts/shared/plans/slug-v2-subplans/`. V1 plans are available through the
  archive refs named in `V1_ARCHIVE.md`, not as active root plans.
- Repo-local skills live in `.codex/skills/`. For V2 hot-path utilities,
  Buck2-derived data structures, interning, hashing, compact collections, or
  memory-accounting work, read
  `.codex/skills/slug-buck2-utility-reuse/SKILL.md` before editing.
- Default to root-only work for small read-only or mechanical changes and one
  worker plus one reviewer for ordinary bounded packets. Use multiple
  independent audits only when the root names distinct unresolved semantic
  questions; correction rereviews inspect only the correction diff.
- Check `git status --short` and inspect dirty diffs before making changes.
  Treat dirty files as active user/agent state unless the user says otherwise.
- Keep status documentation proportional to scheduling changes. During a work
  packet, record evidence in the worker/reviewer handoff; do not make a plan or
  routing-log commit for each audit, correction, or review round. At terminal
  `ACCEPT`, `REPLAN`, or genuine stop, update the owning plan once with compact
  evidence and add one routing-log rollup. Update canonical **Live Status** only
  when its milestone state, blocker, or current packet changes. Do not copy
  completed packet contracts or evidence histories into the canonical plan.
- Prefer folding terminal status updates into the accepted implementation or
  oracle commit. When separation materially helps review, use at most one
  follow-up status commit for the whole packet. Routing history files are
  rollover archives, not a second live log; do not update both the live log and
  an archive for the same packet.
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
- Run a focused fixture-growth review after every five accepted oracle packets,
  or when fixtures have grown by at least 100 files or 10,000 text lines since
  the last review, whichever comes first. Record the exact accepted packet IDs,
  fixture scope, and aggregate before/after file and line counts. If no prior
  checkpoint exists, inventory the current accepted tree as the baseline and
  review all identifiable accepted packets since the latest owner-plan
  evidence. Inventory growth by fixture and repeated subtree, verify every
  retained row remains discriminating, and record a compact baseline/result in
  the oracle-harness owner plan. Pruning must preserve Bazel version/source
  provenance, hermetic replay, failure isolation, and exact expected output;
  never replace self-contained evidence with mutable shared state merely to
  reduce line count.
- Update the owning plan with compact evidence when a result changes the
  project state. Do not use a passing real-world target as proof that structural
  acceptance criteria are complete unless the plan says so.
- Every parity implementation needs an accepted discriminating Bazel 9.2
  oracle or pinned-source regression. Reuse existing evidence when it already
  proves the behavior; add or strengthen an oracle only for an actual gap.
  Require a separate design-only packet only for reserved architecture,
  identity, ownership, public API, DICE, formatter, regex, or stage-boundary
  decisions.

## NOT in scope

- Bazel 8.x compat. `.bazelversion=8.x` → upgrade it.
- WORKSPACE files. Removed in Bazel 9. Unsupported.
- Legacy toolchain resolution. Bzlmod-only.
