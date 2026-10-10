# WP1090 first deadline check caller

Packet: WP-7-302-m7a-first-deadline-caller-r1
Status: accepted — detector known; preceding work/time fit open
Baseline: c23c62f5fa860de16289da5ab91676fe2dd56974 (WP1089 accepted)
Owner: existing Core source-service first-timeout evidence; normal producer continuation.
Compatibility: Slug-native Linux test diagnostic only; production errors, ownership,
DICE and admitted Bazel surfaces unchanged. Windows remains minimal/deferred.

Outcome: identify the exact source callsite which first detects the common work
deadline at a generic ScopeCheck. WP1089 now fits the observed prefix at207.918MiB
of256 but stops at WorkDeadline after246 admissions. Existing site1/domain1/phase1/
return1 identifies GenericScope/Common/ScopeCheck/Deadline, not its caller. The
last Java FileBytes charge precedes body completion. This packet supplies missing
caller evidence for broader normal producer work; it does not select an optimization
or infer CPU-time owner, completed producer, whole-workload fit or speed.
Authority: WP1089 committed receipt; target/wp1089/next-work-frontier.md SHA256
c09fd7ba84b6c9be1381722046fa27f608d36ca18b8534c499cd1e99c90fe961;
current timeout_provenance/scope checks and command/kernel forwarding sites.

Decision: extend the existing first-work Linux-test evidence with optional POD
caller metadata: finite owner code plus u32 source line. Use compiler Location
only transiently to project a closed list of actual current source owners; unknown
must project honest owner0/line0. No retained Location/file identifier/reference,
String/path/request/payload, address mapping, timestamp series, stack/profiler,
new allocator/cache/interner/key/lock or semantic runtime field. Existing compiler
static identifiers are borrowed synchronously; no string allocation/interner needed.
Only armed first-created WorkDeadline at GenericScope/ScopeCheck records this caller.
Existing explicit wait sites/phases/timer origins carry no generic caller. Unarmed,
non-work and previously poisoned scopes cannot invent or replace one. Preserve
first-creator locking, cancellation, return/error/poison precedence and sticky state.
No callbacks or formatting under the lock; report actual evidence POD/layout growth.

Use cfg_attr(all(test,target_os="linux"),track_caller) on the narrow synchronous
check→check_locked→check_locked_at forwarding chain. Capture the projected caller
only in the first generic WorkDeadline branch when evidence armed. Add propagation
attributes only to the existing WorkspaceRuntime/NativeDemandCommand check_source_scope
wrappers and private kernel check_scope. The kernel's map_or closure hides caller;
replace it with an explicit equivalent match (None Ok, Some original check/map_err).
All production signatures and routing stay; cfg-only attributes do not change
normal runtime semantics. A shared checked_io detector line remains unable to name
its Pin/Stat/Open invocation; do not silently widen callsite evidence into stage/time.
Pin exact source bytes used to interpret numeric lines in the native receipt.

Exact source allowlist (all under app/slug_core_v2/src/runtime/):
- source_service/timeout_provenance.rs: caller POD/projection and gated origin field.
- source_service/scope.rs: narrow forwarding/armed first-origin capture only.
- source_service/timeout_provenance/tests.rs: missing synchronous caller discriminators.
- source_service/diagnostic.rs: attributes on the two existing check_source_scope wrappers only.
- path_observation.rs: private check_scope equivalent match/propagation and small existing-module forwarding proof.
- tests/platforms_module_preparation_tests/runtime_activity.rs: append scalar caller fields to existing WP1008_TIMEOUT output after snapshot.
No dice.rs/source reader/IO/admission/profile/resource/global observer change. Root
may update matching Stage9/evidence receipt and scheduling. Existing cohesive source
owners may exceed complexity triggers; helpers<150, estimated30–70production and
50–100proof lines; inspect actual growth. No fixture tree or unrelated extraction.

