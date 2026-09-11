# Current Slug V2 Work Packet

Packet: WP-5-7A-native-probe-observer-implementation-r1

Status: concrete reserved design independently ACCEPTED after one FD-ownership
correction; implementation not started. Only the implementation and harmless
gates below are authorized, never an automatic authentic probe or broad replay.

## Purpose and evidence

afacc2133 accepts the source audit and this reserved design scope. The unchanged
probe last compiled48.043s and stopped at native wall15.0093s, stdout16bytes of
test-start only, stderr0, no exact abseil open; cleanup complete. Evidence
/tmp/slug-sentinel-demand.8dkd3P/logs remains INCONCLUSIVE. No active-phase,
demand/non-demand, performance or historical14GB attribution.

Deliver one opt-in observation of the exact CLI library probe/native API/input
route, sufficient to retain the last committed phase and bounded DICE activity
even when killed. This remains a prerequisite to complete selected-request/
output-conflict R2 proof, not a substitute for actual CLI/daemon conflicts or
positive common execution-view/REAPI sharing.

Classification: Slug-native diagnostics, no semantic/Bazel compatibility change.
Existing exact Bazel9.2 source/registration behavior remains. No new Bazel oracle:
the new surface is an internal observer, not observable compatibility semantics.
DICE api/events.rs, api/user_data.rs and impls/events.rs provide the independent
listener; impls/tests/events.rs::test_events_modern proves compute vs dependency
checking across transactions. Reuse that contract, not its Mutex<Vec> collector.
No DICE engine edit, key/equality/invalidation change, scheduler donor, interner,
semantic cache, source bypass, fallback, fixture substitution or acquisition.
DICE docs and Stage9's retained-DICE/utility rows are concept guidance; existing
Arc/Allocative plus fixed scalar storage suffice, no V1/Buck2 extraction.

Design read8 new owners: DICE impls/events.rs and impls/tests/events.rs, CLI
Cargo.toml and src/lib.rs, installed linux/memfd.h and linux/fcntl.h, installed
memfd_create(2) and mmap(2). Reused the prior13 owners as needed; excerpts<2MiB.
Linux references establish RAM-backed anonymous file lifetime, inherited FDs,
fixed-size seals and MAP_SHARED. No kernel mechanism has been executed here.
Source sizes: runtime/dice.rs12479lines, runtime/mod.rs423, driver318, probe35.

## Build and activation boundary

Add default-off feature native-probe-observer to Core; CLI's same feature only
forwards it. The driver compiles the exact slug_cli_v2 --lib --no-run target with
--features native-probe-observer and keeps strict compiler JSON/artifact selection.
CLI lib.rs rejects feature && !cfg(test) with compile_error: the opt-in CLI binary
cannot be built accidentally. Core remains a normal dependency (not cfg(test));
its explicit diagnostic feature is necessary for the cross-crate hook. Do not
claim a Cargo feature alone makes Core test-only. Only the CLI cfg(test) probe
installs it; default CLI/Core builds compile out all new state and call sites.
No RUSTFLAGS, dependency/profile change, ambient environment activation, or new
production API signature. Other targets/platforms cannot run this diagnostic.

Core exposes feature-gated runtime::probe_observer::{ProbeGuard, Phase} solely
for the probe. Compile the implementation only on Linux x86_64 with 64-bit
atomics/little endian; reject unsupported opt-in builds, do not add a fallback.
ProbeGuard::install_from_fd(OwnedFd) is safe and validates/maps the channel.
The launcher transfers one fresh inherited FD to the native test process: no
Rust owner or concurrent closer exists there. The test alone reads
SLUG_SENTINEL_OBSERVER_FD and performs the sole documented unsafe
OwnedFd::from_raw_fd adoption under that protocol, immediately passing ownership
to install_from_fd. Metadata checks do not prove ownership; this is not a safe
arbitrary-RawFd-consuming helper. No duplicate/close-original sequence. Core
validates/maps then closes OwnedFd on every success/error path, before native
work or any semantic child spawn. Initialization error stops the diagnostic.
No observer means no diagnostic I/O or environmental reads in the Core API.

A !Send initiating guard installs one Arc in thread-local storage; nested install
is rejected, concurrent threads have isolated slots. The exact existing one-shot
API alone captures it and attaches a feature-only Option<Arc<Observer>> field to
its new WorkspaceRuntime after construction. Other API routes do not capture it.
user_computation_data clones that field into data.tracker only when present;
leave data.activation_tracker, cycle detector, spawner and semantic data unchanged.
Callbacks own only the observer Arc, never runtime/DICE/key/terminal references.
Phase call sites use the captured/runtime observer, not ambient per-worker TLS.

## Storage, callbacks and post-stop transport

Use one512byte anonymous memfd, created/zeroed by the Perl supervisor before fork,
with MFD_ALLOW_SEALING and F_SEAL_GROW|F_SEAL_SHRINK|F_SEAL_SEAL. No file pathname,
huge pages, PROT_EXEC, named scratch file or write seal. Parent keeps one FD;
only the launch child clears CLOEXEC on its channel FD across existing wrappers.
The test's transferred channel FD is closed after mapping and before native work;
wrapper-process copies remain finalizer-owned until complete tree quiescence.
Validate fstat size512, seals,
access mode, zero/version header and alignment before accepting MAP_SHARED.
Parent never modifies or reads the payload while any supervised writer can live.

