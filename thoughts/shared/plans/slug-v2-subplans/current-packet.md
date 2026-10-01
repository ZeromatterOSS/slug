# Current Slug V2 Work Packet

Packet: WP-7-185-m7a-generated-owner-cycle-path-r1
Status: accepted after independent FINAL review; M7A remains partial

## Outcome and inherited authority

At clean main47ed22e5b, implement the complete WP883 contract preserved at
`ec7e75983:thoughts/shared/plans/slug-v2-subplans/current-packet.md`, using WP884's
accepted opaque DiceLiveContextId. Admit module_ctx.path(Label) for an independent
generated owner through its completed certificate, canonical load route and real
immutable materialization. Same-owner metadata rejection precedes evaluation;
reciprocal, three-owner and generated defining-.bzl cycles terminate without
partial effects. No task-local/preflight-only graph or physical-root inference.
Source closure is target/wp883/key-closure.json. Bazel9.2 commit8220c6198837d5c13d53fea211cf3282aa12408a
ModuleExtensionResolutionTest1474–1599/1603–1635 supplies independent, reciprocal,
mixed and self-cycle authority; RepositoryUtils/BaseExternalContext supplies
package-backed Label path authority. No Java runtime enters Slug.

Compatibility: exact existing package/materialization/content contracts and
independent generated Label semantics; cycle diagnostics and coordinator plumbing
are Slug-native. Full configuration, execution/bootstrap and deeper graph memory
remain deferred. Existing DICE structural identities and observed epochs remain
unchanged. Completed producers own retained semantic facts; invocation prefixes
are diagnostics only and cannot publish events or file effects on Need/error.

## Concrete implementation and ownership

Loading adds dependency_cycle.rs with19 full concrete-key/mode node variants,
SmallMap/SmallSet adjacency and synchronous short-lock graph. Every started attempt
has a monotonic scoped ID, cloneable watch receiver and exact idempotent Drop
retirement. finished_computing_key retires its current exact attempt immediately;
late guard Drop compares its ID and cannot erase a replacement. Remove all
outgoing attempt edges on retirement; incoming logical dependencies remain owned
by their active parents, covering add_edge before cancelled-child finish/restart.
Pending targets exist only as active-parent outgoing edges and are traversed only
while their target attempt is active. No standalone finished nodes remain.
Notify outside locks; no
new actor backlog, sticky completed entries or lock across compute. Registry and
graph are nonsemantic service/active-worker plumbing, never DICE validity inputs.
Existing concrete BzlLoadCycleGuard/HostBzlLoadCycleGuard/ExternalBzlLoadCycleGuard
embed the bridge and forward immediate edges without changing their downcast type.
Old pure-family projections remain isolated. Mixed notifications have distinct
terminal/error handling and are consumed by .bzl route/source/child waits too.
Every mixed cycle terminal records existing transient BzlLoadCyclePoisonKey before
returning with current observations, so raced-away dependencies cannot cache it.

Private DependencyCycleGuard::guard_this(future) returns Result<R,DependencyCycle>.
Private compute_guarded(ctx,key) for owner/bridge keys returns
Result<Key::Value,DependencyComputeError>, retaining ordinary DiceError as an anyhow::Error source (no new dice_error
dependency) and typed cycle variants; it races the wait and adds poison on cycle. No detector may be
used by standalone existing acyclic tests, but independent generated admission
requires the real installed guard. Bzl callers use their embedded bridge directly
rather than attempting a wrong concrete guard downcast. Bridge/certificate/route
waits consume cycles too, including checking_deps warm participants.

Core adds runtime/scoped_cycle_detector.rs. One weak compact registry belongs to
RequestRevisionRuntime for its retained Dice. Fresh deferred proxy is installed in
userdata before commit, with typed pending binding handle. Postcommit bind uses
transaction.live_context_id() and selects one actual analysis_cycle_detector for
that exact live cache before compute. Same-cache transactions share graph; changed
versions, foreign engines and reopened same-version epochs do not. Dead weak
entries prune on bind. Userdata environment/effects/trackers remain request-local.
OnceLock proxies cannot migrate/recommit across contexts. Equality-only selected
updater existing_state remains unbound; every compute-capable constructor binds.
Existing WorkspaceRuntime-owned Tokio runtime owns retained legacy detector tasks.
Both existing .bzl and configured actors retire outgoing attempt state on finish,
compact/reindex surviving nodes/dirty IDs and retain referenced Finished targets as
Known placeholders owned by active parents. Unreferenced Known/Finished targets
prune; Cycle nodes retire on their own finish;
no completed history accumulates even under continuously overlapping requests.
Native synchronous cleanup drains dice.wait_for_idle after transaction drop before
Need/retry/accept/release and before/after discard restoration; no detached cleanup.

