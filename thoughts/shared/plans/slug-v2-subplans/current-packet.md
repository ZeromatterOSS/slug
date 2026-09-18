# Current Slug V2 Work Packet

Packet: WP-7-34-m7a-source-spawn-execution-r1
Status: final ACCEPT; checkpoint ready to commit

## Outcome and demand

Execute one closure-selected source-only typed Spawn through a public native
request operation, returning a single accepted result only after source validation.
Use the accepted WP-7-33 Command/Action projection and graph-independent cache
leaf. This advances the compiler/build-script Spawn execution prerequisite in
bootstrap-readiness; generated/tree inputs, scheduling the production closure,
CLI/daemon activation and bootstrap remain open. No broader build-success claim.

Inherit pinned Bazel 9.2 8220c6198837d5c13d53fea211cf3282aa12408a
RemoteExecutionService buildRemoteAction/execute, PlatformUtils, CommandLines and
REAPI remote_execution.proto; WP-7-33's exact content/protocol SHA-256 and named
Slug-native identity/wire-profile distinctions remain unchanged. Native request
ownership follows docs/developers/dice.md and the existing RequestRevisionRuntime
and NativeDemandAbortGuard invariants. No Java delegation or new semantic model.

## Core lifecycle and ownership

Add a private NativeCommandRoot completion hook at drive_command's selected,
associated, prepared-acceptance point before finalize_native. Ordinary roots use
a no-op default. Network work stays outside DICE keys/computations and no lock
spans a transport await. The live transaction, repository session, selected updater,
full epoch, effect selection and abort guard stay with this single native attempt.

A new Core source-action operation wraps the existing SourceStagingKey, retaining
the same prepared inputs and an operation-owned optional result (never a DICE
value). Check ValidatedActionClosure.execution_representative for the exact owner/
action coordinate before staging. A generic caller-supplied SourceActionTransport
stages inputs, then executes its staged value; Core never depends on REAPI. Callbacks
receive immutable prepared inputs, never a DiceComputations or selected updater.

After staging, Core performs a read-only pre-Execute check under the existing
request revision owner: current transaction version, full certificate association,
and repository-session reobservation of every certified demand. Drop the owner
lock before Execute. A precheck mismatch/error aborts without Execute or accepted
publication. After transport result, the existing finalize_native is authoritative:
source/version changes discard the result and retry through the existing bounded
revision loop. Never reuse a staged/result value across a changed attempt. Only
final acceptance exposes the result through AcceptedCommand and its selected events.
Failures/unwind/cancellation preserve abort cleanup and prior accepted snapshot;
remote CAS garbage or a completed content-addressed action may remain remotely.
No local output materialization occurs inside stage/execute or before acceptance.

Public SourceActionResult retains the prepared owner and transport result by Arc;
projection may inspect them only after Core acceptance. Transport state, futures,
channels, wire maps and output buffers are operation-owned and drop on failure,
retry or caller drop. No global cache/interner, new semantic key, lock across
compute, retained file handle or graph-injected command result.

## REAPI adapter and integrity

A request-local adapter owns immutable remote configuration. Stage constructs
SourceSpawnReapiPlan, uses existing verified source uploads and uploads Command/
Action bytes. Verify every required source, virtual file, Directory, Command and
Action digest with bounded read_blob_verified into a discard sink before returning
a staged value. FindMissingBlobs is only an upload optimization: it cannot certify
completed bytes, including NativeLink's in-flight writes. No retry of corrupt data
or source mutation; failure prevents Execute. Channel and instance remain bound to
the same staged plan and are reused for AC/Execute/output reads.

After Core precheck, reuse the existing AC lookup/Execute/result-shape/verified
output-reading implementation through a private shared helper. Preserve FileWrite
bytes, cache behavior and result shape. Accept only WP-7-33's empty requirements,
fixed environment and regular outputs. Unsupported transport policy must fail
closed rather than silently ignore supplied headers/retry/backend options. The
accepted operation returns verified output bytes; it does not invoke the existing
materializer or activate a CLI build. Future output materialization must preserve
declared types/modes and occur after acceptance.

## Scope and proof

Allowlist: Core runtime/dice.rs private hook/context and source module exports;
request_revision.rs read-only precheck; source_staging.rs internal selected-key and
representative access; new runtime/source_execution.rs/tests; runtime/mod.rs public
operation types. REAPI new source_execution.rs/tests and lib.rs; bounded helpers
in executor.rs and source_staging.rs/source_spawn.rs only where required for shared
transport and immutable prepared ownership. Minimal Cargo/BUILD dependencies only
if no existing Rust future support suffices. Canonical/manifest and Stage 7.
No CLI/server change, scheduler, generated/tree input support or materializer edit.

