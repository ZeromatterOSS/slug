# Current Slug V2 Work Packet

Packet: WP-7-42-m7a-requested-prerequisites-r1
Status: accepted

## Outcome and basis

Build one shared prerequisite forest from complete ordinary requested artifacts.
WP741 (dd40b0b1d) accepted explicit root metadata, groups, identity and source
certificates with fourteen focused checks and independent review; its receipt is
preserved in that commit. This packet connects that selection to exact producers
without enabling transport, publication or the legacy CLI executors.

Reuse WP736/737's declared producer lookup, iterative prerequisite traversal and
FileWrite/Spawn admission, WP741's pinned Bazel 9.2 TopLevelArtifactHelper and
OutputGroupInfo selection evidence, and Stage7's validated closure contract.
Pinned Bazel is 8220c6198837d5c13d53fea211cf3282aa12408a in /home/wgray/bazel;
source anchors under src/main/java/com/google/devtools/build/lib/:
- skyframe/CompletionFunction.java:160-164,364-375 turns selected output groups into
  artifact dependency keys; actions/Artifact.java:161-176 maps ordinary derived
  artifacts to their retained generating action key (tree artifacts keep an artifact key).
- actions/ActionLookupData.java:25-28,73-106 identifies actions by owner lookup key and
  action ordinal, with explicit shareability; skyframe/ActionExecutionFunction.java:
  294-315,403-445 gathers input/scheduling dependencies before execution.
- actions/Artifact.java:563-571 and skyframe/ArtifactFunction.java:135-138,275-294 keep
  source content observation separate from generated execution; a zero-action source
  binding does not prove source availability or authorize successful execution.
- analysis/StarlarkProviderValidationUtil.java:25-40 and CachingAnalysisEnvironment.java:
  107-149 reject orphan generated artifacts; actions/Artifact.java:249-273 and
  ActionLookupValue.java:25-32 preserve assigned owner/action identity. Slug's planner
  rejects missing producers at its existing validated-closure handoff.
Exact admitted semantics: selected artifacts require their declared generators and
transitive declared inputs, sources have no action producer, shared exact-key producers appear
once, with only the existing closure-admitted scalar FileWrite coalescing. Structural action/configuration identity and deterministic traversal order remain
Slug-native; no exact Bazel execution order or scheduling-performance claim.

## Owner and invariant

BuildCommandEvaluation::requested_action_prerequisites() borrows this evaluation's
ValidatedActionClosure and consumes its existing requested_artifacts() view. Expose
RequestedActionPrerequisitePlan with selection(), actions(), and artifact_producers();
the latter aligns exactly with selection.artifacts(), with None only for Source and
Some(index) for every selected Derived artifact. Retain original per-request/group
membership, duplicates, aliases, empty analyzed targets, and source-only selections.
Selected cooutputs have distinct artifact bindings even when their producer is shared.

Generalize action_prerequisites.rs behind one private planner engine. Index the
validated closure once, resolve full declared artifact owner/configuration/output kind
before canonicalizing admitted equivalent scalar FileWrite representatives, and share
one iterative DFS state table across all selected artifact seeds. Emit prerequisite-first
in first selected-artifact/dependency occurrence order. Preserve exact input identities
and producer indices. Do not concatenate independent per-root plans, infer producers
from paths, select all actions, or reconstruct configured analyses.

The old ActionPrerequisitePlan owner/ordinal API continues through this same engine;
its selected representative remains last and its error semantics stay intact. Existing
closure-wide output-conflict checks remain authoritative even for unselected actions.
A missing producer, cycle, unsupported reachable action family, or unsupported selected
or reachable output kind fails the entire forest. Admit existing scalar FileWrite/Spawn
and File/Directory only; selected RunfilesTree/Symlink must fail explicitly, including
hidden groups. Unreachable nonconflicting actions do not enter the forest. No partial
success, artifact dropping or path-based fallback.

This is pure command scratch borrowing retained configured results. Reuse existing
SmallMap, Vec, Arc and structural artifact values under Stage9's accepted utility
policy; no new donor, cache, interner, DICE key or retained graph representation.
DICE still owns analysis/invalidation and root-set closure validation. The planner adds no host reads,
source certificate changes, request-revision changes or locks. A successful
borrowed forest is metadata, never execution or publication authority.

## Scope and proof

Allowlist: runtime/action_prerequisites.rs and child tests (a new requested_tests.rs
may share existing private test constructors); minimal evaluation delegate in dice.rs
and public exports in runtime/mod.rs; new native forest tests may reuse the existing
source_staging/test_workspace.rs fixture without changing its shared behavior.
Direct-source native association correction also permits the relevant narrow hook
in dice.rs, a test-only source-observation accessor, and existing certified metadata
failure assertions in runtime/build_requested_target/tests.rs. Current/canonical,
Stage7 and bootstrap-readiness may record accepted scope.
Root owns planner, delegate and scheduling; native and synthetic test workers own
disjoint new child tests; reviewer
owns independent design and final review. No worker edits root files concurrently.
Production growth estimate 100-180 lines plus extraction of the existing 160-line
planner; tests 250-450 lines. These are review triggers. dice.rs is already oversized:
add only a delegate and retain all planning in its cohesive module. Existing planner
extraction reduces the constructor's responsibilities, with no adjacent redesign.

Discriminators: multi-root shared diamond with exact input producer indices; duplicate
requests/aliases/generated subsets/group overlap and multiple cooutputs; source-only,
empty and mixed selections; identical path with distinct owner/configuration/kind and
admitted shared FileWrite representatives; missing/wrong owner/kind, cycle, unsupported
selected/reachable kind or family reject the whole forest; unrequested actions stay
absent; one native output-group selection A/B/A proves request invalidation/restoration.
Reuse the five existing single-action planner checks and WP741 selection evidence.
Native tests use public analysis requests and existing hermetic MODULE prerequisites.
No backend, daemon or publication smoke is needed for this metadata-only boundary.

