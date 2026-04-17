# Plan 14a: Cache Extension Failures in DICE

> Parent: Plan 10 Phase 7 follow-up

## Problem

`ModuleExtensionExecutionKey::compute` and `ExtensionRepoExecutionKey::compute`
both declare `fn validity(x) -> bool { x.is_ok() }`, which tells DICE:
**errors are not cached**. Every time an extension is referenced — from any
`load()` statement, any `FileOpsKey::compute` on a repo from that extension,
any cross-extension Label reference — DICE re-runs the failing computation.

Observed in llvm-project cquery: `go_deps`, `python`,
`rules_kotlin_extensions`, `non_module_deps`, and others fail during
execution. Each failure recreates a stub repo, then returns the error.
Subsequent references re-trigger execution because the error isn't cached.
Wall time blows up: ~5 minutes for the daemon to make any visible progress
through the analysis target.

## Bazel Parity

Bazel caches within a build session via Skyframe regardless of success: a
Skyframe node that errored this build returns the cached error to subsequent
readers within the same invocation. Across invocations, Bazel's lockfile
caches **successful** extension results; failures re-execute next build.

This proposal matches Bazel's within-session semantics. Across-session
caching is orthogonal — covered by the existing lockfile path.

## Design

Change three `validity` impls to return `true` regardless of result:

- `app/kuro_bzlmod/src/extension_execution_dice.rs:571`
  (`ModuleExtensionExecutionKey`)
- `app/kuro_bzlmod/src/repository_execution.rs:190`
  (`RepositoryRuleExecutionKey`)
- `app/kuro_bzlmod/src/repository_execution.rs:418`
  (`ExtensionRepoExecutionKey`)

DICE already tracks the per-transaction cache; transaction end (triggered by
file changes, daemon restart, or explicit invalidation) clears it. The
change is local and low-risk.

## Touchpoints

- Three one-line diffs (described above).
- Cross-reference check: search for other `fn validity` in
  `app/kuro_bzlmod/` and verify none are intentionally error-excluding for
  different reasons.

## Success Criteria

### Automated

- `cargo check -p kuro_bzlmod` clean
- `cargo test -p kuro_bzlmod` — existing 154 passing tests stay green
- `pytest tests/core/bzlmod/` — existing suite passes

### Manual

- Run `kuro cquery @llvm-project//llvm:config` and count occurrences of
  "Extension ... Starlark implementation, falling back to empty specs" in
  the output.
  - Before: each failing extension logs this for every reference, often
    dozens of times per build
  - After: each failing extension logs this exactly once per session
- Elapsed-time check: measure wall time for the cquery invocation to reach
  first "Analysis" line. Expect measurable reduction.

## Risks

1. **Transient errors cached for whole session**: If an extension fails due
   to a transient issue (network timeout, race with another download), the
   failure sticks until daemon restart. Mitigation: user kills daemon if
   they suspect a transient failure. Bazel behaves the same way within a
   build.

2. **Different error types for the same extension across references**:
   Today, if error A occurs at reference site 1 and error B occurs at
   reference site 2 (say, due to state changes between calls), each would
   propagate distinctly. After this change, only the first error is ever
   seen. Mitigation: errors from extension execution are deterministic in
   practice — they reflect .bzl-level bugs, missing Starlark APIs, or
   cross-extension Label refs to unmaterialized repos. These don't vary
   between reference sites.

3. **Masking a real race condition**: If there is a genuine race where an
   extension fails on first access but succeeds on second (because another
   extension materialized its dependency in the meantime), caching the
   first failure prevents the second succeeding. Mitigation: this scenario
   is actually a bug — the "correct" extension-execution order should be
   maintained via DICE dep tracking, not via retry. Surface the race as a
   hard failure; fix via proper dep tracking if it occurs.

## Not Addressed

- Downloads that succeed take as long as they take; this change doesn't
  speed up legitimate work.
- Per-extension-cell amplification (214 repos) is unchanged — only
  re-execution of failures is prevented.
- Extensions with valid outputs that intermittently produce different
  repo sets: this change would lock in the first-observed set.

## Est. Effort

30 minutes + test cycle.
