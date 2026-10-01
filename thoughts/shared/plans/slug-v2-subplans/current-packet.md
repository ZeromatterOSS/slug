# Current Slug V2 Work Packet

Packet: WP-7-184-m7a-dice-live-context-identity-r1
Status: accepted live-context prerequisite; atomic owner-cycle integration next; M7A remains partial

## Outcome and authority

At clean baselineec7e75983, expose existing DICE live-cache identity through a
small read-only DiceTransaction API. WP883's accepted cycle contract requires
it: VersionNumber alone aliases an idle version reopened while old cancellation
workers retain userdata. Public opaque DiceLiveContextId plus live_context_id()
must identify existing engine/version/VersionEpoch, with equal tokens for the
same live cache and different tokens for separate caches/engines. This is the
necessary ownership prerequisite; it admits no generated path behavior.

## Ownership and representation

Source of truth is BaseComputeCtx's existing SharedLiveTransactionCtx version/
epoch and CoreStateHandle's existing process-unique engine ID. Token fields stay
private; Copy/Clone/Dupe/Eq/Hash/Debug/Allocative, no serialization, mutation,
allocation, task, key or cache. Opaque process-local plumbing identity only:
never semantic equality, content identity, persisted namespace or DiceEquality.
Reading/cloning a token cannot keep a transaction/cache alive. Add forwarding
only; preserve all existing version/equality, scheduling, cancellation and
transaction allocation. Hash derive for private VersionEpoch is representational
only. Follow docs/developers/dice.md and utility reuse skill; existing POD/Dupe/
Allocative are leaf reuse, no donor graph/runtime import.

Compatibility is Slug-native runtime plumbing. Existing exact source/package
contracts and structural key/value equality remain unchanged. Generated source
admission, mixed-cycle notification, proxy binding and bootstrap deferred to
WP883's atomic integration successor. Pinned Bazel9.2 mixed-cycle sources are
in ec7e75983's manifest; this API's authoritative ownership is current DICE
VersionTracker.at/drop_at_version and transaction commit/BaseComputeCtx, not a
Bazel parity API. No new oracle or fixture growth.

## Scope and complexity

Allowlist dice/dice/src/api.rs, api/live_context.rs, api/live_context/tests.rs,
api/live_context/cancellation_tests.rs, api/transaction.rs, transaction.rs,
impls/ctx.rs, impls/core/state.rs, impls/core/versions.rs (Hash derive only),
lib.rs. Root owns scheduling docs, docs/developers/dice.md and one compact Stage9
reuse disposition at acceptance. New cohesive token/test module keeps the change
out of growing transaction/ctx responsibilities; existing large files receive
only small getters/forwarders. Expected production<90/test<180 lines is a review
trigger, not a user limit. No engine ID/epoch allocation change, public setters,
DICE engine fork, weak graph registry, proxy activation or unrelated cleanup.

## Discriminators and validation

Independent DESIGN checkpoint before code and FINAL review actual diff/evidence.
Two disjoint workers: public adapter/basic lifecycle tests; gated cancellation
regression. Root serializes Cargo, resolves pinned nightly tools, compiles dice
--lib --no-run JSON under60s preparation cap. Preflight all exact ordinary
selectors once for unchanged executable, run each20s cap. Gates prove clone and
simultaneous transaction equality/hash sharing; changed-version and foreign-
engine inequality; final-drop same-version revival while old critical-section
computation/userdata remain live, then release/cancellation retirement. Use
public commits/key compute/critical_section/semaphores and actual abort/drop;
no private cache injection, sleep-driven races or mirrored field assertions.
Reuse existing impls::transaction::tests::transaction_versions and
impls::core::internals::tests::state_tracks_pending_cancellation if exact names
preflight, rather than broad DICE suite. Short timeout guards ensure fixtures
clean up on failure. Test token liveness explicitly via reopening after retaining
old copied token. Source/effect graph and warm mixed-cycle claims remain unproved.

Direct dependents: bounded cargo check -p slug_loading_v2 -p slug_analysis_v2
-p slug_core_v2 -p slug_cli_v2 --message-format=json, preparation60s cap. No
Slug/Bazel/native daemon runtime in this packet and no binary invocation; no
actual-root retry, extraction/download or RC access. Compile caps preserve
unfinished preparation evidence; never raise a user limit. Format changed
Rust, archive-status/plan-status/diff checks. At acceptance record source/test/
review time, focused gate evidence and residual atomic integration prerequisite,
commit accepted delta. REPLAN only if existing public context cannot expose
identity without scheduler/lifetime mutation; adapt routine getter/test failures
within the reviewed contract.

## Immediate predecessor

WP883 accepted at ec7e75983 establishes19 structural cycle-capable key types,
mixed .bzl receivers distinct from old pure projections, poison invalidation,
attempt retirement and exact-context sharing. Version-only weak registry and
preflight/stack-only cycle guards are rejected. Postcommit binding is source-
proved safe but equality-only existing_state must stay unbound. New token must
precede the deferred proxy/graph/path admission; full bootstrap remains open.

## Accepted evidence and size review

Adapter source adds73 lines, with no allocation/lifetime/equality mutation;
private epoch trait delta is Hash only. Test growth232 lines exceeds the180-line
estimate:147 lines form the synchronized cancellation/drop fixture and85 lines
cover three basic public lifecycle discriminators. Two cohesive small files and
explicit panic/timeout cleanup justify this review trigger; independent source
review found no blocker. Dice --no-run5.474s and all6 exact preflighted gates pass
(.002s sharing; .001s each version/engine, copied-token liveness, old cancellation
reopening, protected transaction versions and pending cancellation). No broad
suite or runtime-root claim. Direct Loading/Analysis/Core/CLI check passes35.317s. Independent DESIGN and
complete FINAL ACCEPT. Plan/archive/diff/scoped format checks pass; evidence
target/wp884/acceptance.json. No source/test/runtime failure in this packet.
Compile total40.791s, focused runtime .007s; review time not separately measured.
Next execute ec7e75983's complete typed graph/proxy/path contract atomically;
review concrete lifecycle/notification adaptation before activation, preserving
legacy concurrent requests, exact live-context binding and no partial effects.
