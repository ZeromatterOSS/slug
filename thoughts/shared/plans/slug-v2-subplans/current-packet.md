# WP1091 cumulative normal producer intervals

Packet: WP-7-303-m7a-cumulative-producer-intervals-r1
Status: accepted — root compute window dominates; producer time fit open
Baseline: 4e409e5ac6262ddfc7f47e7b1d92355b930005ae (WP1090 accepted)
Owner: existing opt-in Core native probe observer and synchronous native driver.
Compatibility: Slug-native finite Linux/x86_64 observer only; admitted exact Bazel
surfaces and Windows minimal/deferred remain. No new semantic state or donor.

Outcome: measure cumulative normal retry time windows for injection, root compute,
acquisition plus merge, and idle drain, so the next implementation addresses measured
work. WP1090 identifies validate_source_progress entry as first deadline detector;
latest compute is10.809065ms wall/10.772424ms processCPU only. Existing two frames
replace earlier intervals. Authority: target/wp1090/next-work-frontier.md SHA256
cc066c17d1a94522c6f2631788002d0dde8174a205edf3547f673643b7fa5c87;
accepted predecessor receipt and actual phase guards. No optimization or cause claim.

Decision: reuse the existing64-word/512B mapping, with version2 interpretation.
Words0–50 preserve frames/events/header except version. Words51–63 hold exactly
four triples(interval count,wall ns,processCPU ns) at51–62 in category order,
and the finite status word at63.
Categories: AttemptInjectionRevision; RootCompute; NeedRepository/NeedEnvironment/
NeedPath combined; new IdleDrain phase appended to enum around the existing normal
post-attempt wait_for_idle. Preserve statement order, no locks across awaits or new
DICE dependency, key, invalidation, Need/error/publication/cancellation change.
Acquisition includes full existing observation and merge bracket; downloads,
executes, validation, seal/drop and other unbracketed work remain explicit residual.
Process CPU is all process threads within windows, not exclusive/key CPU. Normal
selected brackets are serial/non-nested; concurrent observations must not lose
updates, and overlapping windows cannot be construed as exclusive work.

PhaseGuard retains only entry-clock scalar POD as transient phase scratch; capture
entry/exit clocks once per existing frame, then reuse those tuples for totals.
No timeline/history/String/path/label/Location/payload, collection, cache/interner,
callback/allocator, sourceIO, shared timer pool or new mapping/observer allocation.
Borrowed static event tags and existing shared Arc ownership remain. Report actual
PhaseGuard size/alignment and unchanged Observer/Mapping/control allocation.
Use checked clock differences and atomic checked accumulation. Missing/reversed
wall/CPU, count/sum overflow, aborted partial intervals and disabled/suppressed
closures must be visible qualifications, never zero-work or completed proof.
Sticky status cannot be lost concurrently; total triples are interpreted only once
all guards/listeners retire. Preserve existing frame sampling, mapping validation,
Weak lastDrop and existing disable rules; any new post-disable qualification must
be scalar status only, never frame/numeric/clocks publication or revived activity.
Explicit enter/deferred entry/exit are frame-only; selected totals come from guards.

Exact source allowlist: app/slug_core_v2/src/runtime/probe_observer.rs;
probe_observer/mapping.rs; tests/probe_observer_tests.rs; runtime/dice.rs ONLY the
normal wait_for_idle bracket. All abbreviated files share runtime prefix.
Existing cohesive observer/helper/tests remain; dice.rs exceeds2000lines but only
2 adjacent macro calls, no new responsibility/extraction. Helpers<150lines;
estimated80–130 implementation and100–180proof lines, inspect actual growth.
Root may edit scheduling, compact receipt and matching Stage9 representation facts.
No fixture/profile/dependency/source acquisition; existing raw64-word report needs
no Rust change. Root decodes version2 under pinned source, never reinterprets oldv1.
DICE authority: docs/developers/dice.md native drop-before-idle-before-retry contract;
existing begin/finish macro and original normal driver/observer tests. No parity
change requiring new Bazel oracle. Existing exact surfaces get zero fresh credit.

