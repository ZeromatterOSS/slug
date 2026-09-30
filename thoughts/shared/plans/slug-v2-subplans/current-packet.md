# Current Slug V2 Work Packet

Packet: WP-7-169-m7a-save-feature-state-projection-r1
Status: accepted; M7A remains partial

## Outcome and compatibility

Baseline `8b74acf15` (WP868). Admit private zero-argument
`cpp.save_feature_state()` and typed `experimental_save_feature_state` command
admission. The native option, False default, conversion, exec carry, getter and
current private-caller policy are exact. Existing configuration and output
identity remain Slug-native. Enabled feature-state artifact/output-group
production, full query/bootstrap closure and M8 remain unsupported/deferred.
Windows stays minimal or explicitly unsupported under the user's platform scope.

Pinned Bazel 9.2 clean `8220c619`: CppConfigurationApi:136–137 declares the
zero-argument undocumented getter; CppConfiguration:934–937 checks the private
allowlist and :782–783 returns cppOptions.saveFeatureState. CppOptions:819–826
names `experimental_save_feature_state`, Boolean default False, AFFECTS_OUTPUTS;
builtin_exec_platforms:324 carries it to exec. CppConfigurationStarlarkTest:112/
128–129 denies ordinary callers. CcLibraryConfiguredTargetTest:309–316 and
cc_integration_test.sh:254–273 prove enabled feature-state files upstream; that
consumer/action behavior is outside this slice. Both Boolean projections remain
truthful. The unchanged rules_cc 0.2.17 cc_helper:115 checks False first; existing
source fixture and sources.json provenance are reused with no new copied asset.

## Owner and scope

The existing registry/native binding retains the False Boolean. Append
ExperimentalSaveFeatureState to NativeCommandOption, preserving prior ordinals.
Commands captures raw/negated ordered flags; Configuration's existing converter
owns their values. CppFragmentProjection eagerly validates and reads the existing
native Boolean. Loading checks the current private caller on every invocation,
including transferred methods retaining the original configuration. Existing
configured structural keys own source/option inputs and publication equality,
invalidation and restoration. The projection returns a scratch Boolean; retained
representations, owners, locks, caches and fragment layout are unchanged. No donor
or performance claim, hardcoded default, artifact declaration or action is added.

Allowlist: Configuration src/command.rs, native/cpp_fragment.rs and existing
native/cpp_start_end_lib_tests.rs; Loading src/analysis_fragments.rs and existing
analysis_fragments/toolchain_provider_tests.rs; Commands src/common.rs tests;
Analysis tests/subrule/implementation_deps.rs; manifest/canonical and Stage 6's
existing private-option-getter paragraph. Production owners remain cohesive with
small matching siblings; no touched production function exceeds 150 lines and no
production file exceeds 2,000. Estimates were production <30/proof <140 net Rust
lines; actual 18/108. Independent DESIGN and FINAL reviews cover the shared
command/projection/private-access boundary.

## Proof and validation

Configuration's existing matrix records explicit defaults, toggles away and
restores them, protects old True defaults and tests canonical bytes, projection,
config_setting, both exec paths and idempotence. Malformed absence/text/label
state fails eagerly. Loading's private-method matrix covers both values, zero
arity, allowed/forbidden current callers and transferred original configurations;
its crossed target/exec matrix includes the fifth independent Boolean. Commands'
public Cquery parse matrix distinguishes False default, positive True, negated
False and ordered raw flags; null, invalid, whitespace and negated-value forms
remain rejected. Analysis's existing provider/tuple proof covers all 32 flag
combinations, both routes/exec paths and same-DICE source/configuration restoration
while retaining old four flags and zero-action assertions.

Serial pinned Cargo JSON no-run preparation passed: Configuration 1.07s,
Loading 18.90s, Commands 11.44s, Analysis subrule 5.57s. Exact ordinary nonignored
preflight and runtime passed 11/8/2/6 tests, respectively, in
0.032/0.114/0.008/0.614s (27 total, 0.768s). Core check 8.26s and CLI rebuild
20.47s passed. Receipts are in target/wp869. Preparation/runtime/replay caps
60/20/180s are operation caps, not user limits. The authentic replay is needed to
prove the actual public output-group caller advances; Boolean tests alone cannot.

The unchanged authentic Rust-toolchain cquery ran in 76.673s, exit 2. It clears
save_feature_state and next stops at `native filegroup nondefault data is
unsupported`. All three original fixture hashes are unchanged. Actual process
checks before/after are empty; scoped formatting, diff, archive and plan-status
checks pass. This advances the consumer boundary without claiming query success.
Independent DESIGN and FINAL verdicts ACCEPT. Packet window: about 12:13–12:28
UTC on 2026-09-30, including review and validation.

REPLAN if the retained descriptor/exec mapping cannot express this option, access
policy must widen or a new feature-state owner is required. Do not suppress True
or fabricate feature-state files. Full enabled production/linking/bootstrap stays
unproved.

## Predecessor

WP868 `8b74acf15` accepted disabled coverage through the existing native empty
provider. Independent DESIGN/FINAL, 45 focused tests in 0.544s and Core/CLI passed.
Unchanged query in 76.917s, exit 2, reached the missing getter at cc_helper:115;
three original hashes and cleanup passed. Receipts: target/wp868. Enabled coverage
remains deferred.
