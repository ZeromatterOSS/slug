# Current Slug V2 Work Packet

Packet: WP-7-46-m7a-requested-build-activation-r1
Status: accepted

## Outcome and basis

Activate Execute-mode Build in both one-shot CLI and retained daemon through the shared
requested-artifact forest and native selected-output publication. A mixed FileWrite→Spawn
File/Tree request executes exactly its reachable prerequisites and publishes only selected
artifacts, with typed diagnostics and accepted source freshness. WP745 (643578e1a) supplies
typed native errors and retained-runtime recovery; WP742-744 supply selection, producer
binding, verified transfer and Linux GNU publication. Reuse their pinned Bazel 9.2
8220c6198837d5c13d53fea211cf3282aa12408a CompletionFunction.java:160-164,364-375 artifact
selection contract. Artifact/content identity and named error categories remain exact;
command acceptance, structural configuration and JSON execution evidence are Slug-native.
M7A remains partial and M8 unproved. No new action family, Run/runfiles or backend policy.

## Decisions and ownership

Branch Execute-mode Build BEFORE each adapter's ordinary build acceptance. Construct one
ActionChainReapiTransport from immutable command options, invoke
execute_and_publish_requested_actions_with_repository_environment once, and project/publish
its accepted Result once. Borrow the accepted inputs.evaluation for counts; no second
build, adapter action loop, direct materialization or execution inside a terminal projector.
Typed terminal errors use their original category/exit code. Outer operation failures keep
build_runtime_error. Preserve Disabled/CacheOnly behavior, exported-source handling in
those modes, CLI missing-target rejection, command configuration, Bzlmod/registry/env inputs.

Source-only and authored analyzed-empty Execute requests succeed without connecting to the
executor. Loading-only native filegroups remain unsupported by requested selection. The
server metric observation adapter stays nonsemantic; preserve its invalidation count and
input-forwarding test hooks before the branch. Keep the existing legacy server executor as
Run-only because both Run adapters still depend on it; its current admission stays intact.
Remove the CLI Build loop and its obsolete alias/generated-root rejection test. Do not
remove Run's corresponding guard or claim Run activation.

A shared PURE projection in slug_reapi_v2 accepts only RequestedActionResult with accepted
ActionChainRemoteResult metadata. Preserve reapi_native_execution and existing JSON counter,
digest, platform, runtime-mode and daemon invalidation fields. Aggregate execution/cache
counters, action digests, uploads and platform properties from every completed prerequisite.
Derive materialized_outputs ONLY from accepted publication groups: producer ordinal indexes
the remote result table, selected File maps to its verified file digest, selected Directory
to its verified manifest file digests (empty directories add no file digest). Never report
unselected cooutputs or generated intermediates as materialized. Zero-action output has
zero counters and empty digests. Projection performs no I/O, DICE computation or mutation.
No new semantic owner or serde dependency for formatting; output strings are request scratch.

Daemon IPC must preserve supported immutable remote options and must not silently discard
unsupported ones. Carry cache endpoint, instance name, headers, timeout and retries alongside
existing executor/properties in one primitive RemoteRequest with lossless conversion.
Redact header values from its Debug and use a generic malformed-IPC parse error so serde
errors cannot echo malformed header payloads. Execute validates
existing chain policy on both CLI and server: headers/timeouts/retries/distinct CAS endpoints
remain unsupported, without header values in diagnostics. CLI rejects invalid Execute policy
before dispatch; server validates independently for raw requests. Keep Run's existing policy.
Empty or malformed Build targets must fail the entire request, before runtime effects, instead
of filter_map dropping them into a potentially successful empty request. Use the same parser
and explicit error response; no permissive fallback.

## Scope and proof

Production allowlist: slug_cli_v2/src/commands/{build,run}.rs (Run IPC adaptation only),
slug_server_v2/src/{lib,server,reapi}.rs, slug_reapi_v2/src/lib.rs and a new
requested_build.rs pure projection child; config.rs for header-safe parse diagnostics and rejection of bare value-taking remote
flags; explicit options may never silently disappear. Build argv diagnostics also redact
remote headers in one-shot and server analysis-only paths; early parser failures must
not expose valid header arguments.
Focused tests in adjacent new requested_build child modules, CLI tests/cli.rs loading-only
expectation migration and existing server/tests.rs
protocol literals/controls; two focused ignored CLI integration selectors supervised against separate fresh backends, using existing
source-staging workspace and WP744 real backend scaffolding. Root owns REAPI projection,
plan/docs and coordinated validation; workers own disjoint CLI and server adapters/proof.
No Core/DICE/transport/publication semantic change. Expected net production growth 0-200
lines after deleting CLI duplicate execution, plus 300-700 focused proof/harness lines;
estimates trigger review, not hard caps. Large existing adapter files receive only branch,
protocol and helper changes; new proof goes in children rather than expanding monoliths.
CLI Cargo.toml may reuse existing workspace serde_json as a dev-dependency for
structured integration assertions; no new external package or production dependency.
No retained representation, interner or lock change. Keep existing Arc/ownership.

