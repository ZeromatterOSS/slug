# Current Slug V2 Work Packet

Packet: WP-4-7A-configured-conflict-cold-progress-audit-r1
Status: audit complete; exact key-type causal audit selected

## Observable result

Capture bounded progress evidence for the first configured-conflict selector's
cold evaluation after it exceeded the inherited 12-second deadline/15-second
absolute ceiling following accepted generated repository paths. Run one
materially instrumented replay of that same exact selector. This audit changes
no compatibility behavior and cannot accept the combined R2/execution-group
stack or select a semantic implementation or optimization from elapsed time or
a sampled key category.

Immediate predecessor `b75291517` implements accepted attempt-local generated
repository paths. Its unchanged selector was exactly preflighted, then reached
exit 124 after 14.91 seconds and printed only `running 1 test`. The other nine
configured-conflict selectors and F3 were not run. Preserve those stops.

## Learned facts and hypothesis

`app/slug_cli_v2/tests/cli.rs` owns the selected selector. `one_shot_case`
assembles the authentic 28-object fixture, calls `sentinel_outputs`, and only
after that runs the baseline aquery, conflict command, cquery and recovery
checks. A fresh standalone assembly of the same pinned inventory SHA-256
`4337d0756cefc0971a76e12bbeea54ee40c24beb0ff943a4c3bdc60d88ed764f`
took 0.18 seconds and produced the unchanged 8,004,740 source bytes. Every prior
typed terminal came from the cold
`evaluate_workspace_build_command_with_bzlmod_inputs` inside
`sentinel_outputs`. The resource result therefore selects cold semantic
evaluation for diagnosis, not fixture assembly or a later CLI phase.

The first hypothesis is renewed DICE preparation fanout after the generated
local-JDK branch began completing. Earlier authenticated fixture work found and
fixed global path-epoch and external-child first-Need fanout; it also retained
cumulative path fanout as a measured performance concern. This is only a
hypothesis until a later causal discriminator proves it. This audit can only
record a sampled phase and key category for that follow-up.

Reuse the existing opt-in `native-probe-observer` implementation in
`app/slug_core_v2/src/runtime/probe_observer.rs` and its lifecycle tests in
`runtime/tests/probe_observer_tests.rs`. Its fixed 512-byte sealed mapping owns
six independent DICE event counters, double-buffered phase/activity samples,
overflow state and disable-on-drop. DICE's `DiceEventListener` callback supplies
key-type tags without retaining keys or changing dependency recording. Reuse
the supervisor invariants in `tools/v2_oracle/run_payload_demand_probe.sh`:
bounded pipes, session isolation, subreaper cleanup, sealed inherited observer
descriptor and exact-selector listing.

## Audit method and ownership

Make only the frozen temporary, uncommitted diagnostic edits needed to attach
the existing observer to the integration-test thread and to drive the already
selected selector. In `slug_cli_v2/src/lib.rs`, remove only the existing
`native-probe-observer` integration-build `compile_error!` guard. In the exact
integration test, read only `SLUG_SENTINEL_OBSERVER_FD`, adopt it once with
`OwnedFd::from_raw_fd`, install one `ProbeGuard` immediately before the existing
`one_shot_case("build")` call, and hold the guard until that unchanged call
returns or the supervisor terminates the process. No other test body, helper or
feature behavior may change. The exact selector remains
`configured_action_conflicts::one_shot_build_conflict_is_atomic_and_recovers`.
Compile a feature-enabled integration harness within 60 seconds, verify that
the selector is present exactly once and nonignored, then supervise one run with
a 12-second wall deadline and 15-second absolute ceiling.

Place the scratch supervisor copy beside the existing script under
`tools/v2_oracle/` so its unchanged relative-root resolution remains valid and
exclude it from Git. Preserve the accepted observer channel, byte/output caps,
namespace/resource isolation, 12-second timeout, kill/reap finalizer, decoder
and cleanup logic byte-for-byte. Change only the selector name and removal of
the ignored-selection arguments needed for the nonignored integration test;
result wording may identify a progress audit. Run the scratch copy's existing
`self-check` before the replay. The adapter and supervisor must not change the
evaluated request, fixture, target, source bytes, DICE keys, equality,
dependency graph, terminal projection or command behavior. The observer mapping
is process-owned diagnostic memory; the supervisor closes it and kills/reaps
the complete process group on success, failure, timeout or exception. No
mapping, key, event, path, fixture or output is retained in Slug state.

