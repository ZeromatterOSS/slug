# Current Slug V2 Work Packet

Packet: WP-7-43-m7a-requested-execution-r1
Status: accepted

## Outcome and basis

Stage all selected and reachable sources, then execute the complete requested-artifact
forest in one native request and one fresh transport session per attempt. Preserve
requested/group/artifact producer bindings and expose results only after final source
validation. Zero-action requested forests finish natively without calling transport.
WP742 (5ca6f056d) accepted the shared exact producer planner with twelve focused checks;
its full receipt is in that commit. WP741 owns ordinary artifact selection.

Reuse Stage7's WP737 source-certified chain execution, WP738 opt-in single-action
publication, and WP742's pinned Bazel 9.2 artifact/generating-action dependencies.
Pin 8220c6198837d5c13d53fea211cf3282aa12408a: skyframe/CompletionFunction.java:
160-164,364-375 selects Artifact keys; actions/Artifact.java:161-176 and
ActionLookupData.java:73-106 retain generating owner/ordinal identity;
skyframe/ActionExecutionFunction.java:294-315,403-445 waits for declared input keys;
skyframe/ArtifactFunction.java:135-138,275-294 observes source content separately.
Existing REAPI generated File/Directory/CAS verification and wire-profile proof is
reused unchanged. Exact admitted semantic identities/content/bindings stay exact;
structural configuration tokens, sequential traversal and native request lifecycle are
Slug-native. No parallel action scheduling, new action family or Bazel byte-identity claim.

## Owner and invariants

Generalize ActionChainStagingKey with an explicit retained/hashable/Allocative selection
enum: Selected { owner, action } versus Requested. The build request and selection fully
identify the key. Reuse PreparedActionChainInputs and its existing evaluation/source/
observation/certificate fields; replace its owner/ordinal fields with that selection.
Its borrowed plan() returns PreparedActionPlan::{Selected, Requested}; actions() serves
both, requested() exposes the RequestedActionPrerequisitePlan only for Requested, and
selected_action() returns an action only for Selected. This view is command scratch.
No forest plan, remote result, session, file handle or backend state becomes DICE-retained.

Validate the entire plan before observing source content. Requested staging structurally
unions all Source artifacts from selection.artifacts() with Source inputs of planned
actions, using existing SourceArtifactInputObservationKey and exact observed epoch union.
A source returned only by a rule's output group must be certified even if no action uses
it. Shared sources are observed once; every Need/error retains the existing key/native
channel. Source-only and empty analyzed roots keep their requested identity and full
build/source frontier. Permit empty target lists only through the private source-staging
observed-root constructor and existing observed multi-root driver; do not change ordinary
wildcard dispatch/admission. The observed root's MODULE anchor supplies a real frontier
for empty requests. No direct filesystem discovery, inferred source paths or cached bytes.

Add prepare_requested_action_inputs_with_repository_environment and
execute_requested_actions_with_repository_environment using the same preparation and
native completion driver as the selected-action operation. Core must not call a chain
operation independently per artifact/root. Before every Execute, validate the entire
forest frontier, including selected standalone sources. A final source/build mutation
retries the whole request with a fresh session; failures/unwind discard provisional
results and restore prior accepted state through the existing native abort guard.
Source-only/empty plans bypass start/stage/execute/finish and still pass native final
validation. No fabricated backend result or remote connection on that path.

Expose RequestedActionResult<T> with inputs() and output()->Option<&T>; None explicitly
means zero action steps. Existing ActionChainResult<T> stays nonempty. Requested bindings
come from inputs.plan().requested(), aligned to the ordered transport result table.
Generalize the existing REAPI transport via PreparedActionPlan.actions(), preserving
whole-plan preflight before connection, exact generated producer bindings, full action
result validation, same-session/ordinal guards and CAS-only generated provenance.
ActionChainRemoteResult::selected() becomes Option and is Some only for Selected mode;
it must not label the last forest action as the requested result. Existing single-action
call sites assert Some. Results() remains the full ordered table.

Publication remains separately gated: existing single-action publication must obtain
selected_action(), and REAPI output staging explicitly rejects requested mode rather
than using last(). Requested execution has no publication API. Its results remain CAS/
operation-owned. CLI/server legacy guards remain until selected artifacts across owners
can be privately staged and published after one full-frontier validation. No action/output
subset may be silently omitted to gain successful execution.

## Scope, ownership and evidence

Root: runtime/action_chain_staging.rs, minimal dice.rs observed-empty routing/exports,
runtime/mod.rs exports, new staging child tests and scheduling/owner docs.
Execution worker: runtime/action_chain_execution.rs plus its new requested_tests child;
existing execution tests only for shared fixture/helper wiring and necessary API updates.
REAPI worker: slug_reapi_v2/src/action_chain.rs, output_staging.rs and their child tests,
including selected() caller updates. Independent reviewer owns design and final gates.
No concurrent edits to another owner's files. Source_staging.rs may supply an existing
helper only if needed; no source-input/closure/equivalence or publication-owner redesign.
Allow current/canonical, Stage7 and bootstrap-readiness documentation updates, plus
one reusable validation-routing lesson in the orchestration routing log (rolling its
oldest row unchanged into the existing monthly archive to preserve the 20-row limit).

