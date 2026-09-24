# Current Slug V2 Work Packet

Packet: WP-7-132-m7a-configured-feature-flag-info-r1
Status: ACCEPTED; M7A remains partial

## Outcome and compatibility

Implement the Bazel 9.2 configured constructor slice exposed by the authentic
rules_python 1.7.0 `config_settings.bzl:172:9` stop, with one coherent
Starlark-created `config_common.FeatureFlagInfo` slice. Exact selected
behavior: a fresh instance from one required string `value` argument,
positional or named; `type`, simple-string repr, `.value`, `.error == None`,
`.is_valid_value(string) == True`, within-evaluation instance-identity
equality/hash; direct return/forwarding from Starlark rule implementations;
configured-target membership/indexing by the singleton built-in key; and
`config_setting(flag_values)` matching the returned feature provider before
the raw build-setting value. Ordinary Starlark-produced string providers
compare verbatim. Preserve Bazel's label-valued build-setting canonicalization
when native `label_flag` forwards an ordinary target's FeatureFlagInfo: its
BuildSettingProvider classifies the requested flag, not the forwarded actual
target. Starlark `config.label` declarations remain deferred. No fabricated
provider may satisfy a declaration or select. The typed retained Arc and
structural DICE publication cutoff are explicitly Slug-native, with
within-evaluation occurrence/alias identity preserved. Diagnostics and Rust
Unicode string representation are Slug-native. Native `config_feature_flag`,
its transition, finite allowed-value/error states, nested FeatureFlagInfo
transport inside other AnalysisValues, and a complete Python/root build are
unsupported/deferred. M7A remains partial.

## Source, owner and decision

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`:
`ConfigFeatureFlagProvider.java:40-50,56-104,116-148` defines the singleton,
constructor, string value/null error/always-valid instance and reference
equality; `ConfigFeatureFlagProviderApi.java:31-57` exposes fields/method;
`Param.java:94` makes the named argument also positional by default.
`ConfigSetting.java:648-681,816-831` canonicalizes label build settings and
tests FeatureFlagInfo before the raw setting. `LabelBuildSettings.java:93-128`
and `LateBoundAlias.java:66-92` show that native `label_flag` retains a label
BuildSettingProvider while forwarding the actual target's providers.
`ConfigFeatureFlagTest.java:131-231`
proves a Starlark-produced provider drives `flag_values` and target lookup;
native validity tests at :235-313 are outside the slice. Authentic rules_python
1.7.0 `config_settings.bzl:161-205` constructs, reads and returns the key.
An ignored-RC/no-fetch Bazel 9.2 `cquery` of authentic
`@@rules_python+//python/config_settings:python_version` reports type
`FeatureFlagInfo`, current `value`, `error=None`, and true validity;
the 3.12 override changes value to 3.12. Its derived major/minor target
reports 3.12 under a 3.12.2 override. Assert output, not merely exit status:
the Starlark formatter can emit ERROR with exit 0. Reuse this source/oracle
evidence, not a new Stage 1 fixture.

The Loading owner replaces only the `config_common.FeatureFlagInfo` field's
generic declaration key with a dedicated callable singleton of the same
Provider type, display and built-in identity; other `BuiltinProviderKey`
values, notably `PackageSpecificationInfo`, stay noncallable. A dedicated
Loading Starlark value wraps a pointer-sized, Allocative, Dupe
`Arc<CompactString>` FeatureFlagInfo occurrence. Existing provider
collection ownership gains a typed variant; reject a generic
`ProviderOccurrence::Builtin("FeatureFlagInfo")` before required-provider
validation can mistake it for a real instance. Direct returned/forwarded
instances pass through existing Starlark rule publication and configured
target materialization; nested values fail closed. Existing
`ConfiguredNodeAnalysisKey`, source observations, package/Bzl load inputs and
structural configuration own invalidation. `label_flag::predicate` consumes
the already-tracked configured child provider first; its existing declaration
and option path remains fallback when the typed provider is absent. A
malformed builtin-key occurrence is an error, never a fallback. No new DICE
key/input, lock, map, interner, cache, global registry, evaluator borrow or
untracked direct-filesystem read.

## Evidence and validation