Proofs: existing timeout selector/timer/entry/post/final/competition/sticky/disarm
controls remain. Add bounded deterministic expired generic checks at distinct
line!()-based callsites; finite owner/line exact, first sticks across laterlocation,
original error/cancellation unchanged. Unarmed, prior cancellation/limit poison and
explicit wait/timer origin give no caller. Closed projection includes known and
unknown owner controls. One actual private kernel forwarding test proves caller
propagates through the selected wrapper, not merely an internal scope line. Command
attribute chain is reviewed against actual source; reuse existing relevant command
controls, do not build an extra fixture merely to mirror Rust track_caller. Record
actual sizes/alignment separately from heap; no allocating metadata/body/profiling
framework. Expires use existing tiny bounded fixture, no sleep benchmark.

Gates: independent DESIGN before consequential first-origin evidence change;
root actualdiff/pinned format/freeze; independent source/prelaunch and FINAL.
Fresh pinned nightly; serial locked/offline/jobs1 current Core native-feature no-run
CargoJSON. List exact ordinary timeout and relevant kernel forwarding/owner tests,
one exact batched preflight per executable, actual nonzero selected/pass. Current
Core/CLI production check protects equivalent wrapper flow/non-test compilation.
Current ignored native singleton list; ONE distinct instrumented candidate native
with accepted350triples/18env and unchanged caps, held non-inherited FD/hash/pins.
No writers during gates/native; streamed64KiB hashes/linewise CargoJSON. Report
whether generic first caller is available, its pinned source owner/line and actual
frontier/refusal/memory; no CPU/speed/fullfit inference. Unknown stays unknown.
Existing generic lifecycle/contracts reused0fresh; no broad history/replay. Format,
diff/archive/planstatus and compact receipt required; full goal remains ACTIVE.

Caps unchanged: prep60s/RSSuncapped/no workloadcredit; native10s/RSS256MiB/AS512MiB;
live+transfer16MiB/decode+write64MiB/source256/registry128/object1MiB/admitted16MiB;
directory128entries/name1KiB/total16KiB; selectedwork7/default5/lifetime8/cleanup3/
FSIZE16MiB/log1/report4MiB. User12s guideline not a general cap. No duplicate native
or cap rise. Routine compiler/fixture corrections stay within owner; REPLAN only
new ownership/semantic/identity/resource prerequisite. No source acquisition.

DESIGN ACCEPT: target/wp1090/design-review.md SHA256
256f1d0a1653c8499e84d10df81ae347edd578f850c60a2b4af623b94cc7f1b2.
No source/gate credit from DESIGN alone; accepted contract now implementable.

Routine initializer prerequisite: root-only seventh caller file
source_preparation/genuine_root.rs adds caller:None to the existing Linux-test
explicit Headers/Timer origin literal. No flow/assertion/fixture change. Frozen
DESIGN six-owner scope stays; actual-source review covers this mechanical caller.
Baseline extension + current compiler pins include the original/corrected file.

Validation scope correction: full current42-name preflight succeeded, broadbatch
failed2unchanged directUnixListener fixtureEPERM/40observedpasses/0completecredit.
Those exactOSclassifier/lstat tests do not exercise changedcheck_scope; preserve
as unproved. Relevant40covered names thenPASS40/40 samecurrentexe/source, no new
privileges/source/assertionchange. selector-correction.md/environment-correction.md
qualify actualmoduleprefix and boundedgate; no fullOSkernel suite claim.

Acceptance: FINAL target/wp1090/final-review.md SHA256
10001c951ef1ccbf04bd6b1aac14a32444082f515d40acbd1eba6f9b6dc3b2f4.
40 relevant ordinary checks/currentCoreCLI/currentnative-feature compile pass;
2unchanged socketfixtures EPERM unproved and originalbroadgate0credit. Actualcaller
8B/align4; Linux-testorigin20/state24/snapshot24; originalshape5/7/8 comparisons
are not historicalbinaries. ONE7.261364s native207.941MiB/256 hitsWorkDeadline at
245admissions. Firstcaller7:296 pinneddiagnostic validate_source_progress entry,
not precedingCPU/time/IOstage/cause.57pins/350FDhashbrackets stable. No caprise/retry.
[Receipt](wp1090-first-deadline-caller-2026-10-10.json). Goal ACTIVE; next trace
normal producer work reaching this checkpoint using existing phase evidence.
