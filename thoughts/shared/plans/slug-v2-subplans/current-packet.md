# Current Slug V2 Work Packet

Packet: WP-4-7A-configured-conflict-path-observation-activation-dependency-audit-r1
Status: frozen preexecution review returned EXECUTE; sole replay authorized

## Accepted predecessor receipt

The sole exact-`PathObservationKey` event replay is accepted as a bounded
positive result. The exact nonignored selector
`configured_action_conflicts::one_shot_build_conflict_is_atomic_and_recovers`
listed once and began one test, then reached the 12-second wall deadline after
12.0098099708557 seconds in `RootCompute`. Raw status was 9. Observer and sole
reaped run PID were both 735061; process group, children, pipes, telemetry and
bounded output cleanup were clear.

Global event counters were `[52497, 52488, 46078, 46073, 8120, 8116]` with
overflow, dropped samples and activity claim zero. The coherent handoff
lifecycle was produced 221, committed 221, closed 220, pending zero, active one
and flags/reserved zero. Exact `PathObservationKey` events inside those windows
were `[2568, 2568, 1618, 1618, 2568, 2568]`, total 13,508; every filtered count
was at most its matching global count.

The authentic fixture verified 28 objects, 177 registry metadata entries and
8,004,740 bytes at inventory SHA-256
`4337d0756cefc0971a76e12bbeea54ee40c24beb0ff943a4c3bdc60d88ed764f`.
Frozen preexecution SHA-256 values were:

| Artifact | SHA-256 |
|---|---|
| accepted supervisor | `3b4d48a4a7c729ec0c8c17ffd2aed3ddab8677699bdce9d8bb24d97afbeab69c` |
| scratch supervisor | `cde2f2b84f8ba7c995cffb09c4dac989c6522b18e107b0e671f8b7cfa4e83a76` |
| exact temporary Rust diff | `ba842406b59ef65dcf911a0d7cf3bc1ea1d61f7a72f0669b721e8f38e0085122` |
| Core proof harness | `ce1f292da4c1a18d69eabdb54d4c2357e10234cc50256744f7d2fbdaa766f5e1` |
| CLI integration harness | `18e86c12665f068a3bb3e55d15915247f884ca97cbb12a194b0dfeff2bd6f1cd` |
| spawned Slug binary | `2fb664e3d85c6727521f4091fa24ce61670d523b7e98b1a8eb653974daeb701f` |

The final temporary totals were 100/130/81 against 100/130/120 production,
proof and scratch caps. The exact Core selector passed 1/1, the supervisor
self-check passed, and independent preexecution review returned `EXECUTE` only
after adding disabled-state filtering, all publication partials, a fail-closed
feature fixture and an armed post-disable proof. Independent result review
returned `ACCEPT` and selected only the activation classification below. All
temporary source, fixture and scratch artifacts were removed; both worktrees
were clean, and the event audit will not rerun.

This result proves only that ordinary `PathObservationKey` DICE events occurred
during 221 aggregate post-handoff windows. It does not attribute an event to a
merge, path or invalidation; establish causality or necessity; measure cost; or
identify avoidable recomputation.

## Observable result

At the existing legacy `RuntimeActivationTracker::key_activated` boundary in
`runtime/demands.rs`, classify only exact `PathObservationKey` callbacks while
the committed-handoff window is armed. Count evaluated versus reused activation
data and whether the callback's immediate dependency iterator contains an exact
`PathObservationShardKey`. This is explicitly the legacy-delivered subset.
DICE direct cache hits and other reuse paths delivered only through
`key_activated_rich` are excluded, so `R` is not total reuse and these counters
are not a complete activation census.

Publish only six aggregates in words 54--59:

| Word | Aggregate |
|---:|---|
| 54 | evaluated activations |
| 55 | reused activations |
| 56 | activations with an immediate shard-key dependency |
| 57 | activations without that dependency |
| 58 | evaluated activations with that dependency |
| 59 | reused activations with that dependency |

