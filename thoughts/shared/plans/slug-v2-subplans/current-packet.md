# Current Slug V2 Work Packet

Packet: WP-7-170-m7a-native-filegroup-data-r1
Status: accepted; M7A remains partial

## Outcome and compatibility

Baseline `1f7305d9d` (WP869). Advance the unchanged authentic Linux Rust-toolchain
cquery beyond `native filegroup nondefault data is unsupported`. Its precise
requesting label is unproved because the diagnostic has no label and one-shot
materialization uses temporary roots; do not invent one. Admit native filegroup's
selected data prerequisites and source-derived runfiles policy through its
existing configured semantic owner. Exact provider contents, data/src separation,
typed Boolean conversion/default/exec carry, selected edges and conditional
runfiles behavior; configuration/output identity and artifact paths remain
Slug-native. Existing visibility/testonly restriction messages remain bounded
Slug-native refusals. Coverage and nondefault output_group, other nonneutral
attrs, full toolchain/query/bootstrap and M8 are unsupported/deferred. Windows
remains minimal or explicitly unsupported; no Windows execution expansion.

Pinned Bazel 9.2 clean `8220c619`: Filegroup.java:61–105 keeps files/executable
srcs-only and builds default runfiles from data then src DEFAULT_RUNFILES; data
runfiles starts with src files, adds data, and adds src DATA_RUNFILES only when
filegroupRunfilesForData is True. FilegroupRule.java:60–62 declares target-config
LABEL_LIST data allowing any File, without constraints checking. Runfiles.java:
669–675/700–714/752–773 adds data using DATA_RUNFILES: actual File targets always
contribute files; rules contribute stable-wrapped DefaultInfo.files only when
alwaysIncludeFilesToBuildInData is True, followed by their data runfiles. Alias-
ConfiguredTarget.java:100–128/166–168 delegates File/rule provider lookup;
classify aliases by the resolved actual kind. CoreOptions.java:610–624 default
incompatible_always_include_files_in_data is True; builtin_exec_platforms:252
carries it. RuleConfiguredTargetBuilder.java:360–425 propagates _validation from
both explicit srcs/data regardless of runfiles flags; existing hidden-group owner
uses final default runfiles. FilegroupConfiguredTargetTest.java:225–305 and
StarlarkIntegrationTest.java:794–828 discriminate data/default/source separation
and rule files inclusion. False actual-File versus rule inclusion is a direct
source-derived discriminator. Reuse the existing native-filegroup authored
fixture and provenance, no copied source bodies or authentic workspace mutation.

## Owner and implementation

Existing native filegroup analysis owns condition preparation, source-admitted
configured children, cycle/Need/error merging, alias resolution, visibility,
testonly, edges, package closure and provider publication. Select/coerce data with
the existing conditions, join in the same target configuration and keep per-attr
indices. Apply the current prerequisite guards to srcs and data, and classify
actual SourceFile/GeneratedFile kinds only in phase scratch. Alias requested
label owns visibility; actual target owns testonly and File/rule discrimination.
Publish data edges and include data in package/runfiles closure and validations.
Files/executable remain sourced solely from srcs. Compose existing dense stable
files and compile-order runfiles DAGs; preserve auxiliary symlinks, empty filenames,
strictest conflict policy and prefix checks, without a second retained union.

Existing retained CoreOptions registry entry already owns the inclusion Boolean.
Append IncompatibleAlwaysIncludeFilesInData command variant, preserving ordinals;
add SlugConfiguration::always_include_files_in_data using core_bool and read it
for filegroup data. Support both values truthfully; no hardcoded True or command
repair. Existing request overlay/configured keys own structural equality,
invalidation and exec propagation. New collections/kind bits are phase scratch;
existing DefaultInfo/RetainedRunfiles/dense depsets remain DICE retained values
with their existing equality and shutdown lifetimes. No new key/lock/cache,
background task, heap retention or service memory owner. DICE ownership doc and
Stage 9 native filegroup utility row read; reuse unchanged representations,
no donor import or performance claim.

Allowlist: Loading src/package.rs, host_package_load_tests.rs module hook and
new filegroup_data_tests.rs; Analysis src/dice/filegroup.rs; Configuration src/command.rs,
native/configuration.rs and native/tests.rs; Commands src/common.rs existing
Boolean capture tests; Analysis tests/native_filegroup/{mod.rs,BUILD.fixture,
fixture.toml,data.rs}; manifest/canonical, Stage 6 current native-filegroup
paragraph. Existing Configuration module is 2,627 lines but remains the sole
native typed projection/conversion owner; the added core_bool sibling does not
justify adjacent refactoring. Filegroup is 638 lines; prepare exceeds 150 lines
already, but the change extends its single tracked child-join/error/publication
protocol using the existing branches, rather than a new semantic owner. Factor
only if needed to keep shared validation coherent. Estimate production <130 and
proof <330 net Rust lines; review actual size. Independent DESIGN/FINAL required
for selected graph/runfiles lifetime and configuration boundary.

## Reviewed Loading prerequisite correction

