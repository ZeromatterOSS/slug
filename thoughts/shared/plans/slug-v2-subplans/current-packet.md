# Current Slug V2 Work Packet

Packet: WP-7-41-m7a-requested-artifacts-r1
Status: accepted

## Outcome and basis

Activate admitted explicit build roots and expose complete ordinary requested-artifact
selection from BuildCommandEvaluation. This is the next owner boundary needed for
one shared prerequisite forest and ordinary CLI builds of the retained rules_rust
closure. It must not select all declared actions or substitute DefaultInfo alone.
WP740 (15e52a764) accepted effective configured providers and file/alias identity,
with 36 focused checks; its full receipt remains in that commit's manifest.

Pinned Bazel 9.2 8220c6198837d5c13d53fea211cf3282aa12408a, local source:
analysis/TopLevelArtifactHelper.java:204-255 unions FileProvider/DefaultInfo.files
then OutputGroupInfo.default through a stable builder, skips empty groups, and marks
names beginning '_' unimportant. analysis/OutputGroupInfo.java:145-152,207-260
selects sorted default, temp_files_INTERNAL_, _hidden_top_level_INTERNAL_, plus
_validation under ordinary validation policy (buildtool/BuildRequestOptions.java:
179-199; BuildRequest.java:451-458). FileConfiguredTarget.java:48-49,74-100 and
OutputFileConfiguredTarget.java:109-120 own singleton files and generated validations;
AliasConfiguredTarget.java:165-189 forwards actual providers. Reuse WP739/740 pinned
provider/source/alias evidence; no new oracle fixture or donor code.

Exact admitted semantics: ordinary groups, union/order/dedup, important flag, file
subset and alias forwarding. Structural identities and diagnostics remain Slug-native.
Selection is metadata, not execution or publication authority. User output-group
flags, validation toggles/aspects, wildcard expansion, additional native rule families,
external non-source roots and mixed external/root requests, scheduling/transport and CLI execution activation remain
deferred. Retain all selected artifact kinds, including hidden RunfilesTree; the later
scheduler must reject unsupported execution rather than drop selected requirements.

## Owner and contract

BuildCommandRootKey and existing observed/legacy Analysis preparation own requested
root activation. Preserve request order and duplicates separately from deduplicated
configured action owners. Activate explicit Starlark rules, exported regular sources,
generated files and aliases using authoritative ConfiguredNodeResult providers. Resolve
alias actual kind through Analysis, never by assuming a null key denotes a source or
by inferring owners from paths. Preserve requested identity and actual_target identity;
forward providers without rebasing artifacts. Reject unsupported alias actual kinds.

Preserve root-source byte checks, SourceCertificate, repository canonical routing,
Need/outer-error precedence, visibility and request revision behavior. Carry source
certificates through later metadata-analysis errors as well as success. Alias-to-source
activation must observe actual source content through existing tracked source owners,
including external sources, and carry its epoch into final native validation. Source
metadata analysis alone does not prove bytes fresh. Existing external source loading
can observe directories; directory artifact selection is explicitly unsupported and
must not silently produce empty success or manufacture file providers. Analyze admitted
regular external exported sources through the same authoritative Analysis owner.
Null source analyses are metadata roots, excluded from ValidatedActionClosure's
configured owners. Alias/generated dependencies continue to bring in real producers.
LoadedOnly/wildcard/native-unadmitted results may remain loading metadata for existing
callers, but the new selection API returns an explicit unsupported error for them.
An analyzed empty output set and a source-only set are legitimate zero-action selections.

Expose a borrowed evaluation-owned selection via a cohesive Core child module. Return
ordered per-request provenance, actual identity, and ordered nonempty output groups.
Ordinary group order follows the pinned sorted set: _hidden_top_level_INTERNAL_,
_validation, default, temp_files_INTERNAL_. Default uses a stable depset builder with
DefaultInfo.files followed by OutputGroupInfo.default; other groups use the same stable
builder rule. Flatten only this request-local consumer view. Deduplicate artifacts
structurally across requests/groups into one ordered artifact table, with per-group
indices preserving membership and order. Preserve full owner/configuration/output-kind
identity and source labels. No producer lookup, action traversal or execution filtering
belongs in selection. Missing-producer/action-family checks belong in the next shared
prerequisite planner, not a fabricated success here.

New vectors/maps are command scratch borrowing the retained evaluation; existing Arc
configured results and SourceCertificate remain DICE retained and Allocative-accounted.
No new key, global cache, interner, filesystem bypass, or lock across compute. Reuse
Stage9 SmallMap/SmallSet, Arc and AnalysisDepset dispositions and existing DICE dependency
recording/equality/invalidation (docs/developers/dice.md). Existing observed branch/closure
frontier association and native final revision validation remain authoritative. Neither
selection nor new borrowed views can authorize execution after acceptance.