No node ID, demand, path, dependency identity, label or retained payload may
cross the observer boundary. A positive evaluated-with-shard count proves only
the activation kind and an observed immediate dependency during an aggregate
armed window. It does not prove invalidation, causal recomputation, cost or
avoidable work.

## Lifecycle, concurrency and cutoff

Reuse the accepted producer/consumer window lifecycle in words 51--53 and
60--62: a successful installed path merge closes the prior active window and
publishes pending last; its next successful injection commit consumes pending
and publishes active last. Stable cutoff requires produced `P` equal committed
`C` and greater than zero, pending zero, active one, closed `W = C - 1`, and
flags zero.

Use word 63 as an activation-callback claim, restricted to zero or one. After
exact key and initial armed/disabled checks, acquire the claim before consuming
the dependency iterator, then recheck armed and disabled state. Classify the
dependency and activation kind and precheck every affected counter. Publish in
this exact order: evaluated/direct `ED, D, E`; evaluated/no-direct `N, E`;
reused/direct `RD, D, R`; reused/no-direct `N, R`. Thus direct callbacks update
three aggregates and no-direct callbacks update two. Publish claim zero last
with release ordering. Wrong keys and callbacks observed disarmed or disabled
are neutral.
Contention, invalid lifecycle, counter overflow or an invalid claim sets sticky
state and refuses publication. A killed or incomplete classified callback
leaves either claim one or invalid arithmetic; concurrent callback interleaving
cannot create an apparently valid cutoff.

Let `E`, `R`, `D`, `N`, `ED` and `RD` denote words 54--59. After checked
addition, require `E + R = D + N`, `D = ED + RD`, `ED <= E`, `RD <= R`, and
`E + R > 0`. There is no inherited stable global activation total, so impose no
global or event-counter bound. Validate only these checked equations and bounds,
the nonzero total, lifecycle, claim and flags. Any arithmetic overflow, claim
nonzero, invalid lifecycle or zero classified total forces `REPLAN`.

Preserve `RuntimeActivationTracker` rich and root callbacks, demand provenance,
effect tracking and tracker installation exactly. The diagnostic legacy
callback is otherwise empty. Do not change DICE, path-workspace or injection
semantics.

## Proof and sole replay

Add one exact feature-enabled dedicated Core proof. Exercise the production
window and activation helpers through all four legacy-delivered cells: evaluated/direct,
evaluated/no-direct, reused/direct and reused/no-direct. Cover both activation
kinds, exact key downcasts, exact immediate-dependency downcasts, wrong key,
armed/disarmed/disabled neutrality, claim contention, all lifecycle and
classification overflows, every partial two-or-three-counter publication in its
exact order, claim-last release, checked arithmetic and a killed-partial
representation. Prove no identity words, rich-only callback exclusion,
unchanged unrelated observer words, and unchanged rich/root delegation and
demand-provenance behavior. Prepare under 60 seconds and run only
its exact selector under 12/15 seconds.

Derive one excluded scratch supervisor from the accepted bytes. Extend only the
words 51--63 decoder/result and its self-check for lifecycle, claim, every
arithmetic equation and bound, zero total, partial callbacks, both flags,
output cap and normal/deadline/exception cleanup. Preserve namespace isolation,
observer ownership, exact reads, wall deadline, kill/reap finalization and
resource caps.

Compile only the feature-enabled `cli` integration target under 60 seconds.
Only exact selector
`configured_action_conflicts::one_shot_build_conflict_is_atomic_and_recovers`
adopts the inherited observer before its required external-fixture case and
holds the guard throughout. Preflight `--list --exact <selector>` and run only
`--exact <selector> --nocapture`, without `--ignored`, through the 0644 scratch
script's `/bin/bash` launcher and established namespace/ptrace escalation.
Verify a fresh accepted fixture, freeze accepted/scratch supervisors, exact
Rust diff, Core/CLI harnesses and Slug binary, then obtain independent
preexecution review before the one 12/15-second replay.

