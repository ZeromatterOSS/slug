# Current Slug V2 Work Packet

Packet: WP-4-7A-configured-conflict-key-type-count-audit-r1
Status: audit complete; path identity/outcome census selected

## Observable result

Measure exact current DICE event counts for
`ExternalBzlModuleObservationKey` and `PathObservationKey` during one bounded
replay of the already selected configured-conflict test. The predecessor's
latest activity tag was only a sample and its 8,003 compute starts are
motivation, not the denominator for this replay. This audit compares each named
category only with the six total counters captured at the same cutoff. It does
not attribute CPU time, prove retry causation, select a semantic change or
accept the combined R2/execution-group stack.

The accepted generated-path implementation remains at `b75291517`. The
reviewed predecessor audit is recorded at `89e3ad084`: exact selector listing,
valid observer PID/reap identity, zero overflow and dropped samples, released
activity claim, complete cleanup, `RootCompute`, and total counters
52,360/52,350 starts/finishes, 46,028/46,021 dependency-check
starts/finishes, and 8,003/8,000 compute starts/finishes. Its complete latest
tag was an external-Bzl observation finish. The other nine configured-conflict
selectors and F3 remain stopped.

## Compatibility and bounded diagnostic owner

Reuse the opt-in `native-probe-observer` and its existing fixed 512-byte sealed
mapping. Words 0 through 50 own the accepted header, lifecycle state, six total
counters and double-buffered phase/activity frames. Words 51 through 63 are
currently unused and zero. In temporary feature-only code, assign words 51--56
to the six DICE event kinds for the exact static tag
`ExternalBzlModuleObservationKey`, words 57--62 to the same event kinds for
`PathObservationKey`, and word 63 to twelve independent filtered-counter
overflow bits. Keep the existing header, version, size, seals, total counters,
sampling, claim/drop behavior and frames unchanged.

In `Observer::record_event`, increment the existing independent total counter
first. Before the existing sampling return, compare the supplied static tag to
the two exact names and increment at most one corresponding filtered counter.
Set overflow bit `group * 6 + event_kind` only if that filtered counter wraps.
No tag prefix, hash, dynamic identity, key value, path or retained allocation is
admitted. The filtered counters observe callbacks already emitted by DICE and
cannot affect dependency recording, key equality, computation order, source
preparation, terminal selection or publication. Their extra comparisons and
atomics make timing noncomparable and cannot support a performance claim.

Use the same frozen temporary CLI adapter accepted by the predecessor: remove
only the `slug_cli_v2` integration-build feature guard, adopt only
`SLUG_SENTINEL_OBSERVER_FD` once in the exact test, install one `ProbeGuard`
immediately before the unchanged `one_shot_case("build")`, and hold it until
return or supervisor termination. No helper, request, fixture or test behavior
may change.

## Diagnostic proof and replay

Add one temporary exact Core observer unit selector,
`filtered_key_type_counters_are_exact_independent_and_bounded`. It emits each
of the six DICE event kinds for both admitted tags, proves the twelve exact
cells, proves a nonmatching tag changes only the existing total counter, and
forces one filtered wrap to prove the corresponding word-63 overflow bit
without changing another filtered counter or filtered-overflow bit. Assert the
corresponding total increment and every deterministic sampling side effect.
Preserve the existing observer size/lifecycle tests.

The first cold feature-enabled Core unit-harness preparation reached its
60-second ceiling after compiling dependencies and entering `slug_core_v2`.
Exit 124 produced no test executable; no selector, self-check or CLI replay ran.
The temporary Rust source diff SHA-256 was
`bbb1a74bd80e8f9f3f992b8aec3c348b4daf0aefadc66c0dfd1e354f435f875e` and the
scratch supervisor SHA-256 was
`ddf88c7dea14f73d227d2a3fd6b52a06dedef68411477f33eede8b93ea0fb771`.
All temporary files were restored and both worktrees proved clean.

Treat that output as evidence for one split preparation, not authority to raise
or silently retry a limit. Reapply the byte-identical temporary Rust diff and
scratch supervisor. First compile the feature-enabled Core library alone with
`cargo check -p slug_core_v2 --features native-probe-observer --lib` within 60
seconds. Only after it succeeds, reissue the Core unit-harness preparation once
within a fresh 60-second limit; the new hypothesis is that the separately
completed library/dependency boundary leaves only test-module compilation and
linking. Verify both temporary hashes before proceeding. Exact preflight the
selector and run it once under the inherited 12-second deadline/15-second
absolute ceiling. Do not retry either split step if it misses its bound.

Copy `tools/v2_oracle/run_payload_demand_probe.sh` beside the original and keep
the copy excluded from Git. Preserve its observer channel, byte/output caps,
namespace/resource isolation, 12-second timeout, kill/reap finalizer and
cleanup logic byte-for-byte. The permitted scratch changes are the exact
nonignored integration selector launch, decoding/validating words 51--63 under
the two fixed names, decoder self-check cases for the twelve counters and
overflow word, and result wording. Run the scratch self-check before the CLI
replay. Record SHA-256 for the accepted supervisor, scratch supervisor, exact
temporary Rust source diff and both compiled harnesses.

Compile the feature-enabled CLI integration harness within 60 seconds and
exactly preflight
`configured_action_conflicts::one_shot_build_conflict_is_atomic_and_recovers`
once as nonignored. Reassemble the authentic fixture at inventory SHA-256
`4337d0756cefc0971a76e12bbeea54ee40c24beb0ff943a4c3bdc60d88ed764f`.
Then run that selector once through the scratch supervisor with the unchanged
12-second wall deadline and 15-second absolute ceiling. Do not run it directly.

