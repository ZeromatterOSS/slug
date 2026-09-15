# Current Slug V2 Work Packet

Packet: WP-4-7A-configured-conflict-path-observation-activation-dependency-audit-r1
Status: accepted; aggregate activation/event diagnostic chain stopped

## Accepted result

The sole exact nonignored selector
`configured_action_conflicts::one_shot_build_conflict_is_atomic_and_recovers`
listed once and began one test. The frozen supervisor reached the expected
12-second wall deadline after 12.0115311145782 seconds in `RootCompute`, with
raw status 9. Observer and sole reaped run PID were both 750366. Process group,
children, pipes, telemetry and bounded-output cleanup were clear.

Global DICE event counters were `[51869, 51861, 45463, 45460, 8029, 8025]` with
overflow, dropped samples and activity claim zero. The aggregate path handoff
lifecycle was produced 220, committed 220, closed 219, pending zero, active one,
flags zero and activation claim zero.

The six legacy activation aggregates were:

| Aggregate | Count |
|---|---:|
| evaluated (`E`) | 2,488 |
| reused (`R`) | 0 |
| immediate exact shard dependency (`D`) | 2,488 |
| no immediate exact shard dependency (`N`) | 0 |
| evaluated and direct (`ED`) | 2,488 |
| reused and direct (`RD`) | 0 |

All checked equations and bounds passed: `E + R = D + N`, `D = ED + RD`,
`ED <= E`, `RD <= R`, and the classified total was nonzero. Independent result
review returned `ACCEPT`.

This proves exactly 2,488 legacy-delivered evaluated `PathObservationKey`
callbacks whose immediate dependency iterator contained an exact
`PathObservationShardKey` during aggregate armed windows. Rich-only reuse is
outside this census. The result establishes no unique key identity, per-window
pairing, shard activation kind, invalidation source, necessity, cost or
avoidability.

## Preparation receipt

Final temporary totals were 166/217/85 against 180/220/140 production, proof
and scratch caps. The exact Core proof passed 1/1 after two independently
accepted harness recoveries. The feature-enabled CLI integration target built
in 10.07 seconds, the exact selector listed once, and the scratch supervisor
self-check passed normal, deadline and exception cleanup. The fresh fixture
verified 28 objects, 177 registry metadata entries, 8,004,740 source bytes and
inventory SHA-256
`4337d0756cefc0971a76e12bbeea54ee40c24beb0ff943a4c3bdc60d88ed764f`.

Frozen SHA-256 values were:

| Artifact | SHA-256 |
|---|---|
| base supervisor | `d9a012989d595e6da275094d4ce020a8e8a8214e1714df79e8a3285282191179` |
| activation supervisor | `b33d749f9c53897f82ba141519bbdca5c0b05bf049108e71a07bfe2876da1ceb` |
| exact temporary Rust diff | `77817b67e6e0029f44e5e9d28e156f86be7f4877e08f33c6d9d54fb3e0155f11` |
| Core proof harness | `27f71dd21e80c727b7c8775a1001f9750b4c99360bea37ca3b2304c33c9eaeb8` |
| CLI integration harness | `613e25da51dbeaae395bf846062494126c769f05717fcc3f0e3ca83512e830f0` |
| Core proof binary | `f480b06be5e14b7a9b33d6875129618f63156606c688e50fc308c737e81ba36f` |
| Slug binary | `2fe5c70984f30096fe27dd14527a41b928b0d2a21b85a12b068caeb13ad26268` |
| CLI harness binary | `b80d3939a1ac8a6980deaa9f016c17bf08edc4262f1607c576ef05f274eaad45` |

All temporary Rust, supervisor, fixture and run-log artifacts were restored or
removed after review.

## Stop and replan boundary

Do not rerun this selector or the event, shard-key, handoff, shard exposure,
batching or census audits. Another aggregate callback counter cannot establish
causality because injected-key and direct-cache activity may be rich-only and
aggregate cells cannot pair child and parent. Continue this diagnostic
investigation only through a separately reviewed typed invalidation-provenance
design. Do not infer or select optimization work from this result, run F3,
broaden to `ResolvedPathObservationKey`, merge the combined stack or push the
review branch.