## Reviewed integration corrections

Independent design-delta review ACCEPT: source observation errors can have empty
epochs; preserve the original error with an optional certificate instead of panicking.
Only successful eligible source observations require a nonempty certificate. A private
terminal-aware NativeCommandRoot association hook defaults to prior policy. Certified
requested source aliases (including SourceCertified/SourceArtifactInput failures) use
SelectedDependencySuperset: aliases can carry repository dependencies even for a local
actual source. Activation proves actual SourceFile; no null-key inference is introduced.
Direct sources and unrelated analyses retain their policy. Certified regular external
source metadata likewise uses the selected dependency superset; observation-only external
directories retain ClosureRepositories. Certified external metadata Analysis errors use
the existing transient/unavailable-root error policy so the original diagnostic survives. Every reported observation
must still match selected demand/value/Arc; full native closure/revision validation
remains authoritative. Prove a routed external source alias, content mutation, real
post-byte metadata failure with preserved certificate, and empty-epoch error handling.

External package loading must use the same canonical repository inventory key as
Analysis. After the apparent route identifies the canonical repository, reuse or
compute HostCanonicalRepositoryLoadRoute's existing input and observed prefix for
RepositoryPackageLoadKey::new_canonical. Preserve the original apparent route for
byte observation/certification and package-before-source ordering. This removes a real
duplicate BUILD evaluation at its producer identity; never filter duplicate print events.
Need/path-frontier/typed route errors retain their channels. Complete local builtin
module prerequisites are required by canonical metadata (including directory loading).
The protected external test supplies those locally and preserves one BUILD event,
source frontier, invalidation and package/source Need assertions. Its local virtual
probes seed current shards/runtime data and its legacy comparison seeds snapshots.
Independent design-delta review ACCEPT; no Loading/Analysis key or semantics change.

Wildcard selection rejection is proved from the existing observed package-all DICE
loading terminal on a warmed fixture. Its native acceptance/expansion remains deferred;
old StrictPathOnly repository restrictions are not relaxed to obtain a passing test.
Native scalar LoadedOnly and unsupported alias checks remain separate.
The protected external test's final return to the root package uses its existing
scalar //:root loading target, preserving successful metadata, exactly ROOT_BUILD,
and the subsequent pointer-distinct external-frontier check. Its old //:all request
also encountered the deferred native wildcard repository restriction; that unrelated
acceptance claim is not part of this source control. Independent correction review ACCEPT.

The existing CLI/server execution helpers enumerate all closure actions. New alias and
generated roots would implicitly gain wrong execution. Before any runtime/transport or
output side effects, both helpers reject requested Alias/GeneratedFile kinds using a
borrowed requested-root kind projection; existing Rule and mixed source/rule admission
is unchanged. This is a temporary gate, not an execution fallback: the violated invariant
is requested-artifact scheduling, its deletion condition is replacement of both legacy
helpers by the native shared selected-artifact forest and validated publication, and the
next scheduling/CLI-activation packet owns removal. Regression: an alias/generated root
with unrelated producer outputs and invalid backend is rejected before effects, while a
Rule root passes admission. Do not change analyses() or closure execution-view meaning.

## Scope, evidence and validation

Allowlist: Core runtime/dice.rs minimal delegation and root/closure dispatch; new
runtime/requested_artifacts.rs public selection types with runtime/mod.rs exports;
new runtime/build_requested_target.rs activation helper if needed; corresponding child
tests under runtime/requested_artifacts/ and runtime/build_requested_target/ with small
parent wiring; directly affected Core source/alias/external expectation corrections.
Analysis public APIs only if existing node-key compute cannot establish actual kind,
requiring a bounded design-delta review. CLI commands/build.rs and server reapi.rs early legacy-execution guards and focused
child tests; no broader CLI/REAPI production activation. Scheduling
files current/canonical, Stage7 owner and bootstrap readiness may record accepted scope.

The existing dice.rs greatly exceeds 2000 lines and loaded-branch function exceeds150;
move cohesive activation into its child instead of growing the monolith. Estimated
production growth 250-500 lines and tests 300-500 is a review trigger, not a hard cap.
Reviewed error/frontier regressions and two wrapper admission tests exceed the initial
proof estimate; they cover distinct integration owners with shared fixture scaffolding.
Long existing CLI/server execution functions receive only early guards; their selected
scheduler replacement remains the next owner boundary, not a refactor in this packet.
Root owns selection module, exports, integration, scheduling and validation. Activation
worker owns dice.rs/new helper only. Test worker owns new integration child only; root
wires modules. Independent design and final review required.

