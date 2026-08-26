# Current Slug V2 Packet

Packet: `WP-4-7A-bazel-config-int-loading`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owners: loading-owned typed build-setting declaration and frozen rule schema
Base: `1e2759c2`

Result: load and freeze selected `bazel_skylib@1.8.2`
`rules/common_settings.bzl:69-87`. Add the exact integer build-setting kind and
named flag identity to the existing `.bzl` Config/rule declaration owner,
derive the mandatory integer default schema, keep BUILD absence, and reject
integer target invocation before package recording. Do not evaluate a build
setting, parse its CLI value, execute an implementation or advance analysis.

## Accepted starting point and first absent fact

Commit `88304c2f` completes the fixed rustfmt-test dependency declaration.
Commit `1e2759c2` selects the post-rustfmt source-order audit. The audit proves
that `rust/private/rustfmt.bzl:281-356` finishes with already-admitted label
schemas and canonical toolchain strings while both implementations remain
lazy. The alias-only rust-analyzer wrapper also completes.

Recursive source order then enters `rust/private/toolchain.bzl` through
`rust/rust_stdlib_filegroup.bzl`. Its first child maps through the accepted
selected producer view to `bazel_skylib@1.8.2`; no new mapping or source owner
is required. The child has no recursive loads. Provider and attribute
declarations through line 69 are already supported or lazy. Slug's `.bzl`
Config module has string, bool and string-list methods but no `int`, so
`config.int(flag = True)` at line 71 is the first absent evaluated expression.
The adjacent `int_setting` uses `config.int()` at line 81.

## Selected source provenance

The root's locked module graph selects `bazel_skylib@1.8.2`. Preserve these
exact inputs:

- BCR source JSON SHA-256:
  `34a3c8bcf233b835eb74be9d628899bb32999d3e0eadef1947a0a562a2b16ffb`;
- archive SHA-256:
  `6e78f0e57de26801f6f564fa7c4a48dc8b36873e416257a92bbb0937eeac8446`;
- `rules/common_settings.bzl` SHA-256:
  `f3bcedef4b2b2cbe9750d61852917954499c4ba5e83d79fb975ec5814eb76d20`.

The source JSON names the Bazel Skylib 1.8.2 release archive with no strip
prefix. Reuse these locked bytes and the accepted selected-source route. Add no
network oracle, fixture, source observer, repository mapping or materializer.

## Bazel authority and Zabel architectural guidance

Pinned Bazel 9.2 commit
`8220c6198837d5c13d53fea211cf3282aa12408a` is the sole behavior authority.
Its `StarlarkConfigApi.intSetting` declares one named-only boolean `flag`
defaulting to `False`. `StarlarkConfig.intSetting` passes that bit to
`BuildSetting.create(flag, INTEGER)`. `RuleClass.Builder` reads the descriptor
type and adds mandatory, nonconfigurable integer `build_setting_default` plus
optional nonconfigurable string `help`; it also disables later toolchain
resolution for build-setting rules.

The selected Skylib child requires both flag identities. Admit all three
equivalent ABI spellings in one packet:

- `config.int(flag = True)`;
- `config.int()`; and
- `config.int(flag = False)`.

Omitted and explicit `False` have the same structural descriptor identity;
`True` differs. Positional, nonboolean, `None` and unknown arguments reject
through the typed Starlark method ABI. Bazel
`StarlarkAttrTransitionProviderTest` covers both integer setting forms and
integer defaults; `StarlarkOptionsParsingTest` distinguishes flag from
non-flag behavior. Reuse their declaration facts only. Transition execution,
configured values and command-line parsing are later phases and are skipped.

Pinned Zabel commit
`c7298478e2e56262a2f438e9c065325744c9f0fc` is architectural guidance only.
Its `build_rule_declaration.zig` keeps `BuildSettingKind.int` and `flag` in one
evaluator-free `BuildSettingDefinition`, then attaches that definition to the
declared rule. Slug follows only this producer-owned phase split. No Zig code,
layout, diagnostics, evaluator behavior, configured capture, cache or analysis
algorithm may be copied; Zabel supplies no behavior authority.

## Compatibility classification

- **Exact:** `.bzl` `config.int` named-only ABI; named `True`, omitted and
  explicit `False`; INTEGER kind and flag polarity; omitted/false equality and
  true discrimination; mandatory nonconfigurable Integer
  `build_setting_default`; optional string `help`; recursive rule freeze,
  first-export identity and implementation laziness; BUILD absence.
- **Slug-native:** `RootIntBuildSetting` and
  `BuildSettingKind::Integer { flag }` representation; Rust equality/copying;
  existing source/module fingerprint over-invalidation; diagnostics;
  `Allocative`; fail-closed target invocation.
- **Unsupported/deferred:** integer target invocation and default coercion;
  `ctx.build_setting_value`; implementation/provider execution; flag CLI
  parsing; transitions over integer settings; configured values, analysis and
  actions; Bazel toolchain-resolution effects; `config.bool()` without a true
  flag; later Skylib declarations; `attr.label_list(allow_files = True)`;
  M8/M7B and exact Bazel configuration/output identity.

## Natural owner, lifetime and utility reuse

