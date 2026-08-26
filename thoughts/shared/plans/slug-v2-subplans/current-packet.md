# Current Slug V2 Packet

Packet: WP-4-7A-bazel-config-string-descriptor-loading
Milestone: M7A bootstrap-critical command/ruleset breadth
Owners: loading-owned typed build-setting declaration, frozen rule schema, and
the existing narrow root-string configured consumer
Base: 297c2286

Result: finish selected bazel_skylib@1.8.2
rules/common_settings.bzl:149-181. Complete the .bzl String build-setting
descriptor with flag and allow_multiple identity, load string_setting at line
172, preserve the existing BUILD-only true/single string constructor, and fail
unsupported string rule variants before target recording. Do not add non-flag
or multi-value configured semantics.

## Accepted starting point and source frontier

Commit 297c2286 accepts every valid StringList flag/repeatable descriptor and
the source-shaped non-flag list declaration at line 133. Lines 140-155 are a
lazy implementation body. string_flag at lines 157-168 uses the already
accepted config.string(flag=True), accepted StringList attributes, and lazy
implementation. string_setting evaluates config.string() at line 172, the
first absent expression.

Slug's .bzl Config method rejects false/omitted flag, accepts flag positionally,
and exposes no allow_multiple argument. RootStringBuildSetting and
BuildSettingKind::String are unit facts. This is insufficient for Bazel's
descriptor identity. The selected child ends after string_setting; once it
freezes, evaluation returns to rust/private/toolchain.bzl and must audit the
next loaded child separately before reaching that file's body.

## Source provenance

Reuse the selected bazel_skylib@1.8.2 route:

- BCR source JSON SHA-256:
  34a3c8bcf233b835eb74be9d628899bb32999d3e0eadef1947a0a562a2b16ffb;
- archive SHA-256:
  6e78f0e57de26801f6f564fa7c4a48dc8b36873e416257a92bbb0937eeac8446;
- rules/common_settings.bzl SHA-256:
  f3bcedef4b2b2cbe9750d61852917954499c4ba5e83d79fb975ec5814eb76d20.

Add no source route, mapping, observer, fixture, network oracle or
materializer.

## Bazel authority and Zabel architectural guidance

Pinned Bazel 9.2 commit
8220c6198837d5c13d53fea211cf3282aa12408a is sole behavior authority.
StarlarkConfigApi.stringSetting declares named-only Boolean flag and
allow_multiple, both defaulting to False. StarlarkConfig.stringSetting creates
BuildSetting(flag, STRING, allow_multiple, repeatable=false) without forbidding
any Boolean pair. RuleClass.Builder derives mandatory nonconfigurable String
build_setting_default and optional nonconfigurable String help.
StarlarkIntegrationTest testBuildSettingRule_flag,
testBuildSettingRule_settingByDefault, and
testBuildSettingRule_settingByFlagParameter authenticate flag polarity.
ConfigSettingTest.buildsettings_allowMultipleWorks and
StarlarkOptionsParsingTest.testAllowMultipleStringFlag authenticate the
distinct allow_multiple fact; their configured/CLI behavior is not admitted.

The complete declaration matrix contains four pairwise-distinct values:

- flag=True, allow_multiple=False;
- flag=False, allow_multiple=False, including both omitted and explicit false;
- flag=True, allow_multiple=True; and
- flag=False, allow_multiple=True.

Both arguments are named-only; positional, nonboolean, None and unknown forms
reject through the typed ABI.

Pinned Zabel commit c7298478e2e56262a2f438e9c065325744c9f0fc remains
architectural guidance only. Its evaluator-free BuildSettingDefinition keeps
String kind, flag and allow_multiple together before rule attachment. Slug
follows that single producer-owned phase split. No Zig code, layout,
diagnostic, configured behavior, cache or algorithm may be copied.

## Existing configured boundary

Slug already admits one narrow root string setting:
flag=True, allow_multiple=False. Loading records its String default; analysis
uses one structural RootStringSettingValue, supplies one scalar
ctx.build_setting_value, and permits the existing explicit override/transition
vertical. That behavior and Slug-native configuration identity remain
unchanged.

Definitions with flag=False must not enter the explicit CLI override path.
Definitions with allow_multiple=True would require list-valued parsing,
configuration identity, transition and ctx value changes. Reject both families
before PackageRecorder records a target. This lets common_settings declarations
freeze without widening configured behavior. The existing supported
true/single target remains recordable and analyzable.

BUILD currently exposes only the accepted Slug-native
config.string(flag=True) constructor. Bazel's config module is .bzl-only, but
removing this existing Slug surface is outside the packet. Preserve it exactly:
omitted/false remains rejected, positional behavior remains unchanged, and
allow_multiple remains unknown.

## Compatibility classification

- **Exact:** .bzl config.string named-only flag/allow_multiple ABI and false
  defaults; all four descriptor identities; omitted/explicit false equality;
  STRING kind; mandatory nonconfigurable String default and optional
  nonconfigurable help; recursive freeze/export identity and implementation
  laziness.
- **Slug-native:** RootStringBuildSetting { flag, allow_multiple } and
  BuildSettingKind::String { flag, allow_multiple }; Rust equality/copying;
  existing source fingerprint invalidation; preserved BUILD true/single
  constructor; fail-closed unsupported target variants; existing single-value
  root string configuration and diagnostics.
