# Current Slug V2 Work Packet

Packet: WP-7-40-m7a-effective-configured-outputs-r1
Status: accepted

## Outcome and basis

Publish effective output providers on admitted configured rule/source/generated-file
nodes, with alias forwarding. This is the missing producer-owned metadata required
before Core can select ordinary requested artifacts. WP739 (2a7ea14ab) accepts typed
OutputGroupInfo construction; it does not add automatic groups. Current source and
generated nodes publish empty providers and repair DefaultInfo only in a dependency
view. Rule publication omits hidden runfiles groups and inherited validations.

Demanded by the ordinary build of the retained CLI/rules_rust production closure,
whose DefaultInfo executable/runfiles and OutputGroupInfo were source-inventoried in
WP739. No production input is declared unnecessary from lack of execution coverage.
Pinned Bazel 9.2 8220c6198837d5c13d53fea211cf3282aa12408a (local git objects):
RuleConfiguredTargetBuilder.java:145-172 adds binary RunfilesSupport.tree or
nonbinary default_runfiles.getAllArtifacts to _hidden_top_level_INTERNAL_.
Runfiles.java:425-436 unions its artifacts and symlink/root-symlink targets.
RuleConfiguredTargetBuilder.java:535-550,633-658 imports Starlark groups through
stable-order builders; automatic same-name additions union rather than replace.
Even supplied groups without additions undergo that stable-builder normalization.
RuleConfiguredTargetBuilder.java:294-296,360-425 propagates nonempty _validation
from dependency attributes except skip_validations, tool and implicit attributes;
analysis-test rules suppress automatic propagation. Attribute.java:2009-2010,
2119-2126,2273-2274 owns skip/tool/implicit classification. Tool includes explicit
IS_TOOL_DEPENDENCY and tool transitions; Slug's admitted exec/exec-group transitions
supply that classification. Existing analysis-test rule execution is not admitted.
AttributeValueSource.java:24-28,65-71 distinguishes late-bound ':name' from implicit
'$name'. Reuse implementation.late_bound_rule_attributes() for configuration_field
defaults: their leading underscore does not suppress validation propagation unless
another skip/tool policy applies. Hidden subrule dependencies remain excluded.
FileConfiguredTarget.java:48-49,74-100 exposes each file's singleton artifact;
OutputFileConfiguredTarget.java:109-120 forwards only nonempty generator _validation;
AliasConfiguredTarget.java:165-189 forwards the actual providers.

TopLevelArtifactHelper.java:204-239 later unions DefaultInfo.files with explicit
OutputGroupInfo.default; neither may replace the other here. OutputGroupInfo.java:
145-152,246-250 and BuildRequestOptions.java:179-199 establish default selection
(default, temp_files_INTERNAL_, _hidden_top_level_INTERNAL_, plus _validation).
Command selection/flags, scheduling, runfiles action execution and CLI activation
remain subsequent work, not a DefaultInfo-only success path.

Exact admitted metadata: stable configured group completion, automatic hidden group,
filtered transitive validations, singleton file providers, generated validation-only
forwarding and alias preservation. Existing structural configured/artifact/path
identity, diagnostics and dir() ordering remain Slug-native. _validation_transitive
native/builtin override, analysis-test execution, aspects/internal provider merges,
unsupported node families, output selection and transport remain deferred. Keep the
explicit private-override rejection; no native allowlist bypass.

## Ownership and implementation

ConfiguredNodeResult/ProviderCollection remains the retained DICE owner. Complete
rule groups once after DefaultInfo/runfiles support is finalized and before result
publication, in a cohesive new analysis output_groups module. Existing large
starlark_rule.rs/lib.rs receive delegation only. Preserve unrelated providers and
DefaultInfo.files exactly; reject duplicate returned groups before completion.
Use stable/default AnalysisDepset builders with supplied group depsets as children,
then append automatic group inputs. Preserve artifact owners/configurations/kinds
and child graph sharing; do not flatten retained artifact depsets or infer owners
from output spellings. Configured normalization may change a supplied root's order
or collapse canonical empties, following the pinned builder; constructor-only and
nested provider values retain WP739 behavior. Tests must distinguish this boundary.

Hidden group: RunfilesSupport.tree when present, otherwise default_runfiles.files
plus artifact targets of symlinks/root_symlinks; exclude data_runfiles and empty
filenames as artifacts. An empty hidden group is still present for an ordinary rule.
Validation: own explicit group plus each nonempty eligible dependency group, in
retained attribute/dependency order. Use Loaded AttributeSchema policy and prepared
resolved dependency providers, excluding filtered, tool, implicit and hidden/subrule
edges; never walk arbitrary configured graph edges. Add narrow Loading policy
accessors over already-retained flags/configuration/name. No new semantic input.

Make source/generated DefaultInfo authoritative at their existing DICE compute:
source carries its existing canonical Source artifact; generated File carries the
resolved generating configured owner's artifact and only that output, plus the
nonempty generator _validation group. Aliases reuse actual provider collections;
remove dependency-view synthesis so direct and dependency projections agree.
Ordinary ctx.attr label dependencies always expose Targets, including source/null
identities. Remove the legacy source-only File projection that discarded providers
(StarlarkAttributesCollection.java:339-369 prerequisite-backed attribute construction).
File consumers extract through DefaultInfo; subrule single-file projections remain
separate. Independent review accepts this general correction over alias-only repair.
Source aliases exposed a pre-existing panic: an optional configured-only actual
identity cannot represent a null source. Replace it with one actual_target:
ConfiguredNodeKey in ConfiguredNodeResult, retaining actual_configured_target()
as a projection. Forward this node through aliases and prepared dependency views;
compute_actual_child follows the actual node, so invalid source platform/constraint
references fail normally. Source-alias file admissibility identifies an actual
null target with a singleton Source artifact, uses its filename, and preserves
source checks even under skip_analysis_time_filetype_check. Unrelated null nodes
are not treated as files. Independent design-delta review ACCEPT.
Preserve existing visibility/source observation/Need/error/dependency ownership;
no direct filesystem reads, new DICE key, cache, interner or async lock.

