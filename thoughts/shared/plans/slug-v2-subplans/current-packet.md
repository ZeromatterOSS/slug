# Current Slug V2 Work Packet

Packet: WP-7-80-m7a-native-label-flag-r1
Status: accepted (independent final ACCEPT)

## Outcome and compatibility

Implement genuine native label_flag through Loading, typed command preparation,
configured analysis and config_setting. Bazel9.2 tools/cpp/BUILD.tools63–66
declares link_extra_libs with :empty_lib default. This is a prerequisite to the
authentic full cc_toolchain tools package, not a replacement for it.
Default-only forwarding is insufficient: real command overrides must select
providers under canonical typed configuration identity.

Exact slice: native label_flag with mandatory nonconfigurable label default,
help, omitted/explicit universal scope, target/exec forwarding,
canonical command overrides/default elision/last-wins, label-valued config_setting
predicates and literal BUILD aliases to settings. Schema scope default is
universal, but omitted command scope is Default and explicit universal is
Universal. Broader scopes and label transitions explicitly reject before evaluator
invocation; existing scalar/list/set settings/transitions remain unchanged.
Slug-native: structural configuration/output identity and diagnostic text.
Deferred: label_setting, configurable BUILD aliases in setting lookup, nonneutral
extra native attributes, registration aliases, project/exclude exec policies,
loading-query CLI implicit/nodep filtering (already unsupported), complete
tools-package load closure, installed binaries, full cc_toolchain,
execution and bootstrap. M7A partial; M8 unproved.

Predecessor WP779 at993483562 accepts native configurable-srcs filegroup providers,
with39 tests, Core/Query/CLI checks and independent design/final ACCEPT.

## Research and ownership

Pinned Bazel9.2 commit8220c6198837d5c13d53fea211cf3282aa12408a. Paths under
src/main/java/com/google/devtools/build/lib:
rules/LabelBuildSettings.java58–99; packages/RuleClass.java896–905;
rules/LateBoundAlias.java48–80; rules/AliasConfiguredTarget.java91–114,178;
analysis/CommonPrerequisiteValidator.java156–160,730–741;
runtime/StarlarkOptionsParser.java186–274,348–410;
analysis/config/CoreOptionConverters.java331–359;
rules/config/ConfigSetting.java650–700,764–789,816–827;
packages/AggregatingAttributeMapper.java131–165 and DependencyFilter.java31–46.
Adapt LabelBuildSettingTest.java94–344 (default/override/direct/relative/invalid
select and alias). Its345–498 transition cases remain deferred with negative
tests. BuildConfigurationValueTest.labelFlagExecScopes587–629 establishes scope
provenance; nondefault exec-exclusion remains closed.

Loading owns a native LabelFlag target with BuildSettingDeclaration and new Label
variants in existing BuildSettingDefinition/Default. Share a PackageTargetKind
declaration projection for native/Starlark settings. Expose BUILD/global and
native.label_flag through existing coercion/native schema/package-default and
macro-origin owners. Retain default/help/scope provenance and genuine class.
Reject select on default/scope, invalid types and unknown attrs. Query's hidden
:alias implicit default dependency and NODEP default attribute retain distinct
edge classifications. Add QueryEdgeKind::Nodep alongside VisibilityNodep and
Implicit; ordinary unfiltered traversal still visits both. Do not activate the
existing unsupported CLI filters or model these as ordinary alias dependencies.
Use the actual attribute name help (RuleClass.java660), not build_setting_help;
preserve source schema order: inherited attrs, :alias, scope, default, help.

Configuration owns StarlarkOptionValue::Label(CanonicalLabel), distinct canonical
framing and legacy-key tag. Compare canonical value labels for default elision,
never ultimate alias targets. Reuse Arc-backed sorted options and structural
equality/hash; omitted/explicit-universal scope survives admitted exec policy.
No second label store/map/cache/interner.

