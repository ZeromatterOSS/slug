# AGENTS.md

Project-wide instructions for AI agents on slug.

## Bazel version target

**Bazel 9 parity only.** No back-compat for older Bazel or slug's earlier prototype behaviour.

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
- Bazel 9 output path → slug output path, same (modulo `bazel-out`/`buck-out`, deliberately different).
- Bazel 9 MODULE.bazel builds → slug builds, same result.
- Bazel 9 fails → slug fails. Workarounds masking a Bazel 9 failure = bugs.

## Repo workflow for agents

Start from the live checkout, not from memory.

- Read this file, then the relevant roadmap entry in
  `thoughts/shared/plans/2026-01-21-slug-bazel-compatible-build-tool.md`.
- If the user names a prompt or plan, read that prompt/plan before editing.
  Prompts live in `thoughts/shared/prompts/`; subplans live in
  `thoughts/shared/plans/slug-bazel-subplans/`.
- Check `git status --short` and inspect dirty diffs before making changes.
  Treat dirty files as active user/agent state unless the user says otherwise.
- Prefer focused owning-abstraction tests before broad SDK or repo-wide smokes.
  Use broad smokes only after the local bug class is understood.
- Do not run multiple `cargo build` or `cargo test` commands in parallel when
  they share the same target directory; Cargo lock contention obscures signal.
- If a Rust change affects the `slug` binary path used by Python/e2e tests,
  rebuild it with `cargo build -p slug` before invoking `target/debug/slug`.
- Clean stale `slugd` processes before and after Slug smokes or focused
  daemon-sensitive tests.

## DICE and incremental state

DICE is Slug's incremental computation graph. Use it for semantic build state
that must be cached, invalidated, replayed, or shared across requests.

- Represent semantic inputs as DICE keys or tracked DICE dependencies. Do not
  make a value "DICE-owned" by computing it outside DICE and injecting it after
  startup.
- Key equality and hashing must include every input that can change the result:
  command policy, environment policy, file identity/digest, repo mapping,
  toolchain/platform policy, lockfile policy, and any relevant mode flags.
- Prefer explicit invalidation edges over process-global mutable state,
  singleton caches, marker-file trust, or best-effort digests.
- A warm cache hit is correct only when the key or tracked dependency graph
  explains why every Bazel-relevant input is unchanged.
- Use process globals only for non-semantic instrumentation or short-lived
  plumbing, and document why they cannot affect correctness.
- When modeling Bazel Skyframe behavior, mirror the ownership boundary:
  parse inputs, resolution outputs, repo mappings, repository specs,
  materialization state, and lockfile policy should each have auditable
  producers and dependencies rather than being bundled into an opaque session
  object.

### Don't hold a shared mutex across a DICE computation

The precise hazard is a lock held across a `.await` that re-enters DICE where
that same lock can be requested again. Because DICE schedules across a
multi-threaded tokio runtime, freely re-enters other keys, and has NO cycle
detection for analysis/native keys, such a re-entrant acquisition deadlocks
SILENTLY (daemon at low CPU, threads parked in `futex_wait`) rather than
erroring. Default to NOT holding any shared lock across a DICE compute; the
exceptions below are narrow.

- HARD rule: never hold a *thread-blocking* lock (`parking_lot::Mutex`,
  `std::sync::Mutex`) across a `.await` that runs (directly or transitively) a
  DICE computation — `ctx.compute(...)`, `try_compute_join`, `get_*` helpers,
  Starlark evaluation, repository-rule execution, etc. A blocking guard pins an
  OS worker thread; once the runtime can't poll the holder, you deadlock. The
  workarounds people reach for are all wrong here too: `blocking_lock` panics,
  `block_in_place` starves the runtime, and `parking_lot` hard-deadlocks.
- An *async* lock (`tokio::sync::Mutex` / `futures::lock::Mutex`) yields the
  worker instead of blocking it, so holding one across an await is not an
  automatic deadlock — but it still deadlocks if the critical section can
  re-request the same lock (self-recursion or via a dependency key), and a
  global async lock still serializes otherwise-independent DICE work and
  undermines incrementality. Treat it as a last resort, never global, and prove
  the critical section cannot re-enter the same key.
- The per-key locks here are also NON-REENTRANT. If an async path acquires a
  per-key lock and then calls a helper that re-acquires the same key's lock,
  that single computation self-deadlocks. Thread an "already-held" flag (or
  restructure) so the inner call does not re-lock. Worked example: the
  materialization lock — `ExtensionRepoExecutionKey::compute`
  (`repository_execution.rs`) holds the per-canonical-name lock and passes
  `caller_holds_materialization_lock = true` into
  `execute_repository_rule_impl` (`repository_executor.rs`) so it skips the
  inner acquisition.
- Correct patterns, in preference order: (a) let DICE own the serialization —
  two requests for the same key already dedupe to one computation, so you
  usually do not need a manual lock at all; (b) make the serialized state its
  own DICE key; (c) if you must use a manual lock, scope the guard to a
  synchronous critical section that does NO `.await` and NO compute, and drop
  it before awaiting.
- Diagnosing a suspected hang: `gdb` works without sudo on this host
  (`ptrace_scope=0`, daemon owned by the user) —
  `gdb -p <slugd_pid> -batch -ex "set pagination off" -ex "thread apply all bt"`.
  A thread parked in `parking_lot ... lock_slow` / `futex_wait` under a
  `*_compute` frame is the signature of this bug.

## Validation expectations

- Every parity fix needs either observed Bazel 9.0.1 behavior or a local Bazel
  source citation from `/var/mnt/dev/bazel`.
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

## NOT in scope

- Bazel 8.x compat. `.bazelversion=8.x` → upgrade it.
- WORKSPACE files. Removed in Bazel 9. Unsupported.
- Legacy toolchain resolution. Bzlmod-only.