Retained selection enum reuses existing ConfiguredTargetKey/Arc/Allocative under Stage9;
source sets reuse SmallSet and Arc observation values. No donor import, cache/interner,
retained artifact flattening, lock across DICE compute or scheduler framework. Existing
oversized dice.rs gets only bounded routing/exports; staging/execution remain cohesive
children. Estimated production growth 200-350 lines plus local refactoring, proof growth
400-700 lines; review triggers, not caps. New selected mode is actual request identity,
not a compatibility fallback. No compatibility shim or independent alternate scheduler.

Discriminators: requested terminals share a diamond producer once; cooutputs and root/
group bindings remain intact; unrelated actions do not execute; standalone selected
source absent from all action inputs is certified and its mutation retries whole forest;
source-only/rule-returned-source/empty/no-target plans invoke no transport callback;
late failure and unwind expose no accepted partial result; warm request/group changes
restore bindings and results; unsupported later action fails before effects/connection;
REAPI preflight/bind uses all forest steps and results have no singular selected action.
Reuse protected single-action execution/retry/publication and REAPI generated binding,
CAS corruption/eviction proof where their semantic inputs are unchanged. A new backend
smoke is required only if adapter semantics change beyond mode/result projection; any
unproved broader wire/backend scope must be stated explicitly.

Compile Core and REAPI tests --no-run under pinned nightly-2025-09-14, separately bounded
60s preparations. Preflight exact selectors per executable; run focused new staging/
execution/REAPI checks and affected existing single-action lifecycle/publication controls.
Prefer checks below a few seconds; >30s requires strict necessity, 12s is guidance.
No concurrent shared-target Cargo. Format changed Rust, diff check, plan/archive checks;
final independent ACCEPT, commit/main fast-forward and authorized push. Receipts target/wp743.
REPLAN only for a contradicted identity/lifecycle owner or newly required semantic boundary.
M7A partial, M8 unproved; selected-artifact publication and CLI activation follow.

Independent design review ACCEPT: explicit staging selection identity, complete source
frontier, shared driver and attempt-local results preserve native authority. Zero-action
callback bypass and explicit requested-mode publication rejection require final proof.

## Validation receipt

Base 5ca6f056d; review/wp743-requested-execution; Linux GNU,
nightly-2025-09-14. Reused the verified direct pinned tools after the session's snap
rustup launcher failure. core-reapi-no-run-r1 compiled default Core and REAPI library
tests together, exit 0 in 29.601s, within the separate 60s preparation cap. Cargo JSON
selected the Core a0b7415282e22264 and REAPI 83de72d5efb378db executables. This rebuild
replaced the older Core executable with that hash; it is no longer baseline evidence.
Exact selectors were preflighted before execution; the four cfg-gated publication
selectors omitted by the initial test-name scan were separately preflighted and run.

Twenty-two distinct focused checks have passing evidence: two new staging checks,
five new native requested-execution checks, four new portable REAPI/mode checks, eight
protected single-action execution/publication checks and three protected REAPI checks.
core-focused-r1 ran 11, with 9 passes/2 freshness failures in 4.119s; reapi-focused-r1
passed 7/7 in 2.074s. protected-publication-r1 ran 4, with 3 passes/1 failure in 1.776s.
Those groups were mistakenly treated as independent: Core and REAPI fixtures share
observed target/wp737 ancestors. Source path resolution captures parent Lstat metadata,
so sibling fixture creation/removal can trip a freshness gate before an intended failure
injection. The concurrent old-binary probe had the same interference and contributes no
baseline attribution or acceptance evidence.

Only the three affected selectors were rerun serially, with no concurrent test or file
mutation: downstream_staging_checks_producer_sources_and_entire_build_frontier,
final_source_change_retries_whole_chain_with_fresh_session, and
output_transfer_failure_and_unwind_preserve_old_outputs_and_cleanup. All pass unchanged
in serialized-lifecycle-controls, 1.234s. No production correction, assertion weakening,
freshness-policy relaxation or baseline waiver was used. Other passing results remain
valid. Native freshness tests sharing observed ancestors must run serially; disjoint
leaf fixture directories are insufficient isolation.

New evidence proves selected group-only sources are observed and restored A/B/A;
Selected/Requested staging modes stay distinct; shared producers execute once with
complete root/group/cooutput bindings; source mutation before Execute blocks that step,
after Execute retries with a fresh session and drops old output; empty/source-only
forests invoke no callbacks; late errors/unwind restore prior accepted state. REAPI
checks use real preflight/bind/finish code with synthetic verified-result metadata and
an unreachable-endpoint source-only control. No new nonempty multi-root backend smoke
was run: wire upload/execution/CAS verification is unchanged and its accepted evidence
is reused. Both publication layers reject requested mode before output transfer or
last-result lookup. Existing single-action retry/publication controls pass.

Changed Rust formatting, diff, plan-status and archive checks pass (structural.receipt).
Independent final review ACCEPT confirms staging identity, complete frontier validation,
attempt-local execution, zero-action completion and publication separation. Packet wall
time was approximately 21 minutes through final review, including interruptions; compile
29.601s and runtime groups are recorded above. No workflow speedup is claimed.
The user-authorized AGENTS/orchestration update permits two or three independent workers;
skill validation passes. This advances
native requested-forest execution metadata, not selected-artifact publication or CLI
activation. M7A remains partial and M8 unproved. Next: stage selected artifact subsets
across producers/configurations, preserve complete per-action result validation, and
publish under one full-frontier native validation before replacing CLI/server helpers.
