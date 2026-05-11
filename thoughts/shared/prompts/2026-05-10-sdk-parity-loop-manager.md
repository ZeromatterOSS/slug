# Kuro SDK Bazel 9 Parity Loop Manager

Use this prompt for a new agent taking over the ongoing Kuro SDK parity loop.
The agent is responsible for managing the full iterative process, not merely
implementing one blocker-sized slice.

## Workspace

- Kuro source repo: `/var/mnt/dev/kuro`
- ZeroMatter SDK repro repo: `/var/mnt/dev/zeromatter`
- Target goal: make Kuro build `//sdk:sdk_contents` in the zeromatter repo and
  produce output identical to the equivalent Bazel 9 invocation, with
  equivalent or better performance and memory behavior.

## Non-Negotiable Role

You own the loop overall.

Do not stop after one local implementation slice just because that slice is
committed. Continue until one of these is true:

1. `//sdk:sdk_contents` builds under Kuro and output parity has been checked
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

Delegate as much of the loop as possible to subagents. Each full loop iteration
should be assigned to a subagent as a bounded end-to-end task: inspect the
current failure or stall, classify it, update the relevant plan, implement the
smallest systemic fix if one is indicated, run focused verification, run or
prepare the next SDK smoke, clean up daemons, and report exact commands,
statuses, logs, changed files, and next blockers. The manager should avoid
duplicating the subagent's exploration; it should review results, integrate
patches, decide the next iteration, and spawn the next subagent.

When delegating, pass only the minimal necessary context: repo paths, current
commit/worktree status, the active plan(s), the exact command/log/status from
the latest smoke, and the expected output format. For large logs, pass file
paths and ask the subagent to extract line references and summaries rather than
pasting log bodies into the manager thread.

## Required Reading Before Acting

Read these first from `/var/mnt/dev/kuro`:

- `AGENTS.md`
- `thoughts/shared/plans/kuro-bazel-subplans/15-bazel-9-parity.md`
- `thoughts/shared/plans/kuro-bazel-subplans/54-depset-transitive-set-shared-core.md`
- `thoughts/shared/plans/kuro-bazel-subplans/55-symbolic-macro-inherit-attrs.md`
- `thoughts/shared/plans/kuro-bazel-subplans/56-native-intrinsic-provider-shims.md`
- Any other plan referenced by the newest SDK failure.

Then rediscover current state instead of trusting stale prompt details:

```sh
cd /var/mnt/dev/kuro
git status --short
git log --oneline -8
ps -eo pid,ppid,stat,etime,rss,args | rg 'kurod\[' || true
```

If the worktree is dirty, inspect the diff and preserve unrelated/user changes.
Never revert work you did not make unless explicitly asked.

## Standing Kurod Cleanup Rule

There may be many idle `kurod` processes. Clean them up before and after every
Kuro smoke or focused Kuro build. At minimum, use a targeted cleanup like:

```sh
cleanup_kurod() {
  ps -eo pid=,args= | awk '/kurod\[/ {print $1}' | xargs -r kill -TERM
  sleep 2
  ps -eo pid=,args= | awk '/kurod\[/ {print $1}' | xargs -r kill -KILL
}

cleanup_kurod
ps -eo pid,ppid,stat,etime,rss,args | rg 'kurod\[' || true
```

Always report final daemon state. Do not leave long-running Kuro, smoke, or
daemon sessions alive when handing off.

## Parity Rules

- Bazel 9 parity only. No Bazel 8 compatibility and no WORKSPACE support.
- Do not mask Bazel failures. If Bazel 9 fails, Kuro should fail in the same
  shape.
- Do not fix SDK blockers with one-off target or label workarounds.
- Do not optimize for the smallest patch that advances the current smoke.
  Optimize for the narrowest systemic fix that covers the whole demonstrated
  bug class.
- Do not weaken depset mutable-value validation.
- Preserve TransitiveSet streaming, projection, reduction, and action-input
  behavior.
- Do not make Bazel depset a public alias for Kuro/Buck TransitiveSet.
- Do not special-case a label or target unless Bazel itself has that exact
  intrinsic boundary.
- Prefer `Native`, `Intrinsic`, or `NativeShim` terminology. Do not introduce
  new `Synthetic` or `Stub` terminology for valid provider/API facades.
- Use Bazel source or focused Bazel 9 probes for parity decisions.

## Systemic-Fix Bias

For SDK parity work, "smallest systemic fix" means minimal blast radius inside
the abstraction that owns the missing Bazel semantic. It does not mean the
quickest local patch, the first change that gets the current target farther, or
the smallest diff against the latest smoke failure.

