# Current Slug V2 Work Packet

Packet: WP-5-7A-native-probe-observer-design-r1

Status: source audit and reserved design scope independently ACCEPTED; design not started.
Docs only. No instrumentation, compiler, test or runtime execution is authorized.

## Learned facts and selected decision

The source-only phase audit at2cf2a4dd3 inspected5 initial+8 additional owners,
full-file size699829bytes, excerpts<2MiB. Stage5 owns exact source anchors.
The first native output follows runtime construction, request preparation, DICE
attempts/source capture, terminal acceptance, WorkspaceRuntime destruction,
terminal projection, event publication and discarded-terminal destruction.
No prior trace identifies which phase consumed the15s limit.

Existing DICE DiceEventListener is independent of the semantic activation tracker
and currently defaults to NoOpTracker in native UserComputationData. Its six
static-key-type events can expose DICE activity without key/graph serialization.
Existing activation/event owners are semantic; their tracker must remain intact.
Core NativeDemandTestTrace is cfg(test)-only, late-stage and Mutex/Vec-backed;
it is absent from the CLI probe's Core dependency and is not a live timing hook.

Choose one reserved design: a default-off, test-build-only Core observer behind
a narrow opt-in build gate, carried through the existing DiceEventListener, plus
fixed native/teardown/publication phase observations. Preserve the existing CLI
library test target, exact BuildRequest/native API, input fixture and destruction
order. No public production API signature change, general observer framework,
Core unit-test target migration, copied evaluation driver or semantic workaround.
A build-gated cross-crate diagnostic boundary needs independent design acceptance;
this manifest selects that design work, not a frozen implementation or runtime.

Presentation/telemetry is Slug-native. Existing exact Bazel9.2 registration/source
semantics remain unchanged; no new Bazel oracle is applicable to this design.
Use retained DICE api/events.rs and api/user_data.rs as existing supported hook
contracts, plus docs/developers/dice.md for ownership/locking. Reuse Stage5 fixture/
source provenance and accepted finalizer tests; no donor or fallback is selected.

## Required design deliverable

Freeze one concrete implementation contract, with exact allowlist and measured
production/proof/gross caps, addressing all of these before code:

- The build gate is default-off and selected only by the opt-in probe driver.
  Production CLI/default builds retain no observer state or output; no ambient
  user environment turns the observer on. Keep the original native API call.
- The initiating test owns observation resources; at most4096bytes retained
  telemetry state, no key/graph/value/label/error clones or unbounded collections.
  A listener sees static key-type tags and scalar counters only, constant bounded
  work per event. Name overflow, concurrent update and cancellation semantics.
- Preserve the semantic activation tracker, event selection, Needs/order, DICE
  keys/equality/invalidation, source certificates and accepted publication.
  The observer cannot decide success, skip work, change source requests or prolong
  retained terminal/DICE lifetimes beyond existing ownership.
- Name markers for runtime construction, command preflight, DICE attempt/root
  evaluation, Need service, terminal selection/validation/acceptance, runtime
  teardown, diagnostic projection, event publication and terminal release.
  Cover early error/unwind paths without borrowing command scratch across tasks.
  Do not move terminal destruction past output or omit runtime teardown.
- Freeze a bounded, nonblocking transport independent of semantic stdout/stderr.
  Preserve stdout/stderr8192bytes each and total trace/telemetry<=65536bytes
  (subdivide the existing trace budget, do not add a larger output allowance).
  Never block a DICE callback on I/O or a shared lock across await.
- Specify how bounded wall/process-CPU samples complement phase/DICE counters.
  Inclusive DICE duration is not CPU time, last key type is not a causal error,
  and unfinished events alone do not prove waiting/deadlock. Sampling failure
  or dropped observations must be explicit, not fabricated as zero.
- Freeze complete lifecycle: guard installation, concurrent/nested rejection or
  isolation, activation, quiescence, writer/thread/FD close, drop/panic/cancel,
  last Arc release and supervisor forced termination. Reuse the unconditional
  kill/reap/drain finalizer; any extra pipe/thread gets bounded cleanup proof.
- Preserve compiler60/native15/AS2GiB per process/CPU15/FSIZE16MiB, isolated
  networking and exact input staging. No profile/dependency implementation change
  just to obtain a faster build. Any selected gate-only Cargo edit must be explicit.
- Name harmless proof for disabled output, finite capacity/overflow, callback
  concurrency, I/O failure/backpressure, panic/cancellation and final-reference
  release, plus direct default/opt-in compile checks. Do not execute them here.
  At most one future authentic attempt only after implementation and review.

No measured improvement or active-phase diagnosis is claimed. Timing can guide
investigation only; it cannot become a semantic cache key or acceptance shortcut.

## Research scope and complexity

Read the five audit initial owners and its eight named additional owners only as
needed; do not reconstruct accepted audits. Follow at most8 additional directly
referenced hook-emission, transport or lifecycle source/doc/test files, excerpts
<=2MiB. No cache scan, payload/dependency enumeration, source acquisition or runtime.
A new observation helper must isolate instrumentation from oversized dice.rs
(>12000lines); only bounded phase/hook call sites belong in existing owners.
Do not couple semantic discovery, transport and metrics into that module.

Docs writable: this manifest, canonical Live Status, relevant Stage5 design,
routing log only for REPLAN; <=100added lines outside manifest, PROGRESS<=500.
All Rust/driver/Cargo/fixture additions0 in this design packet. Missing ownership,
unbounded callback/output/lifetime, changed semantic behavior, higher limits or
scope overflow is REPLAN. An additional shared public semantic boundary is out.
Independent reserved review is required before any implementation; validate docs,
diff, R2 hash/forward apply, old draft hash and archive's exact known3 failures.

## Preserved evidence and exclusions

a06f3ddfc accepted typed registration identity/bounded diagnostics at473/1380/1853.
Latest authentic evidence /tmp/slug-sentinel-demand.8dkd3P/logs remains
INCONCLUSIVE: compile48.043s succeeded; native wall15.0093s/raw9/driver2,
stdout16bytes test-start only, stderr0, no exact abseil open, RSS unavailable.
Cleanup complete/no survivors; staged trees removed, logs retained. Prior
/tmp/slug-sentinel-demand.PXDhxy/logs remains INCONCLUSIVE independently.
No historical14GB attribution, demand/non-demand or source-closure claim.

Complete output-conflict R2 /tmp/slug-conflict-r2.XZJWwv/candidate.patch SHA256
90c725e40a7aa46f5f0e81112bfe5f91a429679d9bd3824ae7dda9417725d94e remains unaccepted.
Old probe draft SHA2568eef40138afa23caa2601b89e6006de40f7e6d139f40124f4c2c430ae5fdf0a2
unchanged. Never partially restore/ship R2; no CLI/Bazel/broad replay, process
attach, compiler/self-check/native run, credential inspection or semantic fix.
