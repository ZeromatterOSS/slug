# Current Slug V2 Work Packet

Packet: WP-4-7A-configured-conflict-path-merge-injection-handoff-audit-r1
Status: proof-coverage recovery accepted; Core proof pending

## Accepted predecessor receipt

The sole path-shard exposure replay passed every reviewed gate. The dedicated
Core integration target prepared in 9.21 seconds and its exact production-helper
selector passed 1/1. The bounded supervisor passed its normal, deadline and
exception cleanup self-checks. The CLI integration harness prepared in 14.51
seconds and exactly listed the one nonignored configured build-conflict test.
The fresh authentic fixture again reproduced 28 objects, 177 metadata entries
and 8,004,740 bytes at inventory SHA-256
`4337d0756cefc0971a76e12bbeea54ee40c24beb0ff943a4c3bdc60d88ed764f`.

Pre-execution SHA-256 values were:

| Artifact | SHA-256 |
|---|---|
| accepted supervisor | `3b4d48a4a7c729ec0c8c17ffd2aed3ddab8677699bdce9d8bb24d97afbeab69c` |
| scratch supervisor | `810840822f7e11262b9f0959a61b01028bb1c4dc58600337679658c85c348ad8` |
| exact temporary Rust diff | `1f572a52dd00eb27f32b52a9be4fe249c695d0b8926c8b2076880faff001aef1` |
| Core proof harness | `94f146057fe93963333d6fb7c9282891fbcd22f9bbbcba1e355ed12afa8d03f6` |
| CLI integration harness | `a862beca34c4792ce120cdc6caa2036dbe14644a4b5ce2a8a2db229b3c33bc9b` |
| spawned Slug binary | `ad0048114ccd20a65164c4fd97a6d2d3ee9586deca49e1c233df99a307ace9bc` |

The one replay reached its 12-second wall deadline after 12.011358 seconds in
`RootCompute`. The observer and sole reaped run PID were both 678184. Observer
counters were `[51024, 51007, 44784, 44771, 7836, 7832]`; overflow, dropped
samples and activity claim were zero. Selector and run supervision reported no
live group, children, pipes or telemetry error.

The committed cutoff recorded 209 installed path merges, 438 changed shards,
462 newly added demands and 1,528 prior demands resident in changed shards.
Maxima were 18 changed shards and 112 prior residents. Prior-exposure buckets
were `[49, 124, 21, 15, 0]`. All bucket, maximum-presence, cap and aggregate
arithmetic closed. Independent review returned `ACCEPT`: these are only
potential exposures before a later injection, usually across few shards. They
do not prove DICE invalidation, checking, recomputation, cost or avoidable work.

Temporary production/proof/scratch changes were 99/121/81 against 120/130/120
caps. All temporary source and scratch artifacts were removed, both worktrees
were clean, and no selector reran.

## Observable result

Audit the handoff from one installed path merge in
`NativeDemandSession::progress_inner` to the next successfully committed DICE
transaction immediately after `guard.inject_attempt` in `drive_command`.
Record produced and committed merge counts plus only the prior-exposure size
carried by a matched handoff. A positive result means that potential exposure
reached a committed injection transaction. It does not prove that any shard key
was invalidated, requested, checked or recomputed.

Use one aggregate pending slot in the existing observer mapping. An installed
merge produces one bounded prior-exposure count. The next successful updater
commit consumes it and adds that value to committed totals and one fixed bucket:
0, 1--7, 8--31, 32--127 or 128+. Initial and non-path attempt commits with no
outstanding producer are neutral when produced equals committed. Retain no
path, demand, shard identity, epoch, result, transaction, call site or timestamp.

## Producer and consumer boundaries

The producer is feature-only and runs after the successfully constructed merged
epoch has been assigned to `self.path_observations`. As in the accepted shard
audit, admit both prior and installed epochs only when each contains at most
4,096 demands, then compare their accepted `path_observation_shards`
projections and sum prior residents in changed shards. On cap, conversion,
no-change or arithmetic failure, set the sticky error flag and publish no
producer.

Require the idle producer state `marker = 0` and produced equal to committed.
Otherwise set overwrite for a present marker or unmatched for unequal counts,
and refuse. Write the pending exposure value, increment produced, then publish
marker 1 last with release ordering. Thus a cutoff before marker publication
has produced greater than committed and is invalid. The observer retains only
that one integer and marker until consumption.

The consumer runs only after either `commit_native_attempt(updater)` or
`request_revision.commit(updater)` returns the successfully committed
transaction, and before `AttemptInjectionRevision` finishes. If a pending
producer exists, acquire marker 1, require produced equal to committed plus one,
read the value, update the committed bucket and exposure total/max, zero the
pending value, clear the marker, and increment committed last with release
ordering. If no pending producer exists while produced differs from committed,
set the unmatched flag. A commit with produced equal to committed is neutral.
The command loop serializes every producer and consumer helper; no concurrent
caller is permitted. A producer or consumer encountering an intermediate state
sets a flag and refuses. Preserve all updater, revision, transaction, phase,
retry and error behavior.

## Fixed observer layout

Reuse only words 51--63 of the accepted 512-byte observer:

| Word | Aggregate |
|---:|---|
| 51 | installed path merges produced |
| 52 | produced merges reaching successful injection commit |
| 53 | total committed potential prior exposure |
| 54 | maximum committed potential prior exposure |
| 55 | committed exposure 0 |
| 56 | committed exposure 1--7 |
| 57 | committed exposure 8--31 |
| 58 | committed exposure 32--127 |
| 59 | committed exposure 128+ |
| 60 | pending prior-exposure value |
| 61 | pending marker, only 0 or 1 |
| 62 | sticky flags: bit 0 overwrite, bit 1 unmatched, bit 2 cap/conversion/counter overflow |
| 63 | reserved zero |

