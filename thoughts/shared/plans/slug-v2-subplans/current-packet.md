# Current Slug V2 Work Packet

Packet: WP-7-59-m7a-public-cc-feature-configuration-r1
Status: accepted

## Outcome and compatibility

Run unchanged rules_cc 0.2.17 create_cc_toolchain_config_info (including legacy
generation), native retained feature-model construction, and unchanged public
configure_features against a real DICE-analyzed rule context. WP758 at 3a57a616f
accepted the complete native model/selection and publication prerequisite with
22 tests and Core/CLI compilation; it did not admit this public entrypoint.

Exact: Bazel 9.2 feature-scope parsing/composition, admitted Cpp/configuration
projections, command features/host_features and exec remapping, and exec_os from
the default selected execution platform. Slug-native: valid-Unicode strings and
existing structural configuration, repository observation, platform and provider
identity. Host fallback uses structural Rust Host observations. Unsupported:
unresolved default execution platform, unadmitted platform parents, other repo()
package arguments, ObjC-only configuration projections, production FDO/toolchain
provider construction, feature expansion/tool lookup, compile/actions2ctx,
C++ action creation/execution and full Cargo/C++ loading. Existing native-model
WINDOWS/UNKNOWN rejection remains; exec_os itself may return either spelling.
M7A stays partial and M8 unproved.

## Source and semantic ownership

Pinned Bazel source 8220c6198837d5c13d53fea211cf3282aa12408a:
analysis/config/FeatureSet.java:35–108; analysis/RuleContext.java:269–276;
packages/PackageArgs.java:175–177,209–221; skyframe/PackageFunction.java:1211;
packages/PackageCallable.java:61–67 (one nonempty package call per BUILD);
analysis/starlark/StarlarkRuleContext.java:777–785; rules/cpp/
CcStarlarkInternal.java:496–501; analysis/constraints/ConstraintConstants.java:30–95;
rules/cpp/CppConfiguration.java:459,617–695,740–750,773,810,899–924;
analysis/config/CoreOptions.java features/host_features declarations and
src/main/starlark/builtins_bzl/common/builtin_exec_platforms.bzl features remapping.
Follow docs/developers/dice.md; no new DICE key, lock, cache or filesystem bypass.

Feature scopes are parsed separately: disabling beats enabling within a scope,
no_layering_check disables layering_check, and empty/leading-minus names follow
source parsing rather than native feature-model name validation. Repo defaults
are overridden by a single package call, then resolved rule attributes. Enforce
the source once-per-BUILD and nonempty-call guards in both package spellings;
repeated calls are rejected, not extra precedence scopes. Global disables win
all scopes; global enables lose to local disables. Output lists are sorted,
deduplicated fresh mutable copies. A mutated returned list cannot mutate ctx.

The existing observed RepoFile producers own typed repo features. BUILD package
source results (root and selected external/canonical routes) carry them with
their existing observation frontier and Need/error behavior; .bzl-only source
results do not acquire unrelated feature dependencies. PackageEvaluation retains
repo/package feature inputs and includes them in structural equality. Loading
owns pure scope normalization/composition using existing compact utilities.
Analysis consumes resolved rule features and the typed SlugConfiguration global
features; never reconstruct options from retained-owner identity bytes.

Command options are immutable structural occurrences in the existing typed
overlay. Admit features/host_features and the required Cpp booleans together with
source converter semantics. Both exec transition paths share the admitted projection:
features <- host_features; reset coverage, force_pic, apple_generate_dsym,
instrumentation paths and prefetch label to native defaults; propagate coverage
format and dont-enable-host/nonhost as pinned builtin_exec_platforms requires.
Preserve the other already-admitted transition behavior. Cpp projections
read only existing typed retained options: force_pic, apple_generate_dsym,
dont-enable-host/nonhost, LLVM coverage format, instrumentation paths and prefetch
label. ctx.configuration.coverage_enabled directly reads collect_code_coverage.
Do not infer it from presence of an output generator. Preserve existing profile
validation and private-caller restrictions; do not invent defaults.

exec_os receives an actual root ctx through a narrow Rust Starlark demand
capability, with active token checks. Analysis retains the existing default
ConfiguredActionOwnerContext and cheap structural configuration handle. The
projection consumes selected canonical OS setting/value labels, not target CPU,
path flavor, platform name, arbitrary exec group or ambient std::env. A missing OS
setting alone uses structural ActionEnvironmentHostOs; unfamiliar OS value yields
UNKNOWN. Missing selected platform or missing required Host state fails closed.
No evaluator-extra shortcut accepting an authored struct as ctx.

