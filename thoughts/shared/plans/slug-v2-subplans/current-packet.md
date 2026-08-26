# Current Slug V2 Packet

Packet: `WP-4-7A-bazel-config-bool-false-loading`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owners: loading-owned typed build-setting declaration and frozen rule schema
Base: `9685d9a7`

Result: load and freeze selected `bazel_skylib@1.8.2`
`rules/common_settings.bzl:89-105`. Complete the existing Boolean build-setting
descriptor by retaining the named `flag` bit for both true and false forms,
keep BUILD absence, and continue rejecting boolean target invocation before
package recording. Do not evaluate a build setting, parse its CLI value,
execute an implementation or advance analysis.

## Accepted starting point and first absent fact

Commit `9685d9a7` accepts both integer declarations at lines 69-87. It retains
INTEGER kind and flag polarity through rule freeze, derives the mandatory
integer default schema, keeps BUILD absence and fails integer target invocation
closed. The following `bool_flag` declaration at lines 89-96 already uses the
accepted `config.bool(flag = True)` surface and remains lazy.

The next declaration, `bool_setting`, reaches `config.bool()` at line 100.
Slug's `.bzl` Config method currently rejects omitted or false `flag` values,
and its retained `BuildSettingKind::Boolean` cannot distinguish flag polarity.
This is the first absent evaluated expression. No earlier load, mapping,
attribute, doc, schema or implementation surface is missing.

## Selected source provenance

The root's locked module graph selects `bazel_skylib@1.8.2`. Preserve these
authenticated inputs from the accepted route:

- BCR source JSON SHA-256:
  `34a3c8bcf233b835eb74be9d628899bb32999d3e0eadef1947a0a562a2b16ffb`;
- archive SHA-256:
  `6e78f0e57de26801f6f564fa7c4a48dc8b36873e416257a92bbb0937eeac8446`;
- `rules/common_settings.bzl` SHA-256:
  `f3bcedef4b2b2cbe9750d61852917954499c4ba5e83d79fb975ec5814eb76d20`.

The child has no recursive loads. Reuse the accepted selected-source route and
module fingerprint owner. Add no network oracle, fixture, source observer,
repository mapping or materializer.

## Bazel authority and Zabel architectural guidance

Pinned Bazel 9.2 commit
`8220c6198837d5c13d53fea211cf3282aa12408a` is the sole behavior authority.
`StarlarkConfigApi.boolSetting` declares one named-only Boolean `flag` with
default `False`; `StarlarkConfig.boolSetting` passes it to
`BuildSetting.create(flag, BOOLEAN)`. `RuleClass.Builder` derives mandatory,
nonconfigurable Boolean `build_setting_default` and optional nonconfigurable
string `help`, and disables later toolchain resolution for build-setting rules.

Admit all three equivalent ABI spellings in this packet:

- `config.bool(flag = True)`;
- `config.bool()`; and
- `config.bool(flag = False)`.

Omitted and explicit `False` have the same structural descriptor identity;
`True` differs. Positional, nonboolean, `None` and unknown arguments reject
through the typed Starlark method ABI. The selected Skylib source requires the
true and omitted identities. CLI parsing, configured values and implementation
access through `ctx.build_setting_value` are later consumers and are skipped.

Pinned Zabel commit
`c7298478e2e56262a2f438e9c065325744c9f0fc` is architectural guidance only.
Its `build_rule_declaration.zig` keeps `BuildSettingKind.bool` and `flag` in
one evaluator-free `BuildSettingDefinition`, then attaches that descriptor to
the declared rule. Slug follows only this producer-owned phase split. No Zig
code, layout, diagnostics, evaluator behavior, configured capture, cache or
analysis algorithm may be copied; Zabel supplies no behavior authority.

## Compatibility classification

- **Exact:** `.bzl` `config.bool` named-only ABI; named `True`, omitted and
  explicit `False`; BOOLEAN kind and flag polarity; omitted/false equality and
  true discrimination; mandatory nonconfigurable Boolean
  `build_setting_default`; optional nonconfigurable string `help`; recursive
  rule freeze, export identity and implementation laziness; BUILD absence.
- **Slug-native:** `RootBoolBuildSetting { flag }` and
  `BuildSettingKind::Boolean { flag }` representation; Rust equality/copying;
  existing source/module fingerprint over-invalidation; diagnostics;
  `Allocative`; fail-closed target invocation.
- **Unsupported/deferred:** boolean target invocation/default coercion;
  `ctx.build_setting_value`; flag CLI parsing; transitions and configured
  values; analysis, providers and actions; Bazel toolchain-resolution effects;
  `config.string_list()` with a false flag; later Skylib declarations;
  `attr.label_list(allow_files = True)`; M8/M7B and exact Bazel
  configuration/output identity.

## Natural owner, lifetime and utility reuse

Change the existing `.bzl`-only `RootBoolBuildSetting` into a small value that
owns `flag`. Project it immediately at `rule(build_setting = ...)` into
`BuildSettingKind::Boolean { flag }`. The compact Copy enum already flows
through transient `RuleDefinitionGen`, `FrozenRuleDefinition`, package
invocation and `StarlarkRuleImplementation` equality. Its schema projection
remains `AttributeKind::Boolean`.

No evaluator heap survives freeze: retain only the Boolean bit and enum
discriminant. Add no collection, string, Arc, interner, hash, cache or memory
owner. Preserve existing `Allocative` derivations. The Buck2-utility audit
selects the current compact enum/frozen owner and rejects any side registry or
imported utility.

No request overlay, source observation, DICE equality, async transfer or
command result changes. The accepted selected-source and module fingerprint
owners continue to invalidate declaration bytes. No fallback is introduced.

## Implementation boundary

