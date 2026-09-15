# Current Slug V2 Work Packet

Packet: WP-4-7A-configured-conflict-path-shard-dice-event-audit-r1
Status: Core preparation recovery accepted; corrected preparation pending

## Accepted predecessor receipt

The aggregate merge-to-committed-injection handoff audit is accepted. Its sole
configured replay reached the 12-second deadline after 12.0088119506836 seconds
in `RootCompute`. It recorded 218 installed path merges and 218 matching
successful injection commits, 1,591 total potentially exposed prior residents,
a maximum exposure of 85 and buckets `[52, 125, 24, 17, 0]`. Pending value and
marker, flags and reserved state were zero. Observer and reaped PID were both
692127; base counters were `[55217, 55208, 48837, 48832, 8043, 8039]` with
overflow, dropped samples and activity claim zero. Process group, children,
pipes and telemetry cleanup were clear.

The authentic fixture remained 28 objects, 177 metadata entries and 8,004,740
bytes at inventory SHA-256
`4337d0756cefc0971a76e12bbeea54ee40c24beb0ff943a4c3bdc60d88ed764f`.
Frozen pre-execution SHA-256 values were:

| Artifact | SHA-256 |
|---|---|
| accepted supervisor | `3b4d48a4a7c729ec0c8c17ffd2aed3ddab8677699bdce9d8bb24d97afbeab69c` |
| scratch supervisor | `c811d782293e2a876489249d1d41c7e477cbf81181cdf92213141f961a0e9a72` |
| exact temporary Rust diff | `7c9c2cc7b90b6d65ad6a7c0060465029c304c5ffb5257086f864ee57d92dc5e8` |
| initial Core proof harness | `56c8a68c0e2b3c03c09d8f6f61fe1cde8e4da350e4a59c6438ba947da14a91f5` |
| CLI integration harness | `d9f40f34d09cf478925e3b3ab2f5e511a38cf7f620ac3545af3e7f9ab6a94e84` |
| spawned Slug binary | `303be94ae38f921638b23d351dbde850ba79aaa48f220e9852968aa15b8b8701` |

The executed diagnostic was 133 production lines against the transparently
corrected 133-line cap. Initial result review rejected its incomplete 85-line
Core proof. The reviewed proof-only recovery added bucket boundaries 7, 31 and
127, aggregate overflow, every required consumer partial state and invalid
aggregate-before-clear arithmetic. The resulting 95-line Core file plus 14 CLI
proof lines totaled 109/140. Its dedicated target rebuilt in 5.05 seconds, the
exact selector listed once and passed 1/1. No production, supervisor, CLI,
binary, fixture or replay artifact changed, and the CLI did not rerun.
Independent rereview returned `ACCEPT` for the augmented proof and frozen
result. This proves only that aggregate potential shard exposure was handed to
successful injection commits. It does not prove shard-key events, invalidation,
checking, recomputation, causality, cost or avoidable work. All temporary source
and scratch artifacts were removed and both worktrees were clean.

## Observable result

Audit whether any of the six existing DICE event categories for the exact
static tag `PathObservationShardKey` occur while an aggregate observation
window is armed by a successfully committed installed-merge handoff. Count only
`Started`, `Finished`, `CheckDepsStarted`, `CheckDepsFinished`,
`ComputeStarted` and `ComputeFinished` in `Observer::event`.

A positive result establishes only that shard-key events occur after committed
path handoffs within the command-loop windows described below. It cannot
attribute any event to a particular merge, shard, demand or invalidation; prove
that the event was necessary or causal; distinguish checking from useful work;
or measure cost.

## Window lifecycle and ordering

Reuse the serialized command-loop boundaries from the accepted handoff audit,
but retain no exposure value. After a merged path epoch is successfully
assigned in `NativeDemandSession::progress_inner`, close the currently armed
window, if any, and then publish one pending installed merge. The next
successful transaction commit immediately after `guard.inject_attempt` consumes
that pending merge and arms the next window before `AttemptInjectionRevision`
finishes. Initial and non-path commits are neutral when no merge is pending.

The producer first requires no pending marker, produced equal to committed and
`committed = closed + active`. If a window is active, clear its active marker;
then increment produced, increment closed when a window was cleared, and publish
the pending marker last with release ordering. This order makes the state after
every individual write decoder-invalid until publication completes. The
consumer requires a pending marker, produced equal to committed plus one, no
active window and committed equal to closed. It clears pending first, increments
committed, then publishes the active marker last with release ordering. An
absent-pending consumer is neutral only while produced equals committed. Any
overwrite, unmatched count, invalid marker or arithmetic overflow sets sticky
state and refuses further publication.

`Observer::event` first preserves the existing global counters and activity
sampling. It then compares the callback's static tag exactly with
`PathObservationShardKey`. When the active marker is one under acquire ordering,
it increments exactly the counter matching the event variant. Wrong tags and
events observed while disarmed are neutral. Event callbacks may be concurrent;
the audit therefore claims only membership in an atomically observed aggregate
armed interval, not per-event linearization at the close boundary.

## Fixed observer layout and cutoff

Reuse only words 51--63 of the accepted 512-byte mapping:

| Word | Aggregate |
|---:|---|
| 51 | installed path merges produced |
| 52 | pending merges reaching successful injection commit |
| 53 | armed windows closed by a later installed merge |
| 54 | `PathObservationShardKey` `Started` |
| 55 | `PathObservationShardKey` `Finished` |
| 56 | `PathObservationShardKey` `CheckDepsStarted` |
| 57 | `PathObservationShardKey` `CheckDepsFinished` |
| 58 | `PathObservationShardKey` `ComputeStarted` |
| 59 | `PathObservationShardKey` `ComputeFinished` |
| 60 | pending merge marker, only 0 or 1 |
| 61 | active window marker, only 0 or 1 |
| 62 | sticky flags: bit 0 state/order failure, bit 1 lifecycle or filtered-counter overflow |
| 63 | reserved zero |