Proofs: existing fixedmapping/invalidFD/frames/counters/concurrency/disable/Weak/
threadlocal/activationtracker/APIteardown tests preserved. Compact controlledclock
proofs cover repeated category folds, category routing, finish versus aborted, exact
latestframes, missing/reversed clocks, checked count/wall/CPU overflow, concurrent
increments, disable before/after entry and unchanged mapping/control allocation.
No sleeps/benchmark/fixture tree. Actual normal native supplies cumulative intervals.

Gates: independent DESIGN before metadata/lifecycle edit, root source inspection,
pinned changedfile format/freeze, independent actual-source/prelaunch and FINAL.
Fresh pinned nightly original-profile locked/offline/jobs1 Core native-feature
no-run CargoJSON; current ordinary observer owner list, one exact batched preflight
and nonzero actual selected/pass. Named Core/CLI production check plus feature-on
Core check if no-run cannot establish productionfeature compilation. Exact ignored
singleton list then ONE changed-source native under unchanged350sources/18env,
held non-inheritedFD/fullhash brackets/currentpins and capped existing runner.
Preparation60s is required to compile changed Core/current executable; ordinary
observer controls should be seconds. No concurrent Cargo/writers during gates.
Format/diff/archive/planstatus and accepted receipt/commit. Full goal ACTIVE.

Caps inherited unchanged: prep60s/RSSuncapped/no workloadcredit; native10s/
RSS256MiB/AS512MiB; live+transfer16MiB/decode+write64MiB/source256/registry128/
object1MiB/admitted16MiB; directory128/name1KiB/total16KiB; selectedwork7/default5/
lifetime8/cleanup3/FSIZE16MiB/log1/report4MiB. User12s guideline not general ceiling.
No cap rise, identical native retry, speculative allocation or global pool.
REPLAN only new semantic/ownership/resource prerequisite or unmappable intervals;
routine compiler/helper corrections retain failed evidence and correct same owner.
Acceptance must report measured qualified totals, residual windows, refusal/memory
and next substantive work owner; observer test pass is not fullproducer/wholefit.

Immediate predecessor:4e409e5ac records firstcaller7:296;40 relevant controls pass,
2unchanged socketfixtures unproved; native207.941MiB/256 and245admissions then
WorkDeadline. Full producer/time/wholefit/Rustc/actions/certificates/bootstrap open.

DESIGN ACCEPT: target/wp1091/design-review.md SHA256
3eea33230d19e0d37b286e90cbd63d040ac024742c93c5aa9e1fafc511fbfb06. Frozen design/source baseline bound by design-binding.json.

Acceptance: FINAL target/wp1091/final-review.md SHA256
3b89098b0cf37c8914ed7bec567e60a625f2e03a040cc26d99ba659bc05c1e32. All11 currentordinaryobserver tests and original-profile test compile,
default+feature-on CoreCLI checks, isolated exactignoredsingletonlist pass. Original
chainedlist contaminated preparationchildrusage remains0credit; correctedfreshshell
list passed sameexe/selector, actual16608KiB, no nativebody retry or capchange.
ONE native7.260448s/206.277344MiB of256 stopsWorkDeadline/source244admissions.
Retiredversion2/status0 reports321injection .172417830wall/.171438037CPU,
321root6.623002747wall/6.607024650CPU,320acquisition+merge .180601510wall/
.180001992CPU,321idle .013062777wall/.018852055CPU seconds. Root94.76% of measured
serialwindowwall; allthreadwindowedCPU, not exclusivekey/cause/speed.55pins/350FD
brackets exact. Mapping512/Observer8/Arc24 unchanged; PhaseGuard48/align8 vs
explicitoldshape16/align8 is phase scratch, not heapcredit. No string/pool addition.
[Receipt](wp1091-cumulative-producer-intervals-2026-10-10.json). Next trace concrete
repeated-root work under same caps; full producer/time/wholefit and goal remain open.