WP831's rebuilt normalized-PATH `slug build //:probe` stops exactly at the
constructor. Focused tests must discriminate positional/named constructor
success, missing/duplicate/extra/non-string arguments, two same-value
instances being distinct, singleton constructor rematerialization, frozen
identity, type/repr/value/error/validity, and other built-in keys unchanged.
Test typed direct return/forward and configured membership/indexing, missing
key, same-name user key, advertised-only and forged generic-builtin
nonmatches, duplicate provider rejection, nested-value failure, and direct
requested Build without false success. Test `config_setting` provider-first
selection against a deliberately different raw string setting/default,
ordinary rule provider with literal relative/mapped-label contrasts, native
`label_flag` forwarding FeatureFlagInfo with mapped-label canonicalization,
alias normalization, and existing native label-setting fallback.
Prove source/value and configuration A/B/A through one DICE; preserve
protected WP831 Loading, Analysis and Core cases, and existing ordinary
build-setting/label-flag controls.

Resolve the pinned nightly with `rustup which`. Serialize Cargo commands;
compile exact selected Rust test executables with Cargo JSON `--no-run`,
preflight their named nonignored selectors, then run focused Build API,
Loading, Analysis and Core tests. Run direct `cargo check` dependents and
rebuild `slug_cli_v2` before a normalized-PATH authentic `//:probe` replay.
Check/clean `slugd` before and after. Run pinned rustfmt, `git diff --check`,
plan and archive checks. Obtain independent DESIGN and FINAL reviews for the
new retained/public provider boundary. Record the next authentic stop without
widening this packet.

## Scope and stops

Base clean main `0f5e698b7`. Allowlist: this manifest and canonical Current;
compact Stage 4/9 owner notes after acceptance; new
`app/slug_build_api_v2/src/feature_flag_info.rs`,
`app/slug_loading_v2/src/provider/feature_flag_info.rs`;
`app/slug_build_api_v2/src/{lib,providers/mod}.rs`;
`app/slug_loading_v2/src/{package,provider}.rs`;
`app/slug_analysis_v2/src/{analysis_value,starlark_rule}.rs` and
`app/slug_analysis_v2/src/dice/label_flag.rs`; focused tests in the new
modules, `app/slug_loading_v2/src/host_package_load_tests.rs`,
`app/slug_analysis_v2/tests/starlark_rule.rs`,
`app/slug_analysis_v2/tests/native_label_flag/mod.rs`, and
`app/slug_core_v2/src/runtime/tests/build_command_tests.rs`. Large existing
modules remain the cohesive owners of provider publication and configured
predicate selection; the new small typed modules isolate occurrence logic.
Expected roughly 250-400 production and 250-400 proof lines is a review
trigger, not a cap. `REPLAN` if the selected behavior requires nested
AnalysisValue transport, native `config_feature_flag` transitions, a second
provider identity, an untracked semantic input, or a new DICE owner. Do not
claim complete Python, C++ or root bootstrap.

## Immediate predecessor

WP831 accepted at `0f5e698b7` the exact selected .bzl
`config_common.FeatureFlagInfo` Loading key and declaration identity.
Constructor and configured use intentionally failed closed; its authentic
replay reached this packet's `config_settings.bzl:172` frontier. M7A remains
partial.

## Acceptance (2026-09-24 17:16 UTC)

Independent DESIGN and FINAL reviews ACCEPT the bounded selected slice.
Seventeen exact focused/protected Build API, Loading, Analysis and Core tests
passed, including one-DICE source/configuration A/B/A, direct requested Build,
native `label_flag` forwarding a typed provider with mapped expected label,
alias-to-flag, and ordinary-provider literal contrasts. The final requested
label classifier edit was rebuilt, preflighted and rerun (1/1), followed by a
direct Analysis check. Cross-crate `cargo check` and rebuilt `slug_cli_v2`,
pinned rustfmt, diff hygiene, plan/archive checks and `slugd` cleanup pass.
Pinned Bazel 9.2 source and the authentic rules_python `cquery` remain the
oracle basis. The normalized-PATH authentic Slug `//:probe` replay did not
show WP831's constructor error; it first reported missing
`@@bazel_tools//tools/python` BUILD content. Parallel package loading does
not prove the constructor path executed, so this is an observed next error,
not a claim of authentic path passage or complete Python/root bootstrap.
An attempted direct external-target scratch replay was inconclusive and is
not counted as constructor evidence. Native `config_feature_flag`, Starlark
`config.label`, nested transport, complete Python/root build and M7A remain
deferred/open.
