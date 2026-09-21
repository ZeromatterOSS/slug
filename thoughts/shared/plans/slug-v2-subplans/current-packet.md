# Current Slug V2 Work Packet

Packet: WP-7-61-m7a-cc-link-variables-and-rust-linker-r1
Status: accepted

## Outcome and compatibility

Run unchanged rules_cc 0.2.17 create_link_variables and rules_rust 0.73.0
get_linker_and_args on real configured ctx, native C++ feature configuration and
Variables. These are demanded by the finite Rustc/bootstrap closure in
bootstrap-readiness.md; preceding Rustc proof used a None C++ toolchain and could
not establish this branch. WP760 at 3298fc1f2 accepted native variables/expansion
with 32 tests, Core/CLI checks and independent review; its full receipt is in Git.

Exact: Cpp fission_active_for_current_compilation_mode(), should_strip_binaries()
and public linkopts; closed typed command options and both existing exec
projections; admitted unchanged link-variable and Rust linker-helper behavior.
Slug-native: existing structural configuration/artifact/path identity and Rust
Host/valid-Unicode observations. Unsupported/deferred: full C++ toolchain/FDO
production, compiler/link actions/actions2ctx, actual linker execution, full
construct_arguments with a production C++ toolchain, execution-time path/tree
mapping and existing WP760 unsupported variable shapes. This is not full
cc_library/Rustc/bootstrap acceptance. M7A remains partial and M8 unproved.

## Source and invariant contract

Bazel 9.2 revision 8220c6198837d5c13d53fea211cf3282aa12408a:
CppConfiguration.java:174–178 linkopts construction; :299–303 strip derivation;
:375–379 private strip call; :940–944 private fission call and
:437–439 fission membership implementation.
CppConfigurationApi.java:74–80 public linkopts value, :140 fission and :167 strip
zero-argument methods. CppOptions converters and existing native converter tests
own fission yes/no/mode-list and strip enum validation. Builtin exec source
common/builtin_exec_platforms.bzl:290–348 plus FunctionTransitionUtil.java:168–185
owns explicit propagation, mapping and default reset. LinkBuildVariablesTest.java
:502–527 supplies strip3×mode3; CcToolchainTest.java:128–162 fission modes;
CppConfigurationStarlarkTest.java:87–95,109–131 public/private binding.
Skip actual linking/debug artifact generation tests: this packet creates no actions.

Fission reads retained Compilation enum members and tests actual compilation mode.
Strip is ALWAYS, or SOMETIMES in FASTBUILD only. Validate typed families/members,
never infer from path/CPU or return placeholder defaults. linkopts preserves raw
strings, order, duplicates and empty entries, then appends -Wl,--eh-frame-hdr iff
experimental_omitfp. It is a public immutable list value, not a callable getter.
Both boolean methods use the existing private allowlist.

Admit Fission, Strip, Linkopt, HostLinkopt and ExperimentalOmitfp through existing
NativeCommandOption/from_name/canonical_name conversion; only omitfp is boolean.
Existing converters retain last fission/strip occurrence and append repeated
linkopts. Both exec projection paths reset fission to empty, set strip always,
map linkopt from host_linkopt and preserve propagated omitfp/host_linkopt. Preserve
unrelated previously admitted transition behavior; do not claim a full exec map.

SlugConfiguration remains sole structural option owner. Existing immutable command
overlay, conversion scratch, configuration Eq/Hash and configured-node keys explain
invalidation. CppFragmentProjection is an existing shared view; new getters add no
retained representation or semantic store. linkopts output is evaluator-local
immutable collection scratch. No DICE key, lock, cache, interner, dependency, schema
or identity-domain change. Normal key equality/publication, retry, cancellation and
release apply; docs/developers/dice.md remains binding. No compact-utility change
or performance claim; use borrowed string-list projection and shared configuration
rather than storing duplicate derived flags.

Existing pinned rules_cc link/link_build_variables.bzl:101–267 owns copied base
bindings, forcePIC/strip/fission, test flag, paths, user flags and exact PIE removal.
cc_helper_internal.bzl:216–218 calls fission before the feature-enabled check.
rules_rust rust/private/rustc.bzl:361–514 reads real ctx.fragments.cpp.linkopts,
selects action by crate type and expands native tool/argv/env. Its staticlib caller
does not set is_using_linker=False; preserve that source behavior.
Its parenthesized singleton strings are substring tests: lib matches dylib before
the later invalid-type tuple. Preserve that quirk with a positive NODEPS case;
rlib without binary override and unknown types remain negative cases.
No native create_link_variables shim: complete rules_cc public export already loads
the unchanged function. Existing source corpus/archive pins are reused.

## Scope and coordination

Configuration/Loading worker: app/slug_configuration_v2/src/command.rs,
native/cpp_fragment.rs, native/configuration.rs (only shared exec helper),
native/cc_feature_tests.rs; app/slug_loading_v2/src/analysis_fragments.rs and
analysis_fragments/cc_feature_tests.rs. Implement typed projections and binding,
with focused source-derived tests. Large configuration.rs gets bounded dispatch
in its existing cohesive exec helper; no broad refactor.

