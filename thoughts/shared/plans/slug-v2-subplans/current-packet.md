# Current Slug V2 Packet

Packet: WP-4-7A-bazel-config-string-list-false-loading
Milestone: M7A bootstrap-critical command/ruleset breadth
Owners: loading-owned typed build-setting declaration and frozen rule schema
Base: 52d2c6f2

Result: load and freeze selected bazel_skylib@1.8.2
rules/common_settings.bzl:107-138. Complete the existing StringList
build-setting descriptor by retaining flag beside repeatable, accept the
non-flag declaration, preserve BUILD absence, and continue rejecting list
target invocation before package recording. Do not parse CLI values, execute
implementations or advance configured analysis.

## Accepted starting point and first absent fact

Commit 52d2c6f2 accepts both Boolean declarations through line 105, retaining
true versus omitted/false flag identity. The following string_list_flag at
lines 107-114 and repeatable_string_flag at lines 119-129 use already-admitted
true-flag StringList descriptors. Their implementations remain lazy.

string_list_setting then evaluates config.string_list() at line 133. Slug
currently rejects omitted or false flag, and BuildSettingKind::StringList
cannot distinguish flag polarity. This is the first absent evaluated
expression. After this declaration the helper body at lines 140-155 stays
lazy, string_flag uses the accepted true form and accepted string-list
attribute schema, and the next absent expression is config.string() at line
172.

## Selected source provenance

The root's locked graph selects bazel_skylib@1.8.2. Reuse these accepted
inputs:

- BCR source JSON SHA-256:
  34a3c8bcf233b835eb74be9d628899bb32999d3e0eadef1947a0a562a2b16ffb;
- archive SHA-256:
  6e78f0e57de26801f6f564fa7c4a48dc8b36873e416257a92bbb0937eeac8446;
- rules/common_settings.bzl SHA-256:
  f3bcedef4b2b2cbe9750d61852917954499c4ba5e83d79fb975ec5814eb76d20.

The child has no recursive loads. Reuse the accepted selected-source route and
module fingerprint. Add no fixture, network oracle, source observer,
repository mapping or materializer.

## Bazel authority and Zabel architectural guidance

Pinned Bazel 9.2 commit
8220c6198837d5c13d53fea211cf3282aa12408a is the sole behavior authority.
StarlarkConfigApi.stringListSetting declares named-only Boolean flag and
repeatable, both defaulting to False. StarlarkConfig.stringListSetting rejects
repeatable=True unless flag=True, then calls
BuildSetting.create(flag, STRING_LIST, false, repeatable). RuleClass.Builder
derives mandatory nonconfigurable StringList build_setting_default, optional
nonconfigurable string help, and the later toolchain-resolution opt-out.
Pinned ConfigSettingTest.buildsettings_repeatableWithoutFlagErrors
authenticates the exact invalid-combination diagnostic; its repeatable tests
authenticate the distinct true/false repeatability facts.

Admit the complete valid declaration matrix:

- config.string_list(flag = True);
- config.string_list(flag = True, repeatable = False);
- config.string_list(flag = True, repeatable = True);
- config.string_list(); and
- config.string_list(flag = False) or explicit false/false.

Omitted and explicit false/false descriptors are equal. True/false, true/true
and false/false are pairwise distinct. Positional, nonboolean, None, unknown
arguments and false-flag/true-repeatable reject. Configured values, CLI
accumulation and implementation access are later consumers.

Pinned Zabel commit c7298478e2e56262a2f438e9c065325744c9f0fc is
architectural guidance only. Its build_rule_declaration.zig keeps StringList
kind, flag and repeatable together in one evaluator-free
BuildSettingDefinition. Slug follows only that producer-owned phase split. No
Zig code, layout, diagnostic, evaluator behavior, configured capture, cache or
analysis algorithm may be copied; Zabel supplies no behavior authority.

## Compatibility classification

- **Exact:** .bzl config.string_list named-only ABI and defaults; the valid
  flag/repeatable matrix; invalid false/true relation and diagnostic; retained
  STRING_LIST kind, flag and repeatability identity; mandatory nonconfigurable
  list default and optional nonconfigurable help; recursive freeze/export
  identity; implementation laziness; BUILD absence.
- **Slug-native:** RootStringListBuildSetting { flag, repeatable } and
  BuildSettingKind::StringList { flag, repeatable }; Rust equality/copying;
  existing source/module fingerprint over-invalidation; nonrequired
  diagnostics; Allocative; fail-closed target invocation.
- **Unsupported/deferred:** StringList target invocation/default coercion;
  ctx.build_setting_value; CLI comma parsing/repeatable accumulation;
  transitions, configured values, analysis, providers and actions; toolchain
  effects; config.string() with false flag; later Skylib declarations;
  attr.label_list(allow_files = True); M8/M7B and exact Bazel
  configuration/output identity.

## Natural owner, lifetime and utility reuse

Add flag to the existing .bzl-only RootStringListBuildSetting and project both
bits immediately at rule(build_setting = ...) into the existing compact Copy
enum. That enum already flows through transient rule definition, recursive
freeze, invocation and StarlarkRuleImplementation equality. Its schema
projection remains AttributeKind::StringList.

