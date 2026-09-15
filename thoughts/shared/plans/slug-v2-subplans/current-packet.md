# Current Slug V2 Work Packet

Packet: WP-4-7A-configured-conflict-path-frontier-batching-audit-r1
Status: design accepted; diagnostic execution pending

## Observable result

Run one bounded aggregate audit at the path branch of
`NativeDemandSession::progress_inner`. Count how path demands presented by one
progress round divide into unseen and already-known demands, and record the
size distribution of unseen-demand batches. This tests whether the
accepted census's 471 first-Need identities are supplied in broad batches or
through small repeated frontier advances. It does not identify a path or caller,
attribute CPU cost, prove invalidation, select an optimization, or accept the
combined R2/execution-group stack.

The completed census packet used the sole authorized replay and produced a
valid selected cutoff at sequence 5,051: 2,525 entries, 471 exact first-seen
demands and 2,054 repeats. All 471 first computations returned `Need`; repeats
returned 2,050 `Complete` and four `Need`. Capacity overflow was zero and the
outcome arithmetic had no in-flight gap. Observer and census PIDs matched the
sole reaped test PID; observer overflow, drops and claim were zero; the process
tree and descriptors were clean. Independent review accepted this result only
after a transparent manifest correction recorded the frozen executed artifact
at 247 production, 182 proof and 189 scratch lines. The exact Rust diff retained
SHA-256 `1fce7c30f13d5aa0537f2df41d8eb50a9898bc21cdbe84bbe28739294c7699e0`;
no proof or replay was rerun.

## Owner and aggregate counters

Temporarily instrument only the path branch in
`app/slug_core_v2/src/runtime/dice.rs`, immediately after obtaining the
nonempty `NeedPathObservations` and computing which exact demands are absent
from `self.path_observations`. The existing branch remains the sole owner of
deduplication, observation, merge and nonprogress errors. Record one round with:

- total requested demands in `path_needs`;
- unseen demands in `new_demands` before `observe_native` runs;
- already-known demands, exactly requested minus new;
- the maximum unseen-demand batch size; and
- one unseen-batch bucket: 0, 1, 2--3, 4--7, 8--15 or 16+.

Count a zero-unseen round before returning the existing
`PathInternalNonProgress`. Use checked conversion from `usize` and
sticky counter-overflow state. Any conversion failure, counter wrap or maximum
overflow invalidates the audit. Do not retain a demand, path, hash, result,
round identity, call site, epoch or timestamp. Do not change sorting,
deduplication, filtering, `observe_native`, merge order, error selection or the
returned progress value.

## Fixed observer layout

Reuse only the accepted observer's currently unused words 51--63. Keep its
512-byte size, existing words 0--50, descriptor contract, event sampling and
phase publication unchanged. Assign these little-endian atomic words:

| Word | Aggregate |
|---:|---|
| 51 | path progress rounds |
| 52 | total requested demands |
| 53 | unseen demands |
| 54 | already-known demands |
| 55 | maximum unseen-demand batch size |
| 56 | unseen batches of size 0 |
| 57 | unseen batches of size 1 |
| 58 | unseen batches of size 2--3 |
| 59 | unseen batches of size 4--7 |
| 60 | unseen batches of size 8--15 |
| 61 | unseen batches of size 16+ |
| 62 | sticky counter-overflow flag |
| 63 | reserved zero |

Add a feature-only observer method that receives only requested and unseen
counts. Update exactly one bucket first, then requested/unseen/known totals and
maximum, and commit the round counter last. Before refusing any addition that
would wrap, set word 62. The call site passes counts only after it has
constructed `new_demands` and before the existing empty check. The observer is
optional; an absent observer must leave behavior byte-for-byte equivalent.

The supervisor decoder accepts the aggregate only when requested equals unseen
plus known, all six histogram buckets sum exactly to the committed round count,
the maximum is consistent with the highest populated bucket, overflow is zero
and word 63 is zero. Because the round commits last, a killed partial update
fails this post-reap arithmetic instead of being mistaken for a zero-unseen
round. A clear aggregate with no path progress rounds or inconsistent arithmetic
is weak and forces replan. The values remain a committed aggregate cutoff, not
an exact transaction snapshot across the rest of the observer.

