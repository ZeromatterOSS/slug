# Current Slug V2 Packet

Packet: `WP-4-7A-bazel-config-bool-loading`
Milestone: M7A bootstrap-critical command/ruleset breadth
Owners: Stage 4 complete `config` global and retained rule-definition semantics
Base: `6ab6f35d`

Result: accept the live Bazel `config.bool(flag = True)` build-setting
descriptor, retain its type distinctly from the accepted string descriptor,
and prove documented boolean rule definitions freeze through recursive `.bzl`
loading. Boolean build-setting target invocation and configured analysis remain
unsupported and must fail before a target is recorded.

## Learned facts and authority

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`
is sole behavior authority:

- `StarlarkConfigApi.bool` is a method on the fixed `config` module. Its
  `flag` parameter is named-only, boolean and defaults to `False`.
- `StarlarkConfig.boolSetting` returns `BuildSetting.create(flag, BOOLEAN)`.
  `BuildSetting` owns both the value type and command-line-flag bit.
- `StarlarkRuleClassFunctions.createRule` stores the descriptor on the
  `RuleClass.Builder`. `RuleClass.Builder.build` derives mandatory,
  nonconfigurable `build_setting_default` from that exact descriptor type and
  adds string `help`.
- `ConfigSettingTest.buildsettings_convertedType` authenticates
  `config.bool(flag = True)`, a boolean default and typed matching. The broader
  `StarlarkRuleContextTest` build-setting rows establish that analysis reads a
  typed default or typed configuration value; that later behavior is not
  admitted here.
- `ConfigRules` registers `ConfigBootstrap` through
  `ConfiguredRuleClassProvider.Builder.addStarlarkBootstrap`, whose contract is
  explicitly `.bzl`-only. `StarlarkGlobalsImpl` builds fixed BUILD globals
  separately and does not add `config` there.

The accepted rules_rust 0.73.0 archive SHA-256 is
`2d0c8b967b619d5717be8210f52a24c5aa624e3229a38dc4071712db1dd522f2`.
After accepted `rule(doc=...)` and `config.string(flag=True)` declarations,
`rust/private/rustc.bzl:3047-3055` defines
`always_enable_metadata_output_groups` with
`build_setting = config.bool(flag = True)`. A second boolean descriptor follows
at lines 3058-3080; the next distinct descriptor family is
`config.string_list(flag = True)` at line 3093. Fresh query and build reach the
first boolean descriptor; their public wrappers remain `query_error` exit 7
and `build_runtime_error` exit 2 at the repository-session boundary.

Slug's `ConfigModule` is already the one complete `.bzl` config-global owner,
but currently exposes only a zero-sized `RootStringBuildSetting`. The rule
definition, frozen definition and recorded `StarlarkRuleImplementation` retain
that semantic fact as `root_string_build_setting: bool`; this bit participates
in equality and selects a string `build_setting_default` schema. Loading
already has exact boolean attribute coercion. Analysis deliberately recognizes
only the accepted root string setting. Unlike Bazel, Slug's shared
`complete_loading_globals` currently places the same string-only config module
in `.bzl` and BUILD environments; the accepted BUILD projection must not gain
the new boolean method.

Pinned `../zabel` commit
`c7298478e2e56262a2f438e9c065325744c9f0fc` is architecture guidance only.
Its complete typed semantics owner and narrow projections support replacing the
string-only bit with one compact typed fact owned by the rule definition, then
letting schema construction and the existing string accessor project from it.
Copy no Zabel code, representation, scheduler, runtime or behavior; Bazel
remains build-setting authority.

The Buck2 utility-reuse audit found no matching import. A two-variant enum is
smaller and clearer than a second flag, map or interner; existing
`CompactString`, immutable `Arc` slices and `Allocative` coverage remain
unchanged. No Stage 9 extraction-ledger update is required.

## Decision and non-decisions

In `package.rs`, add an evaluation-local zero-sized boolean descriptor returned
only by `.bzl` `config.bool(flag = True)`. Give `loading_globals` the complete
string-plus-bool config projection while `build_file_loading_globals` retains
its current string-only sibling projection. Share the string constructor logic;
do not reconstruct descriptor semantics per evaluator.

Replace the retained string-only boolean with `Option<BuildSettingKind>`
(string or boolean) through `RuleDefinitionGen`, `FrozenRuleDefinition` and
`StarlarkRuleImplementation`. Include that kind in structural equality. Derive
the mandatory `build_setting_default` schema as string or boolean from the kind
and keep the existing string-setting accessors as narrow projections.

Reject omitted/`False` `config.bool` because the admitted command route uses
only `flag = True`; those forms remain unsupported rather than being claimed
by exposing the method. Reject invocation of a boolean build-setting rule at
the callable boundary before `PackageRecorder::starlark_rule`, so no target,
dependency, analysis value or configuration fact is published.

Do not change string build-setting invocation/analysis, configuration
identity, CLI option parsing, transitions, `config_setting(flag_values=...)`,
`ctx.build_setting_value`, provider/action semantics, other `config` methods,
other `rule` parameters, every non-config global placement, DICE keys, events
or public error translation. Do not remove or broaden the current BUILD
string-only projection, and do not retain a second boolean marker or metadata
registry.

## Ownership, identity and lifetime

The `.bzl` config module remains the complete call-shape owner; the BUILD
module is a string-only environment projection and never owns bool semantics.
The frozen rule definition is the sole retained owner of the descriptor kind;
builtin schema and the existing string-only analysis accessor borrow
projections from it.
String and boolean definitions must compare unequal even when implementation,
attributes and capability match. The kind is copied through freeze and target
definition equality; it is never inferred from a default value.

Existing source observation invalidates definition edits before evaluation.
No request input, revision certificate, overlapping-request behavior,
publication or DICE dependency changes. The enum replaces one retained bool,
adds no allocation and remains DICE-owned with the loaded definition. The
boolean Starlark descriptor is evaluator-local. No cache, task, fallback,
interner or async memory is added.

## Files and caps

Allowed files, with base SHA-256 and final line ceiling:

| File | Base SHA-256 | Cap |
|---|---|---:|
| `app/slug_loading_v2/src/package.rs` | `59b191dbdcd4f56c11cbd07bcd1bddab6d52c558e065ce8943964b4db8425676` | 5,240 |
| `app/slug_loading_v2/src/host_package_load_tests.rs` | `a6079c3a7c414a0e421fea1d79392d59bf70cb4e442b46fe3ce79b2447aed937` | 4,035 |

Production additions are <=120, proof additions <=110 and total additions
<=230. Both files exceed the authoring-guide size trigger, but `package.rs`
already owns the complete config global, rule-definition freeze, builtin
schema and invocation boundary. The proof belongs in the existing recursive
external-Bzl/package harness. Splitting any one of those responsibilities in
this packet would create a second semantics owner.

## Proof and validation

Add focused proof that:

- `config.bool(flag = True)` creates a documented rule definition that binds,
  recursively exports and freezes alongside a string build-setting definition;
- the frozen kinds are structurally distinct and select boolean versus string
  `build_setting_default` schema;
- BUILD evaluation cannot resolve `config.bool`, while its current
  `config.string` projection remains unchanged;
- omitted/`False` boolean descriptors fail closed; and
- invoking the boolean rule with a boolean default fails before a package
  target is recorded, while the accepted string-setting route remains green.

Run:

- `cargo fmt --check` and `git diff --check`;
- focused config-bool loading tests;
- full `cargo test -p slug_loading_v2`;
- `cargo check -p slug_core_v2 --locked`;
- `cargo build -p slug_cli_v2 --locked`;
- `scripts/v2_archive_status.sh`, preserving only its current stale
  thoughts-path classification if unchanged; and
- with clean `slugd` lifecycle and fresh output roots, the existing disposable
  rules_rust query and build, recording the next common internal/public
  terminal.

Pinned source/tests and the accepted real archive already discriminate this
declaration contract, so no new Bazel fixture or copied source is authorized.

## Compatibility and STOP

- **Exact:** `.bzl`-only named `config.bool(flag = True)` descriptor
  construction, BUILD absence, distinct boolean rule-definition identity,
  boolean default schema and recursive bind/export/freeze behavior on the live
  rules_rust loading route.
- **Slug-native:** the compact Rust enum, fail-closed nonadmitted invocation
  error, evaluator representation, valid-Unicode source handling and
  nonrequired diagnostics.
- **Unsupported/deferred:** omitted/`False` bool descriptors, boolean target
  invocation and analysis, CLI bool flags, transitions/config matching,
  `ctx.build_setting_value`, `config.string_list` and every other config method,
  later rules_rust toolchains/actions, M8/M7B and exact output bytes.

STOP on dirty overlap, edits outside the two-file allowlist, recording a
boolean build-setting target, analysis/configuration/CLI changes, a second
retained marker or side registry, string-setting regression, globals widening,
BUILD visibility of `config.bool`, source vendoring, Java/JVM, dependency
drift, archive fixture growth, public success claims or scope above the caps.
`REPLAN` before crossing a boundary.
