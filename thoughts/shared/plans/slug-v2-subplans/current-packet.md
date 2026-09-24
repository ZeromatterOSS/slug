# Current Slug V2 Work Packet

Packet: WP-7-136-m7a-configured-condition-alias-r1
Status: ACCEPTED; M7A remains partial

## Outcome and compatibility

Admit the Bazel 9.2 configured-condition behavior needed by the authentic
`rules_java` 9.1.0 generated toolchains: a native `alias` named in
`toolchain.target_settings` may select a `config_setting` through its
configurable `actual`. Match the selected setting under the condition's
configuration, retain the requested alias label for selector lookup and the
declared `target_settings` edge, and use the selected declaration for condition
specialization. Bazel's forwarded `ConfigMatchingProvider.label` names the
selected leaf, a distinct identity not exposed by this packet's Slug result.
The selected condition truth and toolchain eligibility are **exact** within
the existing admitted target/exec configuration and toolchain family. Slug's
structural configuration/path identity and diagnostics remain **Slug-native**;
generic alias-provider forwarding and provider-label inspection, exact Bazel configuration/output bytes,
remote JDK fetching, complete root Build, and unrelated rule families remain
**unsupported/deferred**. Existing accepted exact slices stay exact.

## Pinned source, owner and decision

Pinned Bazel 9.2 commit `8220c6198837d5c13d53fea211cf3282aa12408a`:
`Alias.java:41-55` constructs an alias configured target;
`AliasConfiguredTarget.java:86-125,165-169` selects `actual` and forwards
the selected `ConfigMatchingProvider`; `ConfigSetting.java:150-165` produces
that provider; `Toolchain.java:55-58` consumes it for `target_settings`,
and `RegisteredToolchainsFunction.java:129-145` validates the prerequisite.
The cached `rules_java` 9.1.0
`toolchains/remote_java_repository.bzl:23-33,55-101` writes two real
settings and `version_or_prefix_version_setting = alias(actual = select(...))`;
both generated Java toolchains list that alias in `target_settings`. An
ignored-RC, no-fetch Bazel 9.2 query confirmed the authentic target is an
alias. Upstream configurable-alias and toolchain-setting tests establish
the component behaviors, but no single pinned test combines them.

Analysis's `ConfiguredConditionKey` in `app/slug_analysis_v2/src/dice.rs`
owns match truth, while `prepare_configured_attribute_conditions` and
`load_configured_condition_declarations` own selected declarations and
specificity. Both currently require a direct `PackageTargetKind::ConfigSetting`.
Walk native alias declarations without analyzing an arbitrary selected rule:
use the existing `AliasActual` selector preparation/resolution, DICE-load
each visited alias/leaf package, use the configured-condition cycle guard for
recursive selector conditions and a transient visited-label set for plain
alias chains, then require the selected final target to be a `ConfigSetting`.
Preserve the original alias label in selector truth and condition edges;
load and normalize the selected declaration. Both the matcher and the parent
declaration loader must walk the alias, so the latter directly depends on
alias/leaf packages even when condition truth equality-cuts. Preserve
`Need`, errors, target/exec configuration,
repository mapping, and same-DICE A→B→A restoration. Do not treat the
branches as an OR, admit a wrong-kind leaf, introduce a new condition
provider model/key or retain a new hot-path representation. Existing
immutable label/configuration and compact condition collections remain the
owners; Stage 9 utility disposition is reuse.

## Evidence and validation

Add focused Analysis tests with a configurable alias to distinct settings,
including target and exec configuration, selected match and no-match,
specialization when two leaves have equal truth, alias chains, wrong-kind
selected actual, cycle and missing selected dependency, and same-DICE A→B→A
changes. Extend the existing configurable `target_settings` test with the
configurable-actual alias itself in `target_settings`: both selected leaves
must match in separate configurations, while a third configuration must not
match any toolchain. Protect direct
condition, native configurable-alias, toolchain-setting and observed-mode
tests. Pinned-source regression plus focused tests are the parity evidence:
the isolated ignored-RC Bazel 9.2 `cquery //:probe` probe at
`/tmp/slug-toolchain-alias-r53coc` with `--repository_disable_download`,
explicit local host/target platform and `--output=label_kind` succeeded
for `opt`/`dbg` and rejected `fastbuild`, but Slug currently encounters an
unrelated `repository_ctx.which` limit in that tiny workspace before this
alias; it is diagnostic, not an accepted oracle fixture. Do not broaden this
packet to repair that fixture bootstrap or claim a three-source oracle row.

