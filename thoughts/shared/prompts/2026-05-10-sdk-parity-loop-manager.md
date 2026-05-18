# Slug SDK Bazel 9 Parity Loop Manager

Use this prompt for a new agent taking over the ongoing Slug SDK parity loop.
The agent is responsible for managing the full iterative process, not merely
implementing one blocker-sized slice.

## Workspace

- Slug source repo: `/var/mnt/dev/slug`
- ZeroMatter SDK repro repo: `/var/mnt/dev/zeromatter`
- Target goal: make Slug build `//sdk:sdk_contents` in the zeromatter repo and
  produce output identical to the equivalent Bazel 9 invocation, with
  equivalent or better performance and memory behavior.

## Non-Negotiable Role

You own the loop overall.

Do not stop after one local implementation slice just because that slice is
committed. Continue until one of these is true:

1. `//sdk:sdk_contents` builds under Slug and output parity has been checked
   against Bazel 9.
2. A real blocker prevents forward progress and you leave a clean, explicit
   resume prompt with the exact next action, commands, logs, and state.
3. The user explicitly asks you to stop.

A bounded timeout with ongoing progress is not a stopping condition. If a smoke
times out without a semantic failure, either increase the bound, add focused
instrumentation, run a narrower target, or classify the performance/stall under
the appropriate plan. Keep managing the loop.

Important: do not treat a bounded-memory timeout with fresh analysis progress
as a "real blocker" just because the current slice produced useful handoff
notes. That is only an intermediate observation. Before ending the turn, take
one more concrete loop action unless the user asked to stop: start a longer
bounded smoke, launch a focused repro for the visible waiting target, add the
next instrumentation needed to distinguish slow progress from a stall, or make
the exact performance/stall fix implied by the evidence. Ending immediately
after recording "bounded memory plus ongoing progress" is an unexpected stop.

## Context and Delegation Discipline

Minimize the manager agent's own context usage. Keep the top-level thread for
state, decisions, compact summaries, and integration only. Do not load large
logs, long source files, or broad search results into the manager context when
a subagent can inspect them and report the small set of facts needed for the
next decision.

Keep prompt/context packets concise. For a delegated task, include only:

- repo paths and current branch/worktree status;
- the active plan section and one or two source/doc anchors;
- the exact failing command, status, and log path;
- the focused validation expected before any broad smoke;
- the required final report shape.

Do not paste whole plan files or smoke logs into a subagent prompt. Point to
files and require cited line references or short excerpts.

Delegate as much of the loop as possible to subagents. Each full loop iteration
should be assigned to a subagent as a bounded end-to-end task: inspect the
current failure or stall, classify it, update the relevant plan, implement the
smallest systemic fix if one is indicated, run focused verification, run or
prepare the next SDK smoke, clean up daemons, and report exact commands,
statuses, logs, changed files, and next blockers. The manager should avoid
duplicating the subagent's exploration; it should review results, integrate
patches, decide the next iteration, and spawn the next subagent.

## Required Reading Before Acting

Read these first from `/var/mnt/dev/slug`:

- `AGENTS.md`
- `thoughts/shared/plans/2026-01-21-slug-bazel-compatible-build-tool.md`
- The status, validation, and acceptance sections of the active failure's plan
  only. Use the roadmap's plan table and dependency map to find the right
  subplan; do not preload unrelated plans into context.
- Any source/doc anchors cited by that active plan section.

Then rediscover current state instead of trusting stale prompt details:

```sh
cd /var/mnt/dev/slug
git status --short
git log --oneline -8
ps -eo pid,ppid,stat,etime,rss,args | rg 'slugd\[' || true
```

If the worktree is dirty, inspect the diff and preserve unrelated/user changes.
Never revert work you did not make unless explicitly asked.

## Standing Slugd Cleanup Rule

There may be many idle `slugd` processes. Clean them up before and after every
Slug smoke or focused Slug build. At minimum, use a targeted cleanup like:

```sh
cleanup_slugd() {
  ps -eo pid=,args= | awk '/slugd\[/ {print $1}' | xargs -r kill -TERM
  sleep 2
  ps -eo pid=,args= | awk '/slugd\[/ {print $1}' | xargs -r kill -KILL
}

cleanup_slugd
ps -eo pid,ppid,stat,etime,rss,args | rg 'slugd\[' || true
```

Always report final daemon state. Do not leave long-running Slug, smoke, or
daemon sessions alive when handing off.

## Parity Rules

- Bazel 9 parity only. No Bazel 8 compatibility and no WORKSPACE support.
- Do not mask Bazel failures. If Bazel 9 fails, Slug should fail in the same
  shape.