Core focused fake transport proves unchanged acceptance and one selected event
stream; source or build-only observation mutation after stage => zero Execute;
mutation during Execute => old result dropped and bounded retry accepts current
inputs; stage/execute errors and unwind restore prior accepted state; no callback
receives graph authority. Exercise representative rejection through the Core-owned
check without fabricating a public prepared set. Reuse prior source closure/conflict,
namespace, full-frontier and cancellation proofs where unaffected.

One tiny authored source script uses public Core operation + actual NativeLink to
prove cold Execute, AC replay, exact output bytes/digest and source change. A raw
bad/in-flight required CAS value must not pass the completion verifier or issue
Execute. Reuse the local verifying backend from WP-7-32 and protect the existing
FileWrite cold Execute/AC wire selector. No compiler action or broad fixture/suite.
No local output may appear on precheck/transport/final-validation failure.

Independent design and final review for the lifecycle/public boundary. Pinned
nightly preparation separately, --no-run JSON <=60s per operation, exact selector
preflight. Core tests and tiny wire tests expected under a few seconds; tests over
~30s require strict necessity and none are planned. Retain passing unaffected
checks. CLI direct compile coverage, changed Rust format, plan/archive/diff checks.
Raw receipts target/wp734; preserve unfinished work on the review branch.

REPLAN if callback integration changes DICE semantic ownership, loses events,
commits before the final source check, needs unmodeled execution policy, or cannot
prove completed CAS content before Execute. Do not detach request finalization or
replace it with independent ad hoc filesystem reads. M7A partial; M8 unproved.

Predecessor WP-7-33 accepted/pushed 916b1dd2a: closure-owned source Spawn wire plan,
nine focused gates and all test batches under one second. Actual execution and
request-integrated post-transfer acceptance remained closed.

## Candidate evidence

Independent design ACCEPT; root implementation stays on review/wp734-source-spawn-execution
from 916b1dd2a. New lifecycle/protocol logic is split into source_execution modules;
the large dice.rs driver retains only the private completion seam and borrowed context.
The completion future is statically dispatched, with no allocation for ordinary
roots and no new Send bounds on their terminals. Transport is a trusted extension
point; only Core can construct an accepted SourceActionResult. There is no public
async command-cancellation API in this slice; dropped transport work returns an
error through the existing guard, with panic/unwind and existing cancellation proof.

The supplied adapter rejects headers, explicit retries/timeouts and different CAS/
executor endpoints before connecting. Its staged state binds immutable policy,
channel, plan and completed verified CAS content. Source execution returns verified
bytes with no materialized-output evidence; FileWrite retains its old evidence.
Scalar FileWrite closure sharing tests the Core coordinate rejection, because Spawn
sharing is already rejected before preparation. Incremental events deliberately
suppress unchanged prior prints; a newly changed analysis print is published once
across the source-execution revision retry.

Raw receipts target/wp734: corrected real NativeLink source operation 1/1 in .918s
(cold Execute, AC hit, source edit, advertised in-flight required CAS => actual
adapter error after writer abort/cleanup, zero Execute callback). The first .891s
wire run proved only bounded non-completion; final review required an actual error,
so a timeout now fails the test. Protected FileWrite wire 1/1 in .303s; fresh
backends stopped (143) and removed. Unsupported-policy test 1/1 in .003s.

Eight Core selectors pass: four new operation gates (acceptance/retry/events,
source/build mutation, stage/execute error/unwind, closure representative), read-only
precheck association/version gate, existing synthetic cancellation, existing native
source closure/conflict/staging, and existing synthetic driver progress/events.
Final mixed batch passed five relevant selectors in 1.438s; two extra legacy build
controls failed before root computation on BCR rules_license Connect. Existing
hermetic source/driver controls replace those network-dependent controls (2/2 in
.523s). Representative and corrected event/retry proof passed 2/2 in .682s. A first
event test incorrectly expected an unchanged warm print; corrected to introduce a
new event against the existing incremental publication contract.

Pinned no-run/check preparations exit 0 (Core final 15.563s, REAPI final 12.638s,
CLI/direct Core/REAPI/server coverage 9.911s). Longest prep 30.499s; all individually
capped at 60s. Rustup snap launcher exited 46, so used installed pinned binaries.
Two early preparation corrections were test module path and PublishedCommand API.
Formatting, plan status, archive and diff checks pass. Longest test batch 1.438s;
none requires the user's >~30s exception. Independent final review ACCEPT after actual-error wire correction.

Independent final review confirms the lifecycle/CAS invariants against the actual
diff and the corrected receipts. Observable gate advanced: a public Core operation
executes one source-only typed Spawn through NativeLink and accepts its verified
result only after post-transport source validation. M7A remains partial; M8 unproved.
Recorded preparation time totals 147.218s across ten separate operations, including
two corrected invocations; none exceeded 60s. Packet wall/review time was not
measured across the pause/resume boundary; focused runtime times are above.
