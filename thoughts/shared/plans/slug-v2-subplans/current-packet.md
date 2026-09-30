# Current Slug V2 Work Packet

Packet: WP-7-167-m7a-remove-legacy-whole-archive-projection-r1
Status: accepted; independent DESIGN/FINAL ACCEPT; M7A remains partial

## Outcome and compatibility

Baseline6c18f52a2 (WP866). Advance unchanged authentic Linux Rust-toolchain
cquery beyond missing cpp.incompatible_remove_legacy_whole_archive() at
rules_cc finalize_link_action436 on dynamic link creation. Admit only this
private zero-argument Boolean getter and its typed command option.
Exact option/default/Boolean conversion/exec carry/getter/current-private-caller
policy; existing configuration/output semantic identity remains Slug-native.
Windows remains minimal/refused; full archive semantics/link execution deferred.

Pinned Bazel9.2 clean8220c619 CppConfiguration484-491 checks private
Starlarkification caller and returns cppOptions.removeLegacyWholeArchive;
CppOptions790-799 Boolean defaulttrue, LOADING_AND_ANALYSIS/incompatible;
builtin_exec_platforms309 explicitly carries this option. LibraryLinkingTest
47-85 disables option for legacy generated-library whole archive behavior;
its full link action/execution is outside this getter slice. Existing pinned
rules_cc0.2.17 finalize_link_action436-443 returnsFalse immediately if True;
False and dynamic mode also returnFalse before separate legacy_whole_archive
getter. Generic whole-archive heuristic remains unchanged rules_cc Starlark.

## Owner and scope

Existing descriptor registry/native bindingA12.48 already retains exact Boolean
and default. Append NativeCommandOption variant, from_name/canonical/is_boolean;
existing Commands parser captures ordered raw/negated occurrences and existing
Configuration converts them. CppFragmentProjection validates/reads sole retained
CppOptions value via native_bool; Loading uses existing check(eval) on each
invocation, including transferred methods retaining original configuration.
No hardcoded True, new parser, state/key/cache/option descriptor, semantic owner,
DICE lock, fragment layout, scheduler or action. Configuration and configured
analysis DICE keys/equality own changed option; phase scratch getter returnsbool.
Representation unchanged: no donor import or retained-memory/performance claim.

Allowlist: Configuration src/command.rs, native/cpp_fragment.rs and existing
native/cpp_start_end_lib_tests.rs; Loading src/analysis_fragments.rs and existing
analysis_fragments/toolchain_provider_tests.rs; Commands src/common.rs tests;
Analysis tests/subrule/implementation_deps.rs; manifest/canonical and Stage6
existing private-requested-options paragraph. No new fixture/harness/helper or
embedded source copy; reuse existing matrices. Estimate production<30/proof<120
netRustlines; cohesive production owners are358/804/1099lines and get only
matching arms/small existing-method sibling, tests remain split. Independent DESIGN/FINAL
because public command/projection and private caller boundary.

## Proof and validation

Extend existing Configuration independent-option matrix: defaultTrue/False,
A/B/A structural bytes/projection/config_setting, both exec paths/idempotence,
malformed absence/text/label rejected at constructor; protect other getters.
Loading existing private method matrix proves zeroarity/current allowed and
forbidden caller/transfer original configuration plus independent crossed
values under target/exec. Analysis existing root/subrule provider primitive
proof adds both getters, Boolean/equality checks, crossed independent state,
both-route target/exec and same-DICE source/config A/B/A; no actions.
Commands existing public Cquery parse matrix adds positive/no/raw/orderedflags,
malformednull/invalid/whitespace and negated-value rejection; defaultothers
unchanged. Preserve all old protected assertions/selectors.

Root serial pinned Cargo JSON no-run60s Configurationlib/Loadinglib/Commandslib/
Analysissubrule, exact nonignored preflight and focused20s runtime; Corecheck/
CLIbuild before unchanged three-hash authentic supervised180s replay. Its76s
prerequisite is necessary to prove actual public dynamic caller advances and
reveal next missing bootstrap surface; smaller getters cannot establish closure.
Actual process cleanup, scopedrustfmt/diff/archive/status. Caps not userlimits.
REPLAN if retained descriptor/exec mapping cannot express actual option, caller
policy must widen, or new whole-archive/action owner required. Full linking,
legacy_whole_archive getter/heuristic, resource execution, M7A/M8 unproved.

## Predecessor

WP8666c18f52a2 independent DESIGN/FINAL;38focused0.442s/Core/CLI pass. Unchanged
query76.423s exit2 clears nativeFalseSONAME and next missingcppgetter436;
three original input hashes/cleanup pass. target/wp866; exactST names deferred.

## Focused receipt

Fresh pinned offline Cargo JSON/preflight: Configuration11/11 (0.032s,
prep1.06s), Loading8/8 (0.064s, prep19.21s), Commands2/2 (0.003s, prep11.53s),
Analysissubrule6/6 (0.464s, prep5.57s). All27 exact ordinary nonignored tests
pass0.563s. Corecheck8.09s passes; scopedformat/diff/archive/status pass.
Production net25/proof58 Rustlines within estimates; no newfixture/helper/
semantic representation. CLIbuild20.34s passes. Unchanged authentic query76.470s exit2 clears dynamic
link registration and reaches coverage_common.instrumented_files_info at
cc_helper1049. Three original hashes match before/after; actual process checks
empty before/after. target/wp867; independent FINAL ACCEPT.

Packet window11:20–11:34UTC (2026-09-30); no speedup claim. Independent review
confirms appended ordinals, sole structural value and current-caller authority.
