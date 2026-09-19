# Current Slug V2 Work Packet

Packet: WP-7-45-m7a-requested-command-terminals-r2
Status: accepted

## Outcome and basis

Preserve typed build-error terminals and selected diagnostic events through requested
execution's existing native acceptance owner, without starting transport or publishing
outputs on build errors. This is a prerequisite discovered while auditing CLI/server
activation: both adapters currently project accepted Result<BuildCommandEvaluation,
BuildCommandError>, whereas action-chain staging stringifies build errors and aborts.
Activating that API directly would lose error categories and diagnostic events.
WP744 (65fc9bf7c) accepted selected-subset publication with 21 focused checks including
real REAPI proof; preserve its successful execution/publication and zero-action behavior.

Bazel 9.2 pin 8220c6198837d5c13d53fea211cf3282aa12408a,
CompletionFunction.java:160-164,364-375 owns requested Artifact dependencies. Reuse
WP742-744 producer/selection/content proof and existing native build error/event
ownership in runtime/dice.rs (ObservedBuildCommandRoot and NativeCommandRoot hooks),
runtime/events.rs and docs/developers/dice.md. Named error categories, source identity
and selected evaluation diagnostics stay exact to their accepted owners. Request
acceptance, structural configuration and event reconciliation remain Slug-native.
This packet does not activate CLI/server execution or broaden action families.

## Decision and ownership

PreparedActionChainInputs already retains Arc<Result<BuildCommandEvaluation,
BuildCommandError>>. Reuse it as the sole build outcome owner; add no secondary error
field, side cache, graph rebuild, error reconstruction or filesystem discovery.
For Requested selection only, retain a failed observed build evaluation after
checked_build_frontier and successful SourceCertificate::from_epoch. It carries the
original typed error, the complete observed build frontier/certificate and no action
sources. Do not turn Need, outer observed-root errors, invalid/empty certificates,
plan/producer/source-input errors or transport failures into accepted build errors.
Selected-action staging keeps its existing failure behavior.

Add public PreparedActionChainInputs::evaluation() returning
Result<&BuildCommandEvaluation, &BuildCommandError>. plan() must propagate the retained
error as its existing Arc<str> interface instead of unwrapping. Document that requested
preparation may accept failed evaluation metadata and callers must inspect evaluation().
Successful metadata/count projection borrows this same accepted evaluation.

Staging Key validity accepts completed retained evaluations, including typed failures,
matching the observed-build owner so configured-conflict diagnostic closures remain
readable. Equality requires successful evaluations on both sides; failed evaluations
never cut off by equality. Dependency transience still propagates from DICE. Existing
observation/Need dependencies and allocation ownership remain unchanged. No DICE lock or key-identity change. For retained
requested analysis errors, reuse recursive BuildCommandError::is_analysis_error to allow
unavailable terminal roots and TransientTerminalLocal demand association, exactly as the
observed-build native owner does. Forward BOTH hooks from ActionChainExecutionRoot to
staging. Keep SourceCertifiedCurrentClosure reconciliation and full selected-dependency
association; native finalization revalidates the retained complete frontier.

Execution complete checks the retained evaluation error before requesting a plan or
calling any transport/publication callback. Both requested execute APIs now return
Result<AcceptedCommand<Result<RequestedActionResult<T>, BuildCommandError>>,
BuildCommandError>. Clone the original typed build error into the accepted terminal
only after native acceptance; successful results keep the current result/output and
producer-associated publication fields. Outer failure remains distinct. No compatibility
shim: update all existing internal tests/callers to inspect the explicit result.
Selected action APIs remain nonempty and retain existing error semantics.

All new memory is borrowed metadata or the existing immutable Arc/error clone at terminal
projection. Preserve Stage9 Arc/Allocative conventions; no donor/dependency/interner or
retained hot-path representation change. The existing oversized runtime/dice.rs changes only to forward the completion
callback through its test audit tracker; no native-driver redesign.

## Scope and proof

