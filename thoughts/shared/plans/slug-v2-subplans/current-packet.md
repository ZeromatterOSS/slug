# Current Slug V2 Work Packet

Packet: WP-4-7A-configured-conflict-path-census-audit-r1
Status: design accepted; diagnostic execution pending

## Observable result

Run one bounded, same-selector post-sharding `PathObservationKey` census. At
the natural key owner, count exact `PathObservationDemand` identities as first
seen or repeated and classify completed key computations as `Complete` or
`Need`. Publish only fixed aggregate cutoff snapshots. This audit distinguishes
distinct cold path breadth from repeated completed observations and
retry-producing needs; it does not disclose paths, attribute CPU cost, prove an
invalidation or call-site cause, select an optimization or accept the combined
R2/execution-group stack.

The reviewed key-type audit at `9d2d470b6` recorded same-cutoff compute starts
of 8,094 total, 1,626 external-Bzl and 2,543 path-observation events, with valid
PID, overflow, sampling, output and cleanup evidence. Those separate atomic
counters are diagnostic, not an exact partition. Independent result review
selected only this path census. The historical external identity census already
selected the sibling-need union still present in `bzl_module.rs`; do not repeat
that audit or modify the union. The other nine configured-conflict selectors
and F3 remain stopped.

## Census owner, identity and bounds

Temporarily extend `app/slug_workspace_v2/src/path_observation.rs`, beside
`PathObservationKey::compute`. Install one process-local census before the
selected call begins. Its private state owns a `HashSet<PathObservationDemand>`
under one mutex and uses the type's existing exact `Eq`/`Hash`: namespace,
normalized absolute path, operation and any raw Windows input all participate.
Cap the set at 4,096 identities. Refuse a new identity at capacity, set a sticky
capacity-overflow flag and leave it unclassified; any overflow invalidates the
audit. Retained identities and the census disappear with the test process.
Target-gate the temporary census owner, install API and proof to Linux x86-64,
little endian and 64-bit atomics, exactly matching Core's observer envelope.

On every `PathObservationKey::compute` entry, classify the demand as first-seen
or repeated under the mutex and increment `entered`, `first_seen` or `repeated`.
After the existing shard/fallback logic produces its unchanged `PathOutcome`,
increment exactly one of `first_complete`, `first_need`, `repeat_complete` or
`repeat_need` for the entry classification. Cancellation or termination between
entry and return may leave `entered` greater than the four outcome counters;
publish that difference as in-flight cutoff work. Do not hold the census mutex
across an await. Do not change the shard, epoch fallback, key equality,
validity, demand projection or returned value.

The census retains no result, source, DICE key, epoch or call-site state. It
publishes no identity, hash, namespace, operation or physical/logical path.
Only the capacity, sequence, entered, first/repeated, four outcome counts,
overflow/error flags and installed PID cross the diagnostic boundary.

## Fixed aggregate channel

Keep the accepted 512-byte DICE observer mapping unchanged. The excluded
scratch supervisor creates a second independent 512-byte zeroed memfd, seals it
against grow/shrink/further seals, and passes it only as
`SLUG_PATH_CENSUS_FD`. The temporary CLI adapter adopts that descriptor once
and transfers it through a feature-only Core wrapper to the workspace census;
the existing `SLUG_SENTINEL_OBSERVER_FD` adoption and one `ProbeGuard` remain
otherwise byte-identical. Core validates that the census descriptor is an exact
read-write regular file with the required seals and without `F_SEAL_WRITE`
before delegation. No Cargo manifest or feature graph changes.

The workspace census converts the owned descriptor to `std::fs::File` and uses
target-gated positional standard-library file writes, not mmap or a new
dependency. Every word is encoded little endian. Word 0 is the selector and
must equal 1 or 2. Frame 1 occupies words 1--15 and frame 2 words 16--30; words
31--63 remain zero. Each 15-word frame is, in order: magic
`u64::from_le_bytes(*b"SLGPTH01")`, version 1, installed PID, monotonically
increasing sequence, capacity 4,096, `entered`, `first_seen`, `repeated`,
`first_complete`, `first_need`, `repeat_complete`, `repeat_need`, capacity-
overflow flag, reserved-zero, and checksum. The checksum is the fixed seed
`0x534c554750415448` XOR every preceding word in that frame.

Under the mutex, write the complete inactive 120-byte frame first and commit it
with a separate little-endian selector write at offset zero. Alternate frames
on every entry and outcome update. A failed or killed write leaves the prior
selected frame authoritative; no writer-error absence is claimed. Invalid
selector, selected-frame checksum/header/version/PID, reserved word,
nonmonotonic counts, impossible outcome totals, nonzero capacity overflow or a
nonzero byte outside the defined layout invalidates the audit. A selected
snapshot with sequence below 2 or fewer than 128 entries is too early/weak and
forces replan. Treat every selected frame as an earlier committed cutoff, not
the final state of the terminated computation.

## Diagnostic proofs and replay