CommandConfigurationPreparationKey owns tracked conversion. Three contexts:
default uses declaration package/mapping; CLI value uses root option context
(bare pkg:target means //pkg:target and :target means root); config_setting
expected value uses condition package/mapping. A root flag with external VALUE
must demand observed root mapping after declaration-kind discovery, without new
dependencies for unrelated scalar strings. Union observations and Needs before
publication. Preserve existing native --flag_alias plumbing. Missing/negated/
malformed/invisible label values fail; repeated scalar flags are last-wins.

ConfiguredNodeAnalysisKey owns selected-target preparation, source admission,
observed package inputs, cycle guards, Need/error aggregation and provider
forwarding. Preserve own flag label/class/visibility while actual_target and
Target.label follow selected child. No own action or empty-provider substitute.
Reuse ConfiguredNodeKind::Alias, AliasActual and runfiles-package closure where
semantically appropriate; preserve hidden :alias attribute identity where exposed.
Do NOT reuse filegroup visibility/testonly guards: source skips these checks on
the late-bound any-provider edge. Nonneutral unsupported attrs reject before
publication, not silently disappear.
Final-review correction: the flag's OWN declaring-visibility package-group
dependencies still require tracked preparation and kind validation, independent
of the skipped selected-edge check. Join them under the existing guard, publish
DeclaringVisibility edges and package closure; prove valid/missing/wrong-kind
groups and same-DICE change/restoration. No ignored restricted-visibility input.

Shared tracked literal-alias declaration lookup stops at the native flag, never
its selected actual. Own label/type/default supply internal BuildSettingProvider
facts; no new public provider facade. Preserve cycle/Need behavior and explicitly
reject configurable aliases. Config_setting must also validate selected configured
prerequisites, use own setting label for option lookup, reject duplicate actual
labels per source and normalize expected labels in condition context. Selection
specialization uses source-normalized predicate identity rather than raw spelling.

DICE contract: docs/developers/dice.md and existing observed command/package/node
owners. No new key/lock, direct filesystem scan, injected semantic state or
retained evaluator borrow. Failed/cancelled attempts publish no partial node.
Package/default/provider/mapping changes invalidate tracked computations and
restore by structural/publication equality. Existing CanonicalLabel, Arc/Dupe,
Allocative and compact ordered scratch collections suffice. Retained declarations
and options follow package/configured-node lifetimes; conversion/joins are scratch.
Stage9 contextual command-configuration and native-provider utility decisions
apply. No donor import or performance claim.

Selected-prerequisite validation introduces Condition -> configured node ->
Condition cycles. Extend the EXISTING configured_analysis_cycle_detector CycleNode
to recognize ConfiguredConditionKey (workspace and full configured target), with
diagnostic display for this key family. Guard the condition computation's
dependency awaits once at its owner boundary, preserving existing single-use
guards; add no key/lock/task or separate detector. Existing synchronous receiver
lock must be released before awaits. Genuine flag -> configurable alias ->
condition -> flag regression must terminate with cycle error, and restoration
after breaking the cycle must succeed. Pure alias and filegroup cycles stay tested.

## Scope, routing and checkpoint

Base993483562; codex/wp780-label-flag-review. Estimated production600–1000 and
tests500–900 lines, a review trigger not a cap. Large package.rs/dice.rs receive
thin binding/dispatch hooks; cohesive native semantics go in child modules.

Worker1: Loading src/package/label_flag.rs, src/package.rs, src/attrs.rs,
src/bzl_module.rs and focused tests; Query src/graph.rs native projections/tests.
Publish shared declaration API before integration.
Worker2: Analysis src/dice/label_flag.rs, thin src/dice.rs hooks, condition matching
and src/configured_attribute.rs only for normalized predicate handling; new
tests/native_label_flag and starlark_rule.rs module hook; existing
src/configured_analysis_cycle_detector.rs condition-family extension.
Root: Configuration src/native/configuration.rs/tests; Analysis src/build_setting.rs,
src/command_configuration.rs, src/key.rs, label transition fail-closed hook and
command preparation tests; Core src/runtime/build_requested_target.rs normal
analysis dispatch/wrapper test; BUILD metadata, docs, serial Cargo and integration.
Root also owns the test-only Commands src/query.rs unsupported-filter guard;
no Commands production behavior is activated.
Reviewer: independent design and final actual diff/evidence.

Checkpoint resolutions: one pure Analysis lookup-step classifier returns terminal
setting declaration or literal alias label, using the shared Loading declaration
projection. Owner-local command/configured loops retain their own existing tracked
inventory/observation modes and compact visited sets; both return terminal setting
label plus declaration. Command grouping/default elision uses terminal setting
identity; configured prerequisite validation remains separate.

Keep public ConfiguredConditionMatch unchanged. One shared async tracked helper
supplies normalized predicates to truth computation AND selector preparation.
The latter must directly depend on normalization inputs: a same-truth change of
actual label/mapping must invalidate specialization. Add mandatory normalized_flags
Arc<[(CanonicalLabel, CompactString)]> only to scratch ConfiguredAttributeCondition;
duplicate/refinement keys use actual selected prerequisite label, own setting
label remains separate for option lookup. Normalize all admitted flag rows while
preserving nonlabel expected text and original Loading declarations (also used
by existing transition filtering). No retained condition-value or new key.

The small public QueryEdgeKind::Nodep discriminant is the sole new query
representation; downstream classification checks do not claim CLI filter support.
Independent design ACCEPT includes these resolutions and the condition-cycle
correction. Final review must inspect actual implementation and executed evidence.

## Acceptance

Loading: default required/type/nonconfigurable, BUILD/native, help/scope/provenance
and equality, external mapping, unknown attrs; query class/attrs/default-edge
classifications and unchanged rejection of unsupported CLI filters.
Configuration/command: Label vs same-text String encoding, repo/target differences,
last-wins/default elision/A-B-A, negation/missing/malformed, external VALUE under root
flag name, literal aliases/cycles, mapping A-B-A and three distinct contexts.
Analysis: default/override real ctx provider/files/executable consumers, nested
aliases, source/generated targets, selected missing versus unused default,
target/exec owner identity, zero own actions, private/testonly actual admission;
condition direct/relative/mapped/invalid labels, aliases, duplicate actual label
and specialization. Alias labels sharing an actual remain distinct option values.
Scope/transition/attr negatives reject before publication. Same-DICE declaration,
selected provider/source edit/delete/recreate/A-B-A and configuration switching
preserve package closure and invalidate correctly.

Tier DICE/cross-crate: focused owners plus protected native filegroup, aliases,
conditions, commands, transitions and providers; Core/Query/CLI direct checks.
Resolve pinned toolchain freshly; serial offline/locked Cargo JSON --no-run,
each preparation capped60s. Batch exact preflight then execute unchanged binaries;
zero tests prove nothing. Source-derived regressions; no fresh oracle absent a
specific gap. No CLI/Bazel/daemon/transport run planned; any later binary invocation
requires rebuild. Fixture.toml provenance, format/diff/archive/status checks,
independent final ACCEPT and measured preparation/runtime/packet elapsed receipt.

REPLAN only for a source contradiction or new unbounded owner/identity prerequisite;
routine compiler/invocation/test corrections remain in this packet.

## Acceptance receipt

Independent design and final reviews: ACCEPT. Focused exact tests: Configuration
2/2, Loading 5/5, Analysis 9/9, Query 2/2, Commands 1/1 and Core 1/1.
`cargo check -p slug_core_v2 -p slug_query_v2 -p slug_cli_v2` passed.
The broad Analysis integration run passed 208/210; both residual artifact-path
expectation failures reproduce in a pre-WP780 test executable. The broad Query
run passed 58/59; its sole generic relative-label diagnostic expectation
reproduces at accepted HEAD `993483562` in a detached worktree. These are
pre-existing baseline defects, not waived WP780 behavior. The changed query
fixture's path shards are injected; focused reruns emit no background panic.
`cargo fmt --all --check` reports only pre-existing
`app/slug_bzlmod_v2/src/host_package.rs` formatting drift; changed Rust files
are formatted. Diff, plan-status and archive checks passed. Packet elapsed
approximately 43 minutes across continuation; direct-dependent check 17.6s,
final Analysis/Query focused runtimes below 0.3s each. No Slug binary, daemon,
Bazel, NativeLink transport or BuildBuddy run belongs to this packet.
