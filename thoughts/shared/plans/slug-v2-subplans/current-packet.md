# Current Slug V2 Packet

Packet: `WP-4-7A-bazel-config-string-list-repeatable-loading`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owners: Stage 4 complete `config` global and retained rule-definition semantics
Base: `6811fa84`

Result: accept the first live Bazel
`config.string_list(flag = True, repeatable = True)` descriptor, retain the
repeatability dimension structurally alongside nonrepeatable StringList, and
prove both definitions freeze distinctly. Every list-setting target remains
unsupported and must fail before recording.

## Learned facts and authority

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
is sole behavior authority:

- `StarlarkConfigApi.string_list` declares named-only boolean `flag` and
  `repeatable` parameters, both defaulting to `False`.
- `StarlarkConfig.stringListSetting` rejects `repeatable = True` unless
  `flag = True`, then creates a `BuildSetting` with `Type.STRING_LIST` and a
  separately retained repeatability bit.
- `RuleClass.Builder` derives mandatory, nonconfigurable
  `build_setting_default` from that descriptor type. The
  `ConfigSettingTest.starlarkListFlagSingleValue` rows authenticate a
  nonrepeatable list default; `buildsettings_repeatableWorks` and the parsing
  tests prove repeatable accumulation and the invalid-combination row proves
  `repeatable=True` requires `flag=True`.
- `StarlarkOptionsParser` consumes the retained repeatability bit only for CLI
  parsing. This packet owns descriptor identity only; target/analysis/CLI
  behavior remains outside it.
- Bazel installs fixed `config` through the `.bzl` bootstrap. Slug's accepted
  BUILD projection remains string-only and must not gain `string_list`.

The accepted rules_rust 0.73.0 archive SHA-256 is
`2d0c8b967b619d5717be8210f52a24c5aa624e3229a38dc4071712db1dd522f2`.
Commit `6811fa84` accepts the nonrepeatable definitions at lines 3093 and 3108.
Source order next reaches the first `repeatable = True` use at line 3120;
later `rustc.bzl`, `clippy.bzl`, and `unpretty.bzl` definitions use both forms.
The existing public query/build terminals remain the accepted
repository-session wrappers rather than successful public loads.

Slug now retains `BuildSettingKind::StringList` distinctly through rule
construction, freeze, structural equality, list-default schema and recorded
implementation. `RootStringListBuildSetting` is evaluation-local and zero
sized because only normalized false is admitted. Every list invocation fails
before `PackageRecorder`; string analysis remains a narrow accessor.

Pinned `../zabel` commit
`c7298478e2e56262a2f438e9c065325744c9f0fc` is architecture guidance only.
Its `projectRuleModule` validates a complete typed build-setting declaration
structurally against the invocation, while its allow-multiple/repeatable test
keeps `value_type`, `allow_multiple`, and `repeatable` distinct. That supports
one retained Slug declaration owner and thin schema/analysis projections, not
an evaluator-local marker or side registry. Copy no Zabel code, representation,
scheduler, runtime or behavior; Bazel remains sole compatibility authority.

The Buck2 utility-reuse audit selects no import or Stage 9 ledger update. One
boolean field on the existing descriptor and enum variant uses existing
`Allocative`, `Copy`, freeze and equality paths and adds no allocation,
collection, hash domain, interner or clone-sensitive container.

## Decision and non-decisions

In `package.rs`, give the evaluation-local StringList descriptor its normalized
repeatability bit and accept named `.bzl` calls with `flag = True` and either
boolean repeatability value. Continue rejecting false/omitted `flag` and
positional arguments. Keep `BuildFileConfigModule` string-only.

Widen only the existing enum variant to
`BuildSettingKind::StringList { repeatable: bool }`. Carry the bit through the
existing sole definition, freeze, equality, schema and implementation field;
schema continues to project `AttributeKind::StringList` independent of the
bit. Omission and explicit `False` remain equal, while `True` compares unequal.

Reject StringList rule invocation at the callable boundary before
`PackageRecorder::starlark_rule`, just like the boolean fail-closed boundary.
Do not change accepted String invocation/analysis or Boolean definitions.