- Do not fix SDK blockers with one-off target or label workarounds.
- Do not optimize for the smallest patch that advances the current smoke.
  Optimize for the narrowest systemic fix that covers the whole demonstrated
  bug class.
- Do not weaken depset mutable-value validation.
- Preserve TransitiveSet streaming, projection, reduction, and action-input
  behavior.
- Do not make Bazel depset a public alias for Slug/Buck TransitiveSet.
- Do not special-case a label or target unless Bazel itself has that exact
  intrinsic boundary.
- Prefer `Native`, `Intrinsic`, or `NativeShim` terminology. Do not introduce
  new `Synthetic` or `Stub` terminology for valid provider/API facades.
- Use Bazel source or focused Bazel 9 probes for parity decisions.
- Treat the selected active plan as controlling. Ground structural claims in
  the evidence that plan requires: pinned Bazel source/docs, Slug source/plans,
  or named local Bazel-vs-Slug experiments.

## Systemic-Fix Bias

For SDK parity work, "smallest systemic fix" means minimal blast radius inside
the abstraction that owns the missing Bazel semantic. It does not mean the
quickest local patch, the first change that gets the current target farther, or
the smallest diff against the latest smoke failure.

Before editing code for a new failure, write down the class boundary in the
active plan or subplan:

- What Bazel semantic is missing or wrong?
- Which Slug subsystem owns that semantic?
- What other targets, rules, features, or toolchains would fail for the same
  reason?
- What would count as a one-off workaround for this failure?

Do not implement a patch if its correctness depends on a specific SDK target,
label, repository name, artifact filename, isolation directory, configuration
hash, or observed output path, unless Bazel itself has that exact intrinsic
boundary.

Classify the intended patch before file edits:

1. Systemic parity fix: implements a Bazel semantic at its owning abstraction.
2. Test/instrumentation: proves or localizes a parity class.
3. Temporary diagnostic code: must not be committed.

If the intended patch is only a symptom fix, stop and create or update the
relevant plan instead. Examples of symptom fixes include adding SDK-specific
labels, hardcoding toolchain outputs, chmodding a final output tree to match one
target, adding path remaps for one binary, or special-casing one repository's
generated paths.

If a failure reveals a missing abstraction or incomplete model, create or update
a numbered subplan before implementation. The implementation should then follow
that subplan. Do not continue with an ad hoc code change simply because the
current failure has an obvious local workaround.

When a failure is discovered, first try to reproduce the bug class in a focused
unit or regression test at the owning abstraction. The goal is to build durable
coverage as blockers burn down, not just to move the current SDK smoke forward.
If a direct unit test is impractical because the behavior spans integration
boundaries, add the smallest focused regression test, fixture, or source-cited
assertion that would have failed before the fix, and record why a narrower unit
test was not practical in the active plan. Do not rely on the full SDK smoke as
the only regression test for a semantic bug class.

Do not add duplicate tests. Before adding coverage, search for existing tests
that already exercise the semantic and extend the narrowest existing fixture
when that is clearer than creating a new one. If existing coverage already
proves the behavior, cite the test and run it instead of adding another.

For any plan with a validation matrix, start there. Prefer focused Rust unit
tests for data structures, keys, parsers, and subsystem-local semantics. Prefer
focused Python/fixture integration tests when daemon state, filesystem layout,
external repos, toolchains, or cross-crate command execution are required. Use
the full `//sdk:sdk_contents` smoke to advance or confirm the frontier, not as
the first or only proof of a semantic.

The following are not acceptable parity fixes unless explicitly approved by the
user as temporary diagnostics:

- Hardcoding missing LLVM, rules_rust, or rules_rs linker flags because one SDK
  binary needs them.
- Adding `--remap-path-prefix` entries for one observed output hash or target.
- Chmodding `//sdk:sdk_contents` outputs after the fact to match Bazel modes.
- Special-casing `rules_rs`, `rules_rust`, `llvm`, `zeromatter`, or generated
  canonical repository names outside the abstraction that owns those semantics.
- Treating a successful build as sufficient progress when the produced command
  line is known to differ from Bazel in a structured way.

## Per-Blocker Operating Loop

For every new SDK failure or stall:

1. Capture the exact command, isolation dir, log path, exit status, elapsed
   time, and memory summary.
2. Classify the failure:
   - Use the roadmap plan table, dependency map, and newest failure evidence to
     choose one active plan.
   - If multiple plans plausibly apply, pick the plan that owns the missing
     semantic and cite the others as references, not co-owners.
   - If no plan owns the failure class, create or update a numbered subplan
     before implementing.