Accept evidence only at the expected `RootCompute` wall deadline with one
selected test, valid observer header/frames/counters, coherent lifecycle and
classification arithmetic, zero overflow/drop/activity claim/callback claim,
exact installed/reaped PID, bounded output and complete process/descriptor
cleanup.

## Scope, caps and stops

Durable edits are limited to status/scheduling sections of the canonical plan,
this manifest, Stage 4, bootstrap readiness and configured CLI ledger.
Temporary Rust owners are only `runtime/demands.rs`, `runtime/dice.rs`,
`runtime/probe_observer.rs`, its `mapping.rs`, one dedicated Core proof,
`slug_cli_v2/src/lib.rs` and `slug_cli_v2/tests/cli.rs`, plus one excluded
scratch supervisor. Caps are 180 gross production, 220 gross proof and 140
changed scratch lines. No manifest, DICE crate, workspace/path owner, loading
owner, fixture or oracle input may change.

Restore every temporary source and scratch artifact before recording the
result; prove both worktrees clean except for its documentation receipt and run
plan-status and diff checks. Independently review this design before execution
and the result before selecting any successor.

Do not rerun the event, shard-key, handoff, shard exposure, batching or census
audits; F3; a sibling selector; or the selected selector without this exact
instrumentation. Do not broaden to `ResolvedPathObservationKey`, select
optimization work, raise limits, emit identity material, change semantics,
merge the combined stack or push the review branch.

Independent design rereview returned `ACCEPT` after limiting the audit to the
legacy-delivered callback subset, excluding rich-only reuse, freezing each
two-or-three-counter publication order and removing any invented global
activation bound.

The first Core preparation compile exposed only bounded integration defects:
the legacy dependency iterator required an explicit `next` loop, the included
proof needed direct imports, and observer capture belongs inside tracker
construction rather than in the install signature. Independent recovery review
returned `ACCEPT`; the corrected preparation compiled in 11.57 seconds. The
exact selector then listed once and began one test, but failed before diagnostic
assertions because tracker acquisition constructed the cycle detector outside a
Tokio reactor. Independent recovery review accepted acquiring that tracker
inside the runtime's existing `block_on` and authorized one corrected exact
proof rerun. No CLI build, fixture validation or selected replay was consumed.

The corrected exact Core selector passed 1/1. Preexecution review first required
actual rich/root tracker exercise, every frozen publication partial, independent
`ED <= E`, `RD <= R` and checked `W + 1` discriminators, and one exact reaped
PID. Those corrections passed the exact proof and supervisor self-check;
independent rereview returned `EXECUTE`. Final totals are 166/217/85 against
180/220/140 production, proof and scratch caps. The fresh fixture again verified
28 objects, 177 metadata entries, 8,004,740 bytes and inventory
`4337d0756cefc0971a76e12bbeea54ee40c24beb0ff943a4c3bdc60d88ed764f`.
Frozen SHA-256 values are:

| Artifact | SHA-256 |
|---|---|
| base supervisor | `d9a012989d595e6da275094d4ce020a8e8a8214e1714df79e8a3285282191179` |
| activation supervisor | `b33d749f9c53897f82ba141519bbdca5c0b05bf049108e71a07bfe2876da1ceb` |
| exact Rust diff | `77817b67e6e0029f44e5e9d28e156f86be7f4877e08f33c6d9d54fb3e0155f11` |
| Core proof harness | `27f71dd21e80c727b7c8775a1001f9750b4c99360bea37ca3b2304c33c9eaeb8` |
| CLI integration harness | `613e25da51dbeaae395bf846062494126c769f05717fcc3f0e3ca83512e830f0` |
| Slug binary | `2fe5c70984f30096fe27dd14527a41b928b0d2a21b85a12b068caeb13ad26268` |
| CLI harness binary | `b80d3939a1ac8a6980deaa9f016c17bf08edc4262f1607c576ef05f274eaad45` |

Exactly one portable replay of the frozen nonignored selector is authorized.
There is no replay retry.