Add a small `.bzl`-only `RootIntBuildSetting { flag }` beside the current
string/bool/list Starlark descriptor values. Project it immediately at
`rule(build_setting = ...)` into
`BuildSettingKind::Integer { flag }`. That compact Copy enum already flows
through transient `RuleDefinitionGen`, `FrozenRuleDefinition`, package
invocation and `StarlarkRuleImplementation` equality. Its `attribute_kind`
projection already has `AttributeKind::Integer` and the existing coercion
surface already owns signed 32-bit integers.

No evaluator heap survives freeze: only the boolean flag and enum discriminant
are retained. No new collection, string, Arc, interner, hash, cache or memory
owner is needed. Preserve existing `Allocative` derivations. The Buck2-utility
audit selects this current compact enum/frozen owner and rejects any side
registry or imported utility.

No request overlay, source observation, DICE equality, async transfer or
command result changes. The accepted selected-source and module fingerprint
owners continue to invalidate declaration bytes. No fallback is introduced.

## Implementation boundary

1. Add `.bzl` `config.int` with a named-only `flag: bool = False`; do not add it
   to `BuildFileConfigModule`.
2. Add `BuildSettingKind::Integer { flag }` and map it to
   `AttributeKind::Integer`. Preserve the flag through the existing rule
   projection, freeze and semantic equality paths.
3. Let the existing builtin-schema builder add mandatory nonconfigurable
   integer `build_setting_default` and string `help`; add no parallel schema.
4. Reject any frozen integer build-setting rule in the small existing deferred
   invocation helper before `PackageRecorder` can record a target. Do not touch
   the oversized invocation function or coerce a default.
5. Update the unsupported build-setting diagnostic to name the newly admitted
   integer descriptor. Do not broaden bool/string/list forms.
6. Add no configured consumer, provider, transition behavior, source route,
   registry, cache, DICE key, I/O or public API outside loading.

## Discriminating proof

- Accept named `True`, omitted and explicit `False`; reject positional,
  nonboolean, `None` and unknown arguments.
- Prove omitted and explicit false project to equal integer descriptor facts,
  while true differs.
- Recursively freeze and import source-shaped `int_flag`, `int_setting`, and an
  explicit-false setting. Assert each first-export rule identity, Integer kind,
  flag polarity, mandatory/nonconfigurable integer default schema and `help`.
  Every implementation must fail if called and remain lazy.
- Prove BUILD globals cannot resolve `config.int` while the accepted BUILD
  string surface remains unchanged.
- Invoke true and false integer rules in separate repository-package cases;
  require the exact fail-closed integer diagnostic and absence of target
  recording.
- Keep existing string, bool, string-list, rule-doc and recursive-freeze proofs
  green. Add no fixture, oracle, network request or Bazel run.

## Allowlist and growth caps

Only these files may change from base `1e2759c2`:

| File | Base SHA-256 | Base lines | Final line cap | Purpose |
|---|---|---:|---:|---|
| `app/slug_loading_v2/src/package.rs` | `ddd5943b4ba0c3f19ffff75e1c6933747087e617eef5ac3acfa32e6f8830f583` | 5,770 | 5,825 | integer descriptor, retained kind/schema and invocation gate |
| `app/slug_loading_v2/src/host_package_load_tests.rs` | `fb90513e375394d1684c79e1696c8b12861ad0f98147b1210da442f16e2551eb` | 5,036 | 5,150 | ABI, recursive freeze/identity and fail-closed proofs |

Additions are capped at 50 production lines, 110 proof lines and 160 total.
Deletions do not buy addition budget. No newly added or touched function may
exceed 150 lines. `package.rs` exceeds the 2,000-line review trigger, but the
Config methods, retained build-setting enum and rule projection are one
cohesive declaration lifetime; a new module would split that owner. Avoid the
existing oversized frozen-rule invocation body by changing only its small
deferred-check helper.

## Serial validation and review

Use one Cargo target directory and run commands serially:

```text
CARGO_TARGET_DIR=/tmp/slug-v2-core-runtime-target CARGO_BUILD_JOBS=1 cargo test -p slug_loading_v2 --lib config_int
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

The `build_file_loading` suite may retain only its already-recorded stale
`@external` diagnostic-order failure; every other row must pass. Archive
hygiene may report only the known three retained thoughts paths plus active
packet files. Recheck hashes, physical/addition caps, file allowlist and
touched-function lengths before review.

Because this changes retained semantic identity, terminal independent review
is mandatory before commit. It must verify pinned source order/provenance,
Bazel named/default flag behavior, flag equality/discrimination, frozen
lifetime, Integer schema, BUILD absence, pre-recording rejection, Zabel
guidance-only use, utility reuse, caps and deferred configured behavior.

## STOP / `REPLAN`

STOP and `REPLAN` if implementation requires a file outside the allowlist;
BUILD `config.int`; accepting positional or nonboolean flag shapes; changing
existing bool/string/list descriptor breadth; integer invocation/default
coercion, CLI parsing, transition/configured/analysis/action behavior; a new
schema owner, raw evaluator value, collection, registry, interner, cache, DICE
key, mapping, source observer, I/O or async path; editing the oversized invoke
body; Java/JVM work; Zabel code or behavior adoption; unpinned source; a new
fixture/oracle/network request; a cap violation; or a public Skylib/rules_rust
success claim. After integer declarations freeze, stop at
`common_settings.bzl:100` and audit `config.bool()` separately.