Evidence is valid only if exact listing succeeds; the observer header/version,
installed PID and original six counters validate; installed PID matches the
supervised/reaped test; original and filtered overflow are zero; activity claim
is released; dropped samples are disclosed; output caps remain clear; and
cleanup reports no group, child, pipe, descriptor or telemetry error. Record
all twelve filtered counters beside the six totals. Because each callback
increments its total and filtered cells separately, termination may occur
between those atomic operations. Treat both arrays as same-replay atomic cutoff
snapshots and every ratio as diagnostic, never as an exact partition. The result
may select only a separately reviewed identity/call-site causal audit for the
measured categories, or a broader exact category audit if neither explains
enough event volume. It cannot select an optimization or semantic edit from
count, ratio, sample, timeout or elapsed time.

## Scope, caps and stops

Durable edits are limited to scheduling/status sections in the canonical plan,
this manifest, Stage 4, bootstrap readiness and the configured CLI ledger.
Temporary diagnostic Rust edits are limited to
`app/slug_core_v2/src/runtime/probe_observer.rs`,
`app/slug_core_v2/src/runtime/probe_observer/mapping.rs`,
`app/slug_core_v2/src/runtime/tests/probe_observer_tests.rs`,
`app/slug_cli_v2/src/lib.rs`, and `app/slug_cli_v2/tests/cli.rs`. The mapping
file may add only constants naming the thirteen existing unused words. Allow at
most 70 gross temporary diagnostic production lines, 60 gross temporary proof
lines and 50 changed scratch-supervisor lines; restore all before the receipt.
No Cargo manifest, DICE/loading implementation, fixture, oracle input or
production semantic file may change.

Before recording the result, restore every temporary source edit, remove the
scratch supervisor, fixture and logs, and prove both worktrees clean except for
the allowed documentation receipt. Run `python3 scripts/v2_plan_status.py` and
`git diff --check`. Independently review this design before execution and the
result before it selects any further owner audit.

Independent correction rereview returned `ACCEPT` for the same-replay cutoff
semantics, fixed mapping layout, proof obligations, scratch limits and inference
stops.

Independent resource-limit rereview returned `ACCEPT` for the byte-identical,
separately bounded feature-library and unit-harness preparation split.

## Audit receipt (2026-09-14)

The byte-identical temporary Rust diff retained SHA-256
`bbb1a74bd80e8f9f3f992b8aec3c348b4daf0aefadc66c0dfd1e354f435f875e`.
The accepted and scratch supervisors retained SHA-256
`3b4d48a4a7c729ec0c8c17ffd2aed3ddab8677699bdce9d8bb24d97afbeab69c`
and `ddf88c7dea14f73d227d2a3fd6b52a06dedef68411477f33eede8b93ea0fb771`.
The split feature Core check passed in 31.24 seconds. The Core unit harness
compiled in 23.76 seconds at SHA-256
`d3e4c89acc8bb585cd0126a7ba8764b4e15a4b33489f2fdac50f25ed08d44558`.
After a nonexecuting wrong-module-name preflight found zero tests, listing
supplied the exact module path; exact preflight then found one nonignored test
and the filtered-counter proof passed once. The extended supervisor self-check
passed normal, deadline, exception, decoder and cap paths.

The CLI integration harness compiled in 8.22 seconds at SHA-256
`8158218269f31515fe5627f082a71d991d8b60b1805b4038b1f3a17eab809046`.
Exact preflight found the selected nonignored test once. The authentic fixture
retained 28 objects, 177 registry metadata files, 8,004,740 source bytes and
inventory SHA-256
`4337d0756cefc0971a76e12bbeea54ee40c24beb0ff943a4c3bdc60d88ed764f`.

The sole replay reached the 12.009-second wall deadline in `RootCompute`. Its
same-cutoff total arrays were 48,760/48,745 starts/finishes,
42,341/42,330 dependency-check starts/finishes and 8,094/8,090 compute
starts/finishes. The external-Bzl filtered array was
9,311/9,305, 7,685/7,679 and 1,626/1,626. The path-observation filtered array
was 2,543/2,543, 1,592/1,592 and 2,543/2,543. Thus the two filtered compute-start
snapshots total 4,169 against the same-cutoff total 8,094, about 51.5%; separate
atomic increments make this diagnostic rather than an exact partition. The
latest complete tag was an external-Bzl dependency-check finish.

Observer PID 632485 exactly matched the reaped test PID. Original and filtered
overflow, dropped samples and the released activity claim were zero. Exact
selector listing and output caps passed; cleanup reported no group, child, pipe
or telemetry error. No typed terminal appeared. Every temporary source edit,
scratch supervisor, fixture, log and self-check artifact was removed, and both
worktrees were clean.

Independent result review returned `ACCEPT` only for a post-sharding
`PathObservationKey` identity/outcome census at the natural key owner. It must
count exact `PathObservationDemand` identities as first-seen versus repeated
and classify returns as `Complete` versus `Need`, with bounded capacity,
explicit overflow, aggregate-only output and no physical-path disclosure. This
distinguishes distinct cold breadth from completed repetition or retry-producing
needs. The historical external identity census already selected the sibling-
need union still present in source; neither these counts nor the latest sample
justify repeating it or changing that loop. A census can select only another
reviewed causal audit.

Do not run F3, any sibling configured-conflict selector, the uninstrumented
selected selector or a second measured replay. Do not raise a limit, acquire a
payload, modify the external-child loop, merge the combined stack or push the
review branch. Replan if the fixed mapping has insufficient room, the existing
event listener cannot count both exact tags without semantic coupling, either
split preparation or either harness misses its bound, telemetry is invalid,
scratch safety logic changes or
the exact counts cannot support a bounded next causal question.
