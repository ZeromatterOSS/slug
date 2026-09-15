# Current Slug V2 Work Packet

Packet: WP-4-7A-configured-conflict-path-observation-dice-event-audit-r1
Status: preexecution accepted; sole replay authorized

## Accepted predecessor receipt

The sole `PathObservationShardKey` armed-window replay is accepted as a valid
negative result and returns `REPLAN`. The exact nonignored selector
`configured_action_conflicts::one_shot_build_conflict_is_atomic_and_recovers`
listed once, then reached the 12-second wall deadline after
12.0113899707794 seconds in `RootCompute`. Observer and sole reaped run PID were
both 717911. Process group, children, pipes and telemetry cleanup were clear.
Global event counters were `[58308, 58292, 51889, 51877, 8095, 8091]`; base
overflow, dropped samples and activity claim were zero.

Raw observer words 51--63 were
`[221, 221, 220, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0]`: 221 installed merges reached
221 successful commits, 220 prior windows closed, the final window remained
active, pending/flags/reserved were zero, and all six exact
`PathObservationShardKey` event counters were zero. The decoder therefore
returned `zero-filtered-events` and the supervised command exited 2 as designed.
This is a negative observation across 221 coherently armed windows. It does not
prove global absence outside atomic window edges, invalidation behavior,
causality, cost or avoidable work. It is consistent with
`PathObservationShardKey` being an `InjectedKey`, whose injection is not an
ordinary DICE computation event.

The authentic fixture again verified 28 objects, 177 metadata entries and
8,004,740 bytes at inventory SHA-256
`4337d0756cefc0971a76e12bbeea54ee40c24beb0ff943a4c3bdc60d88ed764f`.
Frozen pre-execution SHA-256 values were:

| Artifact | SHA-256 |
|---|---|
| accepted supervisor | `3b4d48a4a7c729ec0c8c17ffd2aed3ddab8677699bdce9d8bb24d97afbeab69c` |
| scratch supervisor | `3de26b4616b98c1c2f83e1537aa8841000ed5e7bffaaea5101bb9a08ce3a94d5` |
| exact temporary Rust diff | `b2f50117bc7d60f9a6049bfe14a3101b8c2d78c2e50a66f614951c525531d1fa` |
| Core proof harness | `32cdf2435b5558d382d724eb7201bd6e09b0cfc47658802409283671371b5cf5` |
| CLI integration harness | `28ed7bbaaa513b87a4b1a165ad4f0acd81e290967d66f94b6167495657845de9` |
| spawned Slug binary | `81799c7bf7179ca32daf78d85255b519ed0e241ca42deb3bc867a5fb274cc878` |

The corrected dedicated Core target prepared in 4.83 seconds and its exact
selector passed 1/1. The final `cli` integration target prepared in 11.42
seconds and its exact nonignored selector preflight passed. A prior 7.60-second
library-harness preparation was rejected before execution because it selected
the older ignored probe. The final supervisor passed decoder plus normal,
deadline, exception, telemetry-cap and cleanup self-checks. Temporary totals
were 100/130/86 against 100/130/120 production/proof/scratch caps. Independent
preexecution review returned `EXECUTE`; independent result review returned
`REPLAN` and selected only the immediate ordinary dependent below. All
temporary source and scratch artifacts were removed, both worktrees were clean,
and the shard-key-instrumented audit will not rerun.

## Observable result

Audit whether any of the six existing DICE event categories for the exact
static tag `PathObservationKey` occur while an aggregate observation window is
armed by a successfully committed installed-merge handoff. This is the
immediate ordinary `Key` whose `compute` reads `PathObservationShardKey`. Count
only `Started`, `Finished`, `CheckDepsStarted`, `CheckDepsFinished`,
`ComputeStarted` and `ComputeFinished` in `Observer::event`.