Use a fixed [AtomicU64;64] mapping. Freeze byte layout before Rust:
- words0..7: magic/version, installed PID, disabled, overflow flags, dropped
  activity samples, activity try-claim flag, phase committed-slot selector;
- words8..13: six independent DICE event counters in enum declaration order;
- words14..21: two phase frames of4words: Phase ID/status, monotonic ns,
  process-CPU ns, validity bits;
- word22: activity committed-slot selector;
- words23..50: two activity frames of14words: event kind, monotonic ns,
  process-CPU ns, validity bits, full tag length, copied tag length,64tag bytes;
- words51..63: zero reserved; no dynamic fields or address/graph serialization.
Selectors start at0 (no committed record), then alternate1/2. Write inactive
frame atomically word by word, release-store its selector last; after terminal
quiescence decode only the selected complete frame. A killed partial write leaves
the prior frame authoritative. Counters are independent observations, not a
coherent cross-field snapshot; never infer in-flight counts by subtraction.

Every active callback performs one relaxed fetch_add on its event counter;
wrap sets a sticky overflow bit, invalidating totals rather than hiding wrap.
On that kind's first event and each1024 thereafter, try one compare_exchange
claim; contention increments dropped-sample count and returns, never retries.
A claimed sample reads CLOCK_MONOTONIC and CLOCK_PROCESS_CPUTIME_ID, copies at
most64bytes of the static tag and commits its inactive frame, then releases claim.
No allocation, graph formatting, I/O, shared-lock wait or await in callbacks.
Clock failure is a validity bit, not fabricated zero time. Scalar overflow uses
sticky flags, including dropped-counter wrap. Truncated UTF-8 is bytes, escaped
only by the supervisor; never claim a truncated tag is a complete key identity.
No unbounded scan/hash/table or duration pairing. Constant work is an operation
bound, not a hard scheduler/kernel latency guarantee. Phase recording has one
initiating-thread writer and the same two-frame commit rule.

Telemetry is command-retained nonsemantic state:512mapped payload plus at most
512bytes of Arc/control/TLS/guard metadata and bounded stack sample scratch,
well below4096 logical state bytes. Report actual sizes, Arc headers and manual
Allocative mapped-byte accounting; no skip that hides the payload. Linux mapping
rounds to one4096byte VM/backing page; report that OS granularity separately from
logical state and count it against unchanged AS/RSS. No extra thread or stack.
Only the observer Arc owns the mapping; last Arc Drop munmaps512. Closing an FD
does not unmap it. All accesses use atomic references; mapping unsafe is isolated
in mapping.rs with alignment/length/aliasing/Send+Sync/lifetime safety arguments.
The sole exception is the CLI test's documented inherited-FD ownership adoption.

After unconditional kill/reap/drain proves ECHILD, no pipes and no process group,
parent reads exactly512bytes and closes the memfd on every success/error path.
If cleanup is incomplete, do not decode a live payload. Missing installation,
bad header/frame or read failure is explicitly unavailable/INCONCLUSIVE, not zero.
No extra pipe, polling thread, live shared-memory reader or blocking callback
transport. Raw512 + bounded decoded JSON/metadata<=8192bytes; reduce exact-path
strace allowance to57344 so total trace+telemetry stays65536. Existing stdout/
stderr8192 each unchanged. All telemetry formatting is post-stop and cap-checked;
capacity/encoding failure cannot silently publish partial valid evidence.
Backpressure is inapplicable to shared memory; prove fixed-cap writes and output
failure instead. Parent retains bytes after child death without retaining DICE.

## Phases and lifetime

Freeze Phase entry/exit/error IDs for: native API, runtime construction, request
configuration, command preflight, attempt injection/revision, root compute,
terminal closure selection, acceptance preparation, revision finalization,
transaction cleanup, final acceptance, Need service (repository/environment/path),
runtime teardown, diagnostic projection, event publication, terminal release.
Use bounded scalar guards around the existing boundaries; error/unwind records
must not say success. Latest committed marker proves only that boundary was
reached, not a causal leaf or which instruction consumed the remaining deadline.

In the existing API declare an exit guard before WorkspaceRuntime and teardown-
entry guard after it; reverse local destruction marks runtime teardown before
and after original runtime destruction, including early return/unwind. Distinguish
construction failure. Add observer as the last feature-only runtime field; never
reorder existing fields or explicitly drop a semantic value early. Preserve the
tail-returned accepted value and native API signature.
In the test preserve exact BuildRequest and native call; bracket projection,
bind only published = projected.publish(), then bracket the unchanged wildcard
let (_, exit_code, stdout, stderr) = published.into_parts(). Terminal still dies
at that statement before original stdout/stderr/assertion; no retained terminal
binding, duplicate publication, error clone, new exit code or moved output.
Harmless Drop-order proof must discriminate both normal and unwind paths.