Production: runtime/action_chain_staging.rs and action_chain_execution.rs; optional
minimal dice.rs test-tracker forwarding only. Core tests: existing action_chain_staging/tests.rs,
action_chain_execution/tests.rs, requested_tests.rs, requested_publication_tests.rs,
and one new requested_error_tests.rs child. REAPI: minimal typed-result adaptation in
action_chain/requested_tests.rs and output_staging/requested_publication_tests.rs only.
Additional prerequisite scope: dice/dice/src/api/activation_tracker.rs,
dice/dice/src/impls/ctx.rs and impls/tests/activation_tracker.rs;
runtime/events.rs, runtime/demands.rs and a focused events/transient_root_tests.rs
child. Docs: current/canonical, Stage7 and bootstrap-readiness. No CLI/server edits this packet.
Expected production growth 40-100 lines and proof/adaptation 150-300 lines, review
triggers rather than caps. Worker1 owns production; worker2 owns Core tests; root owns
REAPI adaptations, source/contract review and coordinated validation.

Prove a real semantic error→repair→error sequence in one retained runtime, exact typed
error category/message and selected diagnostics through project/publish, zero transport
and publication callbacks on error, native accepted snapshot consistency, and successful
recovery. Cover a non-analysis build error and an analysis failure requiring unavailable
terminal-root/demand handling. Compare appropriate baseline build-command evidence or
explicit expected diagnostics; no weakened event assertions. Requested preparation must
expose the typed error and plan() must fail without panic. Protect Selected-mode errors,
requested success/source-only completion and requested publication/retry.

Compile default Core/REAPI library tests --no-run with pinned nightly-2025-09-14 and
Cargo JSON; compile CLI direct dependent for public API change. Preparations separate,
cap60s. Preflight exact focused selectors. Run native fixture suites serially. Prefer
runtime below a few seconds; >30s needs strict necessity, 12s is guidance. Reuse accepted
WP744 wire/CAS proof because transport/publication algorithms do not change. Format,
diff/plan/archive checks and independent design/final ACCEPT before commit/main
fast-forward and authorized push. Receipts target/wp745. Record elapsed/compile/test time.
REPLAN only for a contradicted native error/certificate owner or a newly required
semantic boundary; routine invocation or implementation corrections stay in packet.

## Reviewed prerequisite correction

R1 exposed two distinct failures in target/wp745/core-corrected-r2.receipt (two tests,
1.196s). Blanket invalid staging wrappers hide configured-conflict diagnostics; the
validity correction above retains their tracked closure. A separate ordinary-only
error→repair→error diagnostic (ordinary-diagnostic-r1.receipt, 1.034s) proves analysis
iteration2 already fails in the ordinary native owner: transient recomputation leaves
its prior persistent root Dirty. Conflict succeeds all three ordinary phases; unchanged
selected evaluation events correctly do not replay after the initial phase.

R2 resolves this prerequisite within the same typed-error acceptance outcome. DICE owns
current computed-value validity, including dependency transience. Extend its activation
tracker with a default root-completion callback using the same reserved root node,
version and ordinal, emitted only after successful compute/projection completion.
Preserve existing root-start ordering and cancellation behavior; cached/transient values
must report their actual validity. No persistent graph transition is fabricated. Callback state uses the existing erased
rich-activation future branch only for tracked parentless requests; ordinary nested
computes keep the unboxed path and original 88-byte compile-time size guard. The first
capture-based candidate failed that guard at 144 bytes and is rejected. Extra boxing is
request scratch released on completion/cancellation, not per dependency computation.
Independent representation review ACCEPT; no runtime speedup claim.
The command event owner records this attestation on the matching current-attempt root.
Only the existing analysis-error unavailable-root policy may omit that exact completed
transient root after activation_closure rejects it as Dirty at the sealed version.
Always query the original roots first, preserving foreign-engine and live-version
validation. Retry only after removing a matching current root; a dirty descendant or
NotVerified error still rejects. Existing unavailable-root handling remains. Never blanket-ignore
Dirty, skip dirty descendants, accept stale-version completions or infer transience
from an error string. Preserve version/root/ordinal validation and source-certified
terminal-local demand association. No extra dependency graph, cache, replay owner,
filesystem read or locking across a DICE computation.