Use existing AnalysisDepset/Arc/CompactString/SmallMap/Allocative and shared
PublicationEqState (Stage 9 accepted retained depset/compact-owner dispositions).
New maps/vectors are completion scratch; retained graph children belong to the
configured result and no evaluator loan escapes. Native source/provider projection
must retain external canonical identities and generated configuration ownership.

## Scope and evidence

Allowlist: Loading attrs.rs narrow policy accessor and focused child tests;
Analysis new output_groups.rs and child tests, minimal lib.rs/starlark_rule.rs wiring;
dice.rs existing source/generated/provider projection branches; result.rs actual-node
identity replacement and subrule.rs source-alias policy only; new integration
children tests/effective_output_groups and tests/file_output_providers with minimal
tests/starlark_rule.rs module wiring. Update affected WP739 output_group_info tests
only for intentional configured completion, preserving fresh/nested assertions.
analysis_value.rs legacy source projection removal; existing Analysis rustc_map_each
and Core/REAPI source-staging/prerequisite/chain test fixtures migrate File extraction
through DefaultInfo, with affected protected gates. No Core/REAPI production changes.
Existing analysis tests may receive narrowly required expectation corrections for
new authoritative file providers/automatic group presence. Build API only if a
small checked accessor proves necessary. Canonical/current, Stage 6, bootstrap
readiness status. No Core/REAPI/CLI/server activation or unbounded file refactor.

Evidence: explicit hidden group union; binary tree vs nonbinary file/symlink targets
and exclusion of data/empty entries; empty hidden presence; own/transitive validation
and skip/implicit/tool/filtered exclusions; stable normalization vs constructor
identity; unchanged DefaultInfo and explicit default; same-DICE A/B/A through a
dependency change; source and generated singleton providers; generated validation
without unrelated generator groups/outputs; alias and dependency equivalence;
protected user/default/output-group sharing and configured ownership tests.
Use pinned source regression evidence; no new Bazel oracle is required. Protect
migrated source fixtures with five Core source/chain cases, two REAPI declaration/
preflight cases and one existing supervised NativeLink source/generated-chain
selected-output publication case. No new wire fixture or transport semantics.

Root owns completion helper/wiring and integration. File-provider worker owns dice.rs
and its new integration child. Loading/policy worker owns attrs.rs and effective
rule-group integration child. Root alone wires shared test parent and edits manifests.
Independent design then final review required. Serialize Cargo preparation, allow
built-executable checks in parallel with disjoint useful work. Exact selector
preflight; tests expected below a few seconds; >30s needs strict necessity.
Pinned no-run preparation separately capped60s; compile Loading/Analysis and named
Core/CLI dependents. Format changed Rust; diff/plan/archive checks; final ACCEPT,
commit/fast-forward main/push with existing authorization. Receipts target/wp740.
Checkpoint start 2026-09-19 02:08:58 UTC. M7A partial, M8 unproved.
REPLAN only if source contradicts the selected ownership or an unmodeled prerequisite
is necessary; normal compiler/fixture/invocation corrections stay within this packet.

## Acceptance receipt

Independent design, bounded identity/projection corrections and final review ACCEPT.
Checkpoint elapsed approximately 20 minutes from recorded start through final review;
no REPLAN or new semantic owner.

Thirty-six unique checks pass. Loading policy 2/2 (0.003s), unchanged Analysis
completion/materializer and protected file-policy units 5/5 (0.007s), final Analysis
integration 21/21 (3.410s), Core source/chain controls 5/5 (1.884s), REAPI declaration/
preflight controls 2/2 (0.933s), and the existing NativeLink selected-output
publication case 1/1 (1.554s). Accepted runtime groups total 7.791s. The integration
group includes direct and aliased source Target/provider equivalence, transitioned
generated ownership and validation-only forwarding, ordinary and late-bound validation
eligibility, runfiles groups, A/B/A, six protected Rustc/external-source cases and
existing provider/default/file/platform/Args/execution-group behavior.

Independent review required a correction for already-admitted late-bound attributes;
the retained late-bound declaration now distinguishes ':name' from implicit '$name'.
Source-alias evidence exposed the legacy provider-discarding File projection; it was
removed for all ordinary ctx.attr dependencies, with explicit File extraction in
affected fixtures. An initial test-only redundant Arc wrapper and a group-count
expectation were corrected. Earlier passing unit checks are reused after unrelated
policy/projection changes. Initial failed receipts remain attributed, not waived.

Eight pinned preparation invocations total 160.739s, maximum 57.199s, each under the
separate 60s cap. Final Loading/Analysis/Core/REAPI no-run compiles and Core/CLI check
pass. Ordinary and ignored selectors were checked exactly. The wire test's first
sandbox socket denial was cleaned; the authorized rerun passed with its backend
terminated and fresh root removed. Wire evidence used a fresh local NativeLink.
Changed Rust formatting, diff/plan/archive checks pass. Receipts: target/wp740/
loading-policy-r2, analysis-unit, analysis-policy-protected, analysis-integration-r3,
core-source-chain-protected, reapi-source-chain-protected,
nativelink_selected_outputs_publish_modes_and_replace_tree and compile/check files.

The observable gate advanced is authoritative effective configured output metadata.
Ordinary requested-group selection/shared scheduling, runfiles-family execution and
CLI activation remain open; M7A partial and M8 unproved.