No evaluator heap survives freeze: only two Boolean bits and the enum
discriminant remain. Add no collection, string, Arc, interner, hash, cache or
memory owner. Preserve Allocative. The Buck2-utility review selects the
existing compact enum/frozen owner and no imported utility. No Stage 9 ledger
change is required because no utility is imported and the accepted retained
runtime/compact-owner decisions remain unchanged.

No request overlay, source observation, DICE key/equality, async transfer or
command result changes. Existing source/module fingerprints continue to
invalidate declaration bytes. There is no fallback.

## Implementation boundary

1. Let .bzl config.string_list accept named-only false/omitted flag when
   repeatable is false; keep it absent from BuildFileConfigModule.
2. Retain both fields in RootStringListBuildSetting and
   BuildSettingKind::StringList. Preserve both through rule projection, freeze
   and equality; keep the schema projection unchanged.
3. Reject false-flag/true-repeatable with Bazel's pinned diagnostic before
   allocating the descriptor.
4. Reuse the existing builtin schema and wildcard StringList pre-recording
   rejection. Do not edit the oversized invoke body or coerce a default.
5. Update the supported-build-setting diagnostic to name the admitted
   StringList false form. Do not broaden string descriptors.
6. Add no configured consumer, provider, transition behavior, source route,
   registry, cache, DICE key, I/O or public API outside loading.

## Discriminating proof

- Accept every valid flag/repeatable combination; reject positional,
  nonboolean, None, unknown and false/true forms, including the exact
  invalid-combination diagnostic.
- Prove omitted and explicit false/false equality and pairwise discrimination
  from true/false and true/true.
- Recursively freeze source-shaped string_list_flag,
  repeatable_string_flag, string_list_setting and explicit false/false. Assert
  kind/bits, mandatory nonconfigurable list default, optional nonconfigurable
  help, export identity and implementation laziness.
- Prove BUILD still cannot resolve config.string_list.
- Invoke true and false list rules in separate repository-package cases;
  require the existing fail-closed diagnostic before target recording.
- Keep integer, Boolean, string, docs and recursive-freeze proofs green. Add no
  fixture, oracle, network request or Bazel run.

## Allowlist and growth caps

Only these files may change from base 52d2c6f2:

| File | Base SHA-256 | Base lines | Final line cap | Purpose |
|---|---|---:|---:|---|
| app/slug_loading_v2/src/package.rs | d0e7ec88ef755f2f645920a34e04124568ffe58b3838ac08b04847fcdf0e0f2c | 5,799 | 5,829 | StringList flag identity and invalid-pair validation |
| app/slug_loading_v2/src/host_package_load_tests.rs | 04c30c1ca1ff4f2e9dd6be2965bfca2acf839f581f238782cad58202cc9417b4 | 5,173 | 5,273 | ABI, freeze/identity/schema and fail-closed proofs |

Additions are capped at 30 production lines, 100 proof lines and 130 total.
Deletions do not buy addition budget. No new or touched function may exceed
150 lines. package.rs exceeds the 2,000-line review trigger, but its Config
methods, retained build-setting enum and projection are one cohesive
declaration lifetime; splitting them would create a second owner. The existing
oversized invoke body is frozen.

## Serial validation and review

Run these commands serially with
CARGO_TARGET_DIR=/tmp/slug-v2-core-runtime-target and CARGO_BUILD_JOBS=1:

- cargo test -p slug_loading_v2 --lib config_string_list
- cargo test -p slug_loading_v2 --lib typed_bazel_config_definitions
- cargo test -p slug_loading_v2 --lib bazel_config_typed_descriptors
- cargo test -p slug_loading_v2 --lib
- cargo test -p slug_loading_v2 --test build_file_loading
- cargo check --locked -p slug_core_v2
- cargo build -p slug_cli_v2
- cargo fmt --check
- git diff --check
- scripts/v2_archive_status.sh

The broad integration may retain only its recorded stale @external
diagnostic-order failure. Archive hygiene may report only the known three
retained thoughts paths plus active packet files. Recheck hashes, caps,
allowlist and function sizes before review.

This retained semantic-identity change requires independent terminal review.
Verify source order/provenance, Bazel default/relation behavior, identity,
frozen lifetime, schema, BUILD absence, pre-recording rejection, Zabel's
guidance-only role, utility reuse, caps and deferred configured behavior.

## STOP / REPLAN

STOP and REPLAN for a file outside the allowlist; BUILD config.string_list;
accepting invalid false/true or positional/nonboolean forms; changing
integer/Boolean/string breadth; target/default/CLI/configured/transition/
analysis/action behavior; a new schema owner, raw evaluator value, collection,
registry, interner, cache, DICE key, source route, observation, I/O or async
path; any invoke edit; Java/JVM work; Zabel code or behavior adoption; unpinned
source; a new fixture/oracle/network request; cap violation; or a public
Skylib/rules_rust success claim. After the non-flag list declaration freezes,
stop at common_settings.bzl:172 and audit config.string() separately.
