# Current Slug V2 Work Packet

Packet: WP-7-129-m7a-java-launcher-field-declaration-r1
Status: ACCEPTED; independent DESIGN and FINAL reviews ACCEPT; M7A remains partial

## Outcome and compatibility

Clear the authentic `@@bazel_tools//tools/jdk:launcher_flag_alias.bzl:36`
Loading stop by admitting `configuration_field(fragment = "java", name =
"launcher")` as a declaration-only private `attr.label` default. Exact selected
Bazel 9.2 Loading: field-name acceptance, null loading default, and retained
field-plus-tools-repository identity through frozen/imported rule declarations.
Slug-native: existing diagnostics and fail-closed configured Java-field error.
Unsupported/deferred: projecting `--java_launcher` into configured labels,
provider-constrained launcher analysis, full JDK package Loading, Java actions,
and any cquery parity for this field. An authentic later stop cannot widen this
packet.

## Source, owner and decision

Pinned Bazel 9.2 `JavaConfiguration.java:220-228` declares the nullable Label
field `launcher` with `defaultInToolRepository = true`; `JavaOptions.java:193-202`
owns `--java_launcher`, default null.
`StarlarkLateBoundDefault.java:61-85,171-225` validates annotated fields at
Bzl initialization; `Attribute.java:1553-1579` distinguishes the null loading
default from configured resolution. The source option default and pinned
authentic default/explicit-flag cquery oracle discriminate those values.
The exact embedded JDK Bzl source uses the
field under a private `_launcher: attr.label(..., providers = _providers)`;
the native JDK BUILD instantiates that rule later. An ignored-RC/no-fetch
Bazel 9.2 oracle in the authentic workspace successfully queries
`@@bazel_tools//tools/jdk:launcher_flag_alias` and shows no ordinary Query
path from `//:probe`. Default cquery succeeds, while a nonnull launcher flag
reaches a separate `CcLauncherInfo` provider failure. Neither observation
authorizes Slug configured Java resolution.

Append `JavaLauncher` to the existing private `JavaConfigurationField` and
flattened public `ConfigurationField` closed enums in
`app/slug_configuration_v2/src/native/configuration_field.rs`. Append rather
than renumber accepted discriminants. The existing one-byte enum,
`ConfigurationFieldIdentity` (field plus canonical tools repository), frozen
rule late-bound attribute slices, source/recursive-manifest fingerprints and
DICE structural equality remain the only owners. No new option projection,
label parser, string/map/Arc carrier, interner, cache, DICE key, lock, or
allocation; ordinary rules pay no cost. `configuration_field_global` already
validates Bzl-only calls and maps `@bazel_tools`; the existing
`SlugConfiguration::configuration_field_label` rejects every Java field before
dependency activation. Keep that fail-closed behavior. This reuses the adopted
no-extraction compact-enum pattern in Stage 9; no Buck2/V1 code is imported.

## Evidence and validation

First compile and exact-preflight a source-shaped `java.launcher` Loading
regression on the clean base, showing failure at the old invalid-field guard.
Then extend the existing selected-Java Configuration and Loading tests to
cover the third field, null loading default, private label-only use, field and
tools-repository equality/hash, one-byte/identity layout, frozen import and
re-export, and retain the `proguard_top`/`bytecode_optimizer`/`missing`
negative boundary. Extend the existing Legacy/Observed Analysis test to
prove launcher configured resolution fails before dependency/toolchain/
implementation activation, with same-DICE launcher/other/launcher restoration.
Reuse pinned source and the authentic oracle; no Stage 1 fixture is needed.
Do not add a Query carrier or configured option semantics.

Resolve the pinned nightly toolchain; compile selected Configuration,
Loading and Analysis test binaries with Cargo JSON `--no-run`, exact
nonignored preflight, and run focused/protected tests serially. Directly check
Configuration/Loading/Analysis/Core/CLI, rebuild `slug_cli_v2` before one
normalized-PATH authentic `//:probe` smoke, and clean/check `slugd` before
and after. Run pinned rustfmt, diff, plan and archive checks. Independent
DESIGN and FINAL review must verify the retained identity and fail-closed
boundary; the replay only proves the selected Loading stop cleared.

## Scope and stops

Base clean main `e9011177f`. Allowlist: this manifest, canonical Current,
compact Stage 4 and matching Stage 9 notes after acceptance; production
`app/slug_configuration_v2/src/native/configuration_field.rs`; focused tests
`app/slug_configuration_v2/src/native/tests.rs`,
`app/slug_loading_v2/tests/subrule_loading.rs`, and
`app/slug_analysis_v2/tests/subrule.rs`. The previous two-field Stage 4 note
may be amended only to record this newly selected declaration and two remaining
deferred Java fields. Expected growth about 20 production/100 proof lines is a
review trigger. `REPLAN` if Bazel's Loading default is not null, the existing
private late-bound owner cannot retain the declaration, enum/identity layout
grows, or clearing this stop requires configured `--java_launcher` projection
or provider semantics. No direct JDK rule or rules_cc implementation is in
scope.

## Acceptance receipt

Independent DESIGN review corrected pinned-source line anchors and accepted
the declaration-only boundary. Before the production change, Cargo JSON
`--no-run` compiled the source-shaped Loading regression; exact nonignored
preflight selected one test and it failed at `invalid configuration field name
'launcher' on fragment 'java'`. Independent FINAL review ACCEPTED the actual
append-only enum mapping, unchanged one-byte/32-byte retained layouts, null
Loading edge, frozen private-label identity, and configured fail-closed proof.

Pinned-nightly Cargo direct checks for Configuration, Loading, Analysis, Core
and CLI and the `slug_cli_v2` rebuild passed. Candidate test binaries were
compiled with Cargo JSON `--no-run` and exact nonignored preflights: the
Configuration selected Java/Proto/Apple controls passed 3/3; selected Loading
Java/Coverage/C++, Bzl-only validation and Apple controls passed 3/3; the
Legacy/Observed Java fail-before-activation and protected Proto Analysis tests
passed 2/2. Two broader Loading tests failed on both the candidate and a
disposable clean `e9011177f` worktree with the same `platform_common.TemplateVariableInfo
loading token changed` and `nonempty label aspects are unsupported` diagnostics;
they are pre-existing failures, not candidate passes. The diagnostic worktree
was removed after comparison.

The rebuilt normalized-PATH authentic `//:probe` exited 2 after clearing
`launcher_flag_alias.bzl:36` and reached the distinct
`@@bazel_tools//tools/jdk:BUILD:48` missing-`java_plugins_flag_alias` Loading
stop. No complete JDK package or configured launcher is claimed. No stale
`slugd` existed before or after replay. Pinned rustfmt, diff, plan and archive
checks passed. M7A remains partial.

## Immediate predecessor

WP828 accepted at `e9011177f`: Stage 5 ports the exact 13-file Bazel 9.2
runtime JDK source subtree, its noncatalog Bazel-input sidecar, and source
route identity. The authentic replay cleared missing JDK BUILD and reached
this valid Java configuration-field declaration; M7A remains partial.