The first 14-row integration run passed three independent rows but eleven shared
fixture rows failed before Analysis: native Loading rejected data=select().
Independent DESIGN delta ACCEPT admits named configurable data in existing global
and native filegroup facades through the existing typed LabelList coercer/recorder.
Preserve omitted/provided-None default provenance and explicit empty; retain data
selectors/concatenations and union their condition keys with srcs in the existing
$config_dependencies attribute. RuleClass:2101–2118 and AggregatingAttributeMapper:
244–304 reject duplicate literal/branch labels but allow cross-branch repetition;
Analysis checks duplicates only in selected concatenation permutations. Use a
distinct File alias for positive artifact deduplication, never repeated literal
canonical labels. The package module exceeds 2,000 lines but this change stays in
its existing native filegroup facade/recorder/duplicate-validation responsibility;
no new Loading/query semantic owner or general attribute activation is introduced.
The expanded production/proof counts will be reviewed against the original
130/330 estimates for these required declaration and permutation discriminators.

## Proof and validation

Extend existing policy test for True default, typed/negated/raw conversion,
structural toggle/restoration and both exec mappings/idempotence; existing
Commands Boolean capture test adds the new flag. Existing fixture with_data
moves from explicit rejection to positive proof. Add authored cases only for
source/rule/File-alias/rule-alias/generated data, data-only executable None,
runfiles separation/order/duplicate/auxiliary preservation, independent crossed
flags and data validations. Data selected edges/config A/B/A and source/rule
provider A/B/A plus delete/recreate, missing selected child, unselected missing
child, cycles, package-group/testonly/visibility guards must retain meaning.
Both legacy and observed routes, root/external and target/exec use existing
Workspace/epoch helpers; serialize filesystem freshness tests sharing ancestors.
Keep old srcs-only forwarding, arbitrary-provider nonforwarding, empty native
coverage and all other refusal assertions intact. Reuse owner DAG regression.

Root serial pinned Cargo JSON no-run 60s Configuration lib, Commands lib,
Loading lib and Analysis lib/starlark_rule, exact ordinary nonignored preflight and focused 20s
runtime groups. Core check and CLI rebuild before unchanged three-hash authentic
supervised 180s replay. The observed 76s prerequisite proves actual bootstrap
consumer progress; smaller source-derived provider proofs cannot establish that
closure. Actual process checks before/after, scoped format, diff/archive/status.
Caps are operation limits, not user limits. REPLAN if source-derived data needs
new runfiles representation, retained owner/locking, widened prerequisite access
or enabled coverage/output-group action semantics. Never discard data, gate rule
files incorrectly, or claim complete cquery/bootstrap from a later refusal.

## Acceptance receipt

Independent DESIGN, Loading design delta and FINAL ACCEPT. The initial proof
failures are preserved: CheckVisibility config_setting expected success instead
of its exact NON_CONFIGURABLE rejection; the new command was initially missing
its CoreOptions descriptor arm; selected data exposed the Loading prerequisite;
Loading proof compiler/selector/BUILD Label mistakes were corrected within the
same owner. No failed or zero-selected invocation is counted as acceptance.
The final fourteen Analysis integration rows pass without dropping selected-data
or protected srcs assertions.

Final serial pinned no-run preparation: Configuration 1.08s, Commands 3.71s,
Loading 7.47s, Analysis starlark_rule 19.17s; earlier owner-lib preparation 13.30s.
Exact nonignored preflight and runtime pass Configuration 4 in 0.016s, Commands 3
in 0.008s, Loading 6 in 0.064s, native DAG owner 2 in 0.003s and Analysis 14 in
0.915s: 29 focused tests, 1.006s. Final Core check 3.86s and CLI rebuild 27.36s
pass. The unchanged authentic query ran 77.271s, exit 2, cleared filegroup data,
and reached the missing Rust host-tools target
`lib/rustlib/x86_64-unknown-linux-gnu/bin/gcc-ld/ld.lld`. Its original three hashes
are unchanged and actual pre/post process checks are empty. Scoped formatting,
diff, archive and plan-status checks pass. Receipts: target/wp870. Query success,
full toolchain/bootstrap closure and M8 remain unproved.

Actual growth: production 172/proof 705 net Rust lines, above the original
130/330 estimates. Independent review accepted the existing-owner Loading
prerequisite, source-derived duplicate phases and separate File/rule aliases,
policies, route/exec, provider/source/selection restoration and negative proofs.
Fixture hygiene reviewed: two authored Rust test modules, 25 small BUILD rows,
existing source/provenance/helpers; no copied source assets or added authentic
workspace files. Alias and RuleClass/AggregatingAttributeMapper source paths are
verified. Native/source/runfiles representations remain unchanged. Packet window:
about 12:32–13:11 UTC on 2026-09-30, including review, corrections and validation.

## Predecessor

WP869 `1f7305d9d` admits retained save_feature_state with False default. Independent
DESIGN/FINAL ACCEPT; 27 focused tests in 0.768s, Core/CLI pass. Unchanged authentic
query 76.673s exit 2 clears getter and reaches filegroup nondefault data refusal;
three hashes and actual process cleanup pass. Receipts: target/wp869.
