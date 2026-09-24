# Current Slug V2 Work Packet

Packet: WP-7-131-m7a-feature-flag-provider-declaration-r1
Status: ACCEPTED; M7A remains partial

## Outcome and compatibility

Clear the authentic rules_python 1.7.0 `python/private/pypi/flags.bzl:158`
Loading stop by exposing Bazel 9.2's `config_common.FeatureFlagInfo` provider
key to `.bzl` declarations. Exact selected behavior: `.bzl`-only field
availability, provider-key type/repr and singleton equality, distinctness from
a user `provider()` with the same exported name, and its ordered identity in
`attr.label(providers = [[BuildSettingInfo], [config_common.FeatureFlagInfo]])`
and `rule(provides = [...])`. Keep `config_common.toolchain_type` unchanged.
Slug-native representation and diagnostics use the existing built-in provider
key, structural `ProviderIdentity::Builtin`, and immutable provider-policy
slices. Configured membership/indexing through this key must fail closed;
constructor invocation must remain unsupported rather than fabricate a
provider. Deferred: `FeatureFlagInfo(value = ...)`, `config_feature_flag`
native rule/transition, configured provider instance and value/error/validity,
Python flag analysis, and any complete root-build claim. A later authentic
stop cannot widen this packet.

## Source, owner and decision

Pinned Bazel 9.2 source commit `8220c6198837d5c13d53fea211cf3282aa12408a`:
`ConfigStarlarkCommonApi.java:30-39` declares the field;
`ConfigStarlarkCommon.java:32-35` returns the singleton
`ConfigFeatureFlagProvider.STARLARK_CONSTRUCTOR`;
`ConfigFeatureFlagProvider.java:40-50,74-104,138-148` distinguishes native
provider-key identity, configured constructor/value and instance identity.
Its `ConfigFeatureFlagTest.java:131-231,295-313` exercises constructor,
indexing and value behavior in Analysis, which this packet does not admit.
The authenticated rules_python 1.7.0 source at
`python/private/pypi/flags.bzl:156-159` uses the key in a top-level nested
provider predicate; `env_marker_setting.bzl:88-101` additionally uses it in
top-level label and rule-provides declarations. Calls and membership/value
reads in those sources are under configured implementation functions. The
selected `flags.bzl` SHA-256 is
`75634214e292a5af5fd61f8ab9d44fa4b28b99c4c49672de577e17922616d070`.
An ignored-RC/no-fetch Bazel 9.2 query of
`@@rules_python+//python/config_settings:_pip_env_marker_default_config`
exits 0 and shows the declaration stack through `flags.bzl:95,116,148`;
this proves Loading, not configured provider behavior.

`ConfigCommonModule` in `app/slug_loading_v2/src/package.rs` owns the field
exposure. Reuse `BuiltinProviderKey::new("FeatureFlagInfo")` in `provider.rs`
as a fixed Loading token, not a new provider store or callable. Its existing
hash/equality and `starlark_provider_identity` lower to
`ProviderIdentity::Builtin("FeatureFlagInfo")`; existing
`declaration_required_providers` and `advertised_provider_ids` retain it in
the same canonical Arc slices and package equality. Add a precise rejection
for that key in `configured_target_provider_identity` before map lookup; the
ordinary built-in keys keep their current behavior. The external Bzl source
evaluation and repository package-load DICE dependencies own invalidation;
root analogues remain unchanged. Add no DICE key/input, lock, registry, map,
interner, cache, source copy or evaluator borrow. Fixed token allocation is
Loading scratch/frozen-module ownership; no hot-path or clone-cost change.

## Evidence and validation

WP830's rebuilt normalized-PATH `slug build //:probe` is the prechange
discriminator: first error is absent `config_common.FeatureFlagInfo` at
`@@rules_python+//python/private/pypi:flags.bzl:158:47`. Replace the two
existing absence assertions for this field. Test field inventory and absence
from direct BUILD, fixed repr/type/equality/hash, ordinary-key and same-name
user-provider separation, direct and imported/re-exported use in nested
provider alternatives and `rule(provides)`, declaration order/dedup and frozen
identity. Prove changed source/provider policy A/B/A on one DICE. Test
constructor rejection and configured membership/indexing rejection, including
against a coincidentally named retained provider; a direct requested Build
must not turn the declaration into a configured success. Preserve protected
`config_common.toolchain_type` and ordinary provider controls.

Resolve the pinned nightly with `rustup which`. Compile selected test binaries
with Cargo JSON `--no-run`; run `scripts/v2_test_preflight.py` for exact
nonignored selectors before execution. Run focused Loading and configured
guard/Analysis tests, then direct checks for Loading, Analysis, Core and CLI.
Rebuild `slug_cli_v2` before one normalized-PATH authentic `//:probe` replay;
if parallel load order masks this gate, use a direct authentic rules_python
query as a positive trace. Clean/check `slugd` before and after. Run pinned
rustfmt, `git diff --check`, plan and archive checks. Obtain independent
DESIGN and FINAL reviews for the provider-key/retained-policy boundary. Reuse
the pinned source and no-fetch oracle; no new Stage 1 fixture.

## Scope and stops

Base clean main `c18e59646`. Allowlist: this manifest and canonical Current;
compact Stage 4 and matching Stage 9 notes after acceptance; production
`app/slug_loading_v2/src/{package,provider}.rs`; focused tests in
`app/slug_loading_v2/src/{host_package_load_tests,provider}.rs` and
`app/slug_analysis_v2/tests/starlark_rule.rs`; a direct Build test in
`app/slug_core_v2/src/runtime/tests/build_command_tests.rs`. Both production
modules are large but remain the cohesive `config_common` and provider-token
owners; a new module would add indirection without separating a semantic
owner. Expected growth about 30 production/160 proof lines is a review
trigger, not a cap. `REPLAN` if exact selected Loading needs a configured
provider constructor/value, a new retained schema or DICE owner, or a second
provider identity representation. No configured Python or JDK package
publication is in scope. M7A remains partial.

## Immediate predecessor

WP830 accepted at `c18e59646`: the JDK `java_plugins_flag_alias` BUILD call
now loads with its empty private default. Direct authentic JDK query advances
to later `proguard_allowlister` executable-rule validation; the wider
`//:probe` replay exposes this independent rules_python provider-key stop.

## Acceptance receipt (2026-09-24)

Independent DESIGN and FINAL reviews both `ACCEPT`. The selected `.bzl`
field and declaration identity are admitted exactly; configured use and
construction remain fail-closed and Slug-native as classified above. Pinned
Bazel 9.2 source plus the ignored-RC/no-fetch authentic rules_python query
establish the Loading boundary; no Stage 1 fixture was added. Six Loading,
two Analysis and two Core focused/protected tests pass, including imported
policy source A/B/A on one DICE and a direct requested Build that fails
Analysis rather than reporting false success. `cargo check` for Loading,
Analysis, Core and CLI, and `cargo build -p slug_cli_v2`, pass. Pinned
rustfmt, diff hygiene, plan and archive checks pass. No stale `slugd` process
remains. The rebuilt normalized-PATH authentic `slug build //:probe` clears
the former `flags.bzl:158` missing-field stop and next fails at
`@@rules_python+//python/private:config_settings.bzl:172:9` on the deferred
configured `config_common.FeatureFlagInfo(value = value)` call. It does not
establish a complete root build, configured Python flag, or M7A acceptance.