Prove public one-shot and retained-daemon mixed FileWrite→Spawn selected File/Tree publication
with real REAPI: cold/warm/source A→B→A, exact bytes, selected materialized evidence, complete
execution/cache counters, unchanged diagnostics not replayed, unrelated actions/cooutputs
absent, configured output location and source recovery. Reuse a tiny existing backend fixture;
no broad workspace build. Add offline adapter controls for source/authored-empty bad endpoint,
analysis/conflict typed failures/recovery, malformed targets and policy preservation/rejection
(including instance/default properties and no header-value exposure). Protect Disabled/CacheOnly
and Run admission plus named existing daemon event/input controls. Pure summary proof covers
selected-vs-unselected producer outputs, empty trees and zero-action evidence. New tests must
exercise the production adapters, not a separate test-only executor or projector.

Compile separately with pinned nightly-2025-09-14 and Cargo JSON, preparation cap60s. Build
slug_cli_v2 before any SLUG_V2_BIN-based proof. Preflight exact unit selectors; run native
fixtures serially where observed ancestor paths overlap. Supervise fresh backend/daemon and
reap owned processes; Unix-socket execution uses existing authorized sandbox escalation.
Prefer runtime below a few seconds; >30s requires strict necessity, 12s is guidance. Name
focused gates and preserve passing evidence; no broad suites absent a gap. Format changed
Rust, diff/plan/archive checks, independent design and final review before checkpoint commit,
main fast-forward and authorized push. Receipts target/wp746. REPLAN only if shared acceptance,
output evidence, remote policy or Run ownership cannot implement this contract; routine
invocation/fixture/compiler corrections stay in this packet.

Independent design review ACCEPT: single native Build operation, pure selected evidence,
complete immutable remote policy and separate Run. Final proof includes header redaction
through parse and argv error paths, both real adapters and retained recovery.

## Acceptance receipt

WP746 activates only the bounded requested Build contract above. The public one-shot and
same-PID daemon proofs each use a fresh NativeLink backend and A/A/B/A requests; three
reachable actions execute out of four declared, with accepted-attempt (hit,miss) counts
(0,3)/(3,0)/(1,2)/(3,0), exact selected File/Tree bytes and 0555 modes, replacement removing
stale children, preserved unrelated outputs, no intermediate/cooutput publication and
expected diagnostic replay. Run remains on its existing execution path.

Pinned preparation passed: current REAPI/server libraries 4.287s, explicit CLI build 1.724s,
CLI integration compilation 6.781s, CLI library tests 2.277s and wire-fixture recompile 1.818s.
An earlier CLI preparation reached its 60s cap; the completed dependency work was reused.
Focused validation passed 28 selectors in 18.577s total runtime: 12 server (2.828s), four
pure REAPI/config (0.004s), four CLI offline (3.361s), two existing CLI adapter controls
(5.411s), three Run parsing/launch controls (0.006s), the preserved legacy Run root guard (0.317s),
one-shot wire (3.716s), and daemon wire
(2.934s). Exact-selector preflights and ignored-selector checks passed. Native fixtures ran
serially; fresh backends terminated and owned daemon descendants were reaped. Receipts are
under target/wp746; committed selectors retain reproducible proof, while backend supervision
reuses tools/v2_oracle_lib/nativelink.py and the existing Linux NativeLink binary.

Corrections preserved assertions: absent output directories are valid after no publication;
four older server fixtures now use local built-in module dependencies; loading-only native
filegroup expectations match requested-selection rejection. The wire fixture precreates an
empty bazel-out before either adapter starts, stabilizing the observed directory namespace
without warming DICE or CAS. First-ever directory creation can otherwise cause a discarded
execution attempt and an accepted cache-hit retry; counters describe accepted results, not
all RPCs across discarded attempts. This existing retry behavior is not changed here.

One protected failure is proven pre-existing: retained_daemon_external_module_cycle_recovers_without_stale_events
fails Disabled-mode Build with build_runtime_error/exit2 instead of unsupported_feature/exit7
at both candidate and exact base 643578e1a. Baseline server/REAPI sources were verified against
32 Git blobs; unchanged dependency sources and package versions were retained. Separate
baseline preparation passed in 19.332s after a 60s preparation cap; the exact failing selector
took 0.073s. Its assertion is unchanged and the diagnostic defect remains open.
An optional older Run conflict fixture exceeded a 12s group estimate before its first case
completed; it supplies no passing evidence and was not repeated. The changed Run IPC is
covered by the passing server wire, CLI registry and parsing/launch controls. An initial
Run-unit selection targeted the main wrapper with zero tests; the library target supplied
all three verified selectors. No broad suite or new Bazel execution was needed.

The three new focused proof modules total 1,034 lines, exceeding the original 300–700
estimate because both product adapters need invocation/lifecycle coverage, raw IPC
needs separate malformed-policy/target and redaction checks, and the pure summary needs
producer-subset negatives. New tests live in three focused children; the wire variants share
one fixture/driver, and offline modules reuse the existing built-in workspace writer.

The next demanded family is default binary runfiles support, documented in bootstrap-readiness.
M7A remains partial and M8 unproved. Preserve this accepted checkpoint before selecting that
bounded implementation contract. Independent final review: ACCEPT, confirming native ownership,
selected publication, typed errors, immutable remote policy, both backend proofs and baseline
failure attribution. Rust formatting, diff, plan consistency and archive checks pass.
