# AGENTS.md

Project-wide instructions for AI agents on slug.

## Compatibility target

**Bazel 9 is the reference for named admitted compatibility surfaces.** Every
active packet must classify changed behavior as **exact**, **Slug-native**, or
**unsupported/deferred**. Existing accepted exact slices remain exact; do not
silently widen a Slug-native or unsupported surface into a parity claim.

- Canonical plan:
  `thoughts/shared/plans/2026-06-26-slug-v2-clean-restart.md`.
  V1 plans are archive material unless the V2 plan requests them.
- Slug production is permanently Rust-native. Do not embed or launch a JVM,
  ship or interpret Java bytecode for Slug semantics, or delegate semantic work
  to Bazel/Java. Pinned Bazel may still run externally as an oracle; no Java
  helper, runtime, bytecode, or probe artifact enters Slug. User-declared build
  actions that happen to execute Java are outside this architecture rule.
- Approved Slug-native divergences are Rust Host observations rather than
  bitwise HotSpot state, Rust valid-Unicode strings/regex rather than exact Java
  UTF-16/`Pattern` edge behavior, and collision-safe, explicitly Slug-native
  configuration/path/action identity bytes rather than Bazel checksum,
  `bazel-out`, or ActionKey bytes.
  Exact Bazel identity-byte reproduction is a later milestone.
- Relaxing identity bytes never relaxes semantic identity or integrity. Every
  admitted configuration-affecting input must participate structurally in DICE
  equality and invalidation; unmodeled inputs fail closed. Keep semantic
  configuration identity, display/path tokens, Bazel checksum, Bazel ActionKey,
  and REAPI/CAS digests as distinct domains. Content, repository, lockfile, and
  REAPI/CAS hashes remain exact for Slug's actual graph.
- Record an unsupported boundary or `REPLAN` when no bounded Rust-native
  implementation exists within the packet's declared compatibility class.
- Port `@bazel_tools` content verbatim from upstream; do not invent it.
- This prototype has no stability or migration obligation to earlier Slug
  versions. No shims, deprecations, or migration support unless the user asks.

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