DICE prior-art basis: local impls/tests/transients.rs and activation_tracker.rs and
MaybeValidDiceValue dependency validity propagation. These are Slug-native provenance
semantics, not a new Bazel parity claim. New memory is a completion flag per command
root, a sealed transient-root ID vector and callback scratch, released with its attempt; no retained semantic value grows.
The existing events owner remains cohesive: the change extends its root selection,
with focused new tests in a child rather than expanding its large colocated test block.
Expected additional production growth 60-130 lines plus focused proof; estimates only.

Prove valid→transient→valid completion metadata, repeated/transient cached requests,
root ordering and projection behavior, and cancellation with no completion. Event tests
must accept an attested transient after prior success only under the allowed policy,
retain strict rejection, and protect dirty/uncomputed roots, dirty descendants, foreign
transactions and version mismatches, including when every root is transient. Run the retained ordinary/requested error→repair→error proof and
exact conflict event suppression. Compile DICE tests and Core/REAPI, check CLI dependent,
then run focused selectors serially. Reuse unaffected earlier proof; no broader suite.
Independent prerequisite design ACCEPT: preserve root-start identity/ordering; use
rejection-driven retry so original engine/liveness checks execute before any omission.
Final review requires the named transient, cancellation, projection and wrong-transaction
negatives plus ordinary/requested warm recovery.

## Activation successor

WP746 migrates Execute-mode Build before each adapter's old build acceptance, using one
requested execute/publish operation and pure accepted-result rendering. Preserve analysis-
only/CacheOnly behavior and CLI missing-target rejection. Loading-only native filegroups
remain unsupported selection; authored empty/source roots succeed without connection.
Project materialized digest evidence from accepted selected publication groups, never all
remote cooutputs, and aggregate execution counters from the complete accepted result table.

Server BuildRequest parsing currently filter_maps malformed target strings; fail explicitly.
Its IPC drops cache, instance_name, headers, timeout and retry policy; carry supported
immutable options or reject unsupported options before dispatch, never silently discard.
Keep header values out of diagnostics. Existing chain policy rejects custom headers,
timeouts/retries and separate CAS endpoints. Do not broaden that policy in activation.
Run still calls the old server run_reapi_build helper via run_reapi_executable; preserve
it as Run-only with existing admission rather than deleting it with ordinary Build loops.
Require real one-shot and retained-daemon requested File/Tree subset proof, cold/warm/
source A/B/A, diagnostics and failure controls. M7A partial; M8 remains unproved.

Independent design review ACCEPT: preserve typed/frontier/native owners, both error
policy hooks and explicit result projection; final error/recovery/effect proof required.

## Acceptance receipt

Baseline 65fc9bf7c, pinned nightly-2025-09-14, default host library tests. Independent
final review ACCEPT. Final exact-selector checks: Core 15/15 (4.287s), DICE 5/5
(0.009s), REAPI source-only 1/1 (0.288s), all serial where native fixtures overlap.
Core includes both retained ordinary and requested analysis/conflict error→repair→error
with exact typed errors/events, failed-operation zero effects, current source frontier,
Selected failure behavior, successful publication/retry, and transient-root negatives.
DICE proves completion validity/identity, cached projections, cancellation and version
ordering; its original 88-byte compute-future assertion remains unchanged.

Final combined DICE/Core/REAPI --no-run preparation passed in 26.323s; CLI dependent
check (including server) passed in 20.174s. Formatting, diff, plan and archive checks pass.
Receipts: target/wp745/{core-final-r3,dice-focused-r1,reapi-source-control-r2,
dice-core-reapi-no-run-r7,cli-check-r2,structural}.receipt. Recorded preparation time
across attempts: 158.492s; runtime including diagnostic/failing attempts: 11.253s;
accepted 21-check runtime: 4.584s. No >30s runtime test was required. WP744 wire/CAS
proof is reused; no transport/publication algorithm changed. The prerequisite advances
typed native acceptance only; CLI/server activation is WP746, M7A partial, M8 unproved.