Guard Drop disables observation and removes its TLS slot without waiting; a
callback already in progress may finish, but owns an Arc and cannot use unmapped
memory. Late callbacks observe disabled and return. No join or invented DICE
quiescence on guard Drop; parent decoding waits for real process-tree termination.
Workers retaining only observer metadata may outlive the guard; last-listener
release, not command acceptance, releases mapping. No cycle or global cache.
Panic/cancellation use the same ownership path; SIGKILL uses kernel mapping/FD
release and the existing supervisor finalizer. Prove reinstallation/isolation and
that an old listener never writes into a later observation.

CPU/wall clocks measure only committed boundaries/samples in the native process;
they are not inclusive key duration, retired instructions, RSS or causal blame.
The final unsampled interval is UNKNOWN: CPU loop, external wait and teardown
cannot be distinguished from silence alone. Sampling omission/drop/overflow and
clock invalidity remain visible. No performance improvement claim or timing-based
semantic acceptance; balanced authentic replays are not authorized by this design.

## Implementation envelope and gates

Exact write allowlist (no other Rust/harness/Cargo/fixture files):
1. app/slug_core_v2/Cargo.toml
2. app/slug_cli_v2/Cargo.toml
3. app/slug_cli_v2/src/lib.rs
4. app/slug_cli_v2/src/payload_demand_probe.rs
5. app/slug_core_v2/src/runtime/mod.rs
6. app/slug_core_v2/src/runtime/dice.rs
7. app/slug_core_v2/src/runtime/probe_observer.rs (new)
8. app/slug_core_v2/src/runtime/probe_observer/mapping.rs (new)
9. app/slug_core_v2/src/runtime/tests/probe_observer_tests.rs (new)
10. tools/v2_oracle/run_payload_demand_probe.sh

Caps650 production/1100 proof/1750 gross added lines, measured against design
commit before implementation acceptance. Design allocation: observer<=300,
mapping<=160, existing runtime call sites<=120, feature/module wiring<=30,
remainder40 production contingency. Proof envelope: tests<=700, driver<=250,
CLI test<=80, remainder70 contingency. Count driver as proof, gating/library
wiring as production. These are estimates/caps, not measured implementation.
The new helper isolates diagnostics/unsafe transport from12479line dice.rs;
only phase/listener call sites belong there. No semantic driver refactor.

Harmless proofs (no authentic fixture needed): fixed layout/accounting, disabled
output/default-build exclusion, invalid FD/size/seals/platform (including owned
descriptor closure on validation failure after adoption), six counters and
overflow, sample truncation/clock failure/contended bounded work, interrupted
inactive-frame commit, concurrent callbacks, nested rejection/thread isolation,
disable with callback in flight, final Weak release/unmap, old/new guard isolation,
Drop order on success/error/panic, DICE event listener plus activation preservation.
Use retained DICE test theme for a tiny injected-key change/dependency-check,
not its collector. Need/cancellation semantics themselves are unchanged.

Driver --self-check must not compile/stage/run authentic inputs. Extend accepted
normal/deadline/exception finalizer proof with memfd inheritance, capped retention
after killed writer, unavailable/corrupt record, output failure and FD cleanup.
Reuse original self-check results except changed channel/cleanup branches.
Add an explicit compile-only driver mode so opt-in compilation cannot launch a
probe. Pinned default Core/CLI cargo check plus focused feature-on Core tests and
CLI --lib --no-run JSON check; feature-enabled non-test CLI rejection is expected.
All compiler commands timeout60; direct harmless tests timeout15, driver self-
checks each<=5. Serialize Cargo; any timeout is investigated, no automatic retry.
Default build output/API/semantic owners unchanged by source/cfg and focused
proof; no default binary replay or broad suites for this diagnostic.

Docs/diff, full R2 hash/forward apply, old draft hash, and archive exact known3
failures remain gates. Independent architecture acceptance is recorded; require
independent implementation/lifecycle close before selecting at most one authentic
attempt. Original compiler60/native15/AS2GiB per process/CPU15/FSIZE16MiB,
isolated networking, exact staging, finalizer and no-acquisition limits remain.
No runtime is automatically authorized by successful tests.

Scope overflow, unsafe lifetime/FD uncertainty, nonconstant callbacks, changed
semantic/drop order, hidden memory/output, new dependency/profile, higher limits
or missing lifecycle proof is REPLAN. Scheduling docs: manifest,
canonical Live Status and Stage5 summary (<=100 added lines outside manifest);
routing only for REPLAN, PROGRESS<=500.

Complete output-conflict R2 /tmp/slug-conflict-r2.XZJWwv/candidate.patch SHA256
90c725e40a7aa46f5f0e81112bfe5f91a429679d9bd3824ae7dda9417725d94e stays preserved/
unaccepted: never partially restore/ship. Old probe draft SHA256
8eef40138afa23caa2601b89e6006de40f7e6d139f40124f4c2c430ae5fdf0a2 unchanged.
No CLI/Bazel/broad replay, process attach, source acquisition, secret inspection
or semantic workaround. Never inspect/print/copy ~/.bazelrc.
