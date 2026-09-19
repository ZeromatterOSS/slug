# Current Slug V2 Work Packet

Packet: WP-7-44-m7a-requested-publication-r1
Status: accepted

## Outcome and basis

Publish precisely the selected Derived artifacts of an ordinary requested forest,
across producers and configurations, after one complete native source validation.
Retain source-only/empty completion, requested/group/artifact bindings and complete
per-action result verification. WP743 (244c6641d) accepted native requested execution
with 22 focused checks; reuse its frontier, retry and zero-callback evidence.

Bazel 9.2 pin 8220c6198837d5c13d53fea211cf3282aa12408a,
skyframe/CompletionFunction.java:160-164,364-375 selects Artifact dependencies via
TopLevelArtifactHelper; reuse WP742 exact declared artifact/producer identity proof.
WP738 (0ec1eb5e7 current-packet manifest) anchors RemoteExecutionService.java:
1350-1384 (finish transfers before moves),915-922 and OutputPermissions.READONLY
(0555), FileSystemUtils.java:454-489 (no group-atomic move guarantee), and
AbstractActionInputPrefetcher.java:365-397,681-728 (tree/file modes). It owns output bytes,
verified CAS transfer, confined hidden siblings, per-artifact atomic replacement,
cleanup and native publication/error classification. Exact admitted artifact/content
and complete result schemas stay exact. Configured path spelling, preserving empty
tree directories, sequential execution and per-artifact namespace commit are
Slug-native. No Bazel path-byte, whole-generation atomicity or performance claim.

## Ownership and interface

Keep discovery/selection/producer bindings in the existing DICE evaluation and
PreparedActionPlan. No new DICE key, retained forest, action cache or semantic side
store. Projection into publication groups is attempt scratch. Derive requested
Derived output groups from selection.artifacts() zipped with artifact_producers(),
never from last action or path inference. Group by producer in first-selected order,
deduplicate identical ActionOutput destinations within each producer (equivalent
scalar FileWrite owners can share one output), preserve requested metadata intact.
Exact declared owner/output lookup and same-configuration representative resolution
remain in the existing planner. Verify subset declarations and structural configuration
agreement before creating capabilities. Sources are observed but never copied.

Add public PlannedActionOutputStaging with read-only action_index() and staging()
accessors, private construction/publication. Change existing ActionChainOutputTransport
stage_outputs to receive one slice of these capabilities for the complete operation.
Selected-action mode supplies one full-output stage for its final action. Requested
mode supplies all selected producer subsets. No compatibility shim or second transport.
ConfiguredOutputOwner remains the sole configuration registration/destination owner;
its new plan method reuses ActionOutputStaging and confined platform implementation.

Reserve the union of selected destination path components across the complete batch
when choosing private sibling names; preserve WP738 collision exclusion across groups.
Create all groups before transfer. REAPI validates session completion, producer indexes,
full producer declaration/result schemas and every selected subset before first transfer.
Keep executor::validate_result_shape/fetch_outputs and output_tree verification unchanged:
every executed action's cooutputs, even unselected ones, are verified before recording
its result. Reconcile the full action.outputs() against that result before projecting
selected paths/kinds. Then reuse stage_output/read_blob_verified for selected bytes;
late missing/corrupt CAS rejects the entire private batch, without local fallback.

Core seals every group after the single transport callback and retains the complete
batch through native finalization. Under the existing revision owner, after full-frontier
validation, preflight EVERY stage before publishing the first. PlannedActionOutputStaging::publish_all owns this
preflight-all/publish-all loop for the native callback. Reuse stage publish for
per-artifact renames; do not drain/drop stages under the revision lock. Earlier artifacts
may change if a later rename fails; preserve existing possible-output-change and abort
restoration diagnostics and return no AcceptedCommand. Cleanup remains after release,
following retired identities; preflight/transfer/seal failures keep old artifacts intact.
No downloads, DICE compute, Starlark or async task join inside publication lock.

Add execute_and_publish_requested_actions_with_repository_environment through the same
private driver/publication policy. RequestedActionResult exposes published_outputs()
as a slice of PublishedPlannedActionOutputs, each with action_index() and outputs()
(PublishedActionOutputs) to retain producer association without path reconstruction.
Execution-only and zero-action results have an empty slice; zero actions still invoke
no transport callback or filesystem staging and pass native final validation. Existing
ActionChainResult retains its optional singular PublishedActionOutputs projection.
No CLI/server activation; existing ordinary-request guards remain until migration.

All new group vectors/maps/handles are attempt-owned; accepted publication metadata
uses Arc slices and paths only, no handles/buffers. Retained terminal clones share the
pending batch via existing Arc/Mutex; no lock across DICE compute. Reuse Stage9 compact
SmallMap/SmallSet and Arc conventions, no donor import or dependency changes.

## Files and validation