Resolve pinned nightly-2025-09-14 tools, compile Core tests --no-run separately with
60s operation cap, preflight exact selectors once per executable, then run only focused
new and protected planner checks. Tests should take at most a few seconds; any >30s
requires strict necessity. Twelve seconds is guidance, not a user deadline. Compile
named direct dependent slug_reapi_v2 after Core's public API change. Format changed
Rust; git diff --check, v2_plan_status.py and v2_archive_status.sh; independent final
ACCEPT before commit, main fast-forward and authorized push. Receipts target/wp742.

No REPLAN for routine compiler/test corrections. A need for new action equivalence,
closure membership or execution authority is a new design boundary. Follow-on work
must stage standalone selected sources as well as action inputs, execute one native
request forest, and publish only selected artifacts across owners/configurations after
full-frontier validation. The legacy CLI/server guards remain until both all-action
helpers are replaced by that path. Runfiles families remain an explicit prerequisite
for broader activation. M7A partial, M8 unproved.

Independent design review ACCEPT: exact artifact ownership, existing representative
policy, prerequisite order and complete root bindings stay authoritative. No general
cross-owner equivalence or execution activation is introduced.

## Reviewed direct-source prerequisite correction

The new standalone source forest test exposed a WP741 activation gap: authoritative
SourceFile analysis can select repository dependencies, but SingletonRootSingle's
terminal association still required StrictPathOnly for that direct source. Ten of
eleven focused checks passed; //:input failed with RepositoryRequests before planning.
Independent design-delta review ACCEPT extends the existing SelectedDependencySuperset
policy only to certificate-bearing authoritative SourceFile success, SourceCertified
Analysis errors and RootSource observation errors, alongside admitted source aliases. Every reported
observation still matches selected demand/value/exact Arc, and full selected native
validation remains unchanged. LoadedOnly/wildcard and uncertified errors
retain their policies. Singleton terminal-demand association remains ClosureOnly;
no new unavailable-root exception is introduced.

The native zero-action test covers standalone source content A/B/A, delete/recreate,
identity and retained certificate bytes. The existing intentionally inconsistent
virtual byte/Lstat metadata-error test proves only error classification/certificate
retention and that uncertified errors keep StrictPathOnly; it does not claim native
acceptance of that state. Test-only observation access does not widen production APIs.
Synthetic identity/error controls plus native membership/restoration/source-lifecycle
proof exceed the initial test-size estimate; they cover distinct producer and native
admission boundaries with reused constructors and the existing workspace writer.

The first correction passed direct-source A/B/A but the explicit delete phase still
failed RepositoryRequests (source-phase-diagnosis). This disproved the initial
assumption that pre-metadata source errors have only path dependencies: package
loading retains repository dependencies too. Reviewed correction includes certified
RootSource errors under the same superset policy, preserving the original source
error. An otherwise identical error with its certificate removed retains StrictPathOnly.
No validation, terminal-demand or unavailable-root policy is widened.

## Validation receipt

Base dd40b0b1d, candidate review/wp742-requested-prerequisites; Linux GNU,
nightly-2025-09-14, default Core/REAPI features. rustup which failed through the snap
launcher; direct pinned cargo/rustc versions match rust-toolchain. No toolchain change.
Receipts under target/wp742 record commands, selectors, exits, elapsed time and output.

Twelve distinct focused checks have passing evidence: three synthetic forest tests,
three native requested-forest tests, five protected single-action planner tests, and
the certified metadata-error policy regression. Exact selectors were preflighted per
executable. core-focused-r1 ran 11 in 1.236s: ten passed and singleton source admission
failed. Reuse the ten unchanged passing checks. The reviewed correction and extended
source lifecycle/error classification pass 2/2 in source-correction-final, 0.789s.
Earlier correction/trace/phase-diagnosis failures are explained above; no failure is
waived as baseline. No broader suite or execution admission is claimed.

Native evidence retains six root occurrences, four ordinary groups, eight distinct
selected artifacts and exact producer mappings over seven planned versus nine declared
actions; File/Directory cooutputs share their producer and unrelated outputs stay absent.
Group A/B/A changes only selected membership while declared actions remain equal.
Empty/direct-source/source-alias requests keep zero actions; standalone source A/B/A,
delete and recreate preserve bytes/error/identity. Synthetic tests retain full owner,
configuration/preference and kind identity before admitted scalar-write sharing, exercise
later failing branches and reject missing producers, cycles and unsupported kinds/families.

Final Core preparation core-no-run-r4: exit 0, 14.414s. Direct REAPI dependent
preparation reapi-no-run-r3: exit 0, 12.710s. All 7 preparations passed;
combined 98.375s, longest 19.030s, each within its separate 60s cap.
All measured runtime attempts including diagnostics totaled 3.634s. Measured interval
from review-branch creation to final-review submission is approximately 888s;
review time was not separately instrumented. No performance-speedup claim.

Pinned rustfmt passes all seven changed/new Rust modules. git diff --check,
v2_plan_status.py and v2_archive_status.sh pass. Independent final review ACCEPT:
exact producer identity, shared prerequisite order, complete artifact bindings and
single-action behavior are preserved, with the source lifecycle/policy proof passing.
Acceptance covers planning only; execution, publication and CLI activation remain open.
No REPLAN: the failed source gate identified a bounded native prerequisite correction
within the retained observation/identity contract. M7A remains partial and M8 unproved.
Next: stage all requested sources and execute the shared forest through one native
request, then replace selected-action-only publication with selected-artifact publication
across owners before removing CLI/server admission guards.