Add one temporary exact workspace unit selector,
`path_observation_census_counts_exact_identities_outcomes_and_capacity`. With a
private 512-byte file it proves install validation, exact first/repeat identity
classification across namespace/path/operation differences, all four
Complete/Need outcome cells, entered/outcome/in-flight arithmetic, alternating
frame/sequence/XOR-checksum commits, no identity bytes in the file, and sticky
capacity overflow at the 4,097th distinct demand. It proves both an intentionally
partial inactive frame and a fully valid higher-sequence inactive frame cannot
replace the older selected snapshot until selector commit.

Add one temporary feature-enabled Core selector,
`path_census_descriptor_validation_delegation_is_owned_and_closed`. It covers
wrong size, nonregular file, non-read-write mode, missing required seals,
forbidden `F_SEAL_WRITE`, consumed/closed ownership on every rejection, and one
successful delegated descriptor. The workspace proof cannot substitute for
this Core boundary proof.

Use split preparation: compile `slug_workspace_v2` and feature-enabled
`slug_core_v2` library paths within separate 60-second limits, then compile each
unit harness once within its own 60-second limit. Exact preflight and run only
the two named selectors, each under the inherited 12-second deadline/15-second
absolute ceiling. Do not retry a failed preparation or proof.

Copy `tools/v2_oracle/run_payload_demand_probe.sh` beside the original and keep
the copy excluded from Git. Preserve its observer channel, output caps,
namespace/resource isolation, 12-second timeout, kill/reap finalizer and cleanup
logic. Extend the same ownership/finalizer path to the census memfd: one parent
owner, one inherited child descriptor, exact 512-byte post-reap read, close on
success/failure/timeout/exception and proof that both descriptors are closed.
Add a bounded census decoder and self-check cases for exact little-endian
offsets, selector values, XOR checksum, valid alternating frames, bad
selector/header/version/PID/checksum/counts/flags, partial inactive frames, a
fully valid higher-sequence uncommitted frame, normal/deadline/exception
retention, weak snapshots, output cap and descriptor cleanup. Run the entire
scratch self-check before the replay.

Compile the feature-enabled CLI integration harness within 60 seconds and
exactly preflight
`configured_action_conflicts::one_shot_build_conflict_is_atomic_and_recovers`
once as nonignored. Reassemble the authentic fixture at inventory SHA-256
`4337d0756cefc0971a76e12bbeea54ee40c24beb0ff943a4c3bdc60d88ed764f`.
Then run that selector once through the scratch supervisor under the unchanged
12-second wall deadline and 15-second absolute ceiling. Record SHA-256 for the
accepted supervisor, scratch supervisor, exact temporary Rust diff and both
harnesses before execution.

Evidence is valid only if exact selector listing succeeds; the accepted DICE
observer header/version/counters, PID, overflow, sampling and released claim
validate; the census selected frame and arithmetic validate; both installed
PIDs match the reaped test; capacity overflow and output caps are clear; the
census has at least 128 entries; dropped samples are disclosed; and cleanup
reports no group, child, pipe, descriptor or telemetry error. Record the
aggregate census snapshot only. A clear census frame means only that the
selected earlier cutoff committed successfully; it does not prove later writes
succeeded. Independent result
review may select only another bounded call-site/invalidation audit or a
broader exact category audit. Census ratios, elapsed time and a deadline cannot
select production or semantic work.

## Scope, caps and stops

Durable edits are limited to scheduling/status sections in the canonical plan,
this manifest, Stage 4, bootstrap readiness and the configured CLI ledger.
Temporary Rust edits are limited to
`app/slug_workspace_v2/src/path_observation.rs`,
`app/slug_core_v2/src/runtime/probe_observer.rs`,
`app/slug_core_v2/src/runtime/tests/probe_observer_tests.rs`,
`app/slug_cli_v2/src/lib.rs`, and `app/slug_cli_v2/tests/cli.rs`. Allow at most
250 gross temporary diagnostic production lines, 180 gross temporary proof
lines and 210 changed scratch-supervisor lines. No Cargo manifest, DICE crate,
loading owner, path shard/epoch semantics, fixture or oracle input may change.

Before recording the result, restore every temporary source edit, remove the
scratch supervisor, census/fixture files and logs, and prove both worktrees
clean except for the allowed documentation receipt. Run
`python3 scripts/v2_plan_status.py` and `git diff --check`. Independently review
this design before execution and the result before selecting a successor.

Independent correction rereview returned `ACCEPT` for the natural owner,
bounded exact identity state, frozen dual-frame wire, separate Core/workspace
proofs, earlier-cutoff semantics, target gate, cleanup and inference stops.

Do not run F3, any sibling configured-conflict selector, the uninstrumented
selected selector or a second census replay. Do not raise a limit, acquire a
payload, emit identity material, change the external-child union, merge the
combined stack or push the review branch. Replan for capacity/flag/checksum
failure, unbounded identity memory, physical-path output, semantic coupling,
compile/proof failure, invalid dual-channel cleanup or weak census evidence.
