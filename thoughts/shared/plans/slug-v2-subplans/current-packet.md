# Current Slug V2 Work Packet

Packet: WP-7-79-m7a-native-filegroup-forwarding-r1
Status: accepted

## Outcome and compatibility

Analyze genuine native filegroup targets with configurable srcs through the
existing configured graph. Publish ordered files, distinct default/data runfiles
and a FilesToRun executable iff flattened files are singleton. Prove nested
filegroups and aliases feed real ctx.file/ctx.files/ctx.executable consumers in
target and exec configurations, without an authored forwarding rule.

Exact admitted slice: configurable srcs, empty data and output_group, no coverage,
both incompatible_filegroup_runfiles_for_data policies. Files are a stable-order
transitive union of selected srcs DefaultInfo.files. Default runfiles merge srcs
default runfiles; data runfiles include files and, under true policy, srcs data
runfiles. Preserve all admitted runfiles contents and source-defined ordering;
singleton source/derived executable; no own actions or RunfilesSupport. Publish
genuine empty InstrumentedFilesInfo under disabled coverage using existing native
provider identity and fields. Arbitrary child providers are not forwarded.
Reuse complete_rule_output_groups for hidden default-runfiles artifacts and
eligible srcs validation propagation (RuleConfiguredTargetBuilder.java145–176);
do not omit native rule completion or substitute data runfiles.
Slug-native: structural configuration/artifact identity and diagnostics.
Deferred: nonempty data/output_group and coverage (explicit configured rejection,
preserving accepted Loading/query declarations); installed module-tool binaries,
label_flag, complete tools/cpp loading closure, full cc_toolchain, status production,
action execution and bootstrap. M7A partial; M8 unproved.

Predecessor WP778 at b56eaf634 accepts genuine C++ build-info registration and
OutputGroupInfo consumed by the authentic helper;120 tests, CLI check and
independent design/final review passed. It does not construct full cc_toolchain.

## Research and semantic owners

Pinned Bazel9.2 commit8220c6198837d5c13d53fea211cf3282aa12408a:
rules/filegroup/Filegroup.java51–136 and FilegroupRule.java40–88;
analysis/PrerequisiteArtifacts.java64–71; analysis/Runfiles.java499–506,638–659,
669–715,738–774; analysis/config/CoreOptions.java66–75;
analysis/test/InstrumentedFilesCollector.java107–108. Paths relative to
src/main/java/com/google/devtools/build/lib. Source regressions adapt
src/test/java/.../rules/filegroup/FilegroupConfiguredTargetTest.java131–155,
225–305 (single/multi/diamond and distinct files/default/data runfiles). Tests for
coverage and nonempty data/output_group are outside this admitted phase and
replaced only by explicit rejection tests, not a parity claim.

Demand: rules_cc0.2.17 semantics.bzl142–155 unconditionally declares executable
exec aggregate-ddi/generate-modmap attrs. Bazel tools/cpp/BUILD.tools83–90 and
modules_tools/BUILD.tools8–22 use nested native filegroups and platform select.
Interface/link tools also use aliases and filegroups. Full-rule audit found
label_flag and authentic compatibility-proxy/protobuf loading prerequisites too;
those remain separate, not bypassed. Module binaries are installed release
artifacts, not actions rebuilt during analysis; later catalog work must choose
explicit release-platform provenance.

Loading already retains complete configurable srcs in native attribute overrides
and flattened candidate labels for unconfigured query. Borrow native_attributes
and use existing configured condition resolver; never analyze flattened all-branch
PackageTargetKind::Filegroup.srcs as selected dependencies. Preserve native schema
and Loading/query behavior. Reject selected nonempty data/output_group and enabled
coverage before node/provider publication; no silently ignored semantic input.
Final-diff design correction: resolve selectors in all retained native attributes,
including package-default values. Apart from handled srcs/visibility/config-deps
and name/generator provenance, require neutral false/empty values; nonneutral
features/tags/deprecation/testonly/constraints/aspect hints/transitive configs/
package metadata/licenses remain explicitly unsupported. Preserve their existing
Loading/query declarations, not silent configured activation. Direct package_group
srcs reject; aliases to package groups are explicitly unsupported (upstream has
alias-specific ambiguity). Testonly actual prerequisites, including aliases and
generated-file owners, are explicitly unsupported regardless of the separate
native output-file/experimental-package policies. Tracked actual-target package
checks do not replace requested alias edges or visibility. Source authority:
CommonPrerequisiteValidator.java343–375,730–777. Independent correction ACCEPT.