A positive result establishes only that direct-dependent events occurred during
armed windows. It cannot attribute an event to a merge, shard, demand or
invalidation; prove necessity or causality; distinguish checking from useful
work; or measure cost. A zero result ends this window-based diagnostic chain;
do not broaden automatically to `ResolvedPathObservationKey`.

## Window lifecycle and ordering

Reuse the accepted merge/commit window protocol and words 51--63 unchanged.
After a merged path epoch is successfully assigned in
`NativeDemandSession::progress_inner`, close the active window, if any, and
publish one pending installed merge. The next successful transaction commit
immediately after `guard.inject_attempt` consumes it and arms the next window
before `AttemptInjectionRevision` finishes. Initial and non-path commits are
neutral when no merge is pending.

The producer first requires no pending marker, produced equal to committed and
`committed = closed + active`. If active, clear that marker; increment produced;
increment closed for the cleared window; then publish pending last with release
ordering. The consumer requires pending one, produced equal to committed plus
one, active zero and committed equal to closed. It clears pending, increments
committed, then publishes active last with release ordering. Invalid state or
arithmetic overflow sets sticky state and refuses publication.

`Observer::event` preserves its global counter and activity work first. It then
compares the static tag exactly with `PathObservationKey`; if active is one
under acquire ordering, it increments only the matching filtered event counter.
Wrong tags and disarmed events are neutral. Concurrent callbacks make this only
atomically observed aggregate window membership, with no per-event
linearization claim at a close edge.

## Fixed layout and valid cutoff

| Word | Aggregate |
|---:|---|
| 51 | installed path merges produced |
| 52 | merges reaching successful injection commit |
| 53 | armed windows closed by a later installed merge |
| 54--59 | exact `PathObservationKey` counters in the six existing event-variant order |
| 60 | pending merge marker, only 0 or 1 |
| 61 | active window marker, only 0 or 1 |
| 62 | sticky state/order or lifecycle/filtered-counter overflow flags |
| 63 | reserved zero |

For produced `P`, committed `C` and closed `W`, require flags/reserved zero,
pending zero, active one, `P = C > 0`, and `W = C - 1`. Reject the unreachable
`active = 0, W = C` state as well as every partial publication state. Require the
checked sum of filtered counters nonzero and each filtered counter no greater
than its corresponding global counter in words 8--13. No cross-event
inequality is valid across concurrent window edges. Zero handoffs, zero
filtered total, filtered/global mismatch, overflow or invalid lifecycle forces
`REPLAN`.

## Proof and sole replay

Add one exact feature-enabled dedicated Core integration selector,
`path_observation_dice_events_follow_committed_path_handoffs`. Drive the
production producer, consumer and filter helpers through neutral commit, first
produce/consume, next-merge close/rearm, exact and wrong tags, disarmed events,
all six variants, overwrite/unmatched states, lifecycle and every filtered
overflow, filtered/global bounds, the unreachable inactive completed state, and
every partial close/produce/consume/arm snapshot. Prove producer and consumer
helpers leave words 0--50 and reserved word 63 unchanged. For the filter helper,
prove inherited global counter/activity behavior is preserved and only the
matching word 54--59 receives its additional filtered update. Prepare the target
once under 60 seconds, then preflight/run only its exact selector under 12/15
seconds.

Derive one excluded scratch supervisor from the accepted bytes. Its decoder and
self-check cover lifecycle equations, marker ranges, every partial cutoff and
consumer-arm state, the unreachable inactive completed state, checked filtered
sum, filtered/global bounds, zero-event rejection, flags, output cap
and normal/deadline/exception cleanup. Preserve namespace isolation, observer
ownership, exact reads, wall deadline and kill/reap finalization.