Before editing code for a new failure, write down the class boundary in the
active plan or subplan:

- What Bazel semantic is missing or wrong?
- Which Kuro subsystem owns that semantic?
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
   - depset/frozen/hashability/provider boundary: Plan 54, and possibly Plan 56.
   - native/intrinsic provider/API boundary: Plan 56.
   - Bazel 9 globals/toolchain/config/bazel_tools parity: Plan 15.
   - symbolic macro inherited attrs: Plan 55.
   - repo/materialization/layout/path issues: Plan 36 or Plan 44.
   - memory/profiling behavior: Plan 51.
3. Update or create the relevant plan before implementing.
4. If it is a bug class, search for other instances of the same class.
5. Identify the owning abstraction and explicitly reject symptom-only patches.
6. Implement the narrowest systemic fix in the plan scope.
7. Add focused Bazel 9 parity tests or source-cited assertions.
8. Run focused verification, then broader verification appropriate to the
   blast radius.
9. Rerun the SDK smoke from `/var/mnt/dev/zeromatter`.
10. Commit each clean completed slice with a clear message.
11. Continue the loop.

## Recommended SDK Smoke

Run from `/var/mnt/dev/zeromatter`, using the Kuro binary built from
`/var/mnt/dev/kuro`:

```sh
cd /var/mnt/dev/zeromatter

cleanup_kurod() {
  ps -eo pid=,args= | awk '/kurod\[/ {print $1}' | xargs -r kill -TERM
  sleep 2
  ps -eo pid=,args= | awk '/kurod\[/ {print $1}' | xargs -r kill -KILL
}

isolation="sdk-parity-$(date +%Y%m%d-%H%M%S)"
log="/tmp/${isolation}.log"

cleanup_kurod
set +e
timeout 900s env KURO_MEMORY_CHECKPOINTS=1 \
  /var/mnt/dev/kuro/scripts/memory_smoke.sh \
    --interval 5 \
    --include-pgrep "kurod\\[zeromatter\\].*${isolation}" \
    -- \
    /var/mnt/dev/kuro/target/debug/kuro \
      --isolation-dir "${isolation}" \
      build //sdk:sdk_contents > "${log}" 2>&1
status=$?
cleanup_kurod
ps -eo pid,ppid,stat,etime,rss,args | rg 'kurod\[' || true
tail -220 "${log}"
exit "${status}"
```

If this times out while still making progress, do not stop. Use the log to
choose one of:

- run a longer bounded smoke;
- run a narrower build for the visible waiting target;
- add or refine Plan 51 instrumentation;
- classify a repeated wait as a performance/stall blocker and continue.

If you choose the classification path, it is still not a final stopping point
by itself. Update the relevant plan, then continue with the next executable
step unless you can state a specific external blocker that prevents all of the
above options.

## Baseline Verification After Meaningful Code Changes

Use the narrowest useful checks first, then broaden:

```sh
cd /var/mnt/dev/kuro
cargo fmt
cargo test -p <touched-crate> <focused-test> -- --nocapture
cargo check -p kuro
cargo build -p kuro
git diff --check
```

Run relevant pytest fixtures under `tests/core/...` when pytest is available.
If pytest is unavailable, state that and use direct Kuro fixture builds where
practical.

Always rerun a bounded zeromatter SDK smoke after meaningful changes.

## Known Recent Frontier Pattern

Do not assume this is still current; verify with a fresh run.

Recent work advanced past:

- C++ toolchain `ctx.toolchains` recursion by using a C++ NativeShim boundary.
- `rules_rust` `depset(deps)` mutable-value failure by making relevant C++
  provider/native-shim values depset-hashable.
- `ctx.attr` source-file File/Target mismatch by exposing source files in
  `attr.label/list(..., allow_files=True)` as Bazel-like `Target` values.
- Bazel 9 target-name punctuation for generated Rust source paths such as
  `src/output_tests/expected/into_bytes_enum.repr(C).expected.rs`.

The last bounded smoke may have timed out during ongoing Rust/Arrow/AWS SDK
analysis rather than failing. Treat that as a cue to continue the loop with a
longer or more instrumented run, not as completion.

## Handoff Requirements

When ending a turn, leave the next agent or user with:

- Current commit hash and worktree status.
- Exact commands run and their results.
- Log paths for SDK smokes and focused repros.
- New blocker classification and linked plan section.
- What was implemented and verified.
- Whether Bazel 9 output parity has been checked.
- Final `kurod[...]` process status.

If you cannot complete the overall goal, make the next action unambiguous.