Root admits independent path only after metadata owner inequality and guarded
natural route. Generated seed owner must match preflight. Preserve exact namespace,
BUILD/package lookup and materialization markers; missing package differs from
missing target. No fabricated roots, producer changes or observation bypass.

## Files and staffing

Loading worker: new dependency_cycle.rs and its tests, cycle_detector.rs,
bzl_module.rs, lib.rs module declaration. Core worker: new scoped_cycle_detector
module/tests, runtime/mod.rs, runtime/dice.rs and request_revision.rs. Root:
module_extension.rs and a cohesive generated-path test module; innate repository,
file-effect, validation/certificate, generated_repository_definition,
canonical_repository_route and canonical_repository_load_route guard sites;
Analysis configured_analysis_cycle_detector.rs legacy-history pruning/regression.
Root also owns registration_diagnostic.rs mixed terminal formatting and raw
transaction binding adapters in Core runtime/tests/repository_host_input_tests.rs,
runtime/tests/build_command_tests.rs, runtime/build_requested_target/tests.rs and
runtime/requested_artifacts/tests.rs, plus cohesive Core generated path and shared-context cycle test modules
(runtime/tests/generated_owner_path_tests.rs and generated_owner_shared_tests.rs).
Root also owns scheduling, docs/developers/dice.md and Stage9 reuse disposition.
Independent reviewer checks concrete lifecycle/design before implementation and
actual diff/evidence before acceptance. Workers must not run concurrent Cargo or
edit other owners' files. Existing large files receive bounded adapter calls;
new graph/plumbing/tests have cohesive separate modules. Expected production
<1200/test<1200 added lines is a review trigger, not a limit. No new dependency,
DICE engine mutation, unsupported producer activation or unrelated cleanup.

## Required validation and integration

Resolve pinned nightly tools through rustup. Root serial Cargo --no-run JSON,
each preparation cap60s; exact selectors batched once per unchanged executable
via v2_test_preflight.py, focused runtimes20s unless justified. Required gates:
1. Independent cold Need/replay, actual immutable roots/namespaces/current-writer
markers, BUILD and dirname/bin; missing package versus missing target.
2. Same-owner metadata rejection; two/three-owner and mixed generated defining-.bzl
cycles; no partial events/effects. Warm cycle insertion/removal identifies actual
fresh notification consumer while historical dependencies check; A/B/A input,
mapping and producer changes preserve observations and poison behavior.
3. Same-context overlapping requests share producer/detector while perrequest data
remain distinct; different/revived contexts isolate, old cancellation can remain
live; Need/retry/drop retire graph attempts and weak entries remain bounded.
4. Full structural identity and mode distinctions, immediate bridge edges, exact
retirement/late token handling (callback→replacement→old-token-drop), repeated
same-context legacy actor retirement/bounded history, actual public DICE cancelled
child replacement callbacks fed into the real graph, inactive/old-outgoing negative
controls, pure .bzl cycle regressions and protected native
request cancellation/revision tests.
5. Direct Loading/Analysis/Core/CLI checks and rebuilt default CLI before one new
actual-root replay. Use fresh external state, unchanged input/CLI hashes, owned
process-group sampling250ms,4GiB RSS+swap stop,180s runtime cap and scoped cleanup.
These are packet operation caps, not newly imposed user limits. Runtime success
claims stop at the actual frontier. No repeated unchanged root retry.

Preserve coupled semantic activation atomically: if gates remain incomplete,
keep candidate on local review branch with baseline and receipt, never accept
partial generated admission. Root inspects graph memory/lifecycle; no performance
speedup claim. Format scoped changed Rust, archive/plan status and diff checks.
REPLAN only for a contradiction of inherited source ownership/complete closure
or unavailable prerequisite, not routine compiler/test correction. At acceptance
record gate/compile/review times, remaining frontier and independent FINAL verdict.

## Immediate predecessor

WP884 at47ed22e5b adds73 readonly adapter lines and232 public lifecycle test lines.
Six exact sharing/version/engine/revival/cancellation gates pass; direct four-crate
check passes. Token retains no cache and never replaces semantic equality. WP883
accepted design and WP880 metadata memory repair remain protected; measured root
peak~302MiB with no sampled swap, generated host-tools Label is the proved frontier.