ConfiguredNodeAnalysisKey/observed driver owns native analysis. Reuse package and
source observations, condition keys, child preparation with source admission,
cycle guard and configured attribute edges. Existing analysis tracks visibility
declarations but does not provide a generic enforcement owner. This packet adds
a bounded srcs-edge admission gate using tracked LoadedPackage.effective_visibility:
public prerequisites are admitted; ordinary same-package prerequisites are admitted
only when neither consumer nor prerequisite (or generated file's generating rule)
has a symbolic-macro origin. Reject ordinary cross-package private references;
restricted and nonpublic macro visibility remain explicitly unsupported. Inspect
requested alias label/visibility, not its actual-target visibility. Reuse tracked
package inputs; no new graph or visibility evaluator. Source authority:
CommonPrerequisiteValidator.java152–154,252–288,304–327. Honor existing full-config
check_visibility=false by skipping this gate through a checked bool projection;
default true follows the bounded admission above. Only selected
children contribute srcs edges; condition packages use existing runfiles-package
closure behavior. Preserve Need/outer/semantic error aggregation and events.
Failed/cancelled attempts publish no partial node. Existing structural equality,
publication_eq, tracked source/package/configuration dependencies own cutoff,
invalidation/restoration and overlapping request sharing. No filesystem scan,
key injection, new DICE key/lock or global state. Read docs/developers/dice.md.

Use ConfiguredNodeKind::Rule and genuine FILEGROUP_RULE_CAPABILITY, subject to
checking existing consumers. The native-node constructor rejects Rule: use existing
collect_runfiles_packages then new_rule(...).with_edges(...), preserving that
constructor invariant without a new kind or weakened assertion.
Reuse ProviderCollection, DefaultInfo::from_effective
(not from_executable, which adds executable to default runfiles), FilesToRunProvider,
dense AnalysisDepset, Runfiles and RunfilesPackageCollector. Preserve mixed child
orders with source-defined stable files / compile-order runfiles roots; inspect
existing builders before choosing a narrow source-exact builder hook.
Freeze a private builder in dice/filegroup.rs over existing public fields:
Postorder files, Default symlink/root/empty-filename roots; conflict policy is
merged before skipping empty child contents, nonempty prefixes must agree with
the existing _main workspace convention, and incompatible orders error rather
than flatten. No BuildAPI public constructor change.
Existing Arc/Dupe/Allocative and compact ordered scratch collections suffice.
Published providers/edges follow configured-node DICE lifetime; joins/builders
are phase scratch and retain no evaluator heap or command borrow. Stage9 existing
Stage6 FilesToRun/runfiles/provider utility dispositions apply. No donor import,
interner/cache, new retained representation or performance claim.

## Scope and routing

Base b56eaf634; codex/wp779-native-filegroup-review. Independent design checkpoint
before edits; final actual-diff/evidence review. Large dice.rs gets thin hooks;
new semantics in child module. Estimate300–650 production and400–700 test lines;
size is a review trigger, not an automatic stop.
The private async prepare function exceeds150 lines: it remains one native-rule
transaction over selector preparation, guarded child join and Need/error precedence.
Provider/runfiles construction and prerequisite policy are separate helpers; no
command, transport, persistence or new key family enters this function. Added
negative/provider-publication gates exceed the initial test estimate; retain those
discriminators rather than splitting their shared fixture into duplicate corpora.

Worker1: Analysis src/dice/filegroup.rs, src/dice.rs module/dispatch/external shape
hooks; src/result.rs small encapsulation hook only if needed. No Loading storage
or broad dispatch changes. Root reviews any additional owner/public API first.
Worker2: Analysis tests/native_filegroup/ child sources and fixture.toml;
tests/starlark_rule.rs module hook. Reuse existing selected-registry/main-external
fixture helpers and exact source-derived regressions; no copied upstream corpus.
Protected tests/effective_output_groups/mod.rs has baseline pre-WP766 path
assertions: compare its package-local expected strings with root_relative_path
and explicitly assert full result configuration roots. Keep shared helpers and
all group membership/order/kind/exclusion/restoration checks. This is test-coordinate
repair, not a production identity change or failure waiver; rerun all four tests.
Reviewer: source/runfiles design and final diff.
Root: integration, serial Cargo, protected tests; Analysis BUILD.bazel test-source
metadata if needed; stage6/9/bootstrap and scheduling. No BuildAPI builder hook.
Root also owns Configuration src/native/configuration.rs checked
filegroup_runfiles_for_data/check_visibility getters and focused native tests.
Configuration src/command.rs admits these two existing CoreOptions booleans through
the normal typed command overlay (parse/name/negation/descriptor), not a test-only
setter. Preserve complete structural identity and native exec inheritance; prove
default/false/negated/restored policies and target-to-exec/default-exec behavior.
Root extends the existing Commands src/common.rs boolean-parser test table with
both flags; no Commands production change. Execute that direct wrapper test.
Root owns Analysis src/analysis_value/empty_instrumented_files.rs plus thin analysis_value.rs
materialize/lower hooks. The empty-only facade uses existing builtin occurrence
identity, exactly two empty depsets, native type/fields/repr/hash/equality, and
rematerializes nested/direct occurrences without activating a constructor or
coverage_common. No new retained provider model.

## Acceptance and validation

Empty/singleton/multiple, duplicate transitive files and stable diamond/mixed-order
DAGs; singleton after dedup; source/derived identity; zero own actions. Distinct
default/data runfiles with both policy values, including existing symlinks/root
symlinks/empty filenames; no executable insertion in default runfiles. Empty
InstrumentedFilesInfo fields and no arbitrary provider forwarding. Genuine
DefaultInfo and ctx.file/files/executable consumers; exec transition retains
derived owner configuration. No host executable-bit inference.

Nested/alias/main/external paths, select match/no-match/error, selected missing
versus unselected unavailable source, visibility and cycles.
Visibility discriminators include cross-package private and same-physical-package
symbolic-macro private rejection; public aliases use alias declaration visibility.
Same-DICE BUILD/provider/source A/B/A and missing/recreate; configuration switching must not leak
files/executable/runfiles/provider/package closure. Explicit unsupported-attribute
and coverage rejection, with no partial provider publication.

Tier DICE/cross-crate: focused new tests plus protected alias/source/visibility/
conditions/runfiles/executable and authentic toolchain-helper/public compile gates.
Compile Analysis and named direct Core/CLI dependents; Query check protects rule
kind/native graph consumers. Resolve pinned rust-toolchain binaries freshly.
Cargo offline/locked JSON --no-run preparation<=60s per operation; serial shared
target. Batch exact preflight on final test executable, then execute and report
selected/pass and elapsed. Tests stay small; >30s requires concrete justification.
No CLI binary/Bazel/daemon run planned. Format changed Rust; diff/archive/status
and fixture provenance checks. Source-derived regressions establish deterministic
parity; fresh Bazel replay is unnecessary without an evidence gap.

Fixture records use Stage1 fixture.toml provenance and structured comparison.
Authored source/provider input rules are allowed; forwarding filegroup is native.
No invented installed-tool source files or empty toolchain providers.

REPLAN only for a required untracked input, no bounded source-correct provider/
runfiles construction, or new semantic identity/ownership decision. Routine
compiler/invocation/test corrections stay in packet. Full-rule/loading/binary
gates remain separate; this prerequisite does not prove bootstrap.

## Acceptance receipt

Accepted against b56eaf634 after independent design and final ACCEPT. Native
filegroup configured analysis now supplies genuine providers to the existing
graph and public ctx consumers within the exact slice above. No new retained
representation, graph key or execution path; full cc_toolchain remains unproved.

Pinned nightly-2025-09-14, offline/locked serial preparation: Configuration
0.95s, Analysis lib4.02s, integration7.55s, Commands4.46s; Core/Query/CLI
dependent check18.09s. Each final Cargo JSON receipt reports build-finished
success. Final exact preflight/runtime receipt:39 passed (Configuration2,
Analysis unit7, integration/protected29 including7 native-filegroup tests,
Commands1),3.558s aggregate runtime, slowest0.627s. Receipt and executable
hashes: target/wp779/final-runtime.log; compile receipts alongside it (ignored
local artifacts). Earlier compile/assertion corrections were superseded by
these final runs, with no waived gate. Independent final review verified receipt
hashes and accepted the actual diff. Formatting, fixture provenance inspection,
diff, archive and plan-status checks pass. No CLI/Bazel/daemon or transport run
is claimed. Branch-to-accept elapsed approximately37 minutes, including user
installation follow-up and review; no performance improvement claim.