Resolve pinned nightly via `rustup which`, serialize Cargo, preflight exact
test selectors and run focused/protected Analysis tests plus Core/CLI checks.
Format changed Rust and run diff, plan and archive hygiene. Rebuild
`slug_cli_v2` before using `SLUG_V2_BIN`; clean `slugd` before and after one
authentic `//:probe` smoke. That smoke is progression evidence, not proof of
full root closure. Obtain independent DESIGN and FINAL reviews of the
configured-condition/DICE boundary.

## Scope and stops

Base clean main `735fb8d9b`. Allowlist: this manifest and canonical Current;
`app/slug_analysis_v2/src/dice.rs` and focused tests in
`app/slug_analysis_v2/tests/starlark_rule.rs`; one compact Stage 6 owner note
after acceptance. Roughly 120-250 Rust/test lines are a review trigger, not
a cap. No generated repository writer, Loading key, generic provider,
configuration identity, toolchain policy, fixture, remote execution or action
change. `REPLAN` if the selected leaf cannot be validated without unrelated
target analysis, if an equality cutoff loses leaf-identity invalidation, if
the authentic stop needs an unmodeled Bazel provider surface, or if the
minimal change creates a new DICE key/retained representation or weakens
wrong-kind/cycle rejection.

## Immediate predecessor

WP835 at `735fb8d9b` accepted the exact known Starlark-rule
`visibility=None` omission and moved the authentic `//:probe` first stop to
`@@rules_java++toolchains+remote_jdk8_linux_toolchain_config_repo//:version_or_prefix_version_setting`
being rejected as not a `config_setting`. M7A remains partial.

## Acceptance (2026-09-24)

Independent DESIGN and FINAL reviews ACCEPT the bounded exact selected-condition
and toolchain-eligibility slice. Analysis now walks native alias declarations
through existing selector preparation/resolution, DICE-loads each visited
alias/leaf package, and requires the selected leaf to be a `config_setting`
without analyzing an arbitrary wrong-kind rule. The condition key and the
declaration parent both walk the alias; the latter's direct package
dependencies preserve selected-leaf specificity when match truth equality-cuts.
No new DICE key, retained representation, provider model or Loading route is
introduced. The transient `SmallSet` visited labels and existing compact
condition rows are reused under the Stage 9 utility disposition.

Four new exact Analysis selectors and three protected selectors pass after
pinned-nightly Cargo JSON `--no-run` compilation. The new tests cover a
configurable-actual alias in `target_settings` (two eligible configurations,
one ineligible), selected-false versus an unselected true leaf, selected and
unselected wrong-kind branches, alias chains and selector-condition cycles,
target/exec configuration, and same-DICE A→B→A including an equal-truth
leaf switch that changes specialization. The direct-condition test helper
now installs the same configured cycle detector as production root requests;
without it, a new self-selector negative hung and was interrupted during
development. Core/CLI checks, CLI rebuild, pinned rustfmt, diff/plan/archive
hygiene and post-smoke `slugd` absence pass.

The broader `starlark_rule` binary passed 223/225. Two untouched tests,
`directory_args::unexpanded_directory_vectors_retain_literal_paths_and_reject_expansion`
and `output_group_info::configured_output_groups_preserve_artifacts_sharing_forwarding_and_a_b_a`,
fail on bare versus Slug-prefixed action-output-path assertions; neither
contains a condition/alias/select/toolchain path. They are unrelated
validation debt, not a clean-main baseline claim or a WP836 waiver. The
rebuilt authentic `//:probe` smoke moved beyond the `rules_java` alias and
first stopped in generated `rules_cc++cc_configure_extension+local_config_cc`
at unsupported `repository_ctx.symlink` (exit 2, about 46 seconds). Full
C++ toolchain configuration, root Build and M7A remain open.