Discriminators: DefaultInfo plus explicit default (overlap and distinct); ordinary
hidden/validation/temp and exclusion of unrelated groups; stable order/important flags;
per-request/group membership and global dedup; equal spelling with distinct owner/config
or kind stays distinct; direct/alias/source/generated subset equivalence; hidden binary
RunfilesTree retained; empty analyzed selection versus LoadedOnly/wildcard/unsupported
actual error; duplicate roots and real producer closure; source mutation and A/B/A on
one runtime; routed external source identity and frontier; existing root source error
and request freshness controls. No claim of shared scheduling or output publication.

Compile pinned Core tests --no-run separately with 60s preparation cap, then preflight
all exact ordinary selectors once per executable and execute bounded groups. Aim for
individual checks below a few seconds; >30s needs strict necessity, 12s is guidance.
Run focused new integration/selection tests and affected protected source/closure tests;
compile direct CLI/server dependents and run their focused no-effects admission tests. Reuse unchanged Analysis/REAPI proof. No daemon/wire smoke
is required without execution changes. Rustfmt changed files, git diff --check,
v2_plan_status.py and v2_archive_status.sh. Final ACCEPT then commit/fast-forward main
and push under existing authorization. Receipts target/wp741. M7A partial, M8 unproved.
REPLAN only on a contradicted ownership contract or newly required semantic prerequisite;
compiler/invocation/fixture corrections remain within the packet.

## Validation receipt

Base: 15e52a764; candidate branch review/wp741-requested-artifacts. Pinned
nightly-2025-09-14 on Linux GNU, default Core/CLI/server test features. Direct pinned
binaries were used after the rustup snap launcher failed; no toolchain substitution.
Runtime selectors were preflighted per executable before execution. Receipts are local
under target/wp741; this summary preserves the relevant evidence in Git.

Fourteen distinct focused checks have passing evidence: eight new Core checks
(structural identity/group union, requested membership/order, runfiles/empty/unsupported,
source and output-group A/B/A, routed external source/alias bytes and digest, certified
metadata failure, legacy source alias, and empty observation failure); four protected
Core checks (generated prerequisites, alias/generated/null closure, Need precedence,
external event/frontier/source lifecycle); and the CLI and server execution guards.
No action or remote execution is claimed by these metadata and admission tests.

- core-final-r1: 12 selected, 10 passed, 2 failed, 2.608s. Preserve its unchanged
  passing evidence. Both failures were corrected and rerun: wildcard fixture
  prerequisites/metadata-only proof and canonical external BUILD producer sharing.
- core-canonical-correction: 3 selected, 2 passed, 1 failed, 1.209s. External-source
  selection and wildcard metadata rejection pass. The protected external check reached
  its late root wildcard transition; the reviewed scalar transition correction then
  passed 1/1 in 0.488s (protected-external-final), including its final Arc checks.
- cli-guard-r2 and server-guard-r2: each 1/1, 0.402s and 0.460s. Alias/generated
  admission rejects before runtime/transport/output effects; mixed source/rule retains
  its existing no-actions diagnostic. Their unchanged proof is reused after Core's
  external canonical-package correction; these guard fixtures contain root-only targets.
- Final Core preparation core-no-run-r8: exit 0, 10.399s. Direct CLI/server dependent
  preparation wrappers-no-run-r3: exit 0, 13.712s, after the last production change.
  Earlier module-path and moved-value compiler errors were corrected. All eleven
  preparation operations totaled 172.942s; longest 36.764s, within each 60s cap.

Three additional old source/event controls fail on both candidate and the retained
15e52a764 Core/REAPI test executable (baseline-source-event-controls):
root_exported_source_revision_bridge_retries_changed_terminal_and_preserves_epoch
lacks WorkspaceSnapshotKey injection; public_multi_source_edit_restore_without_event_replay
unwraps a missing certificate; build_command_root_selects_each_terminal_producer_once_for_duplicate_targets
lacks observation shards and fails event ordering. These are attributed baseline
fixture failures, not passing gates. The fourth baseline failure was the protected
external source fixture's stale manual epoch probe; this packet repairs and passes it.
The new native tests supply current request/observation prerequisites and discriminate
source freshness, repeated roots and group restoration directly. No broad suite claim.

Final rustfmt checks pass for all nine changed/new Rust modules and separately for
the touched protected-test function; unrelated old formatting in its large parent is
preserved. git diff --check, v2_plan_status.py and v2_archive_status.sh pass.
Independent final review ACCEPT: artifact identity, requested-group provenance,
source freshness and DICE ownership are preserved; canonical package sharing fixes
duplicate evaluation, and wrapper guards prevent unintended execution activation.
Packet/review elapsed time was not continuously recorded; compile and runtime durations
above are measured. No REPLAN: corrections retained the accepted ownership contract.
The gate advanced is complete ordinary requested-output metadata. M7A remains partial
and M8 unproved. Next: one shared prerequisite forest seeded from selected artifacts,
retaining zero-action roots and rejecting unsupported selected execution kinds.
