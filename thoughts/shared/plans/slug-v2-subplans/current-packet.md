# Current Slug V2 Work Packet

Packet: WP-7-135-m7a-starlark-rule-none-visibility-r1
Status: ACCEPTED; M7A remains partial

## Outcome and compatibility

Admit the exact Bazel 9.2 Loading behavior for a Starlark `rule()` target
invoked with its known `visibility` attribute set directly to `None`: it is
omitted, inherits the calling BUILD package's default visibility, and retains
default rather than explicit built-in attribute provenance. A legacy `.bzl`
wrapper forwarding an omitted `visibility=None` is the authentic demand.
Explicit `[]` remains declared private; explicit label lists retain their
declared visibility; malformed non-`None` values and unknown attributes,
including unknown attributes set to `None`, still fail. The selected Loading
and Query value/visibility behavior is **exact**. Existing structural
package/repository identities and diagnostics are **Slug-native**; no Bazel
configuration, output-path or ActionKey bytes are claimed. Additional native
rule-class `None` behavior beyond WP813's accepted alias, additional
symbolic-macro visibility behavior beyond accepted slices, full `license`
provider semantics and complete `package_metadata` or root Build remain
**unsupported/deferred** by this packet; existing accepted surfaces are
unchanged.

## Pinned source, owner and decision

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`:
`StarlarkRuleClassFunctions.java:1883-1896` passes Starlark rule kwargs to
`RuleFactory`; `AttributeProvider.java:204-242` validates known names and
skips direct `Starlark.NONE` before type conversion or the explicit bit.
`AttributeProvider.java:302-315` then supplies defaults;
`BuildLanguageOptions.java:635-642` keeps unknown-attribute failure enabled;
`RuleOrMacroInstance.java:564-575` and `Target.java:159-170` resolve the
caller package default. Upstream `StarlarkIntegrationTest.java:244-278`
and `PackageFunctionTest.java:288-314` exercise legacy wrappers forwarding
`None`; `RuleTest.java:260-291` separates explicit empty/private. Bazel's
nonconfigurable visibility schema rejects `select(...)`, including a branch
defaulting to `None` (`BaseRuleClasses.java:229-240`,
`BuildType.java:219-247`). The cached,
authentic `package_metadata` 0.0.3 `licenses/rules/license.bzl:97-115`
defines `license(..., visibility=None)` and forwards that value to
`_license = rule(...)`; its BUILD omits visibility. This is a general
Starlark-rule invocation boundary, not a license-specific attribute kind.
The isolated ignored-RC Bazel 9.2 diagnostic at
`/tmp/slug-license-visibility-RV9bok` observes omitted/forwarded-`None`
targets under public package defaults and a distinct explicit-private
control. Current Slug fails the forwarded-`None` call before Query.

Stage 4's existing `FrozenRuleDefinition::invoke` in
`app/slug_loading_v2/src/package.rs` owns validation and coercion. Treat only
the known `visibility` direct `None` as omitted before
`parse_rule_visibility_argument` and before the built-in visibility
attribute's provenance selection. Keep `parse_rule_visibility_argument`'s
non-`None` type and label validation unchanged; do not add a license special
case or change ordinary attribute defaults. `PackageRecorder::starlark_rule`
continues to retain the caller package's effective visibility with
`VisibilitySource::PackageDefault`. Existing `RootPackageLoadKey` and
repository package inventory/loaded-`.bzl` dependencies own BUILD, macro
source and package-default revisions; no new DICE key, filesystem read,
cache, lock, retained representation or evaluator-heap borrow is allowed.
The complete package equality and existing invalidation edges remain the
semantic identity. This is ordinary Loading call-flow work, not a hot-path
utility or memory-representation change.

## Evidence and validation

Add one minimal offline Stage 1 fixture, `starlark-rule-none-visibility`,
with root MODULE, BUILD and a local `.bzl` legacy wrapper using
`_rule(name=name, visibility=visibility)`. Under a public package default,
define omitted, explicit-`None` and explicit-private targets. Two admitted
`query attr("visibility", ...) --output=label` rows must distinguish the
public omitted/`None` pair from the private control with exact anchored
stdout. Generate with ignored-RC Bazel 9.2 in a fresh run root, verify in
an independent fresh root, then replay with rebuilt Slug; copy no upstream
archive or credentials. The preimplementation temporary probe is diagnostic,
not fixture acceptance. Record pinned source, command, outputs, comparison
class and unsupported exclusions in `fixture.toml`. The WP834 Stage 1 growth
checkpoint reset the trigger; this small fixture does not itself require a
new hygiene checkpoint. The existing `starlark-rule-direct-none-default`
fixture already covers ordinary attributes and unknown-`None` errors, but
its multi-package/default/XML controls are not changed to inject the new
package-default visibility case; reuse its evidence and protect its relevant
rows instead of copying its assets into this fixture.

Focused Loading tests must assert omitted and forwarded-`None` equivalence
for effective visibility, built-in attribute `Default` provenance and
`VisibilitySource::PackageDefault`; contrast explicit empty/private and
explicit public lists, malformed scalar, unknown-attribute `None`, and a
`select(...)` value even when its default branch is `None`.
Exercise direct root and external repository package contexts, and a
same-DICE A→B→A package-default edit with unchanged wrapper source and
restoration. Protect existing Starlark-rule typed-dependency and ordinary
direct-`None` default tests, plus WP813's separate native-alias boundary.
Use current route/package owners and no untracked direct filesystem read.

Resolve the pinned nightly with `rustup which`, serialize Cargo, compile
selected Loading test executable with Cargo JSON `--no-run`, preflight exact
nonignored selectors, run focused/protected tests and direct Core/CLI
dependent checks. Format changed Rust and run `git diff --check` and
plan/archive checks. Rebuild `slug_cli_v2` before fixture replay or authentic
Slug smoke; clean `slugd` before and after. Run one authentic `//:probe`
Build from the preserved workspace to observe the next first error. Its
roughly one-minute duration is justified because a small fixture cannot
establish progression in the actual bootstrap closure; parallel Loading
still cannot prove every path completed. Obtain independent DESIGN and
FINAL reviews of this public Loading boundary.