Compile only the feature-enabled `cli` integration target once under 60 seconds.
Only the selected configured-conflict test adopts the inherited observer before
its external-fixture `one_shot_case` and retains the guard throughout. Its
feature-only workspace uses `SLUG_SENTINEL_SCRATCH/workspace`; ordinary fixture
assembly remains unchanged. Preflight with `--list --exact
configured_action_conflicts::one_shot_build_conflict_is_atomic_and_recovers`,
then run only `--exact
configured_action_conflicts::one_shot_build_conflict_is_atomic_and_recovers
--nocapture`; neither command may use `--ignored`. Verify the fresh accepted
fixture, freeze all hashes and obtain independent preexecution review first.
Invoke the 0644 scratch supervisor through `/bin/bash` under the established
namespace/ptrace escalation and the 12/15-second limits. Accept evidence only at
the expected wall deadline in `RootCompute`, with one selected test, valid
header/frames/counters, zero overflow/drop/claim, exact installed and reaped PID,
bounded output and complete process/descriptor cleanup. F3 and every sibling
remain stopped.

## Scope, caps and stops

Durable edits are limited to scheduling/status sections in the canonical plan,
this manifest, Stage 4, bootstrap readiness and the configured CLI ledger.
Temporary Rust owners are only `runtime/dice.rs`, `probe_observer.rs`, its
`mapping.rs`, `tests/path_observation_dice_events.rs`, `slug_cli_v2/src/lib.rs`
and `slug_cli_v2/tests/cli.rs`; use one excluded scratch supervisor. Caps are
100 gross production, 130 gross proof and 120 changed scratch lines. No Cargo
manifest, DICE crate, workspace/path owner, loading owner, fixture or oracle
input may change.

Restore all temporary source and scratch artifacts before recording the result;
prove both worktrees clean except for the documentation receipt and run the
plan-status and diff checks. Independently review this design before execution
and the result before selecting any successor.

Do not rerun the shard-key audit, handoff, shard, batching or census audit; F3;
a sibling selector; or the selected selector without this exact instrumentation.
Do not raise limits, acquire a payload, emit identity material, change merge,
injection or DICE semantics, broaden to `ResolvedPathObservationKey`, merge the
combined stack or push the review branch.

Independent design rereview returned `ACCEPT` after requiring the final active
window and `W = C - 1`, rejecting inactive completed cutoffs, covering every
consumer-arm partial, preserving inherited event accounting in the filter
proof, and restoring the exact selector, launcher and evidence gates.

## Core preparation launcher recovery

The first bounded Core preparation resolved bare `cargo` through the blocked
system Snap wrapper and exited before Cargo, rustc or any test process started.
It produced no executable and ran no selector, CLI preparation, fixture or
replay. The frozen 100-line production diff has SHA-256
`980b13e379b4219db72a22e637b5456b5d59ec6dce13710dfe777eeef9b76e48`;
the 116-line Core proof has SHA-256
`e2fab12052baf3171e4b0c7a0d76b4fc805b85097f7ade49dacf5b855cd18f50`
and remains 130 lines with the 14-line CLI adapter.

One reviewed recovery may invoke the pinned absolute Cargo binary directly,
with the pinned PATH and `CARGO_TARGET_DIR=/home/wgray/slug/target`, to prepare
the same exact target once under the unchanged 60-second ceiling. All source,
proof, cap, CLI, fixture and replay boundaries remain frozen.
Independent recovery review returned `ACCEPT` because the failed launcher
entered neither Cargo nor rustc. Do not repeat the bare-Cargo command.

The corrected absolute-Cargo preparation entered the intended Core target and
exited 101 after 7.3 seconds on one proof-only tuple assertion mismatch: actual
values were slice references while expected values inferred array references.
The production library compiled, but no integration executable or selector was
produced; the CLI, fixture and replay remain stopped. One reviewed line-neutral
correction may coerce the two expected arrays to slices and prepare the same
target once under the unchanged pinned 60-second command. The production hash
and exact 100/130 caps remain frozen.
Independent recovery review returned `ACCEPT` for the line-neutral slice
coercion and unchanged target, launcher, shared target directory and ceiling.