Do not implement list-setting target invocation, `ctx.build_setting_value`,
CLI parsing/accumulation, transitions,
`config_setting`, configuration identity, providers, toolchains, actions,
other `config` methods, BUILD/MODULE/REPO global placement, DICE keys, events,
or public error translation. Do not infer kind or repeatability from defaults.

## Ownership, revision and lifetime

The complete `.bzl` config module owns descriptor call shape. The frozen rule
definition remains the sole retained owner of the normalized descriptor kind;
schema and analysis are projections. Repeatable and nonrepeatable StringList
definitions compare unequal even when every other field matches.

Existing observed source dependencies invalidate definition edits before
evaluation. No request projection, revision certificate, overlapping-request
behavior, final validation boundary, DICE dependency or publication changes.
The descriptor is evaluator-local; the copied enum remains DICE-retained with
the loaded definition. Existing defaults use immutable compact list storage.
No service cache, command/scratch retention, async transfer, task, fallback,
eviction or shutdown duty is added.

## Files and caps

Allowed files, with base SHA-256 and final line ceiling:

| File | Base SHA-256 | Cap |
|---|---|---:|
| `app/slug_loading_v2/src/package.rs` | `508ecd79e23ea52ab1a1bebb891f5dbdcd1041cd37f707ab24cdf678d19473ec` | 5,267 |
| `app/slug_loading_v2/src/host_package_load_tests.rs` | `0745f5e12051151c599e36aa5df39139121506f65d67d75193362adccd6b0f43` | 4,171 |

Production additions are <=30, proof additions <=60 and total additions
<=90. Both files exceed the authoring-guide size trigger, but `package.rs`
already owns this complete config/rule/schema/invocation path and the proof
extends its existing recursive external-Bzl/package harness. A split would
create a second semantics owner for a single enum variant and descriptor.

## Proof and validation

Extend focused proof that:

- recursive `.bzl` export/freeze retains repeatable and nonrepeatable
  StringList as unequal kinds while both select `AttributeKind::StringList`;
- omission, explicit `repeatable = False`, and `repeatable = True` succeed with
  `flag=True`, while positional arguments and false/omitted `flag` fail closed;
- BUILD cannot resolve `config.string_list`, while accepted BUILD string and
  `.bzl` bool behavior remain unchanged; and
- invoking a repeatable list-setting rule with a list default still fails
  before any target is recorded, while nonrepeatable and prior types remain
  green.

Run serially:

- `cargo fmt --check` and `git diff --check`;
- focused config-string-list loading tests;
- full `cargo test -p slug_loading_v2`;
- `cargo check -p slug_core_v2 --locked`;
- `cargo build -p slug_cli_v2 --locked`;
- `scripts/v2_archive_status.sh`, preserving only its known three-path
  thoughts classification if unchanged; and
- with clean `slugd` lifecycle and fresh output roots, the existing disposable
  rules_rust query/build, recording the next source-order stop separately from
  the unchanged public repository-session wrappers.

Pinned source/tests and the accepted archive already discriminate this
definition contract. No new oracle fixture, copied source or network change is
authorized; skipped upstream repeatable/analysis tests exercise deferred
phases, while the nonrepeatable default test is adapted into the focused
typed-schema proof.

## Compatibility and STOP

- **Exact:** `.bzl`-only named `config.string_list(flag = True, repeatable =
  True|False)` construction, BUILD absence, structurally distinct repeatable
  identity, common list-typed default schema and recursive bind/export/freeze
  on the live source route.
- **Slug-native:** compact enum/evaluator representation, fail-closed target
  and repeatable errors, valid-Unicode handling and nonrequired diagnostics.
- **Unsupported/deferred:** omitted/false `flag`, all StringList
  targets/analysis/CLI values/transitions/config matching,
  `ctx.build_setting_value`, later rules_rust/toolchain/action surfaces,
  M8/M7B and exact output bytes.

STOP on dirty overlap, edits outside the two-file allowlist, repeatability
outside the existing descriptor/kind owner, list target recording,
analysis/configuration/CLI changes, side metadata, BUILD visibility, String or
Boolean regression, source vendoring, Java/JVM, dependency drift, fixture
growth, public-success claims or any cap breach. `REPLAN` before crossing a
boundary.