New memory is compact DICE-retained package/source semantic data or phase-local
context projections. Arc/CompactString and small ordered maps/sets follow Stage 9
existing utility dispositions. No borrowed heap/scratch escapes publication.
Normal DICE equality/invalidation/cancellation releases apply, including repo-only
feature edits/restoration. No new digest domain, interner or performance claim.

## Scope and coordination

Package worker: app/slug_bzlmod_v2/src/repo_file.rs, host_package.rs and focused
child tests; app/slug_loading_v2/src/package.rs, bzl_module.rs, lib.rs and new
package_features.rs/child tests. Existing host_package_load_tests.rs gets include
dispatch only; host_package_attempt_tests.rs gets its required input and retry
retention discriminator. Retain/validate repo and package feature lists,
propagate observed inputs through existing package sources, and implement pure
feature composition. Do not route feature semantics through IgnoreMatcher.
tests/build_file_loading.rs merges the one existing repeated-package fixture
into one call while preserving its loaded-Label parameter assertions.

Configuration worker: app/slug_configuration_v2/src/command.rs, native/
configuration.rs, cpp_fragment.rs, mod.rs and focused child tests; lib.rs exports
only if needed; app/slug_loading_v2/src/analysis_fragments.rs and new child tests.
Admit typed options, exec remapping, Cpp projections, coverage and Host getters.
app/slug_commands_v2/src/common.rs limited to feature option binder regressions.

Root: app/slug_analysis_v2/src/starlark_rule.rs and new cohesive child module;
app/slug_loading_v2/src/cc_common.rs and new narrow context-capability module;
Loading lib.rs export coordinated with package worker. New
app/slug_analysis_v2/tests/cc_configure_features/mod.rs plus registration in
tests/starlark_rule.rs; cargo_runfiles_tests.rs gets only its required context
initializer. BUILD.bazel compile_data only as necessary. Scheduling
docs, bootstrap-readiness, Stage 4 package-default boundary note and Stage 9
architecture row only as changed.
No dependency/lockfile/source-corpus edits. Large existing files receive bounded
fields/dispatch; new semantic algorithms and tests belong in cohesive child modules.
Estimate 800–1,500 production and 700–1,200 test lines; growth triggers responsibility
review, not weakened admission. Root owns Cargo, integration and commit.

Independent reviewer checks design before implementation and final actual diff.
Any newly discovered prerequisite/ownership choice returns to root before editing.

Independent design review ACCEPT: complete scope ownership and both exec paths
are sound. Standard platform OS matching uses exact @@platforms identity (production
selected-module routing already special-cases it); legacy platforms+ fixtures do
not establish the positive OS case. Both repository lanes require final evidence.

Reviewed fixture ownership adjustment: use the observed canonical-external
configured root, as in the accepted Rustc proof, for complete unchanged imports.
The preexisting root-origin local-override cross-package import restriction
remains deferred; no production loading workaround is added. Package/REPO edits
follow the subject's owning repository, while platform targets remain in root.
Platform positives use explicit @@platforms constraint labels. Fixture preparation
supplies both demanded generic-bootstrap platforms+ and selected platforms local
materialization identities; it never normalizes configured labels. Legacy apparent
@platforms normalization is not claimed by this exact canonical OS projection.

## Evidence and validation

Source-derived regressions cover contradictory scopes/order/alias/edge names,
fresh mutable ctx lists, resolved configurable rule features, package and repo
feature-only equality/invalidation/restoration, feature occurrence conversion and
target/exec host mapping. Selected-platform tests cover canonical label matching,
missing versus unfamiliar OS setting/value, target versus exec OS, unresolved
platform failure and structural Host/platform/configuration changes.

Authentic wrapper proof reuses exactly eight existing bodies: rules_cc action_names,
cc_toolchain_config_lib, common/semantics, private/cc_internal, private/toolchain_config/
{cc_toolchain_config_info,legacy_features,configure_features}, plus skylib paths.
rules_cc archive SHA256
283fa1cdaaf172337898749cf4b9b1ef5ea269da59540954e51fba0e7b8f277a.
Do not set no_legacy_features. An explicitly authored capability/FDO envelope around
the authentic native model is a boundary input, not a production toolchain/FDO
provider claim. Cover mode/default/request/unsupported selection, header/module-map
gates, PIC error, coverage format, host/nonhost policy, FDO/CSFDO and profile-mode/
Propeller/prefetch branches as supported by these projections. ObjC remains an
explicit unsupported facade boundary. Verify same-DICE structural configuration
and package changes affect published selected/requested membership.