## Proof and one replay

Add one exact feature-enabled Core selector,
`path_frontier_batching_counts_round_totals_histogram_max_and_overflow`. It
installs the existing observer channel and directly records bounded synthetic
rounds covering zero unseen, every histogram bucket, mixed known/unseen
arithmetic, maximum retention, commit order and sticky overflow. It proves
words 0--50 remain owned by the accepted observer contract and word 63 remains zero. No production
session, fixture or filesystem path is needed for this aggregation proof.

Compile the feature-enabled Core library path and then the exact Core unit
harness within separate 60-second limits. Preflight and run only the named
selector under the inherited 12-second deadline and 15-second absolute ceiling.
Do not retry a failed preparation or proof.

Copy the accepted supervisor to an excluded scratch file and extend only its
observer decoder/result projection for words 51--63. Add bounded self-checks
for every bucket boundary, zero-unseen rounds, maximum consistency, arithmetic
failure, overflow, reserved words and output cap. Preserve the accepted
namespace/resource isolation, exact 512-byte observer read, 12-second timeout,
kill/reap finalizer and normal/deadline/exception cleanup self-checks.

Compile the feature-enabled CLI integration harness within 60 seconds and
exactly preflight the nonignored selector
`configured_action_conflicts::one_shot_build_conflict_is_atomic_and_recovers`.
Reassemble the authentic 28-object fixture at inventory SHA-256
`4337d0756cefc0971a76e12bbeea54ee40c24beb0ff943a4c3bdc60d88ed764f`.
Record hashes for the accepted supervisor, scratch supervisor, exact temporary
Rust diff, Core harness, CLI harness and spawned Slug binary before executing.
Run the selector once through the scratch supervisor under the unchanged
12-second wall deadline and 15-second absolute ceiling. F3 and every sibling
selector remain stopped.

Evidence is valid only if exact selector listing succeeds; observer
header/version/PID, existing counters, overflow, sampling and released claim
validate; path rounds are nonzero; batching arithmetic and the reserved word
validate; the installed PID equals the reaped test; output caps are clear; and
cleanup reports no group, child, pipe, descriptor or telemetry error. Record
only the aggregate batching snapshot. Ratios, elapsed time and a deadline cannot
select production work. Independent result review may select only another
bounded call-site or shard-invalidation audit.

## Scope, caps and stops

Durable edits are limited to scheduling/status sections in the canonical plan,
this manifest, Stage 4, bootstrap readiness and the configured CLI ledger.
Temporary Rust edits are limited to
`app/slug_core_v2/src/runtime/probe_observer.rs`,
`app/slug_core_v2/src/runtime/probe_observer/mapping.rs`,
`app/slug_core_v2/src/runtime/tests/probe_observer_tests.rs`,
`app/slug_core_v2/src/runtime/dice.rs`,
`app/slug_cli_v2/src/lib.rs` and `app/slug_cli_v2/tests/cli.rs`. Allow at most
100 gross temporary diagnostic production lines, 90 gross temporary proof
lines and 120 changed scratch-supervisor lines. No Cargo manifest, DICE crate,
workspace/path owner, loading owner, fixture or oracle input may change.

Before recording the result, restore every temporary source edit, remove the
scratch supervisor, fixture and logs, and prove both worktrees clean except for
the allowed documentation receipt. Run `python3 scripts/v2_plan_status.py` and
`git diff --check`. Independently review this design before execution and the
result before selecting a successor.

Do not rerun the completed census, F3, a sibling configured-conflict selector
or the selected selector without this instrumentation. Do not raise a limit,
acquire a payload, emit identity material, change path or external-child
semantics, merge the combined stack or push the review branch. Replan on
counter overflow, no path rounds, weak/inconsistent arithmetic, compile/proof
failure or invalid cleanup.

Independent correction rereview returned `ACCEPT` for the natural owner,
pre-observation unseen terminology, explicit zero bucket, bucket-first and
round-last aggregate commit, arithmetic decoder, exact scope, caps and stops.
