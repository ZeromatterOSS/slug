# Current Slug V2 Work Packet

Packet: WP-4-7A-configured-conflict-path-shard-exposure-audit-r1
Status: design accepted; diagnostic execution pending

## Accepted predecessor receipt

The sole path-frontier batching replay completed with valid aggregate evidence.
The dedicated Core integration harness prepared in 8.89 seconds after a
reviewed launcher correction and its exact selector passed 1/1. The bounded
supervisor passed normal, deadline and exception cleanup self-checks. The CLI
integration harness prepared in 57.28 seconds, its exact nonignored selector
preflighted once, and the authentic fixture reproduced 28 objects, 177 metadata
entries and 8,004,740 bytes at inventory SHA-256
`4337d0756cefc0971a76e12bbeea54ee40c24beb0ff943a4c3bdc60d88ed764f`.

Pre-execution SHA-256 values were:

| Artifact | SHA-256 |
|---|---|
| accepted supervisor | `3b4d48a4a7c729ec0c8c17ffd2aed3ddab8677699bdce9d8bb24d97afbeab69c` |
| scratch supervisor | `780a82954c745f327778e1972d1df876512cf1d308fbd0cf66514b35c56fb454` |
| exact temporary Rust diff | `d5bd1453f1bafb6ae1f0c594b18ef70cf2f36677c72064dc15dc543feaa33b24` |
| Core proof harness | `ac4512d44b9c7406908da7ae7113f0a2ec9d18eceaa096c60585d4cd202be955` |
| CLI integration harness | `3863fb585423ba878c6e17ab8c42ced4be89ef885485c873be8b2214d22fb283` |
| spawned Slug binary | `adb6d1e917105bef63fca827442d0d1f5c896882b354b086fcb135caf461e7e3` |

The one selected replay reached its 12-second wall deadline after 12.012391
seconds in `RootCompute`. The observer and sole reaped run PID were both
669693. Counters were 51,968 starts, 51,954 finishes, 45,647 dependency-check
starts, 45,637 dependency-check finishes, 7,976 compute starts and 7,972
compute finishes. Overflow, dropped samples and activity claim were zero.

The committed batching cutoff contained 215 path-progress rounds and 468
requested demands, all 468 unseen and zero already known. The six unseen-batch
buckets were `[0, 164, 29, 8, 9, 5]`, with maximum 21. The buckets sum to the
rounds and `468 = 468 + 0`; overflow and reserved state were clear. Selector and
run supervision reported no live group, children, pipes or telemetry error.
Independent result review returned `ACCEPT` and limited the conclusion to many
small unseen frontiers. It does not identify their producer, prove avoidable
work, attribute the deadline or authorize a semantic change.

Temporary production/proof/scratch work was 60/88/76 lines against 100/90/120
caps. Every temporary source edit and scratch artifact was removed, both
worktrees were clean, and no selector was rerun.

## Observable result

Run one bounded aggregate path-shard exposure audit at the successful path
merge boundary in `NativeDemandSession::progress_inner`. Compare the existing
and successfully merged path epochs through the accepted
`path_observation_shards` projection. Count how many shards change and how many
previously observed demands reside in those changed shards before the next
injection. This measures potential next-injection exposure only. It does not
prove that DICE invalidated or recomputed a key, attribute elapsed time, select
an optimization, or accept the combined R2/execution-group stack.

Record only successful path merges, total and maximum changed shards per merge,
total and maximum prior demands resident in changed shards, total newly added
demands, and one fixed prior-exposure bucket per merge: 0, 1--7, 8--31, 32--127
or 128+. Do not retain or publish a demand, path, hash, shard identity, result,
epoch, call site or timestamp.

## Owner and counting boundary

Temporarily instrument only the path branch in
`app/slug_core_v2/src/runtime/dice.rs`. Preserve construction of `new_demands`,
`observe_native` and the existing merged iterator. First construct the merged
`PathObservationEpoch` through the existing fallible owner. Only after that
succeeds, project both the current and merged epochs with
`path_observation_shards`, compare corresponding shard values and compute:

- changed shards, whose prior and merged shard epochs differ;
- prior exposure, the sum of current-epoch demand counts in changed shards; and
- newly added demands, merged epoch length minus current epoch length.

Before projecting, admit both epochs only when each contains at most 4,096
demands, matching the completed census's exact-identity capacity. On excess,
set diagnostic overflow and skip both shard projections. Otherwise clone the
current Arc-backed epoch for the feature-only comparison, assign the successfully
constructed merged epoch to `self.path_observations`, and only then compare and
record the installed state. Each successful merge must add at least one demand;
changed shards must be in 1--64 and cannot exceed newly added demands. Preserve
all sorting, deduplication, observation, merge, assignment, error and returned
progress behavior. An absent observer must leave behavior byte-for-byte
equivalent.

Call the prior-demand count a potential next-injection exposure. A changed
shard value is not evidence that its DICE key was invalidated, requested,
checked or recomputed. Do not instrument the later injection updater or add a
DICE listener, key, retained collection or cache.

## Fixed observer layout

Reuse only the observer's unused words 51--63. Keep its 512-byte size, words
0--50, descriptor contract, event sampling and phase publication unchanged.

| Word | Aggregate |
|---:|---|
| 51 | successful path merges |
| 52 | total changed shards |
| 53 | maximum changed shards in one merge |
| 54 | total prior demands resident in changed shards |
| 55 | maximum prior demands resident in changed shards in one merge |
| 56 | total newly added demands |
| 57 | merges exposing 0 prior demands |
| 58 | merges exposing 1--7 prior demands |
| 59 | merges exposing 8--31 prior demands |
| 60 | merges exposing 32--127 prior demands |
| 61 | merges exposing 128+ prior demands |
| 62 | sticky invalid-conversion/counter-overflow flag |
| 63 | reserved zero |