Public/cross-crate plus DICE/platform tier: pinned toolchain, serialized Cargo
--offline --locked --no-run JSON preparation for Configuration, Bzlmod, Loading,
Analysis and Commands affected harnesses; preflight all exact selectors once per
unchanged binary, then focused tests. Check Core and CLI direct dependents. No
Bazel/Slug binary, daemon or build action needed; no broad suite. Preparation cap
60s per operation; runtime expected seconds. These are packet caps/estimates, not
new user ceilings. Root may partition compilation on an observed cap, not silently
raise it. Format, diff check, plan status and archive status required.

REPLAN only if source contradicts the compatibility contract or a required semantic
fact has no bounded owner in these existing paths. Ordinary implementation,
invocation and test-selection corrections stay within the packet. Acceptance needs
independent final review, compact gate/elapsed receipt and an atomic commit; do not
land partial feature activation across repository/configuration/context owners.

## Implementation and validation receipt

The complete scoped path is implemented. Typed repo/default features propagate
through existing observed root, local-external and canonical package producers,
with source equality/frontiers and retry restoration preserved. A single package
scope plus resolved rule/global features feeds fresh mutable ctx lists. Typed Cpp
and coverage fields, both exec mappings and an active-root-ctx native capability
read structural configuration/default-platform facts. The public wrapper proof
executes all eight unchanged sources with legacy generation enabled. No production
action, foreign-language runtime, source corpus, dependency, DICE key/lock/cache or digest
domain was added. New public attributes are ordinary non-callable values; this
avoids the evaluator's native-attribute direct-call path invoking getters as methods.

Pinned nightly-2025-09-14, offline/locked Cargo JSON preparation and exact preflight
establish 30/30 focused passes: Configuration 4, Bzlmod 2, Loading 10, protected
BUILD parameter fixture 1, Analysis OS/lifetime plus protected Cargo capture 3,
Commands 1, and authentic public wrapper/context-boundary integration 9. The final
integration receipt is target/wp759/tests-1790019463674841317.receipt.json;
the final Loading receipt is tests-1790018934335366973.receipt.json in that directory.
All commands/logs and earlier failed receipts remain under target/wp759.
Latest successful runtimes sum to 0.362s. Core check passes in 16.842s and CLI
check in 17.294s. Preparation/check total including corrections is 254.223s;
the maximum single operation is 30.019s, below the packet's 60s cap. No Slug/Bazel
binary, daemon, build action or broad suite was invoked.

Three ordinary compile corrections added the CompactString import, converted
Starlark errors to anyhow, and supplied a protected test's new context field.
Source review corrected dict-key conversion and package once/nonempty semantics;
focused negative tests exposed and closed native-attribute callability. Fixture
corrections supplied complete observed shards, the existing closed module setup,
required repo observations/module mappings and the reviewed canonical-external
route; no assertion was weakened to admit missing semantic input. The late PIC
negative explicitly suppresses the authentic legacy enabled-pic default. A label
test observes str(label) instead of asserting an unrelated debug representation.
Ten failing test invocations are preserved with their attributed corrections.

Responsibility review: approximately 750 net production and 1,715 net test lines.
Test growth exceeds the 700–1,200 estimate because complete wrapper branches,
three repository observation lanes and lifecycle/OS/private-call discriminators
are separate gates. They remain cohesive child modules; large owners receive
narrow dispatch/fields. All eight source bodies are required load closure, and
unused proof transport fields were removed. No performance claim is made.

Format, diff, plan-status and archive checks pass. Packet elapsed approximately
46 minutes (18:55–19:41 UTC), including research and overlapping design/final
reviews. Gate advanced: unchanged public C++ feature configuration executes on
owned structural inputs and survives DICE publication/invalidation. Production
toolchain/FDO construction, expansion/tool lookup and compile/actions2ctx remain
next boundaries; M7A stays partial and M8 unproved. No semantic REPLAN was needed.

Independent final review ACCEPT: final source, canonical fixture ownership and
closure claims match the bounded contract; all 30 focused gates and Core/CLI
checks are verified. Deferred production toolchain/FDO, expansion, compilation,
execution and loading/mapping boundaries remain explicit.