- **Unsupported/deferred:** non-flag string target invocation/analysis and CLI
  override; allow_multiple target invocation, list parsing, repeated values,
  transitions, ctx value, configured identity, providers and actions; Bazel
  toolchain-resolution effects; later source children; exact Bazel
  configuration/output bytes; M8/M7B.

## Natural owner, lifetime and utility reuse

Put flag and allow_multiple on RootStringBuildSetting, then project both
immediately into the existing compact Copy BuildSettingKind. The enum already
flows through transient/frozen rule definition and semantic equality. Adjust
is_root_string_build_setting to recognize only the already-supported
true/single variant.

No evaluator heap survives freeze. Retain only two Boolean bits and the enum
discriminant; add no string, collection, Arc, interner, hash, cache or DICE
key. Preserve Allocative. The Buck2-utility review selects the current compact
owner and no import. No Stage 9 ledger update is needed.

No request overlay, source observation, async transfer or command-result
change applies. Existing module/source fingerprints own invalidation. There is
no fallback.

## Implementation boundary

1. Change only .bzl Config.string to named-only flag and allow_multiple,
   defaulting both false, and return the complete descriptor.
2. Preserve BuildFileConfigModule.string exactly as true/single-only through
   the existing checked constructor; do not add allow_multiple there.
3. Change BuildSettingKind::String to retain both bits and map every String
   variant to AttributeKind::String.
4. Keep is_root_string_build_setting true only for
   flag=True/allow_multiple=False so the existing configured consumer cannot
   reinterpret other variants.
5. In reject_deferred_attribute_invocation, reject flag=False or
   allow_multiple=True before recording. Do not edit the oversized invoke
   body. Keep the supported true/single path unchanged.
6. Reuse the existing schema builder and update the supported-descriptor
   diagnostic. Add no configured, CLI, transition, provider, action, source,
   DICE, cache, I/O or public API behavior.

## Discriminating proof

- Accept all four .bzl Boolean pairs plus omitted/explicit false equality;
  reject positional, nonboolean, None and unknown arguments separately.
- Recursively freeze true/single, omitted false/false, explicit false/false,
  true/multiple and false/multiple definitions. Assert exact producer export,
  descriptor bits, String default/help schema and implementation laziness.
- Prove BUILD still accepts its existing true constructor but rejects omitted
  false and allow_multiple.
- Invoke false/single, true/multiple and false/multiple rules in separate
  packages; require the new pre-recording diagnostic and no target. Preserve
  the existing true/single package/default proof.
- Run the existing root-string cquery structural configuration test to prove
  the admitted configured path is unchanged.
- Keep integer, Boolean and StringList proof green. Add no fixture, Bazel run
  or network request.

## Allowlist and caps

Only these files may change from base 297c2286:

| File | Base SHA-256 | Base lines | Final cap | Purpose |
|---|---|---:|---:|---|
| app/slug_loading_v2/src/package.rs | 9a27cb7a69ae69ff50774707269e858286598afe5cb28d25b9d0c34312f14962 | 5,801 | 5,851 | String descriptor identity, BUILD preservation, configured gate |
| app/slug_loading_v2/src/host_package_load_tests.rs | 794b3ecd07166a8a1830b18db22e82f8ad28f3d731cfd33152ad714902045643 | 5,206 | 5,356 | ABI, freeze/schema/identity, BUILD and fail-closed proof |

Additions are capped at 50 production, 150 proof and 200 total. Deletions do
not buy addition budget. No new or touched function may exceed 150 lines.
package.rs exceeds 2,000 lines, but Config descriptors, retained kind,
projection and the small pre-recording gate are one cohesive lifetime. The
oversized invoke body is frozen.

## Serial validation

Use CARGO_TARGET_DIR=/tmp/slug-v2-core-runtime-target and CARGO_BUILD_JOBS=1:

- cargo test -p slug_loading_v2 --lib config_string
- cargo test -p slug_loading_v2 --lib bazel_config_typed_descriptors
- cargo test -p slug_loading_v2 --test build_file_loading rule_capabilities_use_exported_class_names_and_keep_native_rules_non_executable
- cargo test -p slug_core_v2 --lib cquery_restores_structural_configuration_and_display_projection
- cargo test -p slug_loading_v2 --lib
- cargo test -p slug_loading_v2 --test build_file_loading
- cargo check --locked -p slug_analysis_v2 -p slug_core_v2
- cargo build -p slug_cli_v2
- cargo fmt --check
- git diff --check
- scripts/v2_archive_status.sh

The broad integration may retain only its recorded stale @external
diagnostic-order failure. Archive hygiene may report only the known three
thoughts paths plus active files. Recheck hashes, caps, allowlist and function
sizes.

Retained semantic identity and the configured-consumer gate require
independent terminal review. Verify all descriptor identities, .bzl versus
BUILD ABI, exact schema, supported-path preservation, unsupported pre-recording
rejection, Zabel's guidance-only role, utility reuse, caps and deferred
multi-value/configured semantics.

## STOP / REPLAN

STOP and REPLAN for a file outside the allowlist; widening BUILD string;
allowing unsupported string variants to record; changing the admitted
true/single configured behavior; parsing flags or multiple values; changing
configuration/transition/ctx/provider/action semantics; a raw evaluator value,
new owner, collection, registry, cache, interner, DICE key, source route,
observation, I/O or async path; any invoke edit; Java/JVM work; Zabel code or
behavior adoption; unpinned source; a fixture/oracle/network request; cap
violation; or a broad Skylib/rules_rust success claim. After common_settings
finishes, stop and audit the next rust/private/toolchain.bzl loaded child
separately.