3. Update or create the relevant plan before implementing.
4. If it is a bug class, search for other instances of the same class.
5. Identify the owning abstraction and explicitly reject symptom-only patches.
6. Reproduce the failure class in a focused unit or regression test before the
   fix whenever feasible. Prefer a unit test in the subsystem that owns the
   semantic; use a focused integration-style test only when the bug cannot be
   expressed at a narrower boundary.
7. First search for existing coverage. If an existing test already covers the
   class, run it and update it only if the new case exposes a missing assertion.
8. Implement the narrowest systemic fix in the plan scope and make the focused
   test pass. The test should encode the Bazel 9 semantic, not the SDK target
   name, current isolation directory, or observed configuration hash.
9. Add any additional Bazel 9 parity tests or source-cited assertions needed
   for confidence in the broader bug class.
10. Run focused verification, then broader verification appropriate to the
   blast radius.
11. Rerun a narrower zeromatter target or fixture when it exercises the same
    frontier faster than `//sdk:sdk_contents`; rerun full `//sdk:sdk_contents`
    only when focused checks pass and you need to advance or confirm the SDK
    frontier.
12. Commit each clean completed slice with a clear message.
13. Continue the loop.

## Frontier SDK Smoke

This is the broad frontier check, not the default validation for every local
change. Prefer focused unit tests, focused integration fixtures, or a narrower
zeromatter target when they exercise the same behavior. Run this after focused
checks pass, when you need to discover the next SDK blocker, or when a change
has enough integration risk that only the full SDK target gives useful signal.

Run from `/var/mnt/dev/zeromatter`, using the Slug binary built from
`/var/mnt/dev/slug`:

```sh
cd /var/mnt/dev/zeromatter

cleanup_slugd() {
  ps -eo pid=,args= | awk '/slugd\[/ {print $1}' | xargs -r kill -TERM
  sleep 2
  ps -eo pid=,args= | awk '/slugd\[/ {print $1}' | xargs -r kill -KILL
}

isolation="sdk-parity-$(date +%Y%m%d-%H%M%S)"
log="/tmp/${isolation}.log"

cleanup_slugd
set +e
timeout 900s env SLUG_MEMORY_CHECKPOINTS=1 \
  /var/mnt/dev/slug/scripts/memory_smoke.sh \
    --interval 5 \
    --include-pgrep "slugd\\[zeromatter\\].*${isolation}" \
    -- \
    /var/mnt/dev/slug/target/debug/slug \
      --isolation-dir "${isolation}" \
      build //sdk:sdk_contents > "${log}" 2>&1
status=$?
cleanup_slugd
ps -eo pid,ppid,stat,etime,rss,args | rg 'slugd\[' || true
tail -220 "${log}"
exit "${status}"
```

If this times out while still making progress, do not stop. Use the log to
choose one of:

- run a longer bounded smoke;
- run a narrower build or focused fixture for the visible waiting target;
- add or refine Plan 51 instrumentation;
- classify a repeated wait as a performance/stall blocker and continue.

If you choose the classification path, it is still not a final stopping point
by itself. Update the relevant plan, then continue with the next executable
step unless you can state a specific external blocker that prevents all of the
above options.

## Baseline Verification After Meaningful Code Changes

Use the narrowest useful checks first, then broaden only as needed:

```sh
cd /var/mnt/dev/slug
cargo fmt
cargo test -p <touched-crate> <focused-test> -- --nocapture
cargo check -p slug
cargo build -p slug
git diff --check
```

Run relevant pytest fixtures under `tests/core/...` when pytest is available.
If pytest is unavailable, state that and use direct Slug fixture builds where
practical.

Do not spend minutes on the full SDK smoke when a focused unit or smaller
integration test proves the same semantic. After meaningful changes, run:

1. The focused test that failed before the fix or would have failed without it.
2. Existing nearby tests that already cover the touched abstraction.
3. `cargo check -p slug` or a touched-crate check when API boundaries changed.
4. A narrower zeromatter target or fixture if the bug only appears through
   repository/toolchain integration.
5. Full `//sdk:sdk_contents` only to advance/confirm the frontier or before a
   handoff claiming SDK progress.

## Known Recent Frontier Pattern

Do not assume any frontier note is current. Rediscover with a fresh focused
check or smoke, then classify the observed blocker against the active plan.

## Handoff Requirements

When ending a turn, leave the next agent or user with:

- Current commit hash and worktree status.
- Exact commands run and their results.
- Log paths for SDK smokes and focused repros.
- New blocker classification and linked plan section.
- What was implemented and verified.
- Whether Bazel 9 output parity has been checked.
- Final `slugd[...]` process status.

If you cannot complete the overall goal, make the next action unambiguous.