Let `P`, `C`, `W` be produced, committed and closed. A valid post-reap cutoff
requires flags and reserved state zero, both markers in range, pending zero,
`P = C > 0`, and `C = W + active`. Require the checked sum of the six filtered
counters to be nonzero and each filtered counter to be no greater than its
corresponding existing global counter in words 8--13. No inequality between
filtered categories is valid: an armed boundary may bisect a concurrent event
pair, and one evaluation may both check dependencies and compute. The decoder
reports all six counts and markers without identity material. Any zero armed
handoff, zero filtered total, filtered/global mismatch, overflow or lifecycle
arithmetic failure forces `REPLAN`.

## Proof and sole replay

Add one exact feature-enabled dedicated Core integration selector,
`path_shard_dice_events_follow_committed_path_handoffs`. Exercise the production
producer, consumer and event-filter helpers through initial neutral commit;
first production and consume; next-merge close and rearm; wrong-tag and
disarmed events; all six exact event variants; overwrite and unmatched states;
lifecycle and each filtered-counter overflow; and every partial close,
producer, consume and arm snapshot. Prove marker-last publication, active-last
arming, close-clear/produce/close-count ordering, filtered/global bounds,
unchanged words 0--50, reserved word 63 and exact tag discrimination. Prepare only this target once under 60 seconds
with the pinned toolchain and shared target directory, then preflight and run
only its exact selector under the existing 12/15-second bounds.

Copy the accepted supervisor to an excluded scratch file and extend only its
words 51--63 decoder and result. Its self-check must cover the lifecycle
equations, marker ranges, all partial cutoffs, each filtered/global bound and
checked sum, zero-event rejection, both sticky flags, output cap and normal/deadline/
exception cleanup. Preserve namespace/resource isolation, observer ownership,
exact reads, wall deadline, kill/reap finalizer and caps.

Compile the feature-enabled CLI integration harness once under 60 seconds,
preflight the same exact nonignored configured build-conflict selector,
reassemble and verify the accepted fixture, and hash accepted/scratch
supervisors, exact Rust diff, Core/CLI harnesses and Slug binary. Run that one
selector once through the scratch supervisor under 12/15 seconds. F3 and every
sibling selector remain stopped. Evidence also requires valid observer
header/version/base counters and sampling, clear base overflow/drop/claim,
exact installed/reaped PID, output caps and complete process/descriptor cleanup.

## Scope, caps and stops

Durable edits are limited to scheduling/status sections in the canonical plan,
this manifest, Stage 4, bootstrap readiness and the configured CLI ledger.
Temporary Rust edits are limited to
`app/slug_core_v2/src/runtime/probe_observer.rs`,
`app/slug_core_v2/src/runtime/probe_observer/mapping.rs`,
`app/slug_core_v2/src/runtime/dice.rs`,
`app/slug_core_v2/tests/path_shard_dice_events.rs`,
`app/slug_cli_v2/src/lib.rs` and `app/slug_cli_v2/tests/cli.rs`. Caps are 100
gross diagnostic production lines, 130 gross proof lines and 120 changed
scratch-supervisor lines. No Cargo manifest, DICE crate, workspace/path owner,
loading owner, fixture or oracle input may change.

Restore all temporary source and scratch artifacts before recording the result;
prove both worktrees clean except for the documentation receipt and run the
plan-status and diff checks. Independently review this design before execution
and the result before selecting a successor.

Do not rerun the handoff, shard, batching or census audit; F3; a sibling
selector; or the selected selector without this event instrumentation. Do not
raise a limit, acquire a payload, emit identity material, change merge,
injection or DICE semantics, merge the combined stack or push the review branch.

Independent design review returned `ACCEPT` after requiring the producer to
increment produced before closed, eliminating a decoder-valid partial cutoff,
and replacing unsound event-pair inequalities with exact filtered-to-global
counter bounds across concurrent window edges.

## Core preparation recovery

The first pinned dedicated-target preparation exited 101 after 4.73 seconds on
one proof-only type mismatch: `libc::pwrite` requires its offset as `off_t`, but
`write_word` supplied `index * 8` as `usize`. The production library compiled;
no test executable, preflight, proof, supervisor, CLI build or replay occurred.
The frozen production diff SHA-256 is
`4183595095e1e951575fd88e21bcd438c50f45fb27accac54f1cc1b90c429b5f`.
The uncorrected exact 130-line proof SHA-256 is
`c6b12f38bf7231babb0e277a207f6b279d827a7d0f4d222b2af82e71cc96f0bc`.

One recovery may change only that proof expression to an explicit checked or
lossless `libc::off_t` conversion and prepare the same dedicated target once
under the unchanged 60-second ceiling. It may then preflight and run only the
same exact selector under 12/15 seconds. Preserve the 99-line production diff,
130-line proof cap, scratch supervisor, CLI and replay stops. The earlier
unpinned `rustfmt` launcher also failed before formatting through the blocked
system Snap wrapper; the corrected absolute pinned `rustfmt` invocation passed
and may not be repeated without a later source change.
Independent recovery review returned `ACCEPT` for a checked multiplication and
`libc::off_t::try_from` conversion or an equivalently proven lossless
conversion, with the same target and limits.