Root owns runtime/action_output_staging.rs, its linux.rs and new plan.rs/plan_tests.rs (existing tests.rs only shared fixture visibility),
configured_output.rs and runtime/mod.rs exports plus scheduling/owner docs. Core worker
owns action_chain_execution.rs, existing tests minimal trait adaptation, requested_tests
only if needed and new requested_publication_tests.rs child. REAPI worker owns
action_chain/output_staging.rs and existing tests plus a new requested_publication_tests
child; minimal visibility in action_chain/requested_tests.rs/tests.rs to reuse fixtures.
Allow minimal dice.rs export wiring if required. No filesystem redesign or native driver
semantic change. Existing large dice.rs gets exports only. Expected production growth
200-350 lines and focused proof 300-600 lines are review triggers, not caps.

Discriminators: selected strict subsets including File/Tree from different producers;
selected cooutput not last action; duplicate aliases/shared FileWrites publish once;
configuration A/B destinations and requested producer associations; unselected outputs
stay absent; empty/source-only does no transport; late transfer failure/unwind and
source mutation retry expose no partial acceptance; global destination preflight rejects
a later changed group before earlier publication. Preserve existing partial rename and
post-publication bookkeeping diagnostics. REAPI portable full-result projection negatives
include missing/extra/wrong-kind UNSELECTED cooutputs and invalid ordinal/subset. A focused
real REAPI multi-producer publication check is required because transfer routing changes;
reuse existing backend fixture, do not create a new harness. Reuse unchanged low-level
confinement, modes, tree replacement, missing/corrupt CAS and native lifecycle proofs.

Compile pinned nightly-2025-09-14 Core/REAPI tests --no-run with Cargo JSON; compile CLI
direct dependent. Preparation separate, cap60s. Preflight exact focused selectors; run
native freshness suites serially when observed ancestors overlap. Prefer tests below a
few seconds; >30s requires strict necessity, 12s is guidance. Format changed Rust, diff,
plan/archive checks and independent design/final ACCEPT before commit/main fast-forward
and authorized push. Receipts target/wp744. Packet elapsed and compile/test times recorded
at acceptance. M7A partial, M8 unproved. REPLAN only if exact destination ownership,
existing native publication lifecycle or required selected families cannot support this
contract; routine compiler/invocation/test corrections stay in packet.

Independent design review ACCEPT, including batch-wide path-component reservation.

## Validation receipt

Base 244c6641d; candidate review/wp744-requested-publication; Linux GNU,
nightly-2025-09-14. The session's verified direct pinned Cargo/rustc/rustdoc tools
were reused. Core/REAPI default-feature library no-run preparation passed in
25.595s. Root then restricted the new native publication test child to Linux GNU,
matching the existing publisher's platform contract; final no-run preparation
passed in 3.549s. No production correction or assertion weakening was required.
Cargo JSON identifies Core a0b7415282e22264 and REAPI 83de72d5efb378db.

All 21 focused checks pass. Exact ordinary selectors were preflighted per executable;
ignored wire selectors were individually listed and executed under supervision.
core-focused-r1: 16/16 in 4.574s (two new batch/configuration/preflight tests,
seven new requested publication lifecycle/binding tests, seven protected selected
publication/confinement/rename controls). reapi-focused-r1: 3/3 in 0.713s (two new
full-schema/subset projection tests and protected schema validation). Native freshness
suites ran serially; no fixture interference or runtime retries were needed.

A fresh local verifying NativeLink ran the new selected-subset File/Tree test across
two producer ordinals: 1/1 in 0.977s. It proves complete remote cooutput metadata,
no retained result blobs, duplicate/source request bindings, only selected local
outputs, empty tree directories and 0555 modes despite false producer mode bits.
The protected selected-action replacement/modes wire control passed 1/1 in 1.565s.
Both backend processes reached terminal state and their temporary roots were removed.
No new backend harness or retained oracle fixture was introduced. Existing missing/
corrupt CAS, source certificate, no-follow cleanup and revision-owner proofs remain
reused where unchanged; this is not a broad remote-execution or platform claim.

CLI direct-dependent check (including server/Core/REAPI) passed in 11.368s.
Changed-Rust formatting, diff, plan-status and archive checks pass (structural.receipt).
Receipts are under target/wp744. Candidate branch elapsed about 16 minutes through
validation, including design/implementation review; preliminary feasibility research
was not separately timed. No workflow speedup claim. Net production growth is about
273 lines; focused proof growth about 764 lines exceeds the initial estimate because
it covers real configuration forwarding, equivalent owners, lifecycle failures and
wire routing while reusing existing fixtures. These tests cover distinct invariants.

Independent final review ACCEPT confirms producer/configuration ownership, complete
result verification, native publication ordering and all 21 focused checks. Linux GNU
library publication is accepted; a later rename can still leave partial filesystem
changes without acceptance. Next: replace the CLI/server legacy all-action
helpers with requested execution/publication, then prove ordinary command behavior.
M7A remains partial, M8 unproved; runfiles/symlink and other deferred families remain
explicit boundaries.
