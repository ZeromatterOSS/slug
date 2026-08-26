# Current Slug V2 Packet

Packet: `WP-4-7A-bazel-config-string-list-nonrepeatable-loading`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owners: Stage 4 complete `config` global and retained rule-definition semantics
Base: `573c25c7`

Result: accept the first live Bazel
`config.string_list(flag = True)` descriptors, with omitted or explicit
`repeatable = False`, retain StringList distinctly from String/Boolean through
recursive definition freeze and list-default schema selection, and reject
every list-setting target before recording it. Repeatable descriptors remain
unsupported and must fail closed.

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
  nonrepeatable list default; the repeatable and invalid-combination rows prove
  that later repeatability cannot be collapsed into the nonrepeatable kind.
- `StarlarkOptionsParser` consumes the retained repeatability bit only for CLI
  parsing. Target/analysis/CLI behavior is outside this packet.
- Bazel installs fixed `config` through the `.bzl` bootstrap. Slug's accepted
  BUILD projection remains string-only and must not gain `string_list`.

The accepted rules_rust 0.73.0 archive SHA-256 is
`2d0c8b967b619d5717be8210f52a24c5aa624e3229a38dc4071712db1dd522f2`.
Source order reaches `rust/private/rustc.bzl:3093` and `:3108`, both
`config.string_list(flag = True)`, before the first
`repeatable = True` use at line 3120. Later `rustc.bzl`, `clippy.bzl`, and
`unpretty.bzl` definitions use both forms. This packet stops exactly before
the first repeatable form; the existing public query/build terminals remain
the accepted repository-session wrappers rather than successful public loads.

Commit `573c25c7` retains one `Option<BuildSettingKind>` with String/Boolean
variants through rule construction, freeze, structural equality, schema and
recorded implementation. Loading already owns exact list-of-string attribute
coercion as immutable `Arc<[CompactString]>`. Boolean invocation already fails
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
additional enum variant uses existing `Allocative`, `Arc`, `CompactString`,
freeze and equality paths and adds no allocation, collection, hash domain,
interner or clone-sensitive container.

## Decision and non-decisions

In `package.rs`, add an evaluation-local StringList descriptor returned only
by named `.bzl` calls with `flag = True` and omitted or explicit
`repeatable = False`. Reject false/omitted `flag`, positional arguments and
`repeatable = True`. Keep `BuildFileConfigModule` string-only.

Add `BuildSettingKind::StringList` and derive `AttributeKind::StringList` for
`build_setting_default`. Carry that kind through the existing sole definition,
freeze, equality, schema and implementation field. The fixed nonrepeatable
state is normalized into the variant: omission and explicit `False` are equal
in Bazel and introduce no distinct semantic input. Before any later
`repeatable = True` form is admitted, its state must become a structurally
retained dimension; this packet may not silently widen the variant.

Reject StringList rule invocation at the callable boundary before
`PackageRecorder::starlark_rule`, just like the boolean fail-closed boundary.
Do not change accepted String invocation/analysis or Boolean definitions.

Do not implement repeatable descriptors, list-setting target invocation,
`ctx.build_setting_value`, CLI parsing/accumulation, transitions,
`config_setting`, configuration identity, providers, toolchains, actions,
other `config` methods, BUILD/MODULE/REPO global placement, DICE keys, events,
or public error translation. Do not infer kind or repeatability from defaults.

## Ownership, revision and lifetime

The complete `.bzl` config module owns descriptor call shape. The frozen rule
definition remains the sole retained owner of the normalized descriptor kind;
schema and analysis are projections. StringList definitions compare unequal
to String and Boolean definitions even when every other field matches.

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
| `app/slug_loading_v2/src/package.rs` | `6ba46935a108979ce8e5b8dcf2230a5b51ab314aac66c872e1c3bc3b246392b6` | 5,294 |
| `app/slug_loading_v2/src/host_package_load_tests.rs` | `c9b4b9f6616850a4224f77c71bdedfee7c55391b3c35ea1e3b94399eeafcfe73` | 4,125 |

Production additions are <=90, proof additions <=90 and total additions
<=180. Both files exceed the authoring-guide size trigger, but `package.rs`
already owns this complete config/rule/schema/invocation path and the proof
extends its existing recursive external-Bzl/package harness. A split would
create a second semantics owner for a single enum variant and descriptor.

## Proof and validation

Extend focused proof that:

- recursive `.bzl` export/freeze retains String, Boolean and nonrepeatable
  StringList as three unequal kinds and selects `AttributeKind::StringList`;
- omission and explicit `repeatable = False` both succeed, while positional
  arguments, false/omitted `flag`, and `repeatable = True` fail closed;
- BUILD cannot resolve `config.string_list`, while accepted BUILD string and
  `.bzl` bool behavior remain unchanged; and
- invoking a list-setting rule with a list default fails before any target is
  recorded, while the accepted string route remains green.

Run serially:

- `cargo fmt --check` and `git diff --check`;
- focused config-string-list loading tests;
- full `cargo test -p slug_loading_v2`;
- `cargo check -p slug_core_v2 --locked`;
- `cargo build -p slug_cli_v2 --locked`;
- `scripts/v2_archive_status.sh`, preserving only its known three-path
  thoughts classification if unchanged; and
- with clean `slugd` lifecycle and fresh output roots, the existing disposable
  rules_rust query/build, recording the first repeatable source-order stop
  separately from the unchanged public repository-session wrappers.

Pinned source/tests and the accepted archive already discriminate this
definition contract. No new oracle fixture, copied source or network change is
authorized; skipped upstream repeatable/analysis tests exercise deferred
phases, while the nonrepeatable default test is adapted into the focused
typed-schema proof.

## Compatibility and STOP

- **Exact:** `.bzl`-only named
  `config.string_list(flag = True[, repeatable = False])` construction, BUILD
  absence, normalized nonrepeatable StringList definition identity, list-typed
  default schema and recursive bind/export/freeze on the live source route.
- **Slug-native:** compact enum/evaluator representation, fail-closed target
  and repeatable errors, valid-Unicode handling and nonrequired diagnostics.
- **Unsupported/deferred:** omitted/false `flag`, `repeatable = True`, all
  StringList targets/analysis/CLI values/transitions/config matching,
  `ctx.build_setting_value`, later rules_rust/toolchain/action surfaces,
  M8/M7B and exact output bytes.

STOP on dirty overlap, edits outside the two-file allowlist, a retained
repeatability claim without structural identity, list target recording,
analysis/configuration/CLI changes, side metadata, BUILD visibility, String or
Boolean regression, source vendoring, Java/JVM, dependency drift, fixture
growth, public-success claims or any cap breach. `REPLAN` before crossing a
boundary.