1. Let `.bzl` `config.bool` accept named-only `flag: bool = False`; do not
   add it to `BuildFileConfigModule`.
2. Change `BuildSettingKind::Boolean` to `Boolean { flag: bool }` and preserve
   that bit through rule projection, freeze and semantic equality. Keep its
   attribute projection Boolean.
3. Reuse the existing builtin-schema builder for mandatory nonconfigurable
   Boolean `build_setting_default` and string `help`; add no parallel schema.
4. Move the unchanged Boolean rejection from the oversized `invoke` body into
   `reject_deferred_attribute_invocation`, beside the integer rejection, and
   delete only the old Boolean block from `invoke`. Match the kind without
   interpreting its flag and do not coerce a default. No other `invoke` edit is
   allowed.
5. Update the supported-build-setting diagnostic to include the admitted false
   Boolean form. Do not broaden string or string-list descriptors.
6. Add no configured consumer, provider, transition behavior, source route,
   registry, cache, DICE key, I/O or public API outside loading.

## Discriminating proof

- Accept named `True`, omitted and explicit `False`; reject positional,
  nonboolean, `None` and unknown arguments.
- Prove omitted and explicit false project to equal Boolean descriptor facts,
  while true differs.
- Recursively freeze and import source-shaped `bool_flag`, `bool_setting` and
  an explicit-false rule. Assert each export's Boolean kind, flag polarity,
  mandatory/nonconfigurable Boolean default schema and `help`; implementations
  must fail if called and remain lazy.
- Prove BUILD globals still cannot resolve `config.bool` while accepted BUILD
  string construction remains unchanged.
- Invoke true and false Boolean rules in separate repository-package cases;
  require the existing fail-closed diagnostic and absence of target recording.
- Keep integer, string, string-list, rule-doc and recursive-freeze proofs green.
  Add no fixture, oracle, network request or Bazel run.

## Allowlist and growth caps

Only these files may change from base `9685d9a7`:

| File | Base SHA-256 | Base lines | Final line cap | Purpose |
|---|---|---:|---:|---|
| `app/slug_loading_v2/src/package.rs` | `9a8a70b49d38489e09a83dd3878a60037b8d075b42ee5144c74ada01211faa49` | 5,799 | 5,834 | Boolean flag identity and exact legacy rejection extraction |
| `app/slug_loading_v2/src/host_package_load_tests.rs` | `d0d07a3c03078f7d16726ee82a4daf74015ffcbcab33422fc61d490b7a14d267` | 5,144 | 5,244 | ABI, recursive freeze/identity and fail-closed proofs |

Additions are capped at 35 production lines, 100 proof lines and 135 total.
Deletions do not buy addition budget. No newly added or touched function may
exceed 150 lines, except the existing oversized `invoke` body is exempt solely
for deleting its exact Boolean rejection block after moving that unchanged
check to the small helper. No other `invoke` line may change. `package.rs`
exceeds the 2,000-line review trigger, but the Config methods, retained
build-setting enum and rule projection are one cohesive declaration lifetime.

## Serial validation and review

Use one Cargo target directory and run commands serially:

```text
CARGO_TARGET_DIR=/tmp/slug-v2-core-runtime-target CARGO_BUILD_JOBS=1 cargo test -p slug_loading_v2 --lib config_bool
CARGO_TARGET_DIR=/tmp/slug-v2-core-runtime-target CARGO_BUILD_JOBS=1 cargo test -p slug_loading_v2 --lib typed_bazel_config_definitions
CARGO_TARGET_DIR=/tmp/slug-v2-core-runtime-target CARGO_BUILD_JOBS=1 cargo test -p slug_loading_v2 --lib bazel_config_typed_descriptors
CARGO_TARGET_DIR=/tmp/slug-v2-core-runtime-target CARGO_BUILD_JOBS=1 cargo test -p slug_loading_v2 --lib
CARGO_TARGET_DIR=/tmp/slug-v2-core-runtime-target CARGO_BUILD_JOBS=1 cargo test -p slug_loading_v2 --test build_file_loading
CARGO_TARGET_DIR=/tmp/slug-v2-core-runtime-target CARGO_BUILD_JOBS=1 cargo check --locked -p slug_core_v2
CARGO_TARGET_DIR=/tmp/slug-v2-core-runtime-target CARGO_BUILD_JOBS=1 cargo build -p slug_cli_v2
cargo fmt --check
git diff --check
scripts/v2_archive_status.sh
```

The `build_file_loading` suite may retain only its recorded stale `@external`
diagnostic-order failure; every other row must pass. Archive hygiene may report
only the known three retained thoughts paths plus active packet files. Recheck
hashes, physical/addition caps, allowlist and touched-function lengths before
review.

Because this changes retained semantic identity, terminal independent review
is mandatory before commit. It must verify source order/provenance, Bazel
named/default flag behavior, equality/discrimination, frozen lifetime, Boolean
schema, BUILD absence, pre-recording rejection, Zabel guidance-only use,
utility reuse, caps and deferred configured behavior.

## STOP / `REPLAN`

STOP and `REPLAN` if implementation requires a file outside the allowlist;
BUILD `config.bool`; accepting positional or nonboolean flag shapes; changing
integer/string/string-list breadth; Boolean invocation/default coercion, CLI
parsing, transition/configured/analysis/action behavior; a new schema owner,
raw evaluator value, collection, registry, interner, cache, DICE key, mapping,
source observer, I/O or async path; editing the oversized invoke body beyond
deleting the existing Boolean rejection block after its exact helper
extraction; Java/JVM work; Zabel code or behavior adoption; unpinned source; a
new fixture/oracle or network request; a cap violation; or a public
Skylib/rules_rust success claim.
After Boolean false declarations freeze, stop at
`common_settings.bzl:133` and audit `config.string_list()` separately.