## Scope and stops

Base clean main `ac3710b76`. Allowlist: this manifest and canonical Current;
`app/slug_loading_v2/src/package.rs` and its existing focused tests in
`app/slug_loading_v2/src/host_package_load_tests.rs`;
`tests/v2_oracle/fixtures/starlark-rule-none-visibility/**`; one compact
Stage 4 owner note after acceptance. Production change should be a narrow
call-site normalization in the existing large cohesive `package.rs` owner;
roughly 80-180 Rust/test lines and five fixture files are review triggers,
not caps. No new native rule class, symbolic-macro semantics, source catalog,
repository route, action/configuration or remote-execution policy change.
`REPLAN` if Bazel 9.2 does not treat direct known-attribute `None` as omission,
if the authentic stop requires license-specific provider semantics, if a
retained owner or new DICE key is needed, or if the candidate can only pass by
weakening non-`None`/unknown-attribute rejection.

## Immediate predecessor

WP834 at `ac3710b76` accepted root/selected-module BUILD metadata and moved
the authentic `//:probe` first stop to `package_metadata` 0.0.3
`licenses/rules/license.bzl:106`, where a forwarded `visibility=None`
reaches Slug's Starlark-rule list parser. M7A remains partial.

## Acceptance (2026-09-24)

Independent DESIGN and FINAL reviews ACCEPT the exact known Starlark-rule
`visibility=None` omission boundary. `FrozenRuleDefinition::invoke` skips
direct `None` only after known-name validation, both before visibility-list
coercion and before built-in explicit-attribute provenance. Non-`None`
parsing and other attribute classes are unchanged. Six exact Loading
selectors and two WP813 native-alias integration selectors were preflighted
and passed after pinned-nightly Cargo JSON `--no-run` compilation (15.28s).
They cover root/external caller package defaults, retained
`VisibilitySource::PackageDefault` and `AttributeProvenance::Default`,
explicit empty/private/public lists, malformed scalar, unknown-name `None`,
`select` with a `None` default, and same-DICE A→B→A restoration.

The five-file/two-row fixture passes fresh ignored-RC Bazel 9.2 generation
and independent verification (4.28s/4.12s) and rebuilt Slug replay. An
initial Slug replay failed at command parsing on two Bazel-only query flags;
removing them and regenerating/reverifying the oracle made both rows pass
without changing their outputs. This is invocation correction, not a
semantic waiver. Core/CLI dependent checks, CLI rebuild, pinned rustfmt,
diff/plan/archive hygiene and post-smoke `slugd` absence pass. Growth since
WP834 is five files/134 lines/two rows, below the Stage 1 trigger.

The authentic `//:probe` Build moves past the `package_metadata`
visibility failure and first reports
`@@rules_java++toolchains+remote_jdk8_linux_toolchain_config_repo//:version_or_prefix_version_setting`
is not a `config_setting`. This is a separate configured/Loading boundary;
parallel evaluation does not prove all other paths completed. Full license
provider semantics, generated Java toolchain configuration, root Build and
M7A remain open. This packet took roughly ten minutes from ready design to
validated acceptance; the authentic smoke was a necessary separate
bootstrap-closure check, not part of the small fixture's parity claim.