The corrected Core target prepared in 4.96 seconds, its exact selector listed
once and passed 1/1. The 81/120-line scratch supervisor passed syntax checking,
then its escalated self-check stopped in the decoder matrix before cleanup
cases. The synthetic valid phase frame wrote phase 6 and status 1 into separate
words, while the protocol packs status into the high 16 bits of the phase word.
No CLI preparation, fixture or replay ran. One reviewed line-neutral
scratch-fixture correction may encode word 14 as `6 | (1 << 16)` and rerun the
self-check once through `/bin/bash`; decoder, source and proof stay frozen.
Independent recovery review returned `ACCEPT` because the correction changes
only the synthetic packed phase/status word.

The corrected supervisor self-check then passed its complete matrix. The
feature-enabled `cli` integration target prepared in 10.10 seconds, its exact
nonignored selector listed once, and a fresh fixture verified at the accepted
28-object inventory hash. Independent preexecution review returned `REVISE`
before any CLI replay: the filtered helper could still write after observer
disable, the Core proof omitted the actual active-clear producer partial, and
feature mode could fall back to ordinary fixture assembly when its inherited
scratch variable was absent.

The bounded correction checks disabled state before filtered activity, adds the
missing partial and an exact post-disable no-write proof, and gives feature mode
a required external workspace while retaining the assembler only outside that
feature. Offset derivation and proof-only line folding preserve exact 100/130
production/proof caps; the scratch stays 81/120. Rerun the changed Core proof,
unchanged supervisor self-check, CLI preparation/preflight, fresh fixture and
artifact freeze before another independent preexecution review. The replay
remains unconsumed.

The corrected Core target prepared in 8.24 seconds and its exact selector
listed once, but the proof failed only at the new post-disable full-array
assertion. It snapshotted before dropping `ProbeGuard`, whose drop intentionally
sets observer word 3 to disabled; that was the sole changed word. No CLI rebuild,
fixture/hash refresh or replay followed. One reviewed line-neutral proof-only
reorder may drop the guard, snapshot the disabled observer, emit the exact-tag
event and compare the full array, then rerun the affected gates. Production and
the exact 100/130 caps stay frozen.
Independent recovery review returned `ACCEPT` because snapshotting after guard
drop includes the intentional disabled transition and isolates the event.

The affected gates passed and the final artifacts were frozen, but independent
preexecution rereview returned `REVISE`: the preceding invalid consumer case
left the final post-disable proof disarmed, so it would pass even without the
disabled guard. The line-neutral correction first resets words 51--63 to a
coherent armed state `P = C = 1`, `W = 0`, pending zero, active one and clear
flags/reserved, then drops the guard, snapshots, emits the exact event and
compares all words. Rerun only the affected Core preparation/proof, refresh the
Rust/Core hashes and obtain rereview. The CLI replay remains unconsumed.

The corrected Core target prepared in 4.67 seconds and its exact selector
listed once and passed 1/1. Final SHA-256 values are: Rust diff
`ba842406b59ef65dcf911a0d7cf3bc1ea1d61f7a72f0669b721e8f38e0085122`,
Core harness `ce1f292da4c1a18d69eabdb54d4c2357e10234cc50256744f7d2fbdaa766f5e1`,
scratch supervisor `cde2f2b84f8ba7c995cffb09c4dac989c6522b18e107b0e671f8b7cfa4e83a76`,
CLI harness `18e86c12665f068a3bb3e55d15915247f884ca97cbb12a194b0dfeff2bd6f1cd`
and Slug binary
`2fb664e3d85c6727521f4091fa24ce61670d523b7e98b1a8eb653974daeb701f`.
The accepted supervisor remains `3b4d48a4a7c729ec0c8c17ffd2aed3ddab8677699bdce9d8bb24d97afbeab69c`.
Independent preexecution rereview returned `EXECUTE` for exactly one specified
escalated `/bin/bash ... --portable-run` invocation. Do not repeat it.