Add a feature-only doc-hidden observer helper receiving borrowed prior and
installed epochs. It first enforces the 4,096-demand cap, then performs the
production `path_observation_shards` comparison and derives changed-shard,
prior-exposure and newly-added counts. It retains none of the borrowed values.
Validate the per-merge bounds, update the histogram bucket first, then checked
totals and maxima, and commit the successful-merge counter last with release
ordering. Set word 62 before refusing an oversized epoch, conversion, invalid
tuple or addition that would wrap.

Let `M` be merges, `C` total changed shards, `Cmax` maximum changed shards, `E`
total prior exposure, `Emax` maximum prior exposure, `N` newly added and
`b0..b4` the five buckets. The decoder accepts only:

- `M > 0`, `sum(b0..b4) = M`, `M <= C <= 64*M` and `C <= N`;
- `1 <= Cmax <= 64`, `Cmax + (M - 1) <= C <= Cmax*M`;
- `Emax <= 4,096`, `E <= 4,096*M` and `N <= 4,096*M`;
- `b1 + 8*b2 + 32*b3 + 128*b4 <= E`;
- `E <= min(7,Emax)*b1 + min(31,Emax)*b2 + min(127,Emax)*b3 + Emax*b4`;
- if `h` is the bucket containing `Emax`, with lower bound `Lh`, then `bh > 0`,
  no bucket above `h` is populated, and
  `E >= (b1 + 8*b2 + 32*b3 + 128*b4) + Emax - Lh`;
- `E = Emax = 0` exactly when `b0 = M` and `b1..b4` are zero; and
- clear word 62 and zero word 63.

All products and sums are checked. Merge-last publication makes a killed
partial update fail arithmetic rather than appear complete. These are
same-channel aggregate cutoffs, not one atomic snapshot with words 0--50.

## Proof and one replay

Add one exact feature-enabled dedicated Core integration selector,
`path_shard_exposure_counts_totals_histogram_max_and_overflow`. It installs the
sealed observer mapping and drives the same epoch-comparison helper with
multiple additions in one shard, additions across shards, prior residents in
changed and unchanged shards, and a zero-add/no-change rejection. It verifies
exact changed-shard, prior-exposure and newly-added results, every histogram
bucket, maxima, the 4,096 cap, checked overflow, commit order, unchanged words
0--50 and reserved word 63. Prepare
only that integration target once under 60 seconds with the pinned toolchain
`PATH` and shared `CARGO_TARGET_DIR`; preflight and run only its exact selector
under the inherited 12-second deadline and 15-second absolute ceiling.

Copy `tools/v2_oracle/run_payload_demand_probe.sh` to an excluded scratch file
and extend only words 51--63 decoding/result projection. Add bounded self-checks
for every bucket boundary, per-merge bounds, maxima, arithmetic, overflow,
reserved state, output cap, and normal/deadline/exception cleanup. Preserve its
namespace/resource isolation, exact 512-byte observer read, wall timeout,
kill/reap finalizer and cleanup ownership.

Compile the feature-enabled CLI integration harness once under 60 seconds and
exactly preflight the same nonignored selector,
`configured_action_conflicts::one_shot_build_conflict_is_atomic_and_recovers`.
Reassemble the unchanged authentic fixture and verify its accepted inventory.
Hash the accepted supervisor, scratch supervisor, exact temporary Rust diff,
Core harness, CLI harness and spawned Slug binary before the sole replay. Run
that selector once through the scratch supervisor under the unchanged 12/15
second bounds. F3 and every sibling selector remain stopped.

Evidence is valid only if selector count, observer header/version/PID, existing
counters, overflow, sampling and released claim validate; shard arithmetic and
reserved state validate; installed PID equals the reaped test; output caps are
clear; and cleanup reports no group, child, pipe, descriptor or telemetry
error. Independent result review may select only a narrower injection-boundary
audit. Replan on overflow, zero successful merges, inconsistent arithmetic,
path/identity output, proof/preparation failure or invalid cleanup.

## Scope, caps and stops

Durable edits are limited to scheduling/status sections in the canonical plan,
this manifest, Stage 4, bootstrap readiness and the configured CLI ledger.
Temporary Rust edits are limited to
`app/slug_core_v2/src/runtime/probe_observer.rs`,
`app/slug_core_v2/src/runtime/probe_observer/mapping.rs`,
`app/slug_core_v2/src/runtime/dice.rs`,
`app/slug_core_v2/tests/path_shard_exposure.rs`,
`app/slug_cli_v2/src/lib.rs` and `app/slug_cli_v2/tests/cli.rs`. Allow at most
120 gross temporary diagnostic production lines, 130 gross temporary proof
lines and 120 changed scratch-supervisor lines. No Cargo manifest, DICE crate,
workspace/path owner, loading owner, fixture or oracle input may change.

Before recording the result, restore every temporary source edit, remove all
scratch artifacts and prove both worktrees clean except for the allowed
documentation receipt. Run `python3 scripts/v2_plan_status.py` and
`git diff --check`. Independently review this design before execution and the
result before selecting a successor.

Do not rerun the completed batching audit, census, F3, a sibling
configured-conflict selector or the selected selector without this new
instrumentation. Do not raise a limit, acquire a payload, emit identity
material, change path/shard/injection semantics, merge the combined stack or
push the review branch.

Independent design review returned `ACCEPT` after requiring post-assignment
publication, the 4,096-demand admission cap, exact production-helper proof,
weighted histogram bounds and explicit maximum-attainment arithmetic.