Accept the audit evidence only when selector listing is exact; the observer
header/version, installed PID and six counters are valid; installed PID matches
the supervised/reaped process; overflow flags and released activity claim are
zero; output caps are clear; the activity tag is complete or its truncated
prefix is explicitly unambiguous; dropped sample count is disclosed; and
cleanup reports no surviving child or open observer descriptor. A timeout is an
expected diagnostic outcome. Any sampled key category, specific or generic,
selects only a deeper bounded causal audit. A typed terminal may be recorded and
reviewed if it appears, but instrumentation overhead and elapsed time cannot
establish a new semantic requirement.

## Scope and stops

Durable edits are limited to scheduling/status sections in the canonical plan,
this manifest, Stage 4, bootstrap readiness and the configured CLI ledger.
Temporary diagnostic edits are limited to
`app/slug_cli_v2/src/lib.rs`, `app/slug_cli_v2/tests/cli.rs`, and the excluded
scratch supervisor copy described above; none may be committed. The authentic
fixture, oracle script, production DICE/loading/analysis behavior and Cargo
manifests remain unchanged.

Before recording the result, restore all temporary source edits, remove scratch
script/fixture/log directories, and prove both worktrees clean except for the
allowed documentation receipt. Record SHA-256 for the accepted supervisor, the
scratch supervisor, and the exact temporary source diff before running. Run
`python3 scripts/v2_plan_status.py` and `git diff --check`. Independently review
this diagnostic design before the run; the result needs another review only if
it claims a semantic owner or changes the allowed next action.

Independent correction rereview returned `ACCEPT` for the frozen adapter,
byte-preserved supervisor, evidence-validity checks and sampled-evidence stop.

## Audit receipt (2026-09-14)

The accepted supervisor SHA-256 was
`3b4d48a4a7c729ec0c8c17ffd2aed3ddab8677699bdce9d8bb24d97afbeab69c`.
The excluded scratch copy changed only the exact selector and the two
nonignored launch arguments; its SHA-256 was
`650785f4a47fedbf79622496f48ee20fa27ec3a2eb3036ee43320c6704f4c10d`.
The exact temporary adapter diff SHA-256 was
`c91f97ab881133fc2edc4926400a15941bf785c5a1688b5630a6626f0a48ef19`.
The scratch supervisor self-check passed normal, deadline and exception cleanup.
Its first sandboxed preflight could not enter `strace` because ptrace was
denied; the approved outside-sandbox invocation then passed without changing
the script bytes.

The feature-enabled integration harness compiled in 22.64 seconds at SHA-256
`45b83d6d7a08e1dabfcf38ad0b0e31edfde42cbbf865c1ec74096f90192deab2`.
Exact preflight found the selected nonignored test once without execution. The
authentic fixture retained 28 objects, 177 registry metadata files, 8,004,740
source bytes and inventory SHA-256
`4337d0756cefc0971a76e12bbeea54ee40c24beb0ff943a4c3bdc60d88ed764f`.

The one permitted replay reached the 12-second wall deadline in `RootCompute`.
The six independent started/finished, check-started/check-finished and
compute-started/compute-finished counters were respectively
52,360/52,350, 46,028/46,021 and 8,003/8,000. The latest complete 31-byte tag
was `ExternalBzlModuleObservationKey`, event kind `Finished`; sampling dropped
zero activities. Observer PID 622972 exactly matched the reaped test PID.
Overflow flags and the released activity claim were zero. Cleanup reported no
live group, child, pipe or telemetry error. The selector listing was exact and
the capped outputs contained only `running 1 test`; no typed terminal appeared.

This valid sample establishes progress and no semantic owner. It selects only a
bounded exact key-type count audit that can distinguish the sampled external-Bzl
category and the historically adjacent `PathObservationKey` category. Every
temporary source edit, scratch script, fixture and log was removed. Both
worktrees were clean before this durable receipt.

Do not run F3, any of the other nine configured-conflict selectors, the
unmodified selected selector, or a second diagnostic replay. Do not raise a
deadline, acquire payloads, change production code, optimize a sampled key
category, merge the combined stack or push the review branch. Replan if the
observer cannot attach without semantic changes, the supervisor cannot prove
cleanup, the harness fails compilation/selection, telemetry is invalid or the
sample is too weak to name the next bounded audit.