Let `P` be produced, `C` committed, `E` committed exposure total, `Emax`
its maximum and `b0..b4` the buckets. A valid post-reap cutoff requires flags
and reserved state zero, pending marker/value zero, `P = C > 0`, bucket sum
equal to `C`, `Emax <= 4,096`, and `E <= 4,096*C`. If `h` is the bucket
containing `Emax`, with lower bound `Lh`, require `bh > 0`, no higher populated
bucket, weighted lower
`b1 + 8*b2 + 32*b3 + 128*b4 <= E`, maximum-attainment lower equal to that sum
plus `Emax - Lh`, and weighted upper
`E <= min(7,Emax)*b1 + min(31,Emax)*b2 + min(127,Emax)*b3 + Emax*b4`.
Require `E = Emax = 0` exactly when only `b0` is populated. Check every sum and
product before use.

## Proof and sole replay

Add one exact feature-enabled dedicated Core integration selector,
`path_merge_injection_handoff_matches_produced_and_committed_exposure`. Drive
the production producer/consumer helpers through neutral initial consumption,
all bucket boundaries, same-shard and cross-shard additions, prior residents in
changed/unchanged shards, successful consume, overwrite refusal, unmatched
state, 4,096 cap, counter overflow, and every partial producer/consumer snapshot
at the value, count, marker, aggregate, clear and commit boundaries. It also
proves commit-last ordering, unchanged words 0--50 and reserved word 63.
Prepare only this target once under 60 seconds with the
pinned toolchain and shared target directory; preflight and run only its exact
selector under the existing 12/15-second bounds.

Copy the accepted supervisor to an excluded scratch file and extend only the
words 51--63 decoder and result. Self-check bucket boundaries, every partial
producer/consumer snapshot, overwrite/unmatched/overflow flags,
maximum-presence arithmetic, output cap and
normal/deadline/exception cleanup. Preserve namespace/resource isolation,
observer ownership, exact read, wall deadline, kill/reap finalizer and caps.

Compile the feature-enabled CLI integration harness once under 60 seconds,
preflight the same exact nonignored configured build-conflict selector,
reassemble and verify the accepted fixture, and hash the accepted/scratch
supervisors, exact Rust diff, Core/CLI harnesses and Slug binary. Run the one
selector once through the scratch supervisor under 12/15 seconds. F3 and every
sibling selector remain stopped.

Evidence also requires valid observer header/version/counters/sampling, clear
base overflow and released claim, exact installed/reaped PID, output caps and
complete process/descriptor cleanup. Replan on pending/unmatched/overwrite or
overflow state, zero committed handoffs, weak arithmetic, path/identity output,
proof/preparation failure or invalid cleanup. Independent result review may
select only a still narrower DICE injection event audit.

## Scope, caps and stops

Durable edits are limited to scheduling/status sections in the canonical plan,
this manifest, Stage 4, bootstrap readiness and the configured CLI ledger.
Temporary Rust edits are limited to
`app/slug_core_v2/src/runtime/probe_observer.rs`,
`app/slug_core_v2/src/runtime/probe_observer/mapping.rs`,
`app/slug_core_v2/src/runtime/dice.rs`,
`app/slug_core_v2/tests/path_merge_injection_handoff.rs`,
`app/slug_cli_v2/src/lib.rs` and `app/slug_cli_v2/tests/cli.rs`. Caps are 133
gross diagnostic production lines, 140 gross proof lines and 120 changed
scratch-supervisor lines. No Cargo manifest, DICE crate, workspace/path owner,
loading owner, fixture or oracle input may change.

Restore all temporary source and scratch artifacts before recording the result;
prove both worktrees clean except for the documentation receipt and run the
plan-status and diff checks. Independently review this design before execution
and the result before selecting a successor.

Do not rerun the shard audit, batching audit, census, F3, a sibling selector or
the selected selector without this handoff instrumentation. Do not raise a
limit, acquire a payload, emit identity material, change merge/injection/DICE
semantics, merge the combined stack or push the review branch.

Independent design rereview returned `ACCEPT` for marker-last production,
pending-clear/count-last consumption, rejection of every partial cutoff and
serialized command-loop ownership.

## Result-review recovery

The sole replay and all six pre-execution hashes are frozen. Independent result
review found its runtime cutoff coherent but returned `REJECT` because the
executed production diff is 133 gross lines, not the planned 130: 10 additions
in `dice.rs`, 116 in `probe_observer.rs`, four in its mapping and three deleted
CLI adapter lines. Correct the production cap transparently to 133 without
changing that diff or rerunning any production/CLI artifact.

The same review found the 85-line Core integration proof omitted bucket upper
boundaries 7, 31 and 127, aggregate counter overflow, and explicit
value/count/marker/aggregate/clear/commit partial-consumer coverage. The bounded
recovery may edit only
`app/slug_core_v2/tests/path_merge_injection_handoff.rs`, remain within the
existing 140-line proof cap, prepare that same dedicated target once under 60
seconds, and preflight/run only its exact selector under 12/15 seconds. Preserve
the frozen production diff, supervisor, CLI harness, Slug binary, fixture and
replay receipt byte-for-byte. Do not rerun the CLI or select a successor until
the augmented proof and the same result pass independent rereview.
Independent recovery review returned `ACCEPT` for the 133-line correction,
proof-only scope, frozen replay and no-CLI-replay stop.