## Consolidated validation receipt (2026-10-01)

Review branch `review/wp885-generated-owner-cycle-path`, base47ed22e5b,
preservation revisionf4ae7b50b. Preservation is not acceptance or partial
activation on main. Independent design and FINAL reviews ACCEPT; all required runtime gates pass.
Independent FINAL ACCEPT covers the actual diff and all45 gates. Production
source is unchanged since the preservation revision; later deltas
correct the shared native fixture, wrap one Loading test panic during scoped
rustfmt, and update acceptance documentation. The Loading format-only delta
preserves its executed semantic evidence.

Pinned nightly2025-09-14: Loading27/27 PASS (aggregate0.348s; no-run30.895s),
Analysis3/3 PASS (0.016s; no-run13.793s), Core protected9/9 and native
independent/missing-package/mapping A/B/A/producer A/B/A4/4 PASS
(5.922/8.827/8.976/8.826s; Core no-run8.331s). Mixed native cold/warm cycle
insertion/removal PASS0.164s (fixture no-run8.836s). Shared genuine reciprocal
roots PASS5.722s (final no-run8.733s), with equal actual live context, distinct
proxies/effect trackers, both delegated roots independently started before
dependency discovery, strict typed cycle terminals and unchanged accepted
events/no generated roots. Total45 focused gates pass across source-valid
receipts; no ignored/zero-test evidence. Executable selection/SHA256, commands,
exit/pass counts and logs are target/wp885/{loading-bounded,analysis-final,
core-restoration,core-archive-diagnostic,core-shared-progress}*. Earlier Core
protected/path source is unchanged by later fixture-only corrections.

Four direct crate check PASS24.907s. Default CLI preparation hit separate60s
caps before dependencies were ready; prepared default CLI PASS24.374s without
feature/profile changes. Actual root on f4ae7b50b terminates naturally exit2
in22.381s including cleanup, advancing beyond generated host-tools Label into
crate_universe's unsupported scratch_path.get_child at extensions.bzl578.
Peak owned group RSS+swap1143025664bytes (1090.074MiB), zero sampled swap,
unchanged tracked inputs/CLI, socket shutdown with group gone/no remaining Slug.
No4GiB/180s cap reached. Receipt /tmp/slug-wp885-20261001/receipts/frontier.json
and inputs-before/after. No unchanged root replay needed.

Retained failures are fixture corrections, not waived gates: unsupported
registry local_path became the admitted deterministic archive/SRI/file URL
route, repacking changes source identity and A restoration restores integrity.
Shared preparation stalled before roots while retained current-thread runtime
was idle; scoped RAII pump drives it during independent external requests.
One native attempt tracker cannot install three times, so requests now own
distinct normal effect trackers. Strict bounded outcome diagnostics then
identified Host ext.bzl Lstat Need: failed terminals prune preparation and
an accepted warm command did not preserve that closure. The final fixture
uses the actual NativeDemandAbortGuard Need/retry/progress protocol before
the pump, fresh per-round trackers, transaction drop/idle drain and path-only
needs; no direct input fabrication or Need-as-terminal acceptance. Source
preparation has a cumulative5s async bound/16 rounds, external commit/cleanup
a separate5s bound, and both genuine roots retain their2s barrier/completion
bound; overall focused operation cap remains20s. All runtime evidence runs
serially; fixture-only corrections do not change production ownership.

Plan/archive/diff checks PASS. WP880 memory repair remains accepted: advisory
metadata replaces whole-file payload retention; old root daemon peak308796KiB
and sampled swap zero. Current1090.074MiB run reaches a deeper frontier.
Full bootstrap/deeper graph memory remain unproved. Preparation/test/review
time is attributed above; elapsed source packet work spans retained turns
(first scratch receipt18:56:48UTC), not a claimed workflow speedup.

FINAL review confirms full structural identity, pending-parent/attempt lifecycle,
pure-family projections, poison invalidation, source-owned preparation and
native no-publication. Production/test growth exceeds the original review
trigger; the independent reviewer accepts cohesive graph/proxy modules and
distinct public-DICE cancellation, historical-checking, native A/B/A/archive/
independent-request discriminators. Elapsed receipt span110min13s from first
scratch creation to final review20:47:01UTC includes retained turns and
corrections; no speedup claim. Coupled activation is accepted atomically.
Next ready work is a bounded source/ownership audit of scratch_path.get_child
and its actual crate_universe consumer closure; no adjacent scratch I/O is
admitted by this packet.