Integration worker: app/slug_analysis_v2/tests/rustc_map_each/cc_linker.rs and
mod.rs bounded shared workspace/request helper; optional sibling .bzl subject
already covered by existing BUILD glob. Load complete already-vendored modules,
invoke real Rust helper with authored capability envelopes and native internals;
no copied/replaced/extracted production function bodies. Root owns direct builder
tests in tests/cc_feature_expansion/link_variables.rs plus module registration,
and app/slug_commands_v2/src/common.rs binder regression only. Analysis production,
configuration representation, build action sinks and fixture source corpus excluded.

Root owns serialized Cargo, integration, docs and atomic commit. Reviewer owns
independent design/final invariant audit. Docs allowlist: manifest/canonical and
bootstrap-readiness plus Stage6 only for changed architectural contract. Estimate
100–220 production and 450–850 proof lines; growth triggers responsibility review.

## Evidence and validation

Projection matrix: all3 compilation modes × strip modes; fission no/yes/subsets,
case/dedup/invalid and overwrite; raw ordered repeated linkopts+omitfp; both exec
paths/reset/remap/idempotent selected platform; structural option discrimination.
Binding: joined command forms, missing/negated nonbool rejection, boolean forms;
private callers/arity; public immutable/noncallable linkopts. Reuse converter tests.

Direct unchanged link builder: forcePIC, conditional strip/debug preservation,
fission+per_object_debug_info guards, linker/archiver distinction, exact PIE removal,
path/test/base bindings, FDO rejection and no false full-provider claim.
Authentic Rust helper: crate-action selection, exact tool/argv/environment,
Rust-preference filtered flags and real File, unsupported crate errors; same-DICE
command changes/restoration. Keep authored envelopes explicit. Protect WP760's
5 expansion integration tests and existing rustc_map_each configured proofs after
shared helper refactoring. No new upstream source corpus bodies or oracle invocation;
one short authored .bzl caller supplies explicit boundary inputs.

Resolve nightly-2025-09-14 via rustup; serialized offline/locked Cargo JSON no-run
for Configuration, Loading, Commands and Analysis starlark_rule integration.
Preflight exact selectors once per unchanged executable; Core/CLI compile checks
cover public command/configuration dependents. Preparation cap60s per operation,
test runtime expected seconds, not a user ceiling. No Slug/Bazel binary, daemon,
actions or broad suite needed. Format, diff, plan-status and archive checks.
Preserve failed receipts and correct ordinary errors in scope. REPLAN only if
real helper invocation requires a new semantic owner/unsupported prerequisite
outside this contract, never by replacing it with an authored helper clone.

Independent design ACCEPT: pinned getter/conversion/allowlist/exec semantics,
including omitfp-derived flags, fit existing structural configuration ownership.
Direct-builder and authentic Rust-helper proofs discriminate the admitted scope.

## Terminal evidence

Pinned nightly-2025-09-14 tools resolved with rustup. Serialized offline/locked
Cargo JSON preparation, all exit0: Configuration lib1.117s, Commands lib11.303s,
Loading lib18.355s, Analysis starlark_rule10.795s then6.327s/6.325s after proof
corrections; Core check6.024s and CLI check6.425s. Commands are
`cargo test --offline --locked -p <owner> --lib --no-run --message-format=json`,
Analysis replaces `--lib` with `--test starlark_rule`; checks use `cargo check`.
Eight operations total66.671s, maximum18.355s, below per-operation60s cap.

Exact selector preflight preceded each changed executable; each selected test
executed with `--exact <name> --nocapture`, exit0 and exactly one pass:

- Configuration: all7 `native::cc_feature_tests` plus protected
  `native::tests::cpp_fragment_projection_distinguishes_target_and_exec_compilation_modes`.
- Loading: all4 `analysis_fragments::cc_feature_tests` (2new,2protected).
- Commands: `common::tests::cpp_link_configuration_flags_use_existing_typed_command_carrier`
  and protected feature-configuration flag binding.
- Analysis integration: 2 `cc_feature_expansion::link_variables` and 3
  `rustc_map_each::cc_linker`; protected5 Rustc,5 expansion and9 configure tests.

38 selected/38 passed; latest per-selector total1.461s. Ignored local receipts
in target/wp761: tests-1790022760662758747 (Configuration),
tests-1790022787278566702 (Commands), tests-1790022849974490645 (Loading),
tests-1790023048344120474 (all24 integration), each `.receipt.json` plus logs.
Preserved three failed test invocations: two shared an incorrect expected sysroot
ordering, and one incorrectly expected lib rejection. Both corrected against
unchanged upstream source, preserving full exact assertions. No production fix or
ownership replan was needed. Full failing output remains local; bounded negative
assertions now avoid dumping the complete configured graph on unexpected success.

Approximately115 production and880 proof lines. Independent responsibility review
accepted modest proof-estimate growth: the layers discriminate conversion, binding,
direct builder and authentic caller separately; no duplicated upstream corpus or
new infrastructure. Formatting, diff, plan-status and archive checks pass.
No Slug/Bazel binary, daemon, build action or remote invocation ran. Compile and
link execution/full toolchain producers remain the next separately scoped gates.

Independent final ACCEPT verified the actual diff, corrected source expectations,
38 passing receipts and Core/CLI checks. About17 minutes elapsed (2026-09-21
20:22–20:39 UTC); review overlapped implementation, not separately timed.
Observable gate advanced: unchanged public link-variable production and Rust
linker-helper selection from real configured inputs, with no new semantic owner.
